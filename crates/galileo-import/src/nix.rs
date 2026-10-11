use galileo_nix::{AttrKey, AttrPath, BinOp, Binding, Expr, Formal, Param, StrPart};
use rnix::ast::{self, AstNode, HasEntry};
use std::fmt;

#[derive(Debug)]
enum Error {
    Syntax(rnix::ParseError),
    Unsupported {
        kind: rnix::SyntaxKind,
        range: rnix::TextRange,
        construct: String,
        path: String,
    },
    Missing(&'static str),
    Integer(std::num::ParseIntError),
    Float(std::num::ParseFloatError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(error) => write!(f, "{error}"),
            Self::Unsupported {
                kind,
                range,
                construct,
                path,
            } => {
                write!(
                    f,
                    "unsupported Nix import construct {construct} at {path} (node {kind:?}, {range:?})"
                )
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

pub fn parse(source: &str) -> Result<Expr, String> {
    convert(source).map_err(|error| error.to_string())
}

fn convert(source: &str) -> Result<Expr, Error> {
    let root = rnix::Root::parse(source).ok().map_err(Error::Syntax)?;
    expr(required(root.expr(), "root expression")?)
}

fn required<T>(value: Option<T>, field: &'static str) -> Result<T, Error> {
    value.ok_or(Error::Missing(field))
}

fn unsupported(node: &impl AstNode) -> Error {
    let syntax = node.syntax();
    let construct = match syntax.kind() {
        rnix::SyntaxKind::NODE_LET_IN => "let in".into(),
        rnix::SyntaxKind::NODE_LEGACY_LET => "let".into(),
        rnix::SyntaxKind::NODE_ASSERT => "assert".into(),
        rnix::SyntaxKind::NODE_HAS_ATTR => "?".into(),
        rnix::SyntaxKind::NODE_PATH_ABS => "absolute path".into(),
        rnix::SyntaxKind::NODE_PATH_SEARCH => "angle path".into(),
        kind => format!("{kind:?}"),
    };
    let mut paths = syntax
        .ancestors()
        .filter(|node| node.kind() == rnix::SyntaxKind::NODE_ATTRPATH_VALUE)
        .filter_map(|binding| {
            binding
                .children()
                .find(|node| node.kind() == rnix::SyntaxKind::NODE_ATTRPATH)
        })
        .map(|path| path.to_string().replace('\n', " "))
        .collect::<Vec<_>>();
    paths.reverse();
    Error::Unsupported {
        kind: syntax.kind(),
        range: syntax.text_range(),
        construct,
        path: if paths.is_empty() {
            "<root>".into()
        } else {
            paths.join(".")
        },
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
        ast::Expr::IfElse(node) => Expr::If {
            cond: boxed(node.condition(), "if condition")?,
            then: boxed(node.body(), "then expression")?,
            else_: boxed(node.else_body(), "else expression")?,
        },
        ast::Expr::With(node) => Expr::With {
            scope: boxed(node.namespace(), "with scope")?,
            body: boxed(node.body(), "with body")?,
        },
        ast::Expr::UnaryOp(node) => {
            let operand = boxed(node.expr(), "unary operand")?;
            match required(node.operator(), "unary operator")? {
                ast::UnaryOpKind::Invert => Expr::Not(operand),
                ast::UnaryOpKind::Negate => Expr::Neg(operand),
            }
        }
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

pub fn lift(root: &Expr) -> Result<galileo_syntax::record::RecordDecl, String> {
    use galileo_syntax::record::RecordDecl;
    if let Some(module) = lift_module(root)? {
        return Ok(RecordDecl::Module(module));
    }
    lift_file(root).map(RecordDecl::File)
}

fn lift_lets(mut body: &Expr) -> Result<(Vec<galileo_syntax::record::Let>, &Expr), String> {
    let mut lets = Vec::new();
    while let Expr::Apply { func, arg } = body {
        let Expr::Lambda {
            param: Param::Ident(name),
            body: inner,
        } = func.as_ref()
        else {
            break;
        };
        lets.push(galileo_syntax::record::Let {
            name: lift_name(name, "<root>")?,
            value: lift_expr(arg, name)?,
            line: 1,
        });
        body = inner;
    }
    Ok((lets, body))
}

fn lift_module(root: &Expr) -> Result<Option<galileo_syntax::record::NixModule>, String> {
    use galileo_syntax::record::{ModuleItem, NixModule};
    let Expr::Lambda { param, body } = root else {
        return Ok(None);
    };
    match param {
        Param::Formals {
            formals, at: None, ..
        } if !formals.is_empty() && formals.iter().all(|formal| formal.default.is_none()) => {}
        _ => return Ok(None),
    }
    let arguments = lift_arguments(param, "<root>")?;
    let (lets, body) = lift_lets(body)?;
    let names = lets
        .iter()
        .map(|value| value.name.clone())
        .collect::<Vec<_>>();
    let mut items = lets.into_iter().map(ModuleItem::Let).collect::<Vec<_>>();
    let Expr::AttrSet {
        rec: false,
        bindings,
    } = body
    else {
        return Ok(None);
    };
    for binding in bindings {
        let Binding::Attr { path, value } = binding else {
            return Ok(None);
        };
        let location = lift_location("", path);
        match path.as_slice() {
            [key] if lift_key_name(key) == Some("imports") => {
                items.push(ModuleItem::Imports(lift_expr(value, &location)?));
            }
            [key] if lift_key_name(key) == Some("config") => {
                let (value, when) = lift_guarded(value);
                let Expr::AttrSet {
                    rec: false,
                    bindings,
                } = value
                else {
                    return Ok(None);
                };
                if bindings.is_empty() {
                    return Ok(None);
                }
                items.push(ModuleItem::Config {
                    when: when.map(|value| lift_expr(value, &location)).transpose()?,
                    entries: lift_entries(bindings, &location)?,
                    line: 1,
                });
            }
            [key, rest @ ..] if lift_key_name(key) == Some("options") => {
                if !lift_options(rest, value, &names, &location, &mut items)? {
                    return Ok(None);
                }
            }
            _ => return Ok(None),
        }
    }
    Ok(Some(NixModule {
        arguments,
        items,
        line: 1,
    }))
}

fn lift_options(
    prefix: &[AttrKey],
    value: &Expr,
    names: &[String],
    location: &str,
    items: &mut Vec<galileo_syntax::record::ModuleItem>,
) -> Result<bool, String> {
    use galileo_syntax::record::{ModuleItem, OptionDecl, OptionField, OptionKind};
    if let Some(arg) = lift_lib_call(value, "mkEnableOption") {
        let literal = matches!(arg, Expr::Str(_));
        let bound = matches!(arg, Expr::Ident(name) if names.contains(name));
        if prefix.is_empty() || !(literal || bound) {
            return Ok(false);
        }
        items.push(ModuleItem::Option(OptionDecl {
            path: lift_path(&prefix.to_vec(), location)?,
            kind: OptionKind::Enable(lift_expr(arg, location)?),
            fields: Vec::new(),
        }));
        return Ok(true);
    }
    if let Some(arg) = lift_lib_call(value, "mkOption") {
        let Expr::AttrSet {
            rec: false,
            bindings,
        } = arg
        else {
            return Ok(false);
        };
        let mut fields = Vec::new();
        let mut ty = None;
        let mut seen = Vec::new();
        for binding in bindings {
            let Binding::Attr { path, value } = binding else {
                return Ok(false);
            };
            let [key] = path.as_slice() else {
                return Ok(false);
            };
            let Some(name) = lift_key_name(key) else {
                return Ok(false);
            };
            if !["type", "default", "description"].contains(&name) || seen.contains(&name) {
                return Ok(false);
            }
            seen.push(name);
            if name == "type" {
                let Some(value) = lift_type(value) else {
                    return Ok(false);
                };
                ty = Some(value);
            } else {
                fields.push(OptionField {
                    name: name.to_owned(),
                    value: lift_expr(value, &format!("{location}.{name}"))?,
                    line: 1,
                });
            }
        }
        let Some(ty) = ty else {
            return Ok(false);
        };
        if prefix.is_empty() {
            return Ok(false);
        }
        items.push(ModuleItem::Option(OptionDecl {
            path: lift_path(&prefix.to_vec(), location)?,
            kind: OptionKind::Type(ty),
            fields,
        }));
        return Ok(true);
    }
    let Expr::AttrSet {
        rec: false,
        bindings,
    } = value
    else {
        return Ok(false);
    };
    if bindings.is_empty() {
        return Ok(false);
    }
    for binding in bindings {
        let Binding::Attr { path, value } = binding else {
            return Ok(false);
        };
        let nested = prefix.iter().chain(path).cloned().collect::<Vec<_>>();
        if !lift_options(&nested, value, names, &lift_location(location, path), items)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn lift_lib_call<'a>(value: &'a Expr, name: &str) -> Option<&'a Expr> {
    let Expr::Apply { func, arg } = value else {
        return None;
    };
    (lift_lib_name(func, &[])? == name).then_some(arg.as_ref())
}

fn lift_lib_name<'a>(value: &'a Expr, prefix: &[&str]) -> Option<&'a str> {
    let Expr::Select {
        expr,
        path,
        default: None,
    } = value
    else {
        return None;
    };
    if !matches!(expr.as_ref(), Expr::Ident(name) if name == "lib") {
        return None;
    }
    let keys = path.iter().map(lift_key_name).collect::<Option<Vec<_>>>()?;
    match keys.split_last() {
        Some((last, rest)) if rest == prefix => Some(last),
        _ => None,
    }
}

fn lift_key_name(key: &AttrKey) -> Option<&str> {
    match key {
        AttrKey::Ident(name) => Some(name),
        AttrKey::Str(parts) => match parts.as_slice() {
            [StrPart::Text(name)] => Some(name),
            _ => None,
        },
        AttrKey::Dynamic(_) => None,
    }
}

fn lift_type(value: &Expr) -> Option<galileo_syntax::types::Type> {
    use galileo_syntax::types::Type;
    let named = |name: &str, args| Type::Named {
        name: name.into(),
        args,
        span: galileo_syntax::Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        },
    };
    if let Expr::Apply { func, arg } = value {
        return match lift_lib_name(func, &["types"])? {
            "listOf" => Some(named("List", vec![lift_type(arg)?])),
            "nullOr" => Some(Type::Maybe(Box::new(lift_type(arg)?))),
            _ => None,
        };
    }
    let name = match lift_lib_name(value, &["types"])? {
        "bool" => "Bool",
        "int" => "Int",
        "str" => "Text",
        "path" => "Path",
        "port" => "Port",
        _ => return None,
    };
    Some(named(name, Vec::new()))
}

fn lift_file(root: &Expr) -> Result<galileo_syntax::record::NixFile, String> {
    use galileo_syntax::record::{FileValue, NixFile};
    let (arguments, body) = match root {
        Expr::Lambda {
            param: param @ Param::Formals { .. },
            body,
        } => (Some(lift_arguments(param, "<root>")?), body.as_ref()),
        _ => (None, root),
    };
    let (lets, body) = lift_lets(body)?;
    let value = match body {
        Expr::AttrSet {
            rec: false,
            bindings,
        } if !bindings.is_empty() => FileValue::Entries(lift_entries(bindings, "<root>")?),
        _ => FileValue::Expr(lift_expr(body, "<root>")?),
    };
    Ok(NixFile {
        arguments,
        lets,
        value,
        line: 1,
    })
}

fn lift_error(construct: &str, location: &str) -> String {
    format!("unsupported Nix import construct {construct} at {location}")
}

fn lift_name(name: &str, location: &str) -> Result<String, String> {
    if name.contains(['`', '\n']) {
        return Err(lift_error("unrepresentable name", location));
    }
    Ok(name.to_owned())
}

fn lift_arguments(
    param: &Param,
    location: &str,
) -> Result<galileo_syntax::record::Arguments, String> {
    let Param::Formals {
        formals,
        ellipsis,
        at,
    } = param
    else {
        return Err(lift_error("module identifier parameter", location));
    };
    if at.is_some() {
        return Err(lift_error("@", location));
    }
    if let Some(formal) = formals.iter().find(|formal| formal.default.is_some()) {
        return Err(lift_error(
            "formal default",
            &format!("{location}.{}", formal.name),
        ));
    }
    if formals.is_empty() {
        return Err(lift_error("empty formals", location));
    }
    Ok(galileo_syntax::record::Arguments {
        names: formals
            .iter()
            .map(|formal| lift_name(&formal.name, location))
            .collect::<Result<_, _>>()?,
        open: *ellipsis,
    })
}

fn lift_location(parent: &str, path: &AttrPath) -> String {
    let keys = path
        .iter()
        .map(|key| match key {
            AttrKey::Ident(name) => name.clone(),
            AttrKey::Str(parts) => parts
                .iter()
                .map(|part| match part {
                    StrPart::Text(text) => text.as_str(),
                    StrPart::Interp(_) => "${...}",
                })
                .collect::<String>(),
            AttrKey::Dynamic(_) => "${...}".into(),
        })
        .collect::<Vec<_>>()
        .join(".");
    if parent.is_empty() || parent == "<root>" {
        keys
    } else {
        format!("{parent}.{keys}")
    }
}

fn lift_path(path: &AttrPath, location: &str) -> Result<galileo_syntax::record::AttrPath, String> {
    use galileo_syntax::record::Segment;
    let segments = path
        .iter()
        .map(|key| match key {
            AttrKey::Ident(name) => Ok(Segment::Name(lift_name(name, location)?)),
            AttrKey::Str(parts) => {
                let text = parts
                    .iter()
                    .map(|part| match part {
                        StrPart::Text(text) if text.contains('\r') => {
                            Err(lift_error("carriage return in string", location))
                        }
                        StrPart::Text(text) => Ok(Some(text.as_str())),
                        StrPart::Interp(_) => Ok(None),
                    })
                    .collect::<Result<Option<String>, String>>()?;
                match text {
                    Some(text) => Ok(Segment::Text(text)),
                    None => Ok(Segment::Computed(lift_expr(&Expr::Str(parts.clone()), location)?)),
                }
            }
            AttrKey::Dynamic(value) => Ok(Segment::Computed(lift_expr(value, location)?)),
        })
        .collect::<Result<_, String>>()?;
    Ok(galileo_syntax::record::AttrPath { segments, line: 1 })
}

fn lift_entries(
    bindings: &[Binding],
    parent: &str,
) -> Result<Vec<galileo_syntax::record::Entry>, String> {
    use galileo_syntax::record::Entry;
    bindings
        .iter()
        .map(|binding| {
            let Binding::Attr { path, value } = binding else {
                return Err(lift_error("inherit", parent));
            };
            let location = lift_location(parent, path);
            let path = lift_path(path, &location)?;
            let (value, when) = lift_guarded(value);
            let when = when.map(|value| lift_expr(value, &location)).transpose()?;
            match value {
                Expr::AttrSet {
                    rec: false,
                    bindings,
                } if !bindings.is_empty() => Ok(Entry::Nested {
                    path,
                    when,
                    entries: lift_entries(bindings, &location)?,
                }),
                Expr::IndentedStr(parts) if when.is_none() => Ok(Entry::Value {
                    path,
                    when,
                    value: lift_block(parts, &location)?,
                }),
                Expr::IndentedStr(_) => Err(lift_error("guarded indented string", &location)),
                _ => Ok(Entry::Value {
                    path,
                    when,
                    value: lift_expr(value, &location)?,
                }),
            }
        })
        .collect()
}

fn lift_guarded(value: &Expr) -> (&Expr, Option<&Expr>) {
    if let Expr::Apply { func, arg } = value {
        if let Expr::Apply {
            func,
            arg: condition,
        } = func.as_ref()
        {
            if let Expr::Select {
                expr,
                path,
                default: None,
            } = func.as_ref()
            {
                if matches!(expr.as_ref(), Expr::Ident(name) if name == "lib")
                    && matches!(path.as_slice(), [key] if lift_key_name(key) == Some("mkIf"))
                {
                    return (arg, Some(condition));
                }
            }
        }
    }
    (value, None)
}

fn lift_text(
    parts: &[StrPart],
    location: &str,
) -> Result<Vec<galileo_syntax::expr::TextPart>, String> {
    use galileo_syntax::expr::TextPart;
    parts
        .iter()
        .map(|part| match part {
            StrPart::Text(text) if !text.contains('\r') => Ok(TextPart::Text(text.clone())),
            StrPart::Text(_) => Err(lift_error("carriage return in string", location)),
            StrPart::Interp(value) => Ok(TextPart::Interpolation(lift_expr(value, location)?)),
        })
        .collect()
}

fn lift_expr(value: &Expr, location: &str) -> Result<galileo_syntax::expr::Expr, String> {
    use galileo_syntax::expr::{BinOp as Op, ExprKind as Kind, Field, Formal, Param as Parameter};
    let boxed = |value| lift_expr(value, location).map(Box::new);
    let kind = match value {
        Expr::Int(value) if *value >= 0 => Kind::Int(*value),
        Expr::Ident(name) => match name.as_str() {
            "true" => Kind::Bool(true),
            "false" => Kind::Bool(false),
            "null" => Kind::Null,
            _ => Kind::Name(lift_name(name, location)?),
        },
        Expr::Str(parts) => Kind::Text(lift_text(parts, location)?),
        Expr::IndentedStr(_) => return Err(lift_error("inline indented string", location)),
        Expr::Path(path) if !path.starts_with('<') => Kind::Path(path.clone()),
        Expr::Path(_) => return Err(lift_error("angle path", location)),
        Expr::List(values) => Kind::List(
            values
                .iter()
                .map(|value| lift_expr(value, location))
                .collect::<Result<_, _>>()?,
        ),
        Expr::AttrSet {
            rec: false,
            bindings,
        } => Kind::Record(
            bindings
                .iter()
                .map(|binding| {
                    let Binding::Attr { path, value } = binding else {
                        return Err(lift_error("inherit", location));
                    };
                    let location = lift_location(location, path);
                    Ok(Field {
                        path: lift_path(path, &location)?,
                        value: lift_expr(value, &location)?,
                    })
                })
                .collect::<Result<_, String>>()?,
        ),
        Expr::Select {
            expr,
            path,
            default: None,
        } => {
            let mut value = lift_expr(expr, location)?;
            for key in path {
                let Some(name) = lift_key_name(key) else {
                    return Err(lift_error("selection by non-name key", location));
                };
                value = galileo_syntax::expr::Expr {
                    kind: Kind::Field(Box::new(value), lift_name(name, location)?),
                    span: galileo_syntax::Span {
                        start: 0,
                        end: 0,
                        line: 1,
                        column: 1,
                    },
                };
            }
            return Ok(value);
        }
        Expr::Apply { .. } => {
            let mut function = value;
            let mut arguments = Vec::new();
            while let Expr::Apply { func, arg } = function {
                arguments.push(lift_expr(arg, location)?);
                function = func;
            }
            arguments.reverse();
            Kind::Call(boxed(function)?, arguments)
        }
        Expr::Lambda { param, body } => {
            let parameter = match param {
                Param::Ident(name) => Parameter::Name {
                    name: lift_name(name, location)?,
                    ty: None,
                },
                Param::Formals { .. } => {
                    let arguments = lift_arguments(param, location)?;
                    Parameter::Formals {
                        fields: arguments
                            .names
                            .into_iter()
                            .map(|name| Formal {
                                name,
                                default: None,
                            })
                            .collect(),
                        open: arguments.open,
                    }
                }
            };
            Kind::Lambda(vec![parameter], boxed(body)?)
        }
        Expr::If { cond, then, else_ } => Kind::If(boxed(cond)?, boxed(then)?, boxed(else_)?),
        Expr::With { scope, body } => Kind::With(boxed(scope)?, boxed(body)?),
        Expr::BinOp { op, lhs, rhs } => {
            let op = match op {
                BinOp::Or => Op::Or,
                BinOp::And => Op::And,
                BinOp::Eq => Op::Eq,
                BinOp::Ne => Op::Ne,
                BinOp::Lt => Op::Lt,
                BinOp::Le => Op::Le,
                BinOp::Gt => Op::Gt,
                BinOp::Ge => Op::Ge,
                BinOp::Update => Op::Update,
                BinOp::Add if lift_has_text(lhs) || lift_has_text(rhs) => Op::Concat,
                BinOp::Add => Op::Add,
                BinOp::Sub => Op::Sub,
                BinOp::Mul => Op::Mul,
                BinOp::Concat => Op::Concat,
                BinOp::Div => return Err(lift_error("/", location)),
                BinOp::Impl => return Err(lift_error("->", location)),
            };
            Kind::Binary(op, boxed(lhs)?, boxed(rhs)?)
        }
        Expr::Not(value) => Kind::Not(boxed(value)?),
        Expr::Int(_) | Expr::Neg(_) => return Err(lift_error("negation", location)),
        Expr::Float(_) => return Err(lift_error("float", location)),
        Expr::Uri(_) => return Err(lift_error("URI", location)),
        Expr::AttrSet { rec: true, .. } => return Err(lift_error("rec", location)),
        Expr::Select {
            default: Some(_), ..
        } => return Err(lift_error("or default", location)),
        Expr::HasAttr { .. } => return Err(lift_error("?", location)),
        Expr::Let { .. } => return Err(lift_error("let in", location)),
        Expr::Assert { .. } => return Err(lift_error("assert", location)),
    };
    Ok(galileo_syntax::expr::Expr {
        kind,
        span: galileo_syntax::Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        },
    })
}

fn lift_has_text(value: &Expr) -> bool {
    match value {
        Expr::Str(_) | Expr::IndentedStr(_) => true,
        Expr::BinOp {
            op: BinOp::Add,
            lhs,
            rhs,
        } => lift_has_text(lhs) || lift_has_text(rhs),
        _ => false,
    }
}

fn lift_block(parts: &[StrPart], location: &str) -> Result<galileo_syntax::expr::Expr, String> {
    let mut text = String::new();
    for part in parts {
        match part {
            StrPart::Text(part) if !part.contains("{{") && !part.contains(['\r', '\t']) => {
                text.push_str(part)
            }
            StrPart::Text(_) => {
                return Err(lift_error("unrepresentable text-block content", location));
            }
            StrPart::Interp(_) => text.push_str("{{...}}"),
        }
    }
    if !text.ends_with('\n') || text.lines().any(|line| line.trim() == "'''") {
        return Err(lift_error(
            "text block without trailing newline or with closing marker",
            location,
        ));
    }
    Ok(galileo_syntax::expr::Expr {
        kind: galileo_syntax::expr::ExprKind::TextBlock(lift_text(parts, location)?),
        span: galileo_syntax::Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        },
    })
}
