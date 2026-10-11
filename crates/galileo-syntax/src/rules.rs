use crate::expr::{parse_expr, BinOp, Expr, ExprKind, Param, Query, TextPart};
use crate::{units, Arg, Block, Error, Kind, Line, Reading, Token, TokenKind, Unit, Vocabulary};
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
    "a rule is `rule name: conclusion`, then `when condition` and one `and condition` per line";

pub(crate) fn read_rule(v: &Vocabulary, block: &Block, earlier: &[Rule]) -> Result<Rule, Error> {
    let line = &block.line;
    let name = match line.tokens.as_slice() {
        [_, Token {
            kind: TokenKind::Word(name),
            ..
        }, colon, head @ ..] if symbol(colon, ":") && !head.is_empty() => name.clone(),
        [_, Token { kind: TokenKind::Word(name), .. }] => {
            let head = block.children.first().map(|b| b.line.text.trim()).unwrap_or("conclusion");
            return Err(Error::at(line, format!("write `rule {name}: {head}` on one line, then `when condition` and one `and condition` per line")));
        }
        _ => {
            return Err(Error::at(
                line,
                SHAPE_OF_RULE,
            ))
        }
    };
    if let Some(other) = earlier.iter().find(|r| r.name == name) {
        return Err(Error::at(
            line,
            format!("a rule named `{name}` is already on line {}", other.line),
        ));
    }
    if block.children.is_empty() {
        return Err(Error::at(line, SHAPE_OF_RULE));
    }
    let mut vars = BTreeMap::new();
    let mut body = Vec::new();
    for (index, condition) in block.children.iter().enumerate() {
        if let Some(deeper) = condition.children.first() {
            if condition.line.tokens.len() == 1 && word(&condition.line.tokens[0]) == "when" {
                return Err(Error::at(&condition.line, format!("write `when {}` on one line, then one `and condition` per line", deeper.line.text.trim())));
            }
            return Err(Error::at(&deeper.line, "conditions don't nest"));
        }
        let tokens = &condition.line.tokens;
        if tokens.first().is_some_and(|t| word(t) == "or") {
            return Err(Error::at(&condition.line, "a rule body has no `or`; write a second rule with the same head for the other case"));
        }
        let prefix = if index == 0 { "when" } else { "and" };
        if tokens.len() < 2 || word(&tokens[0]) != prefix {
            return Err(Error::at(&condition.line, format!("write `{prefix} condition` on one line")));
        }
        let tokens = &tokens[1..];
        let literal = v.line_literal(&condition.line, tokens, &mut vars)?;
        body.push((condition.line.no, literal));
    }
    let head_units = units(line, &line.tokens[3..])?;
    let reading = v.read(line, &head_units, Some(&mut vars))?;
    let shape = &v.shapes[reading.shape];
    if shape.kind != Kind::Derived {
        return Err(Error::at(
            line,
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
        let mut pending = vec![(tokens, Vec::new(), vars.clone())];
        let mut found = Vec::new();
        let mut ambiguous = Vec::new();
        let mut failure = None;
        while let Some((rest, body, table)) = pending.pop() {
            if rest.is_empty() {
                failure = Some(Error::at(line, "expected a condition"));
                continue;
            }
            let mut ends = Vec::new();
            if !aggregate_ahead(rest) {
                let mut offset = 0;
                while let Some(at) =
                    top_level(&rest[offset..], |t| matches!(word(t), "and" | "where"))
                {
                    let at = offset + at;
                    if word(&rest[at]) == "where" {
                        break;
                    }
                    ends.push(at);
                    offset = at + 1;
                }
            }
            let separated = !ends.is_empty();
            ends.push(rest.len());
            for end in ends {
                let mut local = table.clone();
                let part = &rest[..end];
                let literal = if separated && end == rest.len() {
                    match part {
                        [not, sentence @ ..] if word(not) == "not" => self
                            .read_tokens(line, sentence, &mut local)
                            .map(Literal::Not),
                        _ => self.read_tokens(line, part, &mut local).map(Literal::Sentence),
                    }
                } else if end == 0 {
                    Err(Error::at(line, "expected a condition"))
                } else {
                    self.literal(line, part, &mut local)
                };
                match literal {
                    Ok(literal) => {
                        if end != rest.len() || !body.is_empty() {
                            if let Some(at) = top_level(part, |t| matches!(word(t), "or" | "where"))
                                .filter(|&at| word(&part[at]) == "or")
                            {
                                failure = Some(Error {
                                    line: part[at].span.line,
                                    column: part[at].span.column,
                                    message: "an `or` beside a separating `and` needs parentheses: write `(a or b) and c` or `a or (b and c)`".into(),
                                    text: Some(line.text.clone()),
                                });
                                continue;
                            }
                        }
                        let mut next = body.clone();
                        next.push(literal);
                        if end == rest.len() {
                            found.push((next, local));
                        } else {
                            pending.push((&rest[end + 1..], next, local));
                        }
                    }
                    Err(error) => {
                        if end == rest.len() {
                            if let Some(readings) = error
                                .message
                                .strip_prefix("this can be read more than one way:\n")
                            {
                                ambiguous.push(readings.to_string());
                            }
                        }
                        if failure.is_none() {
                            failure = Some(error);
                        }
                    }
                }
            }
        }
        if !ambiguous.is_empty() {
            ambiguous.extend(
                found
                    .iter()
                    .map(|(body, _)| format!("  as {}", self.condition_text(body))),
            );
            return Err(Error::at(
                line,
                format!(
                    "this can be read more than one way:\n{}",
                    ambiguous.join("\n")
                ),
            ));
        }
        match found.len() {
            0 => Err(failure.unwrap_or_else(|| Error::at(line, "expected a condition"))),
            1 => {
                let (body, table) = found.pop().unwrap();
                *vars = table;
                Ok(body)
            }
            _ => Err(Error::at(
                line,
                format!(
                    "this can be read more than one way:\n{}",
                    found
                        .iter()
                        .map(|(body, _)| format!("  as {}", self.condition_text(body)))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            )),
        }
    }

    pub(crate) fn condition_text(&self, body: &[Literal]) -> String {
        body.iter()
            .map(|literal| match literal {
                Literal::Sentence(reading) | Literal::Not(reading) => {
                    let shape = &self.shapes[reading.shape];
                    let from = if shape.module == self.module {
                        String::new()
                    } else {
                        format!(" from {}", shape.module)
                    };
                    format!(
                        "{}{}{from}: {}",
                        if matches!(literal, Literal::Not(_)) {
                            "not "
                        } else {
                            ""
                        },
                        shape.name,
                        self.shape_text(reading.shape)
                    )
                }
                other => format!("{other:?}"),
            })
            .collect::<Vec<_>>()
            .join(" and ")
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
        let mut found = Vec::new();
        let mut ambiguous = Vec::new();
        let mut sentence_error = None;
        let mut attempt = |reading: Result<Literal, Error>, table| match reading {
            Ok(reading) => found.push((reading, table)),
            Err(error) => {
                if let Some(readings) = error.message.strip_prefix("this can be read more than one way:\n") {
                    ambiguous.push(readings.to_string());
                }
                if !error.message.starts_with("this doesn't match any declared sentence shape") || sentence_error.is_none() {
                    sentence_error = Some(error);
                }
            }
        };
        let mut table = vars.clone();
        let sentence = self.read_tokens(line, tokens, &mut table).map(Literal::Sentence);
        attempt(sentence, table);
        if let [not, rest @ ..] = tokens {
            if word(not) == "not" {
                let mut table = vars.clone();
                let sentence = self.read_tokens(line, rest, &mut table).map(Literal::Not);
                attempt(sentence, table);
            }
        }
        let mut table = vars.clone();
        let expression = (|| {
            if let [first, eq, rest @ ..] = tokens {
                if let (Some(name), true) = (variable(first), symbol(eq, "=")) {
                    let literal = self.binding(line, name.clone(), rest, &table)?;
                    if let Literal::Bind(_, e) = &literal {
                        let ty = match &e.kind {
                            ExprKind::Bool(_) | ExprKind::Not(_) | ExprKind::Binary(BinOp::And | BinOp::Or | BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge, _, _) => Some("Bool".to_string()),
                            ExprKind::Int(_) | ExprKind::Binary(BinOp::Add | BinOp::Sub | BinOp::Mul, _, _) => Some("Int".to_string()),
                            ExprKind::Text(_) | ExprKind::TextBlock(_) => Some("Text".to_string()),
                            ExprKind::Path(_) => Some("Path".to_string()),
                            ExprKind::Var(var) => table.get(var).cloned(),
                            _ => None,
                        };
                        if let Some(ty) = ty {
                            self.fits(&Arg::Var(name), &ty, &mut table).map_err(|message| Error::at(line, message))?;
                        }
                    }
                    return Ok(literal);
                }
                if let (Some(name), "in") = (variable(first), word(eq)) {
                    let e = self.expr(line, rest, &table)?;
                    let ty = match &e.kind {
                        ExprKind::Var(var) => table.get(var).cloned(),
                        ExprKind::Int(_) => Some("Int".into()),
                        ExprKind::Bool(_) => Some("Bool".into()),
                        ExprKind::Text(_) | ExprKind::TextBlock(_) => Some("Text".into()),
                        ExprKind::Path(_) => Some("Path".into()),
                        _ => None,
                    };
                    if let Some(ty) = ty.filter(|ty| !ty.starts_with("nix ")) {
                        let Some(item) = ty.strip_prefix("List ") else {
                            return Err(Error::at(line, "membership needs a List expression"));
                        };
                        self.fits(&Arg::Var(name.clone()), item, &mut table).map_err(|message| Error::at(line, message))?;
                    }
                    return Ok(Literal::In(name, e));
                }
            }
            self.boolean_literal(line, tokens, &table)
        })();
        let expression_error = match expression {
            Ok(literal) => {
                found.push((literal, table));
                None
            }
            Err(error) => Some(error),
        };
        if !ambiguous.is_empty() || found.len() > 1 {
            let text = units(line, tokens)?.iter().map(|unit| unit.text.as_str()).collect::<Vec<_>>().join(" ");
            ambiguous.extend(found.iter().map(|(literal, _)| match literal {
                Literal::Sentence(reading) | Literal::Not(reading) => format!("  as {}{}", if matches!(literal, Literal::Not(_)) { "not " } else { "" }, self.shape_text(reading.shape)),
                other => format!("  as {}: {text}", match other {
                    Literal::In(_, _) => "membership",
                    Literal::Bind(_, _) => "binding",
                    _ => "expression",
                }),
            }));
            return Err(Error::at(line, format!("this can be read more than one way:\n{}", ambiguous.join("\n"))));
        }
        if let Some((literal, table)) = found.pop() {
            *vars = table;
            return Ok(literal);
        }
        if units(line, tokens)?.iter().filter_map(lone).any(|t| {
            matches!(&t.kind, TokenKind::Word(w) if !matches!(w.as_str(), "not" | "and" | "or" | "in" | "where" | "all" | "the" | "count" | "sum" | "min" | "max" | "first" | "by" | "sorted" | "if" | "then" | "else" | "with" | "true" | "false" | "null"))
        }) && !matches!(tokens, [first, eq, ..] if variable(first).is_some() && symbol(eq, "=")) {
            return Err(sentence_error.unwrap());
        }
        Err(expression_error.unwrap())
    }

    pub(crate) fn line_literal(
        &self,
        line: &Line,
        tokens: &[Token],
        vars: &mut BTreeMap<String, String>,
    ) -> Result<Literal, Error> {
        let literal = self.literal(line, tokens, vars);
        if top_level(tokens, |t| matches!(word(t), "and" | "where"))
            .is_some_and(|at| word(&tokens[at]) == "and")
            && !matches!(&literal, Ok(Literal::Sentence(_) | Literal::Not(_)))
            && !literal.as_ref().err().is_some_and(|error| error.message.starts_with("this can be read more than one way:\n"))
        {
            return Err(Error::at(line, "write one condition per line with `when` then `and`; a boolean binding is `?x = (?a and ?b)`"));
        }
        literal
    }

    fn boolean_literal(
        &self,
        line: &Line,
        tokens: &[Token],
        vars: &BTreeMap<String, String>,
    ) -> Result<Literal, Error> {
        let e = self.expr(line, tokens, vars)?;
        let mut pending = vec![&e];
        while let Some(expr) = pending.pop() {
            let atom = match &expr.kind {
                ExprKind::Not(inner) => { pending.push(inner); None }
                ExprKind::Binary(BinOp::And | BinOp::Or, left, right) => { pending.extend([left.as_ref(), right.as_ref()]); None }
                ExprKind::Var(name) => Some(Arg::Var(name.clone())),
                ExprKind::Name(name) => Some(Arg::Name(name.clone())),
                ExprKind::Bool(value) => Some(Arg::Name(value.to_string())),
                ExprKind::Int(n) => Some(Arg::Int(*n)),
                ExprKind::Text(_) | ExprKind::TextBlock(_) => Some(Arg::Text(String::new())),
                ExprKind::Path(path) => Some(Arg::Path(path.clone())),
                ExprKind::Null | ExprKind::List(_) | ExprKind::Record(_) | ExprKind::Lambda(_, _) | ExprKind::Binary(BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Concat | BinOp::Update, _, _) => {
                    return Err(Error::at(line, "a boolean literal needs a Bool expression"));
                }
                _ => None,
            };
            if let Some(atom) = atom {
                self.fits(&atom, "Bool", &mut vars.clone()).map_err(|message| Error::at(line, message))?;
            }
        }
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
            ExprKind::Field(e, _) | ExprKind::Not(e) => walk(e),
            ExprKind::Lambda(params, body) => {
                for param in params {
                    if let Param::Formals { fields, .. } = param {
                        for field in fields {
                            if let Some(default) = &field.default {
                                walk(default)?;
                            }
                        }
                    }
                }
                walk(body)
            }
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
