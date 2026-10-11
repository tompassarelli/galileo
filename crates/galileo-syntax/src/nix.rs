use galileo_nix as n;

use crate::record::{
    Arguments, AttrPath, BinOp, Entry, Expr, ExprKind, FileValue, ModuleItem, NixFile, NixModule,
    OptionDecl, OptionKind, Param, Segment, TextPart,
};
use crate::types::Type;
use crate::{lex, Error};
use galileo_core as ir;
use std::collections::{BTreeMap, BTreeSet};

/// Answers the `for` query at a source line, given the enclosing `for` blocks' bindings: one row per binding.
pub type Answers<'a> = &'a dyn Fn(usize, &[(String, ir::Value)]) -> Result<Vec<Vec<ir::Value>>, String>;

pub fn no_answers(_: usize, _: &[(String, ir::Value)]) -> Result<Vec<Vec<ir::Value>>, String> {
    Err("a `for` block has no Nix lowering without the program's answers".into())
}

pub struct Lower<'a> {
    pub text_concat: &'a BTreeSet<(usize, usize)>,
    pub answers: Answers<'a>,
}

impl Lower<'_> {

    pub fn lower_module(&self, module: &NixModule) -> Result<n::Expr, Error> {
        let mut bound: BTreeMap<String, Option<ir::Value>> = module.arguments.names.iter().map(|name| (name.clone(), None)).collect();
        let interleaved = module.items.iter().position(|item| !matches!(item, ModuleItem::Let(_)))
            .is_some_and(|first| module.items[first + 1..].iter().any(|item| matches!(item, ModuleItem::Let(_))));
        let mut lets = Vec::new();
        let mut bindings = Vec::new();
        for item in &module.items {
            let first = bindings.len();
            match item {
                ModuleItem::Let(binding) => {
                    let value = self.scoped_expr(&binding.value, &bound)?;
                    let known = crate::lower::known_expr(&binding.value, &bound);
                    bound.insert(binding.name.clone(), known);
                    lets.push((binding.name.clone(), value));
                },
                ModuleItem::Option(option) => bindings.push(self.option_binding(option, &bound)?),
                ModuleItem::Imports(value) => bindings.push(n::attr("imports", self.scoped_expr(value, &bound)?)),
                ModuleItem::Config { when, entries, .. } => bindings.push(n::attr(
                    "config",
                    self.guarded(when.as_ref(), n::attrs(self.entry_bindings(&expand(entries, &[], self.answers)?, &bound)?), &bound)?,
                )),
                ModuleItem::For(block) => bindings
                    .extend(self.entry_bindings(&expand(&[Entry::For(block.clone())], &[], self.answers)?, &bound)?),
            }
            if interleaved {
                for binding in &mut bindings[first..] {
                    if let n::Binding::Attr { value, .. } = binding {
                        *value = self.sequential_lets(lets.clone(), value.clone());
                    }
                }
            }
        }
        let body = if interleaved { n::attrs(bindings) } else { self.sequential_lets(lets, n::attrs(bindings)) };
        Ok(self.formals(&module.arguments, body))
    }

    pub fn lower_file(&self, file: &NixFile) -> Result<n::Expr, Error> {
        let mut bound: BTreeMap<String, Option<ir::Value>> = file.arguments.iter().flat_map(|arguments| &arguments.names).map(|name| (name.clone(), None)).collect();
        let mut lets = Vec::new();
        for binding in &file.lets {
            let value = self.scoped_expr(&binding.value, &bound)?;
            let known = crate::lower::known_expr(&binding.value, &bound);
            bound.insert(binding.name.clone(), known);
            lets.push((binding.name.clone(), value));
        }
        let value = match &file.value {
            FileValue::Expr(value) => self.scoped_expr(value, &bound)?,
            FileValue::Entries(entries) => n::attrs(self.entry_bindings(&expand(entries, &[], self.answers)?, &bound)?),
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

    fn guarded(&self, when: Option<&Expr>, value: n::Expr, bound: &BTreeMap<String, Option<ir::Value>>) -> Result<n::Expr, Error> {
        Ok(match when {
            Some(cond) => n::apply(n::apply(n::select(n::ident("lib"), "mkIf"), self.scoped_expr(cond, bound)?), value),
            None => value,
        })
    }

    fn entry_bindings(&self, entries: &[Entry], bound: &BTreeMap<String, Option<ir::Value>>) -> Result<Vec<n::Binding>, Error> {
        entries
            .iter()
            .map(|entry| match entry {
                Entry::Value { path, value, when } => Ok(n::Binding::Attr {
                    path: self.attrpath(path, bound)?,
                    value: self.guarded(when.as_ref(), self.scoped_expr(value, bound)?, bound)?,
                }),
                Entry::Nested { path, when, entries } => Ok(n::Binding::Attr {
                    path: self.attrpath(path, bound)?,
                    value: self.guarded(when.as_ref(), n::attrs(self.entry_bindings(entries, bound)?), bound)?,
                }),
                Entry::For(block) => Err(unsupported(block.line, "a `for` block")),
            })
            .collect()
    }

    fn attrpath(&self, path: &AttrPath, bound: &BTreeMap<String, Option<ir::Value>>) -> Result<n::AttrPath, Error> {
        path.segments
            .iter()
            .map(|segment| {
                Ok(match segment {
                    Segment::Name(name) => n::AttrKey::Ident(name.clone()),
                    Segment::Text(text) => n::AttrKey::Str(vec![n::StrPart::Text(text.clone())]),
                    Segment::Computed(Expr { kind: ExprKind::Text(parts), .. }) => {
                        n::AttrKey::Str(self.text(parts, bound)?)
                    }
                    Segment::Computed(e) => n::AttrKey::Dynamic(self.scoped_expr(e, bound)?),
                })
            })
            .collect()
    }

    fn option_binding(&self, option: &OptionDecl, bound: &BTreeMap<String, Option<ir::Value>>) -> Result<n::Binding, Error> {
        let mut path = vec![n::AttrKey::Ident("options".into())];
        path.extend(self.attrpath(&option.path, bound)?);
        let line = option.path.line;
        let value = match &option.kind {
            OptionKind::Enable(description) => {
                if !option.fields.is_empty() {
                    return Err(at(line, "an `enable` option takes no fields"));
                }
                n::apply(n::select(n::ident("lib"), "mkEnableOption"), self.scoped_expr(description, bound)?)
            }
            OptionKind::Type(ty) => {
                let mut fields = vec![n::attr("type", self.option_type(ty, line)?)];
                let rank = |name: &str| ["default", "description"].iter().position(|f| *f == name).unwrap_or(2);
                let mut declared: Vec<_> = option.fields.iter().collect();
                declared.sort_by_key(|field| rank(&field.name));
                for field in declared {
                    fields.push(n::attr(&field.name, self.scoped_expr(&field.value, bound)?));
                }
                n::apply(n::select(n::ident("lib"), "mkOption"), n::attrs(fields))
            }
        };
        Ok(n::Binding::Attr { path, value })
    }

    fn option_type(&self, ty: &Type, line: usize) -> Result<n::Expr, Error> {
        let types = |name: &str| n::select(n::ident("lib"), &format!("types.{name}"));
        Ok(match ty {
            Type::Maybe(inner) => n::apply(types("nullOr"), self.option_type(inner, line)?),
            Type::Named { name, args, .. } => match (name.as_str(), args.as_slice()) {
                ("Bool", []) => types("bool"),
                ("Int", []) => types("int"),
                ("Text", []) => types("str"),
                ("Path", []) => types("path"),
                ("Port", []) => types("port"),
                ("List", [inner]) => n::apply(types("listOf"), self.option_type(inner, line)?),
                _ => return Err(at(line, "expected an option type from SPEC §10.2: Bool, Int, Text, Path, Port, List T or maybe T")),
            },
            Type::Nix(_) => return Err(at(line, "`nix T` is not an option type from SPEC §10.2")),
        })
    }

    pub fn expr(&self, e: &Expr) -> Result<n::Expr, Error> {
        self.scoped_expr(e, &BTreeMap::new())
    }

    fn scoped_expr(&self, e: &Expr, bound: &BTreeMap<String, Option<ir::Value>>) -> Result<n::Expr, Error> {
        Ok(match &e.kind {
            ExprKind::Null => n::ident("null"),
            ExprKind::Bool(b) => n::ident(if *b { "true" } else { "false" }),
            ExprKind::Int(i) => n::Expr::Int(*i),
            ExprKind::Text(parts) => n::Expr::Str(self.text(parts, bound)?),
            ExprKind::TextBlock(parts) => n::Expr::IndentedStr(self.text(parts, bound)?),
            ExprKind::Path(p) => n::path(p),
            ExprKind::Name(name) => {
                if crate::check::PRELUDE.contains(&name.as_str()) && !bound.contains_key(name) {
                    return Err(at(e.span.line, &format!("`{name}` is a Galileo prelude function; call it with known arguments")));
                }
                n::ident(nix_name(name, e)?)
            },
            ExprKind::List(items) => n::Expr::List(items.iter().map(|e| self.scoped_expr(e, bound)).collect::<Result<_, _>>()?),
            ExprKind::Record(fields) => n::attrs(
                fields
                    .iter()
                    .map(|f| Ok(n::Binding::Attr { path: self.attrpath(&f.path, bound)?, value: self.scoped_expr(&f.value, bound)? }))
                    .collect::<Result<_, Error>>()?,
            ),
            ExprKind::Field(inner, name) => match self.scoped_expr(inner, bound)? {
                n::Expr::Select { expr: base, mut path, default: None } if !inner.grouped() => {
                    path.push(n::AttrKey::Ident(name.clone()));
                    n::Expr::Select { expr: base, path, default: None }
                }
                base => n::select(base, name),
            },
            ExprKind::Call(func, args) => {
                if let ExprKind::Name(name) = &func.kind {
                    if crate::check::PRELUDE.contains(&name.as_str()) && !bound.contains_key(name) {
                        let value = crate::lower::known_expr(e, bound)
                            .ok_or_else(|| at(e.span.line, &format!("`{name}` could not be computed from known arguments")))?;
                        return self.scoped_expr(&nix_value(&value, e)?, bound);
                    }
                }
                args.iter().try_fold(self.scoped_expr(func, bound)?, |f, arg| Ok::<_, Error>(n::apply(f, self.scoped_expr(arg, bound)?)))?
            }
            ExprKind::Lambda(params, body) => {
                let mut inner = bound.clone();
                for param in params {
                    match param {
                        Param::Name { name, .. } => { inner.insert(name.clone(), None); }
                        Param::Formals { fields, .. } => {
                            for field in fields { inner.insert(field.name.clone(), None); }
                        }
                    }
                }
                params.iter().rev().try_fold(self.scoped_expr(body, &inner)?, |body, param| {
                    Ok::<_, Error>(match param {
                        Param::Name { name, .. } => n::Expr::Lambda { param: n::Param::Ident(nix_name(name, e)?.into()), body: Box::new(body) },
                        Param::Formals { fields, open } => n::Expr::Lambda {
                            param: n::Param::Formals {
                                formals: fields.iter().map(|field| Ok(n::Formal {
                                    name: nix_name(&field.name, e)?.into(),
                                    default: field.default.as_ref().map(|default| self.scoped_expr(default, bound)).transpose()?,
                                })).collect::<Result<_, Error>>()?,
                                ellipsis: *open,
                                at: None,
                            },
                            body: Box::new(body),
                        },
                    })
                })?
            }
            ExprKind::If(c, t, f) => n::Expr::If {
                cond: Box::new(self.scoped_expr(c, bound)?),
                then: Box::new(self.scoped_expr(t, bound)?),
                else_: Box::new(self.scoped_expr(f, bound)?),
            },
            ExprKind::Binary(op, l, r) => n::Expr::BinOp {
                op: self.binop(*op, l, r),
                lhs: Box::new(self.scoped_expr(l, bound)?),
                rhs: Box::new(self.scoped_expr(r, bound)?),
            },
            ExprKind::Not(inner) => n::Expr::Not(Box::new(self.scoped_expr(inner, bound)?)),
            ExprKind::With(scope, body) => n::Expr::With { scope: Box::new(self.scoped_expr(scope, bound)?), body: Box::new(self.scoped_expr(body, bound)?) },
            ExprKind::NixModule(arguments, entries) => {
                let mut inner = bound.clone();
                for name in &arguments.names { inner.insert(name.clone(), None); }
                self.formals(arguments, n::attrs(self.entry_bindings(entries, &inner)?))
            },
            ExprKind::Var(_) | ExprKind::Query(_) => {
                return Err(Error { line: e.span.line, column: e.span.column, message: "a query or `?` variable has no Nix lowering".into(), text: None })
            }
        })
    }

    fn text(&self, parts: &[TextPart], bound: &BTreeMap<String, Option<ir::Value>>) -> Result<Vec<n::StrPart>, Error> {
        let mut out = Vec::new();
        for part in parts {
            let part = match part {
                TextPart::Text(text) => n::StrPart::Text(text.clone()),
                TextPart::Interpolation(e) => match crate::lower::known_expr(e, bound) {
                    Some(ir::Value::Text(text)) => n::StrPart::Text(text),
                    Some(ir::Value::Int(value)) => n::StrPart::Text(value.to_string()),
                    Some(ir::Value::Path(_)) => n::StrPart::Interp(self.scoped_expr(e, bound)?),
                    Some(_) => return Err(at(e.span.line, "text interpolation needs Text, Int or a Nix value")),
                    None => n::StrPart::Interp(self.scoped_expr(e, bound)?),
                },
            };
            match (out.last_mut(), part) {
                (Some(n::StrPart::Text(previous)), n::StrPart::Text(text)) => previous.push_str(&text),
                (_, part) => out.push(part),
            }
        }
        Ok(out)
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

fn expand(entries: &[Entry], bound: &[(String, ir::Value)], answers: Answers) -> Result<Vec<Entry>, Error> {
    let mut out = Vec::new();
    for entry in entries {
        match entry {
            Entry::Value { path, value, when } => out.push(Entry::Value {
                path: splice(path, bound)?,
                value: substitute(value, bound)?,
                when: when.as_ref().map(|w| substitute(w, bound)).transpose()?,
            }),
            Entry::Nested { path, when, entries } => out.push(Entry::Nested {
                path: splice(path, bound)?,
                when: when.as_ref().map(|w| substitute(w, bound)).transpose()?,
                entries: expand(entries, bound, answers)?,
            }),
            Entry::For(block) => {
                if let Some(name) = block.vars.iter().find(|v| bound.iter().any(|(b, _)| b == *v)) {
                    return Err(at(block.line, &format!("`?{name}` is already bound by an enclosing `for`")));
                }
                for row in answers(block.line, bound).map_err(|message| at(block.line, &message))? {
                    if row.len() != block.vars.len() {
                        return Err(at(block.line, "the query's rows don't match the `for` variables"));
                    }
                    let mut inner = bound.to_vec();
                    inner.extend(block.vars.iter().cloned().zip(row));
                    out.extend(expand(&block.entries, &inner, answers)?);
                }
            }
        }
    }
    Ok(out)
}

fn lookup<'a>(bound: &'a [(String, ir::Value)], name: &str) -> Option<&'a ir::Value> {
    bound.iter().rev().find(|(b, _)| b == name).map(|(_, value)| value)
}

fn splice(path: &AttrPath, bound: &[(String, ir::Value)]) -> Result<AttrPath, Error> {
    let mut segments = Vec::new();
    for segment in &path.segments {
        let Segment::Computed(e) = segment else {
            segments.push(segment.clone());
            continue;
        };
        let ExprKind::Var(name) = &e.kind else {
            segments.push(Segment::Computed(substitute(e, bound)?));
            continue;
        };
        match lookup(bound, name) {
            None => segments.push(segment.clone()),
            Some(ir::Value::Entity(entity)) => segments.push(Segment::Name(entity.name.clone())),
            Some(ir::Value::OptionPath(names)) => segments.extend(names.iter().cloned().map(Segment::Name)),
            Some(ir::Value::Text(text)) => segments.push(Segment::Name(text.clone())),
            Some(other) => return Err(at(path.line, &format!("`?{name}` holds {other:?}, which names no attribute"))),
        }
    }
    Ok(AttrPath { segments, line: path.line })
}

fn substitute(e: &Expr, bound: &[(String, ir::Value)]) -> Result<Expr, Error> {
    let each = |items: &[Expr]| items.iter().map(|i| substitute(i, bound)).collect::<Result<Vec<_>, _>>();
    let boxed = |inner: &Expr| substitute(inner, bound).map(Box::new);
    let parts = |parts: &[TextPart]| {
        parts
            .iter()
            .map(|part| match part {
                TextPart::Interpolation(e) => substitute(e, bound).map(TextPart::Interpolation),
                text => Ok(text.clone()),
            })
            .collect::<Result<Vec<_>, Error>>()
    };
    let kind = match &e.kind {
        ExprKind::Var(name) => match lookup(bound, name) {
            Some(value) => return nix_value(value, e),
            None => return Ok(e.clone()),
        },
        ExprKind::Text(p) => ExprKind::Text(parts(p)?),
        ExprKind::TextBlock(p) => ExprKind::TextBlock(parts(p)?),
        ExprKind::List(items) => ExprKind::List(each(items)?),
        ExprKind::Record(fields) => ExprKind::Record(
            fields
                .iter()
                .map(|f| Ok(crate::expr::Field { path: splice(&f.path, bound)?, value: substitute(&f.value, bound)? }))
                .collect::<Result<_, Error>>()?,
        ),
        ExprKind::Field(inner, name) => ExprKind::Field(boxed(inner)?, name.clone()),
        ExprKind::Call(function, arguments) => ExprKind::Call(boxed(function)?, each(arguments)?),
        ExprKind::Lambda(params, body) => ExprKind::Lambda(
            params.iter().map(|param| Ok(match param {
                Param::Formals { fields, open } => Param::Formals {
                    fields: fields.iter().map(|field| Ok(crate::expr::Formal {
                        name: field.name.clone(),
                        default: field.default.as_ref().map(|default| substitute(default, bound)).transpose()?,
                    })).collect::<Result<_, Error>>()?,
                    open: *open,
                },
                other => other.clone(),
            })).collect::<Result<_, Error>>()?,
            boxed(body)?,
        ),
        ExprKind::If(c, t, f) => ExprKind::If(boxed(c)?, boxed(t)?, boxed(f)?),
        ExprKind::Binary(op, l, r) => ExprKind::Binary(*op, boxed(l)?, boxed(r)?),
        ExprKind::Not(inner) => ExprKind::Not(boxed(inner)?),
        ExprKind::With(scope, body) => ExprKind::With(boxed(scope)?, boxed(body)?),
        other => other.clone(),
    };
    Ok(Expr { kind, span: e.span.clone() })
}

fn nix_value(value: &ir::Value, site: &Expr) -> Result<Expr, Error> {
    let kind = match value {
        ir::Value::Null => ExprKind::Null,
        ir::Value::Bool(b) => ExprKind::Bool(*b),
        ir::Value::Int(i) => ExprKind::Int(*i),
        ir::Value::Text(t) => ExprKind::Text(vec![TextPart::Text(t.clone())]),
        ir::Value::Path(p) => ExprKind::Path(p.clone()),
        ir::Value::List(items) => ExprKind::List(items.iter().map(|i| nix_value(i, site)).collect::<Result<_, _>>()?),
        ir::Value::Nix { normalized } => {
            let line = lex(normalized).map_err(|e| at(site.span.line, &e.message))?.remove(0);
            return crate::expr::parse_expr(&line.tokens, &line)
                .map(|parsed| Expr { kind: parsed.kind, span: site.span.clone() })
                .map_err(|e| at(site.span.line, &e.message));
        }
        other => return Err(at(site.span.line, &format!("{other:?} has no Nix value"))),
    };
    Ok(Expr { kind, span: site.span.clone() })
}
