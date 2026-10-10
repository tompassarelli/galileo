use crate::expr::{parse_expr, BinOp, Expr, ExprKind, Query, TextPart};
use crate::{units, Block, Error, Kind, Line, Reading, Token, TokenKind, Unit, Vocabulary};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub name: String,
    pub head: Reading,
    pub body: Vec<(usize, Literal)>,
    pub line: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aggregate {
    Count,
    Sum,
    Min,
    Max,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Literal {
    Sentence(Reading),
    Not(Reading),
    Compare(BinOp, Expr, Expr),
    Holds(Expr),
    Bind(String, Expr),
    In(String, Expr),
    IsA(String, String),
    Aggregate {
        var: String,
        op: Aggregate,
        of: Option<Expr>,
        body: Vec<Literal>,
    },
    First {
        var: String,
        of: String,
        by: Expr,
        body: Vec<Literal>,
    },
}

const SHAPE_OF_RULE: &str =
    "a rule is `rule name`, then its conclusion, then `when` with its conditions indented beneath";

pub(crate) fn read_rule(v: &Vocabulary, block: &Block, earlier: &[Rule]) -> Result<Rule, Error> {
    let line = &block.line;
    let name = match line.tokens.as_slice() {
        [_, Token {
            kind: TokenKind::Word(name),
            ..
        }] => name.clone(),
        _ => {
            return Err(Error::at(
                line,
                "a rule is named: `rule enabled-by-default`",
            ))
        }
    };
    if let Some(other) = earlier.iter().find(|r| r.name == name) {
        return Err(Error::at(
            line,
            format!("a rule named `{name}` is already on line {}", other.line),
        ));
    }
    let [head, when] = block.children.as_slice() else {
        return Err(Error::at(line, SHAPE_OF_RULE));
    };
    let is_when = matches!(when.line.tokens.as_slice(), [Token { kind: TokenKind::Word(w), .. }] if w == "when");
    if !is_when || when.children.is_empty() || !head.children.is_empty() {
        return Err(Error::at(line, SHAPE_OF_RULE));
    }
    let mut vars = BTreeMap::new();
    let mut body = Vec::new();
    for condition in &when.children {
        if let Some(deeper) = condition.children.first() {
            return Err(Error::at(&deeper.line, "conditions don't nest"));
        }
        let literal = v.literal(&condition.line, &condition.line.tokens, &mut vars)?;
        body.push((condition.line.no, literal));
    }
    let head_units = units(&head.line, &head.line.tokens)?;
    let reading = v.read(&head.line, &head_units, Some(&mut vars))?;
    let shape = &v.shapes[reading.shape];
    if shape.kind != Kind::Derived {
        return Err(Error::at(
            &head.line,
            format!(
                "a rule concludes a derived sentence, but `{}` is {:?}",
                shape.name, shape.kind
            ),
        ));
    }
    Ok(Rule {
        name,
        head: reading,
        body,
        line: line.no,
    })
}

impl Vocabulary {
    pub fn read_query(&self, line: &Line, query: &Query) -> Result<Vec<Literal>, Error> {
        let conj = match query {
            Query::All { conj, .. } | Query::The { conj, .. } => conj,
        };
        self.condition(line, conj, &mut BTreeMap::new())
    }

    /// Reads `literal and literal …`; an aggregate, `first` or nested `where` takes the rest of the condition as its own.
    pub fn condition(
        &self,
        line: &Line,
        tokens: &[Token],
        vars: &mut BTreeMap<String, String>,
    ) -> Result<Vec<Literal>, Error> {
        let mut parts = Vec::new();
        let mut rest = tokens;
        loop {
            let end = if aggregate_ahead(rest) {
                rest.len()
            } else {
                match top_level(rest, |t| matches!(word(t), "and" | "where")) {
                    Some(i) if word(&rest[i]) == "and" => i,
                    _ => rest.len(),
                }
            };
            if end == 0 {
                return Err(Error::at(line, "expected a condition"));
            }
            parts.push(&rest[..end]);
            if end == rest.len() {
                break;
            }
            rest = &rest[end + 1..];
        }
        if parts.len() > 1 {
            if let Some(or) = parts.iter().find_map(|p| top_level(p, |t| word(t) == "or").map(|i| &p[i])) {
                return Err(Error {
                    line: or.span.line,
                    column: or.span.column,
                    message: "an `or` beside a separating `and` needs parentheses: write `(a or b) and c` or `a or (b and c)`".into(),
                    text: Some(line.text.clone()),
                });
            }
        }
        parts.into_iter().map(|p| self.literal(line, p, vars)).collect()
    }

    pub fn literal(
        &self,
        line: &Line,
        tokens: &[Token],
        vars: &mut BTreeMap<String, String>,
    ) -> Result<Literal, Error> {
        if matches!(tokens.first().map(|t| &t.kind), Some(TokenKind::Variable(name)) if name.is_empty()) {
            return Err(Error::at(line, "expected a variable `?name`"));
        }
        if let [first, eq, rest @ ..] = tokens {
            if let (Some(name), true) = (variable(first), symbol(eq, "=")) {
                return self.binding(line, name, rest, vars);
            }
            if let (Some(name), "in") = (variable(first), word(eq)) {
                return Ok(Literal::In(name, self.expr(line, rest, vars)?));
            }
        }
        if let [first, is, a, ty] = tokens {
            if let (Some(name), "is", "a") = (variable(first), word(is), word(a)) {
                if self.types.get(word(ty)).is_some_and(Option::is_none) {
                    return Ok(Literal::IsA(name, word(ty).into()));
                }
            }
        }
        if let [not, rest @ ..] = tokens {
            if word(not) == "not" && self.is_sentence(line, rest)? {
                return Ok(Literal::Not(self.read_tokens(line, rest, vars)?));
            }
        }
        if self.is_sentence(line, tokens)? {
            return Ok(Literal::Sentence(self.read_tokens(line, tokens, vars)?));
        }
        let e = self.expr(line, tokens, vars)?;
        Ok(match e.kind {
            ExprKind::Binary(
                op @ (BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge),
                left,
                right,
            ) => Literal::Compare(op, *left, *right),
            _ => Literal::Holds(e),
        })
    }

    fn binding(
        &self,
        line: &Line,
        var: String,
        rest: &[Token],
        vars: &BTreeMap<String, String>,
    ) -> Result<Literal, Error> {
        let op = match rest.first().map(word) {
            Some("count") => Aggregate::Count,
            Some("sum") => Aggregate::Sum,
            Some("min") => Aggregate::Min,
            Some("max") => Aggregate::Max,
            Some("first") => return self.first(line, var, &rest[1..], vars),
            _ => return Ok(Literal::Bind(var, self.expr(line, rest, vars)?)),
        };
        let keyword = word(&rest[0]);
        let Some(at) = top_level(rest, |t| word(t) == "where") else {
            return Err(Error::at(
                line,
                format!("`{keyword}` needs `where` and the condition it ranges over"),
            ));
        };
        let of = match &rest[1..at] {
            [] if op == Aggregate::Count => None,
            _ if op == Aggregate::Count => {
                return Err(Error::at(
                    line,
                    "`count` takes no value; write `?n = count where …`",
                ))
            }
            [] => {
                return Err(Error::at(
                    line,
                    format!("`{keyword}` needs a value, as in `?n = {keyword} ?x where …`"),
                ))
            }
            value => Some(self.expr(line, value, vars)?),
        };
        let body = self.condition(line, &rest[at + 1..], &mut vars.clone())?;
        Ok(Literal::Aggregate { var, op, of, body })
    }

    fn first(
        &self,
        line: &Line,
        var: String,
        rest: &[Token],
        vars: &BTreeMap<String, String>,
    ) -> Result<Literal, Error> {
        let expected = "`first` is written `?r = first ?x by <expr> where <condition>`";
        let (Some(of), Some(by)) = (rest.first().and_then(variable), rest.get(1)) else {
            return Err(Error::at(line, expected));
        };
        let Some(at) = top_level(rest, |t| word(t) == "where") else {
            return Err(Error::at(line, expected));
        };
        if word(by) != "by" || at < 3 {
            return Err(Error::at(line, expected));
        }
        let by = self.expr(line, &rest[2..at], vars)?;
        let body = self.condition(line, &rest[at + 1..], &mut vars.clone())?;
        Ok(Literal::First { var, of, by, body })
    }

    fn expr(
        &self,
        line: &Line,
        tokens: &[Token],
        vars: &BTreeMap<String, String>,
    ) -> Result<Expr, Error> {
        if tokens.is_empty() {
            return Err(Error::at(line, "expected an expression"));
        }
        let e = parse_expr(tokens, line)?;
        self.nested_queries(line, &e, vars)?;
        Ok(e)
    }

    fn nested_queries(
        &self,
        line: &Line,
        e: &Expr,
        vars: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        let walk = |e: &Expr| self.nested_queries(line, e, vars);
        match &e.kind {
            ExprKind::Query(q) => {
                let (conj, sorted_by) = match q {
                    Query::All { conj, sorted_by, .. } => (conj, sorted_by.as_deref()),
                    Query::The { conj, .. } => (conj, None),
                };
                self.condition(line, conj, &mut vars.clone())?;
                sorted_by.map_or(Ok(()), walk)
            }
            ExprKind::Text(parts) | ExprKind::TextBlock(parts) => parts.iter().try_for_each(|p| match p {
                TextPart::Interpolation(e) => walk(e),
                TextPart::Text(_) => Ok(()),
            }),
            ExprKind::List(items) => items.iter().try_for_each(walk),
            ExprKind::Record(fields) => fields.iter().try_for_each(|f| walk(&f.value)),
            ExprKind::Field(e, _) | ExprKind::Not(e) | ExprKind::Lambda(_, e) => walk(e),
            ExprKind::Call(f, args) => {
                walk(f)?;
                args.iter().try_for_each(walk)
            }
            ExprKind::If(a, b, c) => [a, b, c].into_iter().try_for_each(|e| walk(e)),
            ExprKind::Binary(_, a, b) | ExprKind::With(a, b) => {
                walk(a)?;
                walk(b)
            }
            _ => Ok(()),
        }
    }

    fn is_sentence(&self, line: &Line, tokens: &[Token]) -> Result<bool, Error> {
        let units = units(line, tokens)?;
        if units
            .iter()
            .filter_map(lone)
            .any(|t| matches!(t.kind, TokenKind::Symbol(_)))
        {
            return Ok(false);
        }
        let words = self.template_words();
        Ok(units
            .iter()
            .filter_map(lone)
            .any(|t| matches!(&t.kind, TokenKind::Word(w) if words.contains(w.as_str()))))
    }

    fn read_tokens(
        &self,
        line: &Line,
        tokens: &[Token],
        vars: &mut BTreeMap<String, String>,
    ) -> Result<Reading, Error> {
        self.read(line, &units(line, tokens)?, Some(vars))
    }
}

fn lone(unit: &Unit) -> Option<&Token> {
    match unit.tokens.as_slice() {
        [token] => Some(token),
        _ => None,
    }
}

fn aggregate_ahead(tokens: &[Token]) -> bool {
    matches!(tokens, [first, eq, op, ..]
        if variable(first).is_some()
            && symbol(eq, "=")
            && matches!(word(op), "count" | "sum" | "min" | "max" | "first"))
}

pub(crate) fn top_level(tokens: &[Token], hit: impl Fn(&Token) -> bool) -> Option<usize> {
    let mut depth = 0i32;
    for (i, token) in tokens.iter().enumerate() {
        match word(token) {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            _ if depth == 0 && hit(token) => return Some(i),
            _ => {}
        }
    }
    None
}

pub(crate) fn variable(token: &Token) -> Option<String> {
    match &token.kind {
        TokenKind::Variable(name) if !name.is_empty() => Some(name.clone()),
        _ => None,
    }
}

pub(crate) fn symbol(token: &Token, text: &str) -> bool {
    matches!(&token.kind, TokenKind::Symbol(s) if s == text)
}

pub(crate) fn word(token: &Token) -> &str {
    match &token.kind {
        TokenKind::Word(w) | TokenKind::Symbol(w) => w,
        _ => "",
    }
}
