use muove_nix as n;

use crate::record::{
    Arguments, AttrPath, BinOp, Entry, Expr, ExprKind, FileValue, ModuleItem, NixFile, NixModule,
    OptionDecl, Param, Segment, TextPart,
};
use crate::types::OptionType;
use crate::Error;
use std::collections::BTreeSet;

pub struct Lower<'a> {
    pub text_concat: &'a BTreeSet<(usize, usize)>,
}

impl Lower<'_> {

    pub fn lower_module(&self, module: &NixModule) -> Result<n::Expr, Error> {
        let mut lets = Vec::new();
        let mut bindings = Vec::new();
        for item in &module.items {
            match item {
                ModuleItem::Let(binding) => lets.push((binding.name.clone(), self.expr(&binding.value)?)),
                ModuleItem::Option(option) => bindings.push(self.option_binding(option)?),
                ModuleItem::Imports(value) => bindings.push(n::attr("imports", self.expr(value)?)),
                ModuleItem::Config { when, entries, .. } => bindings.push(n::attr(
                    "config",
                    self.guarded(when.as_ref(), n::attrs(self.entry_bindings(entries)?))?,
                )),
                ModuleItem::For(block) => return Err(unsupported(block.line, "a module-level `for` block")),
            }
        }
        let body = self.sequential_lets(lets, n::attrs(bindings));
        Ok(self.formals(&module.arguments, body))
    }

    pub fn lower_file(&self, file: &NixFile) -> Result<n::Expr, Error> {
        let lets = file
            .lets
            .iter()
            .map(|binding| Ok((binding.name.clone(), self.expr(&binding.value)?)))
            .collect::<Result<Vec<_>, Error>>()?;
        let value = match &file.value {
            FileValue::Expr(value) => self.expr(value)?,
            FileValue::Entries(entries) => n::attrs(self.entry_bindings(entries)?),
        };
        let body = self.sequential_lets(lets, value);
        Ok(match &file.arguments {
            Some(arguments) => self.formals(arguments, body),
            None => body,
        })
    }

    fn sequential_lets(&self, lets: Vec<(String, n::Expr)>, body: n::Expr) -> n::Expr {
        lets.into_iter().rev().fold(body, |body, (name, value)| {
            n::apply(
                n::Expr::Lambda { param: n::Param::Ident(name), body: Box::new(body) },
                value,
            )
        })
    }

    fn formals(&self, arguments: &Arguments, body: n::Expr) -> n::Expr {
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

    fn guarded(&self, when: Option<&Expr>, value: n::Expr) -> Result<n::Expr, Error> {
        Ok(match when {
            Some(cond) => n::apply(n::apply(n::select(n::ident("lib"), "mkIf"), self.expr(cond)?), value),
            None => value,
        })
    }

    fn entry_bindings(&self, entries: &[Entry]) -> Result<Vec<n::Binding>, Error> {
        entries
            .iter()
            .map(|entry| match entry {
                Entry::Value { path, value, when } => Ok(n::Binding::Attr {
                    path: self.attrpath(path)?,
                    value: self.guarded(when.as_ref(), self.expr(value)?)?,
                }),
                Entry::Nested { path, when, entries } => Ok(n::Binding::Attr {
                    path: self.attrpath(path)?,
                    value: self.guarded(when.as_ref(), n::attrs(self.entry_bindings(entries)?))?,
                }),
                Entry::For(block) => Err(unsupported(block.line, "a `for` block")),
            })
            .collect()
    }

    fn attrpath(&self, path: &AttrPath) -> Result<n::AttrPath, Error> {
        path.segments
            .iter()
            .map(|segment| {
                Ok(match segment {
                    Segment::Name(name) => n::AttrKey::Ident(name.clone()),
                    Segment::Text(text) => n::AttrKey::Str(vec![n::StrPart::Text(text.clone())]),
                    Segment::Computed(e) => n::AttrKey::Dynamic(self.expr(e)?),
                })
            })
            .collect()
    }

    fn option_binding(&self, option: &OptionDecl) -> Result<n::Binding, Error> {
        let mut path = vec![n::AttrKey::Ident("options".into())];
        path.extend(self.attrpath(&option.path)?);
        let line = option.path.line;
        let value = if let OptionType::Enable(description) = &option.ty {
            if !option.fields.is_empty() {
                return Err(at(line, "an `enable` option takes no fields"));
            }
            n::apply(n::select(n::ident("lib"), "mkEnableOption"), self.expr(description)?)
        } else {
            let mut fields = vec![n::attr("type", self.option_type(&option.ty, line)?)];
            let rank = |name: &str| ["default", "description"].iter().position(|f| *f == name).unwrap_or(2);
            let mut declared: Vec<_> = option.fields.iter().collect();
            declared.sort_by_key(|field| rank(&field.name));
            for field in declared {
                fields.push(n::attr(&field.name, self.expr(&field.value)?));
            }
            n::apply(n::select(n::ident("lib"), "mkOption"), n::attrs(fields))
        };
        Ok(n::Binding::Attr { path, value })
    }

    fn option_type(&self, ty: &OptionType, line: usize) -> Result<n::Expr, Error> {
        let types = |name: &str| n::select(n::ident("lib"), &format!("types.{name}"));
        Ok(match ty {
            OptionType::Bool => types("bool"),
            OptionType::Int => types("int"),
            OptionType::Text => types("str"),
            OptionType::Lines => types("lines"),
            OptionType::Path => types("path"),
            OptionType::Port => types("port"),
            OptionType::Package => types("package"),
            OptionType::ListOf(inner) => n::apply(types("listOf"), self.option_type(inner, line)?),
            OptionType::Maybe(inner) => n::apply(types("nullOr"), self.option_type(inner, line)?),
            OptionType::AttrsOf(inner) => n::apply(types("attrsOf"), self.option_type(inner, line)?),
            OptionType::OneOf(choices) => {
                n::apply(types("enum"), n::Expr::List(choices.iter().map(|e| self.expr(e)).collect::<Result<_, _>>()?))
            }
            OptionType::Enable(_) => return Err(at(line, "`enable` is a whole option type, not an element type")),
        })
    }

    pub fn expr(&self, e: &Expr) -> Result<n::Expr, Error> {
        Ok(match &e.kind {
            ExprKind::Null => n::ident("null"),
            ExprKind::Bool(b) => n::ident(if *b { "true" } else { "false" }),
            ExprKind::Int(i) => n::Expr::Int(*i),
            ExprKind::Text(parts) => n::Expr::Str(self.text(parts)?),
            ExprKind::TextBlock(parts) => n::Expr::IndentedStr(self.text(parts)?),
            ExprKind::Path(p) => n::path(p),
            ExprKind::Name(name) => n::ident(nix_name(name, e)?),
            ExprKind::List(items) => n::Expr::List(items.iter().map(|e| self.expr(e)).collect::<Result<_, _>>()?),
            ExprKind::Record(fields) => n::attrs(
                fields
                    .iter()
                    .map(|f| Ok(n::Binding::Attr { path: self.attrpath(&f.path)?, value: self.expr(&f.value)? }))
                    .collect::<Result<_, Error>>()?,
            ),
            ExprKind::Field(inner, name) => match self.expr(inner)? {
                n::Expr::Select { expr: base, mut path, default: None } => {
                    path.push(n::AttrKey::Ident(name.clone()));
                    n::Expr::Select { expr: base, path, default: None }
                }
                base => n::select(base, name),
            },
            ExprKind::Call(func, args) => {
                args.iter().try_fold(self.expr(func)?, |f, arg| Ok::<_, Error>(n::apply(f, self.expr(arg)?)))?
            }
            ExprKind::Lambda(params, body) => {
                params.iter().rev().try_fold(self.expr(body)?, |body, param| {
                    Ok::<_, Error>(match param {
                        Param::Name { name, .. } => n::Expr::Lambda { param: n::Param::Ident(nix_name(name, e)?.into()), body: Box::new(body) },
                        Param::Formals(arguments) => self.formals(arguments, body),
                    })
                })?
            }
            ExprKind::If(c, t, f) => n::Expr::If {
                cond: Box::new(self.expr(c)?),
                then: Box::new(self.expr(t)?),
                else_: Box::new(self.expr(f)?),
            },
            ExprKind::Binary(op, l, r) => n::Expr::BinOp {
                op: self.binop(*op, l, r),
                lhs: Box::new(self.expr(l)?),
                rhs: Box::new(self.expr(r)?),
            },
            ExprKind::Not(inner) => n::Expr::Not(Box::new(self.expr(inner)?)),
            ExprKind::With(scope, body) => n::Expr::With { scope: Box::new(self.expr(scope)?), body: Box::new(self.expr(body)?) },
            ExprKind::NixModule(arguments, entries) => self.formals(arguments, n::attrs(self.entry_bindings(entries)?)),
            ExprKind::Var(_) | ExprKind::Query(_) => {
                return Err(Error { line: e.span.line, column: e.span.column, message: "a query or `?` variable has no Nix lowering".into(), text: None })
            }
        })
    }

    fn text(&self, parts: &[TextPart]) -> Result<Vec<n::StrPart>, Error> {
        parts
            .iter()
            .map(|part| {
                Ok(match part {
                    TextPart::Text(t) => n::StrPart::Text(t.clone()),
                    TextPart::Interpolation(e) => n::StrPart::Interp(self.expr(e)?),
                })
            })
            .collect()
    }

    fn binop(&self, op: BinOp, l: &Expr, r: &Expr) -> n::BinOp {
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
            BinOp::Concat if self.text_concat.contains(&(l.span.start, r.span.end)) => n::BinOp::Add,
            BinOp::Concat => n::BinOp::Concat,
        }
    }
}

fn nix_name<'a>(name: &'a str, e: &Expr) -> Result<&'a str, Error> {
    if n::KEYWORDS.contains(&name) {
        return Err(Error { line: e.span.line, column: e.span.column, message: format!("`{name}` is a Nix keyword, so it can't name a Nix binding or reference; rename it"), text: None });
    }
    Ok(name)
}

fn at(line: usize, message: &str) -> Error {
    Error { line, column: 1, message: message.into(), text: None }
}

fn unsupported(line: usize, what: &str) -> Error {
    at(line, &format!("{what} has no Nix lowering yet"))
}
