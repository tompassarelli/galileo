use muove_nix::{AttrKey, AttrPath, BinOp, Binding, Expr, Formal, Param, StrPart};
use rnix::ast::{self, AstNode, HasEntry};
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Syntax(rnix::ParseError),
    Unsupported {
        kind: rnix::SyntaxKind,
        range: rnix::TextRange,
    },
    Missing(&'static str),
    Integer(std::num::ParseIntError),
    Float(std::num::ParseFloatError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(error) => write!(f, "{error}"),
            Self::Unsupported { kind, range } => {
                write!(f, "unsupported node {kind:?} at {range:?}")
            }
            Self::Missing(field) => write!(f, "missing {field}"),
            Self::Integer(error) => write!(f, "invalid integer: {error}"),
            Self::Float(error) => write!(f, "invalid float: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Syntax(error) => Some(error),
            Self::Integer(error) => Some(error),
            Self::Float(error) => Some(error),
            _ => None,
        }
    }
}

pub fn parse(source: &str) -> Result<Expr, Error> {
    let root = rnix::Root::parse(source).ok().map_err(Error::Syntax)?;
    expr(required(root.expr(), "root expression")?)
}

fn required<T>(value: Option<T>, field: &'static str) -> Result<T, Error> {
    value.ok_or(Error::Missing(field))
}

fn unsupported(node: &impl AstNode) -> Error {
    Error::Unsupported {
        kind: node.syntax().kind(),
        range: node.syntax().text_range(),
    }
}

fn boxed(value: Option<ast::Expr>, field: &'static str) -> Result<Box<Expr>, Error> {
    expr(required(value, field)?).map(Box::new)
}

fn expr(node: ast::Expr) -> Result<Expr, Error> {
    Ok(match node {
        ast::Expr::Ident(node) => Expr::Ident(node.to_string()),
        ast::Expr::Literal(node) => match node.kind() {
            ast::LiteralKind::Integer(value) => {
                Expr::Int(value.to_string().parse().map_err(Error::Integer)?)
            }
            ast::LiteralKind::Float(value) => {
                Expr::Float(value.to_string().parse().map_err(Error::Float)?)
            }
            ast::LiteralKind::Uri(value) => Expr::Uri(value.to_string()),
        },
        ast::Expr::Str(node) => {
            let parts = string(&node)?;
            if node.to_string().starts_with("''") {
                Expr::IndentedStr(parts)
            } else {
                Expr::Str(parts)
            }
        }
        ast::Expr::PathRel(node) => path(&node)?,
        ast::Expr::List(node) => Expr::List(node.items().map(expr).collect::<Result<_, _>>()?),
        ast::Expr::AttrSet(node) => Expr::AttrSet {
            rec: node.rec_token().is_some(),
            bindings: bindings(&node)?,
        },
        ast::Expr::Select(node) => Expr::Select {
            expr: boxed(node.expr(), "selected expression")?,
            path: attrpath(required(node.attrpath(), "selected attribute path")?)?,
            default: node.default_expr().map(expr).transpose()?.map(Box::new),
        },
        ast::Expr::Apply(node) => Expr::Apply {
            func: boxed(node.lambda(), "application function")?,
            arg: boxed(node.argument(), "application argument")?,
        },
        ast::Expr::Lambda(node) => Expr::Lambda {
            param: param(required(node.param(), "lambda parameter")?)?,
            body: boxed(node.body(), "lambda body")?,
        },
        ast::Expr::BinOp(node) => Expr::BinOp {
            op: binop(&node)?,
            lhs: boxed(node.lhs(), "left operand")?,
            rhs: boxed(node.rhs(), "right operand")?,
        },
        ast::Expr::Paren(node) => expr(required(node.expr(), "parenthesized expression")?)?,
        other => return Err(unsupported(&other)),
    })
}

fn path(node: &impl AstNode) -> Result<Expr, Error> {
    if node.syntax().children().next().is_some() {
        return Err(unsupported(node));
    }
    Ok(Expr::Path(node.syntax().to_string()))
}

fn string(node: &ast::Str) -> Result<Vec<StrPart>, Error> {
    node.normalized_parts()
        .into_iter()
        .map(|part| match part {
            ast::InterpolPart::Literal(text) => Ok(StrPart::Text(text)),
            ast::InterpolPart::Interpolation(node) => {
                expr(required(node.expr(), "string interpolation")?).map(StrPart::Interp)
            }
        })
        .collect()
}

fn attrpath(node: ast::Attrpath) -> Result<AttrPath, Error> {
    node.attrs()
        .map(|attr| match attr {
            ast::Attr::Ident(node) => Ok(AttrKey::Ident(node.to_string())),
            ast::Attr::Str(node) => string(&node).map(AttrKey::Str),
            ast::Attr::Dynamic(node) => {
                expr(required(node.expr(), "dynamic attribute")?).map(AttrKey::Dynamic)
            }
        })
        .collect()
}

fn bindings(node: &impl HasEntry) -> Result<Vec<Binding>, Error> {
    node.entries()
        .map(|entry| match entry {
            ast::Entry::AttrpathValue(node) => Ok(Binding::Attr {
                path: attrpath(required(node.attrpath(), "binding attribute path")?)?,
                value: expr(required(node.value(), "binding value")?)?,
            }),
            ast::Entry::Inherit(node) => {
                let from = node
                    .from()
                    .map(|from| expr(required(from.expr(), "inherit source")?))
                    .transpose()?;
                let names = node
                    .attrs()
                    .map(|attr| match attr {
                        ast::Attr::Ident(name) => Ok(name.to_string()),
                        other => Err(unsupported(&other)),
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Binding::Inherit { from, names })
            }
        })
        .collect()
}

fn param(node: ast::Param) -> Result<Param, Error> {
    Ok(match node {
        ast::Param::IdentParam(node) => {
            Param::Ident(required(node.ident(), "parameter name")?.to_string())
        }
        ast::Param::Pattern(node) => {
            let formals = node
                .pat_entries()
                .map(|entry| {
                    Ok(Formal {
                        name: required(entry.ident(), "formal name")?.to_string(),
                        default: entry.default().map(expr).transpose()?,
                    })
                })
                .collect::<Result<_, Error>>()?;
            let at = node
                .pat_bind()
                .map(|bind| {
                    required(bind.ident(), "bound parameter name").map(|name| name.to_string())
                })
                .transpose()?;
            Param::Formals {
                formals,
                ellipsis: node.ellipsis_token().is_some(),
                at,
            }
        }
    })
}

fn binop(node: &ast::BinOp) -> Result<BinOp, Error> {
    use ast::BinOpKind as Op;
    Ok(match required(node.operator(), "binary operator")? {
        Op::Concat => BinOp::Concat,
        Op::Update => BinOp::Update,
        Op::Add => BinOp::Add,
        Op::Sub => BinOp::Sub,
        Op::Mul => BinOp::Mul,
        Op::Div => BinOp::Div,
        Op::And => BinOp::And,
        Op::Equal => BinOp::Eq,
        Op::Implication => BinOp::Impl,
        Op::Less => BinOp::Lt,
        Op::LessOrEq => BinOp::Le,
        Op::More => BinOp::Gt,
        Op::MoreOrEq => BinOp::Ge,
        Op::NotEqual => BinOp::Ne,
        Op::Or => BinOp::Or,
        Op::PipeRight | Op::PipeLeft => return Err(unsupported(node)),
    })
}
