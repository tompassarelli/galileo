use muove_nix as n;

use crate::record::{
    Arguments, AttrPath, BinOp, Entry, Expr, ExprKind, ModuleItem, NixModule, OptionDecl, Param,
    Segment, TextPart,
};
use crate::types::OptionType;
use crate::Error;

pub fn lower_module(module: &NixModule) -> Result<n::Expr, Error> {
    let mut lets = Vec::new();
    let mut bindings = Vec::new();
    for item in &module.items {
        match item {
            ModuleItem::Let(binding) => lets.push((binding.name.clone(), expr(&binding.value)?)),
            ModuleItem::Option(option) => bindings.push(option_binding(option)?),
            ModuleItem::Imports(value) => bindings.push(n::attr("imports", expr(value)?)),
            ModuleItem::Config { when, entries, .. } => bindings.push(n::attr(
                "config",
                guarded(when.as_ref(), n::attrs(entry_bindings(entries)?))?,
            )),
            ModuleItem::For(block) => return Err(unsupported(block.line, "a module-level `for` block")),
        }
    }
    let body = sequential_lets(lets, n::attrs(bindings));
    Ok(formals(&module.arguments, body))
}

fn sequential_lets(lets: Vec<(String, n::Expr)>, body: n::Expr) -> n::Expr {
    lets.into_iter().rev().fold(body, |body, (name, value)| {
        n::apply(
            n::Expr::Lambda { param: n::Param::Ident(name), body: Box::new(body) },
            value,
        )
    })
}

fn formals(arguments: &Arguments, body: n::Expr) -> n::Expr {
    let formals = arguments
        .names
        .iter()
        .map(|name| n::Formal { name: name.clone(), default: None })
        .collect();
    n::Expr::Lambda {
        param: n::Param::Formals { formals, ellipsis: arguments.open, at: None },
        body: Box::new(body),
    }
}

fn guarded(when: Option<&Expr>, value: n::Expr) -> Result<n::Expr, Error> {
    Ok(match when {
        Some(cond) => n::apply(n::apply(n::select(n::ident("lib"), "mkIf"), expr(cond)?), value),
        None => value,
    })
}

fn entry_bindings(entries: &[Entry]) -> Result<Vec<n::Binding>, Error> {
    entries
        .iter()
        .map(|entry| match entry {
            Entry::Value { path, value, when } => Ok(n::Binding::Attr {
                path: attrpath(path)?,
                value: guarded(when.as_ref(), expr(value)?)?,
            }),
            Entry::Nested { path, when, entries } => Ok(n::Binding::Attr {
                path: attrpath(path)?,
                value: guarded(when.as_ref(), n::attrs(entry_bindings(entries)?))?,
            }),
            Entry::For(block) => Err(unsupported(block.line, "a `for` block")),
        })
        .collect()
}

fn attrpath(path: &AttrPath) -> Result<n::AttrPath, Error> {
    path.segments
        .iter()
        .map(|segment| {
            Ok(match segment {
                Segment::Name(name) => n::AttrKey::Ident(name.clone()),
                Segment::Text(text) => n::AttrKey::Str(vec![n::StrPart::Text(text.clone())]),
                Segment::Computed(e) => n::AttrKey::Dynamic(expr(e)?),
            })
        })
        .collect()
}

fn option_binding(option: &OptionDecl) -> Result<n::Binding, Error> {
    let mut path = vec![n::AttrKey::Ident("options".into())];
    path.extend(attrpath(&option.path)?);
    let line = option.path.line;
    let value = if let OptionType::Enable(description) = &option.ty {
        if !option.fields.is_empty() {
            return Err(at(line, "an `enable` option takes no fields"));
        }
        n::apply(n::select(n::ident("lib"), "mkEnableOption"), expr(description)?)
    } else {
        let mut fields = vec![n::attr("type", option_type(&option.ty, line)?)];
        let rank = |name: &str| ["default", "description"].iter().position(|f| *f == name).unwrap_or(2);
        let mut declared: Vec<_> = option.fields.iter().collect();
        declared.sort_by_key(|field| rank(&field.name));
        for field in declared {
            fields.push(n::attr(&field.name, expr(&field.value)?));
        }
        n::apply(n::select(n::ident("lib"), "mkOption"), n::attrs(fields))
    };
    Ok(n::Binding::Attr { path, value })
}

fn option_type(ty: &OptionType, line: usize) -> Result<n::Expr, Error> {
    let types = |name: &str| n::select(n::ident("lib"), &format!("types.{name}"));
    Ok(match ty {
        OptionType::Bool => types("bool"),
        OptionType::Int => types("int"),
        OptionType::Text => types("str"),
        OptionType::Lines => types("lines"),
        OptionType::Path => types("path"),
        OptionType::Port => types("port"),
        OptionType::Package => types("package"),
        OptionType::ListOf(inner) => n::apply(types("listOf"), option_type(inner, line)?),
        OptionType::Maybe(inner) => n::apply(types("nullOr"), option_type(inner, line)?),
        OptionType::AttrsOf(inner) => n::apply(types("attrsOf"), option_type(inner, line)?),
        OptionType::OneOf(choices) => {
            n::apply(types("enum"), n::Expr::List(choices.iter().map(expr).collect::<Result<_, _>>()?))
        }
        OptionType::Enable(_) => return Err(at(line, "`enable` is a whole option type, not an element type")),
    })
}

pub fn expr(e: &Expr) -> Result<n::Expr, Error> {
    Ok(match &e.kind {
        ExprKind::Null => n::ident("null"),
        ExprKind::Bool(b) => n::ident(if *b { "true" } else { "false" }),
        ExprKind::Int(i) => n::Expr::Int(*i),
        ExprKind::Text(parts) => n::Expr::Str(text(parts)?),
        ExprKind::TextBlock(parts) => n::Expr::IndentedStr(text(parts)?),
        ExprKind::Path(p) => n::path(p),
        ExprKind::Name(name) => n::ident(name),
        ExprKind::List(items) => n::Expr::List(items.iter().map(expr).collect::<Result<_, _>>()?),
        ExprKind::Record(fields) => n::attrs(
            fields
                .iter()
                .map(|f| Ok(n::Binding::Attr { path: attrpath(&f.path)?, value: expr(&f.value)? }))
                .collect::<Result<_, Error>>()?,
        ),
        ExprKind::Field(inner, name) => match expr(inner)? {
            n::Expr::Select { expr: base, mut path, default: None } => {
                path.push(n::AttrKey::Ident(name.clone()));
                n::Expr::Select { expr: base, path, default: None }
            }
            base => n::select(base, name),
        },
        ExprKind::Call(func, args) => {
            args.iter().try_fold(expr(func)?, |f, arg| Ok::<_, Error>(n::apply(f, expr(arg)?)))?
        }
        ExprKind::Lambda(params, body) => {
            params.iter().rev().try_fold(expr(body)?, |body, param| {
                Ok::<_, Error>(match param {
                    Param::Name { name, .. } => n::Expr::Lambda { param: n::Param::Ident(name.clone()), body: Box::new(body) },
                    Param::Formals(arguments) => formals(arguments, body),
                })
            })?
        }
        ExprKind::If(c, t, f) => n::Expr::If {
            cond: Box::new(expr(c)?),
            then: Box::new(expr(t)?),
            else_: Box::new(expr(f)?),
        },
        ExprKind::Binary(op, l, r) => n::Expr::BinOp {
            op: binop(*op),
            lhs: Box::new(expr(l)?),
            rhs: Box::new(expr(r)?),
        },
        ExprKind::Not(inner) => n::Expr::Not(Box::new(expr(inner)?)),
        ExprKind::With(scope, body) => n::Expr::With { scope: Box::new(expr(scope)?), body: Box::new(expr(body)?) },
        ExprKind::NixModule(arguments, entries) => formals(arguments, n::attrs(entry_bindings(entries)?)),
        ExprKind::Var(_) | ExprKind::Query(_) => {
            return Err(Error { line: e.span.line, column: e.span.column, message: "a query or `?` variable has no Nix lowering".into(), text: None })
        }
    })
}

fn text(parts: &[TextPart]) -> Result<Vec<n::StrPart>, Error> {
    parts
        .iter()
        .map(|part| {
            Ok(match part {
                TextPart::Text(t) => n::StrPart::Text(t.clone()),
                TextPart::Interpolation(e) => n::StrPart::Interp(expr(e)?),
            })
        })
        .collect()
}

fn binop(op: BinOp) -> n::BinOp {
    match op {
        BinOp::Or => n::BinOp::Or,
        BinOp::And => n::BinOp::And,
        BinOp::Eq => n::BinOp::Eq,
        BinOp::Ne => n::BinOp::Ne,
        BinOp::Lt => n::BinOp::Lt,
        BinOp::Le => n::BinOp::Le,
        BinOp::Gt => n::BinOp::Gt,
        BinOp::Ge => n::BinOp::Ge,
        BinOp::Update => n::BinOp::Update,
        BinOp::Add => n::BinOp::Add,
        BinOp::Sub => n::BinOp::Sub,
        BinOp::Mul => n::BinOp::Mul,
        BinOp::Concat => n::BinOp::Concat,
    }
}

fn at(line: usize, message: &str) -> Error {
    Error { line, column: 1, message: message.into(), text: None }
}

fn unsupported(line: usize, what: &str) -> Error {
    at(line, &format!("{what} has no Nix lowering yet"))
}
