use std::collections::BTreeMap;
use std::fmt;

use crate::expr::{Expr, ExprKind, Param, Query, TextPart};
use crate::record::{
    parse_record_file, Arguments, AttrPath, Entry, FileError, FileValue, ForBlock, ModuleItem,
    RecordDecl, Segment,
};
use crate::Span;

const PRELUDE: &[&str] = &[
    "builtins",
    "map",
    "filter",
    "fold",
    "join",
    "count",
    "sum",
    "min",
    "max",
    "first",
    "lookup",
    "lowercase",
    "contains",
    "starts-with",
    "trim",
    "words",
    "first-word",
    "rest-words",
    "split",
    "length",
];

pub struct SourceFile {
    pub name: String,
    pub text: String,
    pub decls: Vec<RecordDecl>,
}

impl SourceFile {
    pub fn parse(name: &str, text: &str) -> Result<Self, FileError> {
        Ok(Self {
            name: name.into(),
            text: text.into(),
            decls: parse_record_file(name, text)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub text: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}\n    {}",
            self.file, self.line, self.column, self.message, self.text
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    Options(Vec<String>),
    Unknown,
}

struct Declared {
    file: String,
    line: usize,
}

struct Frame {
    names: BTreeMap<String, Ty>,
    open: bool,
}

pub fn check(files: &[SourceFile]) -> Vec<Diagnostic> {
    let mut options = BTreeMap::new();
    for file in files {
        for decl in &file.decls {
            if let RecordDecl::Module(module) = decl {
                for item in &module.items {
                    if let ModuleItem::Option(option) = item {
                        if let Some(path) = static_path(&option.path) {
                            options.insert(
                                path,
                                Declared {
                                    file: file.name.clone(),
                                    line: option.path.line,
                                },
                            );
                        }
                    }
                }
            }
        }
    }
    let mut diagnostics = Vec::new();
    for file in files {
        let mut checker = Checker {
            file,
            options: &options,
            scopes: vec![Frame {
                names: PRELUDE.iter().map(|n| (n.to_string(), Ty::Unknown)).collect(),
                open: false,
            }],
            diagnostics: &mut diagnostics,
        };
        for decl in &file.decls {
            checker.decl(decl);
        }
    }
    diagnostics
}

fn static_path(path: &AttrPath) -> Option<Vec<String>> {
    path.segments
        .iter()
        .map(|s| match s {
            Segment::Name(n) | Segment::Text(n) => Some(n.clone()),
            Segment::Computed(_) => None,
        })
        .collect()
}

struct Checker<'a> {
    file: &'a SourceFile,
    options: &'a BTreeMap<Vec<String>, Declared>,
    scopes: Vec<Frame>,
    diagnostics: &'a mut Vec<Diagnostic>,
}

impl Checker<'_> {
    fn decl(&mut self, decl: &RecordDecl) {
        match decl {
            RecordDecl::Module(module) => {
                self.enter_arguments(&module.arguments);
                for item in &module.items {
                    self.item(item);
                }
                self.scopes.pop();
            }
            RecordDecl::File(file) => {
                let entered = file.arguments.as_ref().map(|a| self.enter_arguments(a));
                self.push(false);
                for binding in &file.lets {
                    let ty = self.expr(&binding.value);
                    self.bind(&binding.name, ty);
                }
                match &file.value {
                    FileValue::Expr(expr) => {
                        self.expr(expr);
                    }
                    FileValue::Entries(entries) => self.entries(entries),
                }
                self.scopes.pop();
                if entered.is_some() {
                    self.scopes.pop();
                }
            }
        }
    }

    fn enter_arguments(&mut self, arguments: &Arguments) {
        self.push(false);
        for name in &arguments.names {
            let ty = if name == "config" {
                Ty::Options(Vec::new())
            } else {
                Ty::Unknown
            };
            self.bind(name, ty);
        }
    }

    fn item(&mut self, item: &ModuleItem) {
        match item {
            ModuleItem::Let(binding) => {
                let ty = self.expr(&binding.value);
                self.bind(&binding.name, ty);
            }
            ModuleItem::Option(option) => {
                self.path(&option.path);
                for field in &option.fields {
                    self.expr(&field.value);
                }
            }
            ModuleItem::Imports(expr) => {
                self.expr(expr);
            }
            ModuleItem::Config { when, entries, .. } => {
                if let Some(when) = when {
                    self.expr(when);
                }
                self.entries(entries);
            }
            ModuleItem::For(block) => self.for_block(block),
        }
    }

    fn entries(&mut self, entries: &[Entry]) {
        for entry in entries {
            match entry {
                Entry::Value { path, value, when } => {
                    self.path(path);
                    self.expr(value);
                    if let Some(when) = when {
                        self.expr(when);
                    }
                }
                Entry::Nested {
                    path,
                    when,
                    entries,
                } => {
                    self.path(path);
                    if let Some(when) = when {
                        self.expr(when);
                    }
                    self.entries(entries);
                }
                Entry::For(block) => self.for_block(block),
            }
        }
    }

    fn for_block(&mut self, block: &ForBlock) {
        self.expr(&block.source);
        self.push(false);
        for var in &block.vars {
            self.bind(&format!("?{var}"), Ty::Unknown);
        }
        self.entries(&block.entries);
        self.scopes.pop();
    }

    fn path(&mut self, path: &AttrPath) {
        for segment in &path.segments {
            if let Segment::Computed(expr) = segment {
                self.expr(expr);
            }
        }
    }

    fn expr(&mut self, expr: &Expr) -> Ty {
        match &expr.kind {
            ExprKind::Null
            | ExprKind::Bool(_)
            | ExprKind::Int(_)
            | ExprKind::Path(_) => Ty::Unknown,
            ExprKind::Text(parts) | ExprKind::TextBlock(parts) => {
                for part in parts {
                    if let TextPart::Interpolation(inner) = part {
                        self.expr(inner);
                    }
                }
                Ty::Unknown
            }
            ExprKind::Name(name) => self.lookup(name, name, &expr.span),
            ExprKind::Var(name) => self.lookup(&format!("?{name}"), &format!("?{name}"), &expr.span),
            ExprKind::List(items) => {
                for item in items {
                    self.expr(item);
                }
                Ty::Unknown
            }
            ExprKind::Record(fields) => {
                for field in fields {
                    self.path(&field.path);
                    self.expr(&field.value);
                }
                Ty::Unknown
            }
            ExprKind::Field(base, name) => {
                let base = self.expr(base);
                self.field(base, name, &expr.span)
            }
            ExprKind::Call(callee, args) => {
                self.expr(callee);
                for arg in args {
                    self.expr(arg);
                }
                Ty::Unknown
            }
            ExprKind::Lambda(params, body) => {
                self.push(false);
                for param in params {
                    match param {
                        Param::Name { name, .. } => self.bind(name, Ty::Unknown),
                        Param::Formals(arguments) => {
                            for name in &arguments.names {
                                self.bind(name, Ty::Unknown);
                            }
                        }
                    }
                }
                self.expr(body);
                self.scopes.pop();
                Ty::Unknown
            }
            ExprKind::If(cond, then, otherwise) => {
                self.expr(cond);
                self.expr(then);
                self.expr(otherwise);
                Ty::Unknown
            }
            ExprKind::Binary(_, lhs, rhs) => {
                self.expr(lhs);
                self.expr(rhs);
                Ty::Unknown
            }
            ExprKind::Not(inner) => {
                self.expr(inner);
                Ty::Unknown
            }
            ExprKind::With(scope, body) => {
                self.expr(scope);
                self.push(true);
                self.expr(body);
                self.scopes.pop();
                Ty::Unknown
            }
            ExprKind::Query(query) => {
                self.push(false);
                match query {
                    Query::All {
                        vars, sorted_by, ..
                    } => {
                        for var in vars {
                            self.bind(&format!("?{var}"), Ty::Unknown);
                        }
                        if let Some(key) = sorted_by {
                            self.expr(key);
                        }
                    }
                    Query::The { var, .. } => self.bind(&format!("?{var}"), Ty::Unknown),
                }
                self.scopes.pop();
                Ty::Unknown
            }
            ExprKind::NixModule(arguments, entries) => {
                self.enter_arguments(arguments);
                self.entries(entries);
                self.scopes.pop();
                Ty::Unknown
            }
        }
    }

    fn lookup(&mut self, key: &str, shown: &str, span: &Span) -> Ty {
        for frame in self.scopes.iter().rev() {
            if let Some(ty) = frame.names.get(key) {
                return ty.clone();
            }
            if frame.open {
                return Ty::Unknown;
            }
        }
        let candidates: Vec<&str> = self
            .scopes
            .iter()
            .flat_map(|f| f.names.keys().map(String::as_str))
            .collect();
        let mut message = format!("`{shown}` isn't defined here.");
        if let Some(near) = closest(key, candidates) {
            message.push_str(&format!(" Did you mean `{near}`?"));
        }
        self.report(span.start, span.line, message);
        Ty::Unknown
    }

    fn field(&mut self, base: Ty, name: &str, span: &Span) -> Ty {
        let Ty::Options(prefix) = base else {
            return Ty::Unknown;
        };
        let mut path = prefix.clone();
        path.push(name.to_string());
        let children: Vec<&str> = self
            .options
            .keys()
            .filter(|p| p.len() > prefix.len() && p.starts_with(&prefix))
            .map(|p| p[prefix.len()].as_str())
            .collect();
        if children.is_empty() || children.contains(&name) {
            return if self.options.contains_key(&path) {
                Ty::Unknown
            } else {
                Ty::Options(path)
            };
        }
        let mut message = format!(
            "`{name}` isn't an option of `{}`.",
            if prefix.is_empty() {
                "config".to_string()
            } else {
                prefix.join(".")
            }
        );
        if let Some(near) = closest(name, children) {
            let mut near_path = prefix.clone();
            near_path.push(near.to_string());
            message.push_str(&format!(" Did you mean `{near}`?"));
            if let Some(declared) = self
                .options
                .iter()
                .find(|(p, _)| p.starts_with(&near_path))
                .map(|(_, d)| d)
            {
                message.push_str(&format!(" (declared at {}:{})", declared.file, declared.line));
            }
        }
        self.report(span.end - name.len(), span.line, message);
        Ty::Unknown
    }

    fn report(&mut self, offset: usize, line: usize, message: String) {
        let text = &self.file.text;
        let line_start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
        let line_end = text[offset..].find('\n').map_or(text.len(), |i| offset + i);
        self.diagnostics.push(Diagnostic {
            file: self.file.name.clone(),
            line,
            column: text[line_start..offset].chars().count() + 1,
            message,
            text: text[line_start..line_end].trim_end().to_string(),
        });
    }

    fn push(&mut self, open: bool) {
        self.scopes.push(Frame {
            names: BTreeMap::new(),
            open,
        });
    }

    fn bind(&mut self, name: &str, ty: Ty) {
        if let Some(frame) = self.scopes.last_mut() {
            frame.names.insert(name.to_string(), ty);
        }
    }
}

fn closest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (name.chars().count() / 3).max(1);
    candidates
        .into_iter()
        .map(|c| (distance(name, c), c))
        .filter(|(d, _)| *d <= limit)
        .min()
        .map(|(_, c)| c)
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(diagonal + usize::from(ca != cb));
            diagonal = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}
