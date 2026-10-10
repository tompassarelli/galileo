use crate::ast::*;

pub const KEYWORDS: &[&str] = &["if", "then", "else", "assert", "with", "let", "in", "rec", "inherit", "or"];

pub fn print(expr: &Expr) -> String {
    let mut p = Printer { out: String::new(), indent: 0 };
    if let Expr::Lambda { param: param @ Param::Formals { .. }, body } = expr {
        p.param(param);
        p.s(":\n\n");
        p.expr(body, 15);
    } else {
        p.expr(expr, 15);
    }
    p.out.push('\n');
    p.out
}

struct Printer {
    out: String,
    indent: usize,
}

fn prec(expr: &Expr) -> u8 {
    match expr {
        Expr::Select { .. } => 1,
        Expr::Apply { .. } => 2,
        Expr::Neg(_) => 3,
        Expr::HasAttr { .. } => 4,
        Expr::BinOp { op, .. } => op.prec(),
        Expr::Not(_) => 8,
        Expr::Lambda { .. } | Expr::Let { .. } | Expr::If { .. } | Expr::With { .. } | Expr::Assert { .. } => 15,
        Expr::Float(f) if *f < 0.0 => 3,
        Expr::Int(i) if *i < 0 => 3,
        _ => 0,
    }
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '\'' || c == '-') && !KEYWORDS.contains(&s)
}

impl Printer {
    fn s(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn newline(&mut self) {
        self.out.push('\n');
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
    }

    fn expr(&mut self, expr: &Expr, max: u8) {
        let nested_module = self.indent > 0
            && matches!(expr, Expr::Lambda { param: Param::Formals { .. }, .. });
        if prec(expr) > max || nested_module || matches!(expr, Expr::BinOp { .. }) {
            self.s("(");
            self.bare(expr);
            self.s(")");
        } else {
            self.bare(expr);
        }
    }

    fn bare(&mut self, expr: &Expr) {
        match expr {
            Expr::Int(i) => self.s(&i.to_string()),
            Expr::Float(f) => self.s(&format!("{f:?}")),
            Expr::Uri(u) | Expr::Ident(u) | Expr::Path(u) => self.s(u),
            Expr::Str(parts) => self.string(parts),
            Expr::IndentedStr(parts) => self.indented(parts),
            Expr::List(items) => self.list(items),
            Expr::AttrSet { rec, bindings } => {
                if *rec {
                    self.s("rec ");
                }
                self.block("{", bindings, "}");
            }
            Expr::Select { expr, path, default } => {
                self.expr(expr, if matches!(expr.as_ref(), Expr::Select { .. }) { 0 } else { 1 });
                self.s(".");
                self.attrpath(path);
                if let Some(d) = default {
                    self.s(" or ");
                    self.expr(d, 1);
                }
            }
            Expr::HasAttr { expr, path } => {
                self.expr(expr, 3);
                self.s(" ? ");
                self.attrpath(path);
            }
            Expr::Apply { func, arg } => {
                let applied_lambda = matches!(func.as_ref(), Expr::Lambda { .. });
                if applied_lambda {
                    self.s("(");
                }
                self.expr(func, 2);
                self.s(" ");
                if applied_lambda {
                    self.indent += 1;
                    self.expr(arg, 1);
                    self.indent -= 1;
                    self.s(")");
                } else {
                    self.expr(arg, 1);
                }
            }
            Expr::Lambda { param, body } => {
                self.param(param);
                self.s(": ");
                self.expr(body, 15);
            }
            Expr::Let { bindings, body } => {
                self.block("let", bindings, "in");
                self.s(" ");
                self.expr(body, 15);
            }
            Expr::If { cond, then, else_ } => {
                self.s("if ");
                self.expr(cond, 15);
                self.s(" then ");
                self.expr(then, 15);
                self.s(" else ");
                self.expr(else_, 15);
            }
            Expr::With { scope, body } => {
                self.s("with ");
                self.expr(scope, 15);
                self.s("; ");
                self.expr(body, 15);
            }
            Expr::Assert { cond, body } => {
                self.s("assert ");
                self.expr(cond, 15);
                self.s("; ");
                self.expr(body, 15);
            }
            Expr::BinOp { op, lhs, rhs } => {
                let p = op.prec();
                let (l, r) = match op.assoc() {
                    Assoc::Left => (p, p - 1),
                    Assoc::Right => (p - 1, p),
                    Assoc::None => (p - 1, p - 1),
                };
                self.expr(lhs, l);
                self.s(" ");
                self.s(op.symbol());
                self.s(" ");
                self.expr(rhs, r);
            }
            Expr::Not(e) => {
                self.s("!");
                self.expr(e, 8);
            }
            Expr::Neg(e) => {
                self.s("-");
                self.expr(e, 2);
            }
        }
    }

    fn string(&mut self, parts: &[StrPart]) {
        self.s("\"");
        for (i, part) in parts.iter().enumerate() {
            match part {
                StrPart::Text(t) => {
                    let mut chars = t.chars().peekable();
                    while let Some(c) = chars.next() {
                        match c {
                            '"' => self.s("\\\""),
                            '\\' => self.s("\\\\"),
                            '\n' => self.s("\\n"),
                            '\r' => self.s("\\r"),
                            '\t' => self.s("\\t"),
                            '$' if chars.peek() == Some(&'{')
                                || (chars.peek().is_none() && matches!(parts.get(i + 1), Some(StrPart::Interp(_)))) =>
                            {
                                self.s("\\$")
                            }
                            c => self.out.push(c),
                        }
                    }
                }
                StrPart::Interp(e) => self.interp(e),
            }
        }
        self.s("\"");
    }

    fn indented(&mut self, parts: &[StrPart]) {
        self.s("''\n");
        let mut line_start = true;
        for (i, part) in parts.iter().enumerate() {
            match part {
                StrPart::Text(t) => {
                    let mut chars = t.chars().peekable();
                    while let Some(c) = chars.next() {
                        if line_start && c != '\n' {
                            for _ in 0..=self.indent {
                                self.s("  ");
                            }
                        }
                        line_start = c == '\n';
                        match c {
                            '\'' if chars.peek() == Some(&'\'') => {
                                chars.next();
                                self.s("'''")
                            }
                            '\'' if chars.peek().is_none() && i + 1 == parts.len() => self.s("${\"'\"}"),
                            '$' if chars.peek() == Some(&'{')
                                || (chars.peek().is_none() && matches!(parts.get(i + 1), Some(StrPart::Interp(_)))) =>
                            {
                                self.s("''$")
                            }
                            c => self.out.push(c),
                        }
                    }
                }
                StrPart::Interp(e) => {
                    if line_start {
                        for _ in 0..=self.indent {
                            self.s("  ");
                        }
                    }
                    self.interp(e);
                    line_start = false;
                }
            }
        }
        if line_start {
            for _ in 0..self.indent {
                self.s("  ");
            }
        }
        self.s("''");
    }

    fn interp(&mut self, e: &Expr) {
        self.s("${");
        self.expr(e, 15);
        self.s("}");
    }

    fn list(&mut self, items: &[Expr]) {
        if items.is_empty() {
            self.s("[ ]");
            return;
        }
        let mut inline = Printer { out: String::from("["), indent: self.indent };
        for item in items {
            inline.s(" ");
            inline.expr(item, 1);
        }
        inline.s(" ]");
        if items.len() <= 6
            && !items.iter().any(|item| matches!(item, Expr::AttrSet { .. }))
            && self.indent * 2 + inline.out.chars().count() <= 80
        {
            self.s(&inline.out);
            return;
        }
        self.s("[");
        self.indent += 1;
        for item in items {
            self.newline();
            self.expr(item, 1);
        }
        self.indent -= 1;
        self.newline();
        self.s("]");
    }

    fn block(&mut self, open: &str, bindings: &[Binding], close: &str) {
        if bindings.is_empty() {
            self.s(open);
            self.s(" ");
            self.s(close);
            return;
        }
        self.s(open);
        self.indent += 1;
        for b in bindings {
            self.newline();
            self.binding(b);
        }
        self.indent -= 1;
        self.newline();
        self.s(close);
    }

    fn binding(&mut self, b: &Binding) {
        match b {
            Binding::Attr { path, value } => {
                self.attrpath(path);
                self.s(" = ");
                self.expr(value, 15);
                self.s(";");
            }
            Binding::Inherit { from, names } => {
                self.s("inherit");
                if let Some(f) = from {
                    self.s(" (");
                    self.expr(f, 15);
                    self.s(")");
                }
                for n in names {
                    self.s(" ");
                    self.key(&AttrKey::Ident(n.clone()));
                }
                self.s(";");
            }
        }
    }

    fn attrpath(&mut self, path: &AttrPath) {
        for (i, k) in path.iter().enumerate() {
            if i > 0 {
                self.s(".");
            }
            self.key(k);
        }
    }

    fn key(&mut self, k: &AttrKey) {
        match k {
            AttrKey::Ident(n) if is_ident(n) => self.s(n),
            AttrKey::Ident(n) => self.string(&[StrPart::Text(n.clone())]),
            AttrKey::Str(parts) => self.string(parts),
            AttrKey::Dynamic(e) => self.interp(e),
        }
    }

    fn param(&mut self, param: &Param) {
        match param {
            Param::Ident(n) => self.s(n),
            Param::Formals { formals, ellipsis, at } => {
                self.s("{");
                let mut first = true;
                for f in formals {
                    self.s(if first { " " } else { ", " });
                    first = false;
                    self.s(&f.name);
                    if let Some(d) = &f.default {
                        self.s(" ? ");
                        self.expr(d, 15);
                    }
                }
                if *ellipsis {
                    self.s(if first { " ..." } else { ", ..." });
                    first = false;
                }
                self.s(if first { "}" } else { " }" });
                if let Some(a) = at {
                    self.s(" @ ");
                    self.s(a);
                }
            }
        }
    }
}
