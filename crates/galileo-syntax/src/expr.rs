use crate::record::{argument_list, entries, Arguments, AttrPath, Entry, Segment};
use crate::types::{parse_type, Type};
use crate::{lex, Block, Error, Line, Span, Token, TokenKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprKind {
    Null,
    Bool(bool),
    Int(i64),
    Text(Vec<TextPart>),
    TextBlock(Vec<TextPart>),
    Path(String),
    Name(String),
    Var(String),
    List(Vec<Expr>),
    Record(Vec<Field>),
    Field(Box<Expr>, String),
    Call(Box<Expr>, Vec<Expr>),
    Lambda(Vec<Param>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    With(Box<Expr>, Box<Expr>),
    Query(Query),
    NixModule(Arguments, Vec<Entry>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextPart {
    Text(String),
    Interpolation(Expr),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub path: AttrPath,
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Param {
    Name {
        name: String,
        ty: Option<Type>,
    },
    Formals { fields: Vec<Formal>, open: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Formal {
    pub name: String,
    pub default: Option<Expr>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Query {
    All {
        vars: Vec<String>,
        conj: Vec<Token>,
        sorted_by: Option<Box<Expr>>,
        head: Option<Box<Expr>>,
    },
    The {
        var: String,
        conj: Vec<Token>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Update,
    Add,
    Sub,
    Mul,
    Concat,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Assoc {
    Left,
    Right,
    None,
}

const OPERATORS: &[(&str, BinOp, u8, Assoc)] = &[
    ("or", BinOp::Or, 1, Assoc::Left),
    ("and", BinOp::And, 2, Assoc::Left),
    ("==", BinOp::Eq, 4, Assoc::None),
    ("!=", BinOp::Ne, 4, Assoc::None),
    ("<", BinOp::Lt, 4, Assoc::None),
    ("<=", BinOp::Le, 4, Assoc::None),
    (">", BinOp::Gt, 4, Assoc::None),
    (">=", BinOp::Ge, 4, Assoc::None),
    ("//", BinOp::Update, 5, Assoc::Right),
    ("+", BinOp::Add, 6, Assoc::Left),
    ("-", BinOp::Sub, 6, Assoc::Left),
    ("*", BinOp::Mul, 7, Assoc::Left),
    ("++", BinOp::Concat, 8, Assoc::Right),
];

const NOT_PRECEDENCE: u8 = 3;

pub const KEYWORDS: &[&str] = &[
    "all", "and", "else", "false", "if", "in", "not", "null", "or", "then", "true", "when",
    "where", "with",
];

pub fn parse_expr(tokens: &[Token], line: &Line) -> Result<Expr, Error> {
    let mut parser = Parser::new(tokens, line, &[]);
    parser.whole()
}

pub(crate) fn parse_line_expr(tokens: &[Token], block: &Block) -> Result<(Expr, bool), Error> {
    let mut parser = Parser::new(tokens, &block.line, &block.children);
    let expr = parser.whole()?;
    Ok((expr, parser.used_children))
}

pub(crate) fn parse_attrpath<'a>(
    tokens: &'a [Token],
    line: &Line,
) -> Result<Option<(AttrPath, &'a [Token])>, Error> {
    let mut parser = Parser::new(tokens, line, &[]);
    Ok(parser.attrpath()?.map(|path| (path, &tokens[parser.at..])))
}

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
    line: &'a Line,
    children: &'a [Block],
    used_children: bool,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token], line: &'a Line, children: &'a [Block]) -> Self {
        Self {
            tokens,
            at: 0,
            line,
            children,
            used_children: false,
        }
    }

    fn whole(&mut self) -> Result<Expr, Error> {
        let expr = self.expr()?;
        match self.peek() {
            Some(token) => Err(self.error_at(token, format!("unexpected {}", shown(token)))),
            None => Ok(expr),
        }
    }

    fn expr(&mut self) -> Result<Expr, Error> {
        self.binary(0)
    }

    fn binary(&mut self, min: u8) -> Result<Expr, Error> {
        let start = self.at;
        let mut lhs = if self.peek_is_word("not") {
            self.at += 1;
            let operand = self.binary(NOT_PRECEDENCE * 2)?;
            self.node(start, ExprKind::Not(Box::new(operand)))
        } else {
            self.postfix()?
        };
        while let Some(&(name, op, precedence, assoc)) = self.peek().and_then(operator) {
            let left = precedence * 2;
            if left < min {
                break;
            }
            self.at += 1;
            if precedence > NOT_PRECEDENCE && self.peek_is_word("not") {
                return Err(self.error_next(format!(
                    "`not` is never the bare operand of `{name}`; write `a {name} (not b)`"
                )));
            }
            let right = if assoc == Assoc::Right {
                left
            } else {
                left + 1
            };
            let rhs = self.binary(right)?;
            lhs = self.node(start, ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)));
            if assoc == Assoc::None {
                if let Some(next) = self.peek() {
                    if operator(next).is_some_and(|o| o.2 == precedence) {
                        return Err(self.error_at(next, "comparisons don't chain; add parentheses"));
                    }
                }
            }
        }
        Ok(lhs)
    }

    fn postfix(&mut self) -> Result<Expr, Error> {
        let start = self.at;
        let mut expr = self.primary()?;
        loop {
            if self.eat_symbol(".") {
                let name = self.ident("expected a field name after `.`")?;
                expr = self.node(start, ExprKind::Field(Box::new(expr), name));
            } else if self.eat_symbol("(") {
                let args = self.sequence(")", |p| p.expr())?;
                expr = self.node(start, ExprKind::Call(Box::new(expr), args));
            } else {
                return Ok(expr);
            }
        }
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        let start = self.at;
        let Some(token) = self.peek().cloned() else {
            return Err(self.error_end("expected an expression"));
        };
        self.at += 1;
        let kind = match &token.kind {
            TokenKind::Integer(digits) => ExprKind::Int(
                digits
                    .parse()
                    .map_err(|_| self.error_at(&token, "integer out of range"))?,
            ),
            TokenKind::Text(raw) => ExprKind::Text(self.text_parts(raw, &token, false)?),
            TokenKind::TextBlock(raw) => ExprKind::TextBlock(self.text_parts(raw, &token, true)?),
            TokenKind::Path(path) => ExprKind::Path(path.clone()),
            TokenKind::Variable(name) if !name.is_empty() => ExprKind::Var(name.clone()),
            TokenKind::QuotedName(name) => ExprKind::Name(name.clone()),
            TokenKind::Word(word) => match word.as_str() {
                "true" => ExprKind::Bool(true),
                "false" => ExprKind::Bool(false),
                "null" => ExprKind::Null,
                "if" => {
                    let cond = self.expr()?;
                    self.expect_word("then", "expected `then` in `if … then … else …`")?;
                    let then = self.expr()?;
                    self.expect_word("else", "expected `else` in `if … then … else …`")?;
                    let otherwise = self.expr()?;
                    ExprKind::If(Box::new(cond), Box::new(then), Box::new(otherwise))
                }
                "with" => {
                    let scope = self.expr()?;
                    self.expect_symbol(":", "expected `:` in `with expr: expr`")?;
                    let body = self.expr()?;
                    ExprKind::With(Box::new(scope), Box::new(body))
                }
                "all" => ExprKind::Query(self.query(word)?),
                "the" if self.peek_variable() => ExprKind::Query(self.query(word)?),
                "nix" if self.peek_is_word("module") => self.nix_module()?,
                reserved if KEYWORDS.contains(&reserved) => {
                    return Err(self.error_at(
                        &token,
                        format!("expected an expression, found `{reserved}`"),
                    ))
                }
                _ => ExprKind::Name(word.clone()),
            },
            TokenKind::Symbol(s) if s == "-" => {
                return Err(self.error_at(
                    &token,
                    "negation is not in the MVP; write `0 - x`",
                ))
            }
            TokenKind::Symbol(s) if s == "[" => ExprKind::List(self.sequence("]", |p| p.expr())?),
            TokenKind::Symbol(s) if s == "{" => {
                ExprKind::Record(self.sequence("}", |p| p.field())?)
            }
            TokenKind::Symbol(s) if s == "(" => {
                if self.lambda_ahead() {
                    if self.peek_is_symbol(")") {
                        return Err(self.error_next(
                            "expected a lambda parameter, as in `(x) => expr`",
                        ));
                    }
                    let params = self.sequence(")", |p| p.param())?;
                    self.expect_symbol("=>", "expected `=>` after lambda parameters")?;
                    let body = self.expr()?;
                    ExprKind::Lambda(params, Box::new(body))
                } else {
                    let inner = self.expr()?;
                    self.expect_symbol(")", "expected `)`")?;
                    return Ok(Expr {
                        kind: inner.kind,
                        span: self.span(start),
                    });
                }
            }
            _ => {
                return Err(self.error_at(
                    &token,
                    format!("expected an expression, found {}", shown(&token)),
                ))
            }
        };
        Ok(self.node(start, kind))
    }

    fn field(&mut self) -> Result<Field, Error> {
        let path = match self.attrpath()? {
            Some(path) => path,
            None => return Err(self.error_next("expected a record field `name: expr`")),
        };
        self.expect_symbol(":", "expected `:` after a record field name")?;
        let value = self.expr()?;
        Ok(Field { path, value })
    }

    fn param(&mut self) -> Result<Param, Error> {
        if self.eat_symbol("{") {
            let mut fields = Vec::new();
            let mut open = false;
            loop {
                if self.eat_symbol("...") {
                    if fields.is_empty() {
                        return Err(self.error_next("expected a formal before `...`"));
                    }
                    open = true;
                    self.expect_symbol("}", "expected `}` after `...`")?;
                    break;
                }
                let name = self.ident("expected a formal `name` or `name = expr`")?;
                let default = if self.eat_symbol("=") { Some(self.expr()?) } else { None };
                fields.push(Formal { name, default });
                if self.eat_symbol("}") {
                    break;
                }
                self.expect_symbol(",", "expected `,` or `}` after a formal")?;
            }
            return Ok(Param::Formals { fields, open });
        }
        let name = self.ident("expected a parameter name")?;
        let ty = if self.eat_symbol(":") {
            let begin = self.at;
            let mut depth = 0i32;
            while let Some(token) = self.peek() {
                match word(token) {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" if depth == 0 => break,
                    ")" | "]" | "}" => depth -= 1,
                    "," if depth == 0 => break,
                    _ => {}
                }
                self.at += 1;
            }
            if begin == self.at {
                return Err(self.error_next("expected a parameter type after `:`"));
            }
            Some(parse_type(&self.tokens[begin..self.at], self.line)?)
        } else {
            None
        };
        Ok(Param::Name { name, ty })
    }

    fn query(&mut self, head: &str) -> Result<Query, Error> {
        let mut vars = Vec::new();
        let mut value = None;
        let variable_list = head == "the" || (self.peek_variable() && self.peek_word_at(1) == ",");
        if variable_list {
            loop {
                match self.peek().map(|t| &t.kind) {
                    Some(TokenKind::Variable(name)) if !name.is_empty() => vars.push(name.clone()),
                    _ => return Err(self.error_next("expected a variable `?name`")),
                }
                self.at += 1;
                if head == "the" || !self.eat_symbol(",") { break; }
            }
        } else {
            let begin = self.at;
            let mut depth = 0i32;
            while let Some(token) = self.peek() {
                match word(token) {
                    "where" if depth == 0 => break,
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" if depth == 0 => break,
                    ")" | "]" | "}" => depth -= 1,
                    "," if depth == 0 => break,
                    _ => {}
                }
                self.at += 1;
            }
            let expression = parse_expr(&self.tokens[begin..self.at], self.line)?;
            if let ExprKind::Var(name) = &expression.kind {
                vars.push(name.clone());
            } else {
                value = Some(Box::new(expression));
            }
        }
        self.expect_word("where", "expected `where` after the query head")?;
        let begin = self.at;
        let mut depth = 0i32;
        while let Some(token) = self.peek() {
            match word(token) {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" if depth == 0 => break,
                ")" | "]" | "}" => depth -= 1,
                "," if depth == 0 => break,
                _ => {}
            }
            self.at += 1;
        }
        if begin == self.at {
            return Err(self.error_next("expected a condition after `where`"));
        }
        let conj = self.tokens[begin..self.at].to_vec();
        if head == "the" {
            return Ok(Query::The {
                var: vars.remove(0),
                conj,
            });
        }
        let sorted_by = if self.peek_is_symbol(",")
            && self.peek_word_at(1) == "sorted"
            && self.peek_word_at(2) == "by"
        {
            self.at += 3;
            Some(Box::new(self.expr()?))
        } else {
            None
        };
        Ok(Query::All {
            vars,
            conj,
            sorted_by,
            head: value,
        })
    }

    fn nix_module(&mut self) -> Result<ExprKind, Error> {
        let expected =
            "expected `nix module(name, …)` ending its line, with its entries indented beneath";
        self.at += 1;
        if !self.peek_is_symbol("(") {
            return Err(self.error_next(expected));
        }
        let close = self
            .matching_close(self.at)
            .ok_or_else(|| self.error_next(expected))?;
        let arguments = argument_list(&self.tokens[self.at + 1..close])
            .ok_or_else(|| self.error_next(expected))?;
        self.at = close + 1;
        if self.at != self.tokens.len() || self.children.is_empty() || self.used_children {
            return Err(self.error_at(&self.tokens[close], expected));
        }
        self.used_children = true;
        Ok(ExprKind::NixModule(arguments, entries(self.children)?))
    }

    fn attrpath(&mut self) -> Result<Option<AttrPath>, Error> {
        let mut segments = Vec::new();
        loop {
            let Some(token) = self.peek().cloned() else {
                break;
            };
            let segment = match &token.kind {
                TokenKind::Word(name) if name != "when" && name != "for" => {
                    Segment::Name(name.clone())
                }
                TokenKind::QuotedName(name) => Segment::Name(name.clone()),
                TokenKind::Text(raw) => {
                    let parts = self.text_parts(raw, &token, false)?;
                    match parts.as_slice() {
                        [TextPart::Text(text)] => Segment::Text(text.clone()),
                        _ => Segment::Computed(Expr {
                            kind: ExprKind::Text(parts),
                            span: token.span,
                        }),
                    }
                }
                TokenKind::Symbol(s) if s == "{" => {
                    self.at += 1;
                    let start = self.at;
                    let inner = self.expr()?;
                    if !self.peek_is_symbol("}") {
                        return Err(self.error_next("expected `}` closing a computed name"));
                    }
                    if matches!(inner.kind, ExprKind::Text(_)) {
                        let raw = self.tokens[start..self.at]
                            .iter()
                            .find_map(|token| match &token.kind {
                                TokenKind::Text(raw) => Some(raw),
                                _ => None,
                            })
                            .unwrap();
                        return Err(self.error_at(
                            &token,
                            format!("a text key takes no braces; write \"{raw}\""),
                        ));
                    }
                    Segment::Computed(inner)
                }
                _ => break,
            };
            segments.push(segment);
            self.at += 1;
            if !self.eat_symbol(".") {
                return Ok(Some(AttrPath {
                    segments,
                    line: self.line.no,
                }));
            }
        }
        if segments.is_empty() {
            Ok(None)
        } else {
            Err(self.error_next("expected a name after `.`"))
        }
    }

    fn sequence<T>(
        &mut self,
        close: &str,
        mut item: impl FnMut(&mut Self) -> Result<T, Error>,
    ) -> Result<Vec<T>, Error> {
        let mut items = Vec::new();
        if self.eat_symbol(close) {
            return Ok(items);
        }
        loop {
            items.push(item(self)?);
            if self.eat_symbol(close) {
                return Ok(items);
            }
            if !self.eat_symbol(",") {
                return Err(self.error_next(format!("expected `,` or `{close}`")));
            }
        }
    }

    fn text_parts(&self, raw: &str, token: &Token, block: bool) -> Result<Vec<TextPart>, Error> {
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut at = 0;
        while at < raw.len() {
            let c = raw[at..].chars().next().unwrap();
            if !block && c == '\\' {
                let escaped = raw[at + 1..].chars().next().unwrap_or('\\');
                text.push(match escaped {
                    'n' => '\n',
                    't' => '\t',
                    other => other,
                });
                at += 1 + escaped.len_utf8();
                continue;
            }
            let opener = if block { "{{" } else { "{" };
            if raw[at..].starts_with(opener) {
                let from = at + opener.len();
                let closer = if block { "}}" } else { "}" };
                let end = closing(raw, from, closer)
                    .ok_or_else(|| self.error_at(token, "unterminated interpolation"))?;
                if !text.is_empty() {
                    parts.push(TextPart::Text(std::mem::take(&mut text)));
                }
                let line = token.span.line
                    + if block {
                        1 + raw[..at].matches('\n').count()
                    } else {
                        0
                    };
                let column = if block {
                    from - raw[..at].rfind('\n').map_or(0, |n| n + 1) + 1
                } else {
                    token.span.column + 1 + raw[..from].chars().count()
                };
                let offset = if block {
                    token.span.start
                } else {
                    token.span.start + 1 + from
                };
                parts.push(TextPart::Interpolation(self.interpolation(
                    &raw[from..end],
                    token,
                    line,
                    column,
                    offset,
                )?));
                at = end + closer.len();
                continue;
            }
            text.push(c);
            at += c.len_utf8();
        }
        if !text.is_empty() || parts.is_empty() {
            parts.push(TextPart::Text(text));
        }
        Ok(parts)
    }

    fn interpolation(
        &self,
        source: &str,
        token: &Token,
        line: usize,
        column: usize,
        offset: usize,
    ) -> Result<Expr, Error> {
        let lead = source.len() - source.trim_start().len();
        let relocate = |error: Error| Error {
            line,
            column: column + lead + error.column.saturating_sub(1),
            message: error.message,
            text: Some(self.line.text.clone()),
        };
        let lines = lex(source.trim_start()).map_err(relocate)?;
        let [inner] = lines.as_slice() else {
            return Err(self.error_at(token, "an interpolation needs one expression"));
        };
        let tokens: Vec<Token> = inner
            .tokens
            .iter()
            .map(|t| Token {
                kind: t.kind.clone(),
                span: Span {
                    start: offset + lead + t.span.start,
                    end: offset + lead + t.span.end,
                    line,
                    column: column + lead + t.span.column - 1,
                },
            })
            .collect();
        parse_expr(&tokens, self.line)
    }

    fn lambda_ahead(&self) -> bool {
        self.matching_close(self.at - 1)
            .and_then(|close| self.tokens.get(close + 1))
            .is_some_and(|t| is_symbol(t, "=>"))
    }

    fn matching_close(&self, open: usize) -> Option<usize> {
        let mut stack = Vec::new();
        for (index, token) in self.tokens.iter().enumerate().skip(open) {
            match word(token) {
                "(" => stack.push(")"),
                "[" => stack.push("]"),
                "{" => stack.push("}"),
                close @ (")" | "]" | "}") => {
                    if stack.pop() != Some(close) {
                        return None;
                    }
                    if stack.is_empty() {
                        return Some(index);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn ident(&mut self, expected: &str) -> Result<String, Error> {
        match self.peek().map(|t| &t.kind) {
            Some(TokenKind::Word(name)) if !KEYWORDS.contains(&name.as_str()) => {
                let name = name.clone();
                self.at += 1;
                Ok(name)
            }
            Some(TokenKind::QuotedName(name)) => {
                let name = name.clone();
                self.at += 1;
                Ok(name)
            }
            _ => Err(self.error_next(expected)),
        }
    }

    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.at)
    }

    fn peek_word_at(&self, ahead: usize) -> &'a str {
        self.tokens.get(self.at + ahead).map_or("", word)
    }

    fn peek_is_word(&self, w: &str) -> bool {
        matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Word(x)) if x == w)
    }

    fn peek_is_symbol(&self, s: &str) -> bool {
        self.peek().is_some_and(|t| is_symbol(t, s))
    }

    fn peek_variable(&self) -> bool {
        matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Variable(_)))
    }

    fn eat_symbol(&mut self, s: &str) -> bool {
        let found = self.peek_is_symbol(s);
        if found {
            self.at += 1;
        }
        found
    }

    fn expect_symbol(&mut self, s: &str, message: &str) -> Result<(), Error> {
        if self.eat_symbol(s) {
            Ok(())
        } else {
            Err(self.error_next(message))
        }
    }

    fn expect_word(&mut self, w: &str, message: &str) -> Result<(), Error> {
        if self.peek_is_word(w) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.error_next(message))
        }
    }

    fn node(&self, start: usize, kind: ExprKind) -> Expr {
        Expr {
            kind,
            span: self.span(start),
        }
    }

    fn span(&self, start: usize) -> Span {
        let first = &self.tokens[start].span;
        let last = &self.tokens[self.at - 1].span;
        Span {
            start: first.start,
            end: last.end,
            line: first.line,
            column: first.column,
        }
    }

    fn error_at(&self, token: &Token, message: impl Into<String>) -> Error {
        Error {
            line: token.span.line,
            column: token.span.column,
            message: message.into(),
            text: Some(self.line.text.clone()),
        }
    }

    fn error_next(&self, message: impl Into<String>) -> Error {
        match self.peek() {
            Some(token) => self.error_at(token, message),
            None => self.error_end(message),
        }
    }

    fn error_end(&self, message: impl Into<String>) -> Error {
        Error {
            line: self.line.no,
            column: self.line.indent + self.line.text.chars().count() + 1,
            message: message.into(),
            text: Some(self.line.text.clone()),
        }
    }
}

fn operator(token: &Token) -> Option<&'static (&'static str, BinOp, u8, Assoc)> {
    let text = match &token.kind {
        TokenKind::Word(w) if w == "and" || w == "or" => w,
        TokenKind::Symbol(s) => s,
        _ => return None,
    };
    OPERATORS.iter().find(|(op, ..)| op == text)
}

fn closing(raw: &str, from: usize, close: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut at = from;
    while at < raw.len() {
        if depth == 0 && raw[at..].starts_with(close) {
            return Some(at);
        }
        let c = raw[at..].chars().next().unwrap();
        match c {
            '\n' => return None,
            '"' => {
                at = closing(raw, at + 1, "\"")? + 1;
                continue;
            }
            '\\' if close == "\"" => at += 1,
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        at += c.len_utf8();
    }
    None
}

pub(crate) fn shown(token: &Token) -> String {
    match &token.kind {
        TokenKind::Word(w) | TokenKind::Symbol(w) => format!("`{w}`"),
        TokenKind::Variable(v) => format!("`?{v}`"),
        TokenKind::QuotedName(n) => format!("`{n}`"),
        TokenKind::Integer(i) => format!("`{i}`"),
        TokenKind::Text(_) => "a text literal".into(),
        TokenKind::TextBlock(_) => "a text block".into(),
        TokenKind::Path(p) => format!("`{p}`"),
    }
}

pub(crate) fn word(token: &Token) -> &str {
    match &token.kind {
        TokenKind::Word(w) | TokenKind::Symbol(w) => w,
        _ => "",
    }
}

pub(crate) fn is_symbol(token: &Token, symbol: &str) -> bool {
    matches!(&token.kind, TokenKind::Symbol(s) if s == symbol)
}
