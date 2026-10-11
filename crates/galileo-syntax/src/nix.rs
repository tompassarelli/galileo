use galileo_nix as n;

use crate::record::{
    Arguments, AttrPath, BinOp, Entry, Expr, ExprKind, FileValue, ForHead, ModuleItem, NixFile, NixModule,
    OptionDecl, OptionKind, Param, Segment, TextPart,
};
use crate::types::Type;
use crate::check::NixScope;
use crate::{lex, Error};
use galileo_core as ir;
use std::collections::{BTreeMap, BTreeSet};

/// Answers the `for` query whose header starts at a source offset, given the enclosing `for` blocks' bindings: one row per binding.
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
        let expanded = module.items.iter().map(|item| match item {
            ModuleItem::Config { entries, .. } => expand(entries, &[], self.answers).map(Some),
            ModuleItem::For(block) => expand(&[Entry::For(block.clone())], &[], self.answers).map(Some),
            _ => Ok(None),
        }).collect::<Result<Vec<_>, Error>>()?;
        {
            let mut known = bound.clone();
            let mut scope = arguments_scope(Some(&module.arguments));
            for (item, expanded) in module.items.iter().zip(&expanded) {
                match item {
                    ModuleItem::Let(binding) => {
                        names_expr(&binding.value, &known, &mut scope)?;
                        known.insert(binding.name.clone(), crate::lower::known_expr(&binding.value, &known));
                        scope.bind(&binding.name, binding.line);
                    }
                    ModuleItem::Option(_) | ModuleItem::OptionGroup { .. } => {
                        for option in crate::record::item_declarations(item) {
                            names_path(&option.path, &known, &mut scope)?;
                            if let OptionKind::Enable(description) = &option.kind {
                                names_expr(description, &known, &mut scope)?;
                            }
                            for field in &option.fields {
                                names_expr(&field.value, &known, &mut scope)?;
                            }
                        }
                    }
                    ModuleItem::Imports(value) => names_expr(value, &known, &mut scope)?,
                    ModuleItem::Config { when, .. } => {
                        if let Some(when) = when {
                            names_expr(when, &known, &mut scope)?;
                        }
                        names_entries(expanded.as_deref().unwrap_or_default(), &known, &mut scope)?;
                    }
                    ModuleItem::For(_) => names_entries(expanded.as_deref().unwrap_or_default(), &known, &mut scope)?,
                }
            }
        }
        let mut expanded = expanded.into_iter();
        let mut lets = Vec::new();
        let mut bindings = Vec::new();
        for item in &module.items {
            let first = bindings.len();
            let expanded = expanded.next().flatten().unwrap_or_default();
            match item {
                ModuleItem::Let(binding) => {
                    let value = self.scoped_expr(&binding.value, &bound)?;
                    let known = crate::lower::known_expr(&binding.value, &bound);
                    bound.insert(binding.name.clone(), known);
                    lets.push((binding.name.clone(), value));
                },
                ModuleItem::Option(option) => {
                    let mut path = vec![n::AttrKey::Ident("options".into())];
                    path.extend(self.attrpath(&option.path, &bound)?);
                    bindings.push(n::Binding::Attr { path, value: self.option_value(option, &bound)? });
                }
                ModuleItem::OptionGroup { path: group, options } => {
                    let mut path = vec![n::AttrKey::Ident("options".into())];
                    path.extend(self.attrpath(group, &bound)?);
                    let children = options
                        .iter()
                        .map(|option| Ok(n::Binding::Attr { path: self.attrpath(&option.path, &bound)?, value: self.option_value(option, &bound)? }))
                        .collect::<Result<Vec<_>, Error>>()?;
                    bindings.push(n::Binding::Attr { path, value: n::attrs(children) });
                }
                ModuleItem::Imports(value) => bindings.push(n::attr("imports", self.scoped_expr(value, &bound)?)),
                ModuleItem::Config { when, .. } => bindings.push(n::attr(
                    "config",
                    self.guarded(when.as_ref(), n::attrs(self.entry_bindings(&expanded, &bound)?), &bound)?,
                )),
                ModuleItem::For(_) => bindings.extend(self.entry_bindings(&expanded, &bound)?),
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
        let expanded = match &file.value {
            FileValue::Entries(entries) => Some(expand(entries, &[], self.answers)?),
            FileValue::Expr(_) => None,
        };
        {
            let mut known = bound.clone();
            let mut scope = arguments_scope(file.arguments.as_ref());
            for binding in &file.lets {
                names_expr(&binding.value, &known, &mut scope)?;
                known.insert(binding.name.clone(), crate::lower::known_expr(&binding.value, &known));
                scope.bind(&binding.name, binding.line);
            }
            match (&file.value, &expanded) {
                (FileValue::Expr(value), _) => names_expr(value, &known, &mut scope)?,
                (_, Some(entries)) => names_entries(entries, &known, &mut scope)?,
                _ => {}
            }
        }
        let mut lets = Vec::new();
        for binding in &file.lets {
            let value = self.scoped_expr(&binding.value, &bound)?;
            let known = crate::lower::known_expr(&binding.value, &bound);
            bound.insert(binding.name.clone(), known);
            lets.push((binding.name.clone(), value));
        }
        let value = match (&file.value, expanded) {
            (FileValue::Entries(_), Some(entries)) => n::attrs(self.entry_bindings(&entries, &bound)?),
            _ => self.scoped_expr(value_expr(&file.value), &bound)?,
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
        path.segments.iter().map(|segment| self.segment(segment, bound)).collect()
    }

    fn segment(&self, segment: &Segment, bound: &BTreeMap<String, Option<ir::Value>>) -> Result<n::AttrKey, Error> {
        Ok(match segment {
            Segment::Name(name) => n::AttrKey::Ident(name.clone()),
            Segment::Text(text) => n::AttrKey::Str(vec![n::StrPart::Text(text.clone())]),
            Segment::Computed(Expr { kind: ExprKind::Text(parts), .. }) => n::AttrKey::Str(self.text(parts, bound)?),
            Segment::Computed(e) => n::AttrKey::Dynamic(self.scoped_expr(e, bound)?),
        })
    }

    fn option_value(&self, option: &OptionDecl, bound: &BTreeMap<String, Option<ir::Value>>) -> Result<n::Expr, Error> {
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
        Ok(value)
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
            ExprKind::Int(i) => nix_int(*i),
            ExprKind::Text(parts) => n::Expr::Str(self.text(parts, bound)?),
            ExprKind::TextBlock(parts) => n::Expr::IndentedStr(self.text(parts, bound)?),
            ExprKind::Path(p) => n::path(p),
            ExprKind::Name(name) => {
                if crate::check::PRELUDE.contains(&name.as_str()) && !bound.contains_key(name) {
                    return Err(at(e.span.line, &format!("`{name}` is a Galileo prelude function; call it with known arguments")));
                }
                n::ident(nix_name(name, e)?)
            },
            ExprKind::Computed(inner) => n::ident(nix_name(&computed_name(inner, bound)?, e)?),
            ExprKind::List(items) => n::Expr::List(items.iter().map(|e| self.scoped_expr(e, bound)).collect::<Result<_, _>>()?),
            ExprKind::Record(fields) if fields.iter().any(|f| !f.each.is_empty()) => self.scoped_expr(&substitute(e, &[], self.answers)?, bound)?,
            ExprKind::Lambda(params, _) if params.iter().any(|p| matches!(p, Param::Formals { fields, .. } if fields.iter().any(|f| !f.each.is_empty()))) => {
                self.scoped_expr(&substitute(e, &[], self.answers)?, bound)?
            }
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
            ExprKind::Select(inner, segment) => {
                let key = self.segment(segment, bound)?;
                match self.scoped_expr(inner, bound)? {
                    n::Expr::Select { expr: base, mut path, default: None } if !inner.grouped() => {
                        path.push(key);
                        n::Expr::Select { expr: base, path, default: None }
                    }
                    base => n::Expr::Select { expr: Box::new(base), path: vec![key], default: None },
                }
            }
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
                            for field in fields { inner.insert(formal_name(field, bound)?, None); }
                        }
                    }
                }
                params.iter().rev().try_fold(self.scoped_expr(body, &inner)?, |body, param| {
                    Ok::<_, Error>(match param {
                        Param::Name { name, .. } => n::Expr::Lambda { param: n::Param::Ident(nix_name(name, e)?.into()), body: Box::new(body) },
                        Param::Formals { fields, open } => n::Expr::Lambda {
                            param: n::Param::Formals {
                                formals: fields.iter().map(|field| Ok(n::Formal {
                                    name: nix_name(&formal_name(field, bound)?, e)?.into(),
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
            ExprKind::Binary(BinOp::In, l, r) => match crate::lower::known_expr(e, bound) {
                Some(value @ ir::Value::Bool(_)) => return self.scoped_expr(&nix_value(&value, e)?, bound),
                _ => {
                    let side = if crate::lower::known_expr(l, bound).is_none() { l } else { r };
                    return Err(at(side.span.line, &crate::check::in_stage_message(&crate::record::print_expr(side, 0, 0))));
                }
            },
            ExprKind::Binary(op, l, r) => n::Expr::BinOp {
                op: self.binop(*op, l, r),
                lhs: Box::new(self.scoped_expr(l, bound)?),
                rhs: Box::new(self.scoped_expr(r, bound)?),
            },
            ExprKind::Not(inner) => n::Expr::Not(Box::new(self.scoped_expr(inner, bound)?)),
            ExprKind::Neg(inner) => {
                if let Some(ir::Value::Int(value)) = crate::lower::known_expr(inner, bound) {
                    if value.checked_neg().is_none() {
                        return Err(at(e.span.line, &format!("integer overflow: `-` of {value} is out of range")));
                    }
                }
                n::Expr::Neg(Box::new(self.scoped_expr(inner, bound)?))
            }
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
                    Some(ir::Value::Text(_)) if matches!(&e.kind, ExprKind::Name(name) if matches!(bound.get(name), Some(Some(ir::Value::Text(_))))) => {
                        n::StrPart::Interp(self.scoped_expr(e, bound)?)
                    }
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
            BinOp::In => unreachable!("`in` is computed before emission"),
            BinOp::Update => n::BinOp::Update,
            BinOp::Add => n::BinOp::Add,
            BinOp::Sub => n::BinOp::Sub,
            BinOp::Mul => n::BinOp::Mul,
            BinOp::Concat if self.text_concat.contains(&(l.span.start, r.span.end)) => n::BinOp::Add,
            BinOp::Concat => n::BinOp::Concat,
        }
    }
}

fn value_expr(value: &FileValue) -> &Expr {
    match value {
        FileValue::Expr(value) => value,
        FileValue::Entries(_) => unreachable!("entries are expanded"),
    }
}

type Known = BTreeMap<String, Option<ir::Value>>;

fn arguments_scope(arguments: Option<&Arguments>) -> NixScope {
    let mut scope = NixScope::new();
    scope.push();
    for name in arguments.iter().flat_map(|arguments| &arguments.names) {
        scope.bind(name, 0);
    }
    scope
}

fn scope_error(e: &Expr, message: String) -> Error {
    Error { line: e.span.line, column: e.span.column, message, text: None }
}

fn names_entries(entries: &[Entry], known: &Known, scope: &mut NixScope) -> Result<(), Error> {
    for entry in entries {
        match entry {
            Entry::Value { path, value, when } => {
                names_path(path, known, scope)?;
                names_expr(value, known, scope)?;
                if let Some(when) = when {
                    names_expr(when, known, scope)?;
                }
            }
            Entry::Nested { path, when, entries } => {
                names_path(path, known, scope)?;
                if let Some(when) = when {
                    names_expr(when, known, scope)?;
                }
                names_entries(entries, known, scope)?;
            }
            Entry::For(_) => {}
        }
    }
    Ok(())
}

fn names_path(path: &AttrPath, known: &Known, scope: &mut NixScope) -> Result<(), Error> {
    for segment in &path.segments {
        if let Segment::Computed(e) = segment {
            names_expr(e, known, scope)?;
        }
    }
    Ok(())
}

fn names_expr(e: &Expr, known: &Known, scope: &mut NixScope) -> Result<(), Error> {
    match &e.kind {
        ExprKind::Computed(inner) => {
            let name = computed_name(inner, known)?;
            scope.reference(&name).map_err(|message| scope_error(e, message))
        }
        ExprKind::Record(fields) => {
            for field in fields {
                names_path(&field.path, known, scope)?;
                names_expr(&field.value, known, scope)?;
            }
            Ok(())
        }
        ExprKind::Lambda(params, body) => {
            let mut formals: Vec<(String, Option<&Expr>)> = Vec::new();
            for param in params {
                match param {
                    Param::Name { name, .. } => formals.push((name.clone(), None)),
                    Param::Formals { fields, .. } => {
                        for field in fields {
                            if let Some(default) = &field.default {
                                names_expr(default, known, scope)?;
                            }
                            formals.push((formal_name(field, known)?, field.computed.as_ref()));
                        }
                    }
                }
            }
            for (index, (name, computed)) in formals.iter().enumerate() {
                if let Some(computed) = computed {
                    let others: Vec<String> = formals.iter().enumerate().filter(|(other, _)| *other != index).map(|(_, (name, _))| name.clone()).collect();
                    scope.formal(name, &others).map_err(|message| scope_error(computed, message))?;
                }
            }
            let mut inner = known.clone();
            scope.push();
            for (name, computed) in &formals {
                scope.bind(name, computed.map_or(e.span.line, |computed| computed.span.line));
                inner.insert(name.clone(), None);
            }
            let result = names_expr(body, &inner, scope);
            scope.pop();
            result
        }
        ExprKind::NixModule(arguments, entries) => {
            let mut inner = known.clone();
            scope.push();
            for name in &arguments.names {
                scope.bind(name, e.span.line);
                inner.insert(name.clone(), None);
            }
            let result = names_entries(entries, &inner, scope);
            scope.pop();
            result
        }
        _ => {
            let mut children = Vec::new();
            crate::check::sub_exprs(e, &mut |child| children.push(child));
            children.into_iter().try_for_each(|child| names_expr(child, known, scope))
        }
    }
}

fn computed_name(e: &Expr, bound: &BTreeMap<String, Option<ir::Value>>) -> Result<String, Error> {
    match crate::lower::known_expr(e, bound) {
        Some(ir::Value::Text(text)) => Ok(text),
        _ => Err(at(e.span.line, "a computed name `{e}` needs a known `Text`")),
    }
}

fn formal_name(field: &crate::expr::Formal, bound: &BTreeMap<String, Option<ir::Value>>) -> Result<String, Error> {
    match &field.computed {
        Some(e) => computed_name(e, bound),
        None => Ok(field.name.clone()),
    }
}

fn nix_int(value: i64) -> n::Expr {
    match value.checked_neg() {
        Some(magnitude) if value < 0 => n::Expr::Neg(Box::new(n::Expr::Int(magnitude))),
        _ => n::Expr::Int(value),
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
                value: substitute(value, bound, answers)?,
                when: when.as_ref().map(|w| substitute(w, bound, answers)).transpose()?,
            }),
            Entry::Nested { path, when, entries } => out.push(Entry::Nested {
                path: splice(path, bound)?,
                when: when.as_ref().map(|w| substitute(w, bound, answers)).transpose()?,
                entries: expand(entries, bound, answers)?,
            }),
            Entry::For(block) => {
                for inner in unroll(std::slice::from_ref(&block.head), bound, answers, &mut |inner| Ok(inner.to_vec()))? {
                    out.extend(expand(&block.entries, &inner, answers)?);
                }
            }
        }
    }
    Ok(out)
}

fn rows(head: &ForHead, bound: &[(String, ir::Value)], answers: Answers) -> Result<Vec<Vec<ir::Value>>, Error> {
    if let Some(name) = head.vars.iter().find(|v| bound.iter().any(|(b, _)| b == *v)) {
        return Err(at(head.line, &format!("`?{name}` is already bound by an enclosing `for`")));
    }
    let rows: Vec<Vec<ir::Value>> = match (&head.source.kind, head.vars.len()) {
        (ExprKind::List(_), 1) => match crate::lower::known_expr(&substitute(&head.source, bound, answers)?, &BTreeMap::new()) {
            Some(ir::Value::List(items)) => items.into_iter().map(|item| vec![item]).collect(),
            _ => return Err(at(head.line, "a `for` block's list must be known when Galileo builds")),
        },
        _ => answers(head.offset, bound).map_err(|message| at(head.line, &message))?,
    };
    if rows.iter().any(|row| row.len() != head.vars.len()) {
        return Err(at(head.line, "the query's rows don't match the `for` variables"));
    }
    Ok(rows)
}

fn unroll<T>(
    each: &[ForHead],
    bound: &[(String, ir::Value)],
    answers: Answers,
    item: &mut dyn FnMut(&[(String, ir::Value)]) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
    let Some((head, rest)) = each.split_first() else {
        return Ok(vec![item(bound)?]);
    };
    let mut out = Vec::new();
    for row in rows(head, bound, answers)? {
        let mut inner = bound.to_vec();
        inner.extend(head.vars.iter().cloned().zip(row));
        out.extend(unroll(rest, &inner, answers, item)?);
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
            segments.push(Segment::Computed(substitute(e, bound, &no_answers)?));
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

fn substitute(e: &Expr, bound: &[(String, ir::Value)], answers: Answers) -> Result<Expr, Error> {
    let each = |items: &[Expr]| items.iter().map(|i| substitute(i, bound, answers)).collect::<Result<Vec<_>, _>>();
    let boxed = |inner: &Expr| substitute(inner, bound, answers).map(Box::new);
    let parts = |parts: &[TextPart]| {
        parts
            .iter()
            .map(|part| match part {
                TextPart::Interpolation(e) => substitute(e, bound, answers).map(TextPart::Interpolation),
                text => Ok(text.clone()),
            })
            .collect::<Result<Vec<_>, Error>>()
    };
    let kind = match &e.kind {
        ExprKind::Var(name) => match lookup(bound, name) {
            Some(value) => return nix_value(value, e),
            None => return Ok(e.clone()),
        },
        ExprKind::Computed(inner) => ExprKind::Computed(boxed(inner)?),
        ExprKind::Text(p) => ExprKind::Text(parts(p)?),
        ExprKind::TextBlock(p) => ExprKind::TextBlock(parts(p)?),
        ExprKind::List(items) => ExprKind::List(each(items)?),
        ExprKind::Record(fields) => {
            let mut out = Vec::new();
            for f in fields {
                out.extend(unroll(&f.each, bound, answers, &mut |bound| {
                    Ok(crate::expr::Field { each: Vec::new(), path: splice(&f.path, bound)?, value: substitute(&f.value, bound, answers)? })
                })?);
            }
            ExprKind::Record(out)
        }
        ExprKind::Field(inner, name) => ExprKind::Field(boxed(inner)?, name.clone()),
        ExprKind::Select(inner, segment) => ExprKind::Select(boxed(inner)?, Box::new(match segment.as_ref() {
            Segment::Computed(key) => Segment::Computed(substitute(key, bound, answers)?),
            other => other.clone(),
        })),
        ExprKind::Call(function, arguments) => ExprKind::Call(boxed(function)?, each(arguments)?),
        ExprKind::Lambda(params, body) => ExprKind::Lambda(
            params.iter().map(|param| Ok(match param {
                Param::Formals { fields, open } => {
                    let mut out = Vec::new();
                    for field in fields {
                        out.extend(unroll(&field.each, bound, answers, &mut |bound| Ok(crate::expr::Formal {
                            each: Vec::new(),
                            name: field.name.clone(),
                            default: field.default.as_ref().map(|default| substitute(default, bound, answers)).transpose()?,
                            computed: field.computed.as_ref().map(|name| substitute(name, bound, answers)).transpose()?,
                        }))?);
                    }
                    Param::Formals { fields: out, open: *open }
                }
                other => other.clone(),
            })).collect::<Result<_, Error>>()?,
            boxed(body)?,
        ),
        ExprKind::If(c, t, f) => ExprKind::If(boxed(c)?, boxed(t)?, boxed(f)?),
        ExprKind::Binary(op, l, r) => ExprKind::Binary(*op, boxed(l)?, boxed(r)?),
        ExprKind::Not(inner) => ExprKind::Not(boxed(inner)?),
        ExprKind::Neg(inner) => ExprKind::Neg(boxed(inner)?),
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
