use std::collections::BTreeSet;

use crate::*;

pub fn print_idiomatic(expr: &Expr) -> Result<String, String> {
    let mut expr = expr.clone();
    idiomatic(&mut expr)?;
    Ok(print(&expr))
}

fn idiomatic(expr: &mut Expr) -> Result<(), String> {
    if matches!(expr, Expr::Apply { func, .. } if matches!(func.as_ref(), Expr::Lambda { param: Param::Ident(_), .. })) {
        let mut bindings = Vec::new();
        let mut body = expr.clone();
        while let Expr::Apply { func, arg } = &body {
            let Expr::Lambda { param: Param::Ident(name), body: next } = func.as_ref() else {
                break;
            };
            bindings.push((name.clone(), arg.as_ref().clone()));
            body = next.as_ref().clone();
        }
        for (index, (name, value)) in bindings.iter().enumerate() {
            let free = free(value);
            for (captured, _) in &bindings[index..] {
                if free.contains(captured) {
                    return Err(format!("capture error: binding `{name}` mentions `{captured}`, which recursive Nix let would capture"));
                }
            }
        }
        *expr = Expr::Let { bindings: bindings.into_iter().map(|(name, value)| attr(&name, value)).collect(), body: Box::new(body) };
    }
    children(expr, &mut idiomatic)
}

fn children(expr: &mut Expr, visit: &mut impl FnMut(&mut Expr) -> Result<(), String>) -> Result<(), String> {
    fn parts(parts: &mut [StrPart], visit: &mut impl FnMut(&mut Expr) -> Result<(), String>) -> Result<(), String> {
        for part in parts {
            if let StrPart::Interp(expr) = part {
                visit(expr)?;
            }
        }
        Ok(())
    }
    fn keys(keys: &mut [AttrKey], visit: &mut impl FnMut(&mut Expr) -> Result<(), String>) -> Result<(), String> {
        for key in keys {
            match key {
                AttrKey::Str(value) => parts(value, visit)?,
                AttrKey::Dynamic(value) => visit(value)?,
                AttrKey::Ident(_) => {}
            }
        }
        Ok(())
    }
    fn bindings(bindings: &mut [Binding], visit: &mut impl FnMut(&mut Expr) -> Result<(), String>) -> Result<(), String> {
        for binding in bindings {
            match binding {
                Binding::Attr { path, value } => {
                    keys(path, visit)?;
                    visit(value)?;
                }
                Binding::Inherit { from: Some(value), .. } => visit(value)?,
                Binding::Inherit { .. } => {}
            }
        }
        Ok(())
    }
    match expr {
        Expr::Str(value) | Expr::IndentedStr(value) => parts(value, visit)?,
        Expr::List(values) => {
            for value in values {
                visit(value)?;
            }
        }
        Expr::AttrSet { bindings: values, .. } => bindings(values, visit)?,
        Expr::Select { expr, path, default } => {
            visit(expr)?;
            keys(path, visit)?;
            if let Some(value) = default {
                visit(value)?;
            }
        }
        Expr::HasAttr { expr, path } => {
            visit(expr)?;
            keys(path, visit)?;
        }
        Expr::Apply { func, arg } => {
            visit(func)?;
            visit(arg)?;
        }
        Expr::Lambda { param, body } => {
            if let Param::Formals { formals, .. } = param {
                for formal in formals {
                    if let Some(value) = &mut formal.default {
                        visit(value)?;
                    }
                }
            }
            visit(body)?;
        }
        Expr::Let { bindings: values, body } => {
            bindings(values, visit)?;
            visit(body)?;
        }
        Expr::If { cond, then, else_ } => {
            visit(cond)?;
            visit(then)?;
            visit(else_)?;
        }
        Expr::With { scope, body } => {
            visit(scope)?;
            visit(body)?;
        }
        Expr::Assert { cond, body } => {
            visit(cond)?;
            visit(body)?;
        }
        Expr::BinOp { lhs, rhs, .. } => {
            visit(lhs)?;
            visit(rhs)?;
        }
        Expr::Not(value) | Expr::Neg(value) => visit(value)?,
        _ => {}
    }
    Ok(())
}

fn binding_names(bindings: &[Binding]) -> BTreeSet<String> {
    bindings
        .iter()
        .flat_map(|binding| match binding {
            Binding::Attr { path, .. } => match path.first() {
                Some(AttrKey::Ident(name)) => vec![name.clone()],
                _ => Vec::new(),
            },
            Binding::Inherit { names, .. } => names.clone(),
        })
        .collect()
}

fn free(expr: &Expr) -> BTreeSet<String> {
    if let Expr::Ident(name) = expr {
        return BTreeSet::from([name.clone()]);
    }
    let mut names = BTreeSet::new();
    let mut copy = expr.clone();
    children(&mut copy, &mut |child| {
        names.extend(free(child));
        Ok(())
    })
    .expect("free variable traversal is infallible");
    match expr {
        Expr::Lambda { param, .. } => match param {
            Param::Ident(name) => {
                names.remove(name);
            }
            Param::Formals { formals, at, .. } => {
                for formal in formals {
                    names.remove(&formal.name);
                }
                if let Some(name) = at {
                    names.remove(name);
                }
            }
        },
        Expr::Let { bindings, .. } | Expr::AttrSet { rec: true, bindings } => {
            for name in binding_names(bindings) {
                names.remove(&name);
            }
        }
        _ => {}
    }
    if let Expr::Let { bindings, .. } | Expr::AttrSet { bindings, .. } = expr {
        for binding in bindings {
            if let Binding::Inherit { from: None, names: inherited } = binding {
                names.extend(inherited.iter().cloned());
            }
        }
    }
    names
}
