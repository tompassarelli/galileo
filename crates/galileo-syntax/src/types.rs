use crate::expr::shown;
use crate::{Error, Line, Span, Token, TokenKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Maybe(Box<Type>),
    Nix(Box<Type>),
    Named {
        name: String,
        args: Vec<Type>,
        span: Span,
    },
}

pub fn parse_type(tokens: &[Token], line: &Line) -> Result<Type, Error> {
    for (at, token) in tokens.iter().enumerate() {
        let TokenKind::Word(word) = &token.kind else { continue };
        let spelling = match word.as_str() {
            "bool" => "Bool",
            "int" => "Int",
            "text" => "Text",
            "path" => "Path",
            "port" => "Port",
            "list" => "List",
            _ => continue,
        };
        let spelling = if word == "list" {
            let inner = tokens.get(at + 2).filter(|_| tokens.get(at + 1).is_some_and(|t| matches!(&t.kind, TokenKind::Word(w) if w == "of")));
            match inner.map(|t| &t.kind) {
                Some(TokenKind::Word(inner)) => {
                    let mut chars = inner.chars();
                    format!("List {}{}", chars.next().unwrap().to_uppercase(), chars.as_str())
                }
                _ => "List T".into(),
            }
        } else {
            spelling.into()
        };
        return Err(Error {
            line: token.span.line,
            column: token.span.column,
            message: format!("expected a type; use `{spelling}` instead of `{word}`"),
            text: Some(line.text.clone()),
        });
    }
    let mut parser = Parser { tokens, at: 0, line };
    let ty = parser.ty()?;
    parser.done("a type")?;
    Ok(ty)
}

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
    line: &'a Line,
}

impl<'a> Parser<'a> {
    fn ty(&mut self) -> Result<Type, Error> {
        let token = self.next("expected a type")?;
        match &token.kind {
            TokenKind::Word(w) if w == "maybe" => Ok(Type::Maybe(Box::new(self.ty()?))),
            TokenKind::Word(w) if w == "nix" => Ok(Type::Nix(Box::new(self.ty()?))),
            TokenKind::Word(name) if is_name(name) => {
                let mut args = Vec::new();
                while let Some(next) = self.tokens.get(self.at) {
                    match &next.kind {
                        TokenKind::Word(w) if w == "maybe" || w == "nix" => {
                            return Err(self.error_at(
                                next,
                                format!("a type argument starting with `{w}` goes in parentheses, as in `{name} ({w} …)`"),
                            ))
                        }
                        TokenKind::Word(arg) if is_name(arg) => {
                            self.at += 1;
                            args.push(Type::Named {
                                name: arg.clone(),
                                args: Vec::new(),
                                span: next.span.clone(),
                            });
                        }
                        TokenKind::Symbol(s) if s == "(" => {
                            self.at += 1;
                            args.push(self.ty()?);
                            self.expect(")", "expected `)` after a type argument")?;
                        }
                        _ => break,
                    }
                }
                Ok(Type::Named {
                    name: name.clone(),
                    args,
                    span: token.span.clone(),
                })
            }
            _ => Err(self.error_at(token, format!("expected a type, found {}", shown(token)))),
        }
    }

    fn next(&mut self, message: &str) -> Result<&'a Token, Error> {
        let tokens = self.tokens;
        let token = tokens.get(self.at).ok_or_else(|| self.error_end(message))?;
        self.at += 1;
        Ok(token)
    }

    fn expect(&mut self, symbol: &str, message: &str) -> Result<(), Error> {
        match self.tokens.get(self.at) {
            Some(Token { kind: TokenKind::Symbol(s), .. }) if s == symbol => {
                self.at += 1;
                Ok(())
            }
            Some(token) => Err(self.error_at(token, message)),
            None => Err(self.error_end(message)),
        }
    }

    fn done(&self, what: &str) -> Result<(), Error> {
        match self.tokens.get(self.at) {
            Some(token) => Err(self.error_at(token, format!("unexpected {} after {what}", shown(token)))),
            None => Ok(()),
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

    fn error_end(&self, message: impl Into<String>) -> Error {
        let column = self
            .tokens
            .last()
            .map_or(self.line.indent + self.line.text.chars().count() + 1, |t| {
                t.span.column + (t.span.end - t.span.start)
            });
        Error {
            line: self.line.no,
            column,
            message: message.into(),
            text: Some(self.line.text.clone()),
        }
    }
}

fn is_name(word: &str) -> bool {
    word.chars().next().is_some_and(char::is_alphabetic)
}
