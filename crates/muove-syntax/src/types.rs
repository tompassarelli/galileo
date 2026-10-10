use crate::expr::{parse_expr, shown, Expr};
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OptionType {
    Enable(Expr),
    Bool,
    Int,
    Text,
    Lines,
    Path,
    Port,
    Package,
    ListOf(Box<OptionType>),
    AttrsOf(Box<OptionType>),
    Maybe(Box<OptionType>),
    OneOf(Vec<Expr>),
}

const OPTION_WORDS: &[&str] = &[
    "enable", "bool", "int", "text", "lines", "path", "port", "package", "list", "attrs", "maybe",
    "one",
];

pub fn parse_type(tokens: &[Token], line: &Line) -> Result<Type, Error> {
    let mut parser = Parser { tokens, at: 0, line };
    let ty = parser.ty()?;
    parser.done("a type")?;
    Ok(ty)
}

pub fn parse_option_type(tokens: &[Token], line: &Line) -> Result<OptionType, Error> {
    let mut parser = Parser { tokens, at: 0, line };
    let ty = parser.option_type()?;
    parser.done("an option type")?;
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

    fn option_type(&mut self) -> Result<OptionType, Error> {
        let token = self.next("expected an option type from SPEC §10.2's table")?;
        let word = match &token.kind {
            TokenKind::Word(w) => w.as_str(),
            _ => "",
        };
        Ok(match word {
            "enable" => OptionType::Enable(self.text("`enable` needs a description text")?),
            "bool" => OptionType::Bool,
            "int" => OptionType::Int,
            "text" => OptionType::Text,
            "lines" => OptionType::Lines,
            "path" => OptionType::Path,
            "port" => OptionType::Port,
            "package" => OptionType::Package,
            "maybe" => OptionType::Maybe(Box::new(self.option_type()?)),
            "list" => {
                self.expect_word("of", "expected `list of T`")?;
                OptionType::ListOf(Box::new(self.option_type()?))
            }
            "attrs" => {
                self.expect_word("of", "expected `attrs of T`")?;
                OptionType::AttrsOf(Box::new(self.option_type()?))
            }
            "one" => {
                const FORM: &str = "expected `one of \"a\" | \"b\"`";
                self.expect_word("of", FORM)?;
                let mut choices = vec![self.text(FORM)?];
                while self.tokens.get(self.at).is_some_and(|t| matches!(&t.kind, TokenKind::Symbol(s) if s == "|")) {
                    self.at += 1;
                    choices.push(self.text(FORM)?);
                }
                OptionType::OneOf(choices)
            }
            _ => {
                let mut message = format!(
                    "expected an option type from SPEC §10.2's table, found {}",
                    shown(token)
                );
                if let Some(close) = crate::check::closest(word, OPTION_WORDS.iter().copied()) {
                    message.push_str(&format!("; did you mean `{close}`?"));
                }
                return Err(self.error_at(token, message));
            }
        })
    }

    fn text(&mut self, message: &str) -> Result<Expr, Error> {
        match self.tokens.get(self.at) {
            Some(token) if matches!(token.kind, TokenKind::Text(_)) => {
                self.at += 1;
                parse_expr(std::slice::from_ref(token), self.line)
            }
            Some(token) => Err(self.error_at(token, message)),
            None => Err(self.error_end(message)),
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

    fn expect_word(&mut self, word: &str, message: &str) -> Result<(), Error> {
        match self.tokens.get(self.at) {
            Some(Token { kind: TokenKind::Word(w), .. }) if w == word => {
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
