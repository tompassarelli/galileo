use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::expr::{parse_expr, BinOp, Expr, ExprKind, Param, Query, TextPart};
use crate::record::{
    record_decl, Arguments, AttrPath, Entry, FileError, FileValue, ForBlock, ModuleItem,
    OptionKind, RecordDecl, Segment,
};
use crate::schema::Schema;
use crate::types::{parse_type, Type};
use crate::{blocks, lex, read_sentences, Arg, Error, Line, Sentences, Span, Token, TokenKind};

fn staged(nix: bool, ty: Ty) -> Ty {
    if nix {
        Ty::nix(ty)
    } else {
        ty
    }
}

const NIX_ROOTS: &[&str] = &["config", "lib", "pkgs", "builtins", "inputs"];

const PRELUDE: &[&str] = &[
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
    pub signatures: Vec<Signature>,
    pub foreign: Vec<Foreign>,
    pub sentences: Sentences,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Foreign {
    pub path: String,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub result: Type,
    pub line: usize,
}

struct Typed {
    params: Vec<(String, Ty)>,
    result: Ty,
}

const TYPE_NAMES: &[&str] = &["Any", "Bool", "Int", "Text", "Path", "Port", "OptionPath", "List", "Attrs"];

impl SourceFile {
    pub fn parse(name: &str, text: &str) -> Result<Self, FileError> {
        let located = |error| FileError {
            file: name.into(),
            error,
        };
        let mut decls = Vec::new();
        let mut signatures = Vec::new();
        let mut foreign = Vec::new();
        for block in blocks(text).map_err(located)? {
            if let Some(decl) = record_decl(&block) {
                decls.push(decl.map_err(located)?);
            } else if block.children.is_empty() && word(&block.line.tokens, 0) == "define" {
                signatures.push(signature(&block.line).map_err(located)?);
            } else if word(&block.line.tokens, 0) == "foreign" {
                let tokens = &block.line.tokens;
                let path = match tokens.get(3).map(|t| &t.kind) {
                    Some(TokenKind::Path(p) | TokenKind::Text(p)) => Some(p.clone()),
                    _ => None,
                };
                match path {
                    Some(path)
                        if word(tokens, 1) == "nixos-options"
                            && word(tokens, 2) == "from"
                            && tokens.len() == 4 =>
                    {
                        foreign.push(Foreign {
                            path,
                            line: block.line.no,
                        })
                    }
                    _ => {
                        return Err(located(Error::at(
                            &block.line,
                            "a foreign source is `foreign nixos-options from PATH`",
                        )))
                    }
                }
            }
        }
        Ok(Self {
            name: name.into(),
            text: text.into(),
            decls,
            signatures,
            foreign,
            sentences: read_sentences(text).map_err(located)?,
        })
    }
}

fn word(tokens: &[Token], index: usize) -> &str {
    tokens.get(index).map_or("", token_word)
}

fn token_word(token: &Token) -> &str {
    match &token.kind {
        TokenKind::Word(w) | TokenKind::Symbol(w) => w,
        _ => "",
    }
}

fn signature(line: &Line) -> Result<Signature, Error> {
    const FORM: &str = "a signature is `define name(param: Type, ...): Type` with no body";
    let tokens = &line.tokens;
    let form = || Error::at(line, FORM);
    let open = (1..tokens.len()).find(|&i| word(tokens, i) == "(").ok_or_else(form)?;
    let mut depth = 0usize;
    let close = (open..tokens.len())
        .find(|&i| {
            match word(tokens, i) {
                "(" => depth += 1,
                ")" => depth -= 1,
                _ => {}
            }
            depth == 0
        })
        .ok_or_else(form)?;
    if open < 2 || word(tokens, close + 1) != ":" || close + 2 >= tokens.len() {
        return Err(form());
    }
    let name: String = (1..open).map(|i| word(tokens, i)).collect();
    let mut params = Vec::new();
    for param in tokens[open + 1..close].split(|t| token_word(t) == ",") {
        if param.is_empty() {
            continue;
        }
        if param.len() < 3 || word(param, 1) != ":" {
            return Err(form());
        }
        params.push((word(param, 0).to_string(), parse_type(&param[2..], line)?));
    }
    Ok(Signature {
        name,
        params,
        result: parse_type(&tokens[close + 2..], line)?,
        line: line.no,
    })
}

fn option_ty(ty: &Type, errors: &mut Vec<(Span, String)>) -> Ty {
    match ty {
        Type::Maybe(inner) => Ty::Maybe(Box::new(option_ty(inner, errors))),
        Type::Named { name, args, span } => match (name.as_str(), args.as_slice()) {
            ("Bool", []) => Ty::Bool,
            ("Int" | "Port", []) => Ty::Int,
            ("Text", []) => Ty::Text,
            ("Path", []) => Ty::Path,
            ("List", [inner]) => Ty::List(Box::new(option_ty(inner, errors))),
            _ => {
                errors.push((span.clone(), format!("`{name}` is not an option type from SPEC §10.2; use Bool, Int, Text, Path, Port, List T or maybe T.")));
                Ty::Any
            }
        },
        Type::Nix(inner) => {
            let mut inner = inner.as_ref();
            while let Type::Maybe(next) | Type::Nix(next) = inner {
                inner = next;
            }
            if let Type::Named { span, .. } = inner {
                errors.push((span.clone(), "`nix T` is not an option type from SPEC §10.2.".into()));
            }
            Ty::Any
        }
    }
}

fn value_ty(ty: &Type, declared: &BTreeSet<String>, errors: &mut Vec<(Span, String)>) -> Ty {
    let (name, args, span) = match ty {
        Type::Maybe(inner) => return Ty::Maybe(Box::new(value_ty(inner, declared, errors))),
        Type::Nix(inner) => return Ty::nix(value_ty(inner, declared, errors)),
        Type::Named { name, args, span } => (name.as_str(), args, span),
    };
    let arity = match name {
        "List" | "Attrs" => 1,
        _ => 0,
    };
    let known = TYPE_NAMES.contains(&name) || declared.contains(name);
    if !known {
        let mut message = format!("`{name}` is not a type.");
        let candidates = TYPE_NAMES.iter().copied().chain(declared.iter().map(String::as_str));
        if let Some(close) = closest(name, candidates) {
            message.push_str(&format!(" Did you mean `{close}`?"));
        }
        errors.push((span.clone(), message));
        return Ty::Any;
    }
    if args.len() != arity {
        let wanted = if arity == 1 { "one type argument" } else { "no type arguments" };
        errors.push((span.clone(), format!("`{name}` takes {wanted}, but is given {}.", args.len())));
        return Ty::Any;
    }
    let mut arg = || Box::new(value_ty(&args[0], declared, errors));
    match name {
        "Any" => Ty::Any,
        "Bool" => Ty::Bool,
        "Int" | "Port" => Ty::Int,
        "Text" => Ty::Text,
        "Path" => Ty::Path,
        "List" => Ty::List(arg()),
        "Attrs" => Ty::Attrs(arg()),
        _ => Ty::Declared(name.into()),
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
    Any,
    Null,
    Bool,
    Int,
    Text,
    Path,
    Maybe(Box<Ty>),
    List(Box<Ty>),
    Attrs(Box<Ty>),
    Nix(Box<Ty>),
    Options(Vec<String>),
    Members(Vec<String>),
    Fn(Box<Ty>),
    Declared(String),
}

impl Ty {
    fn nix(ty: Ty) -> Ty {
        match ty {
            Ty::Nix(_) => ty,
            other => Ty::Nix(Box::new(other)),
        }
    }

    fn is_nix(&self) -> bool {
        matches!(self, Ty::Nix(_))
    }

    fn unstaged(&self) -> &Ty {
        match self {
            Ty::Nix(inner) => inner.unstaged(),
            other => other,
        }
    }

    fn join(&self, other: &Ty) -> Ty {
        let (a, b) = (self.unstaged(), other.unstaged());
        if a == b {
            return a.clone();
        }
        let maybe = |ty: &Ty| match ty.value() {
            Ty::Null => Some(None),
            Ty::Maybe(inner) => Some(Some(inner.as_ref().clone())),
            _ => None,
        };
        let plain = |ty: &Ty| maybe(ty).map_or(Some(ty.value().clone()), |inner| inner);
        let joined = match (plain(a), plain(b)) {
            (Some(x), Some(y)) if x == y => x,
            (Some(x), None) | (None, Some(x)) => x,
            (None, None) => return Ty::Null,
            _ => Ty::Any,
        };
        if maybe(a).is_some() || maybe(b).is_some() {
            Ty::Maybe(Box::new(joined))
        } else {
            joined
        }
    }

    fn value(&self) -> &Ty {
        match self {
            Ty::Nix(inner) => inner.value(),
            Ty::Options(_) | Ty::Members(_) | Ty::Declared(_) => &Ty::Any,
            other => other,
        }
    }

    fn fits(&self, want: &Ty) -> bool {
        match (self.value(), want.value()) {
            (Ty::Any, _) | (_, Ty::Any) => true,
            (Ty::Null, Ty::Maybe(_)) => true,
            (Ty::Maybe(a), Ty::Maybe(b)) => a.fits(b),
            (Ty::Maybe(_), _) => false,
            (a, Ty::Maybe(b)) => a.fits(b),
            (Ty::List(a), Ty::List(b)) | (Ty::Attrs(a), Ty::Attrs(b)) => a.fits(b),
            (a, b) => a == b,
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value() {
            Ty::Null => write!(f, "null"),
            Ty::Bool => write!(f, "Bool"),
            Ty::Int => write!(f, "Int"),
            Ty::Text => write!(f, "Text"),
            Ty::Path => write!(f, "Path"),
            Ty::Maybe(inner) => write!(f, "maybe {inner}"),
            Ty::List(inner) => write!(f, "List {inner}"),
            Ty::Attrs(inner) => write!(f, "Attrs {inner}"),
            _ => write!(f, "Any"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Declared {
    pub file: String,
    pub line: usize,
    pub ty: Ty,
    pub description: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Occurrence {
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub ty: Ty,
    pub option: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default)]
pub struct Index {
    pub options: BTreeMap<Vec<String>, Declared>,
    pub typed: Vec<Occurrence>,
    pub text_concat: BTreeMap<String, BTreeSet<(usize, usize)>>,
}

impl Index {
    pub fn uses<'a>(&'a self, path: &'a [String]) -> impl Iterator<Item = &'a Occurrence> + 'a {
        self.typed.iter().filter(move |t| t.option.as_deref() == Some(path))
    }

    pub fn at(&self, file: &str, offset: usize) -> Option<&Occurrence> {
        self.typed
            .iter()
            .filter(|t| t.file == file && t.start <= offset && offset <= t.end)
            .min_by_key(|t| t.end - t.start)
    }
}

struct Frame {
    names: BTreeMap<String, Ty>,
    lines: BTreeMap<String, usize>,
    open: bool,
}

pub fn check(files: &[SourceFile], schema: &Schema) -> Vec<Diagnostic> {
    analyze(files, schema).0
}

pub fn analyze(files: &[SourceFile], schema: &Schema) -> (Vec<Diagnostic>, Index) {
    let mut diagnostics = Vec::new();
    let declared: BTreeSet<String> = files
        .iter()
        .flat_map(|f| f.sentences.vocabulary.types.keys().cloned())
        .collect();
    let mut signatures = BTreeMap::new();
    for file in files {
        let mut errors = Vec::new();
        for signature in &file.signatures {
            let typed = Typed {
                params: signature
                    .params
                    .iter()
                    .map(|(name, ty)| (name.clone(), value_ty(ty, &declared, &mut errors)))
                    .collect(),
                result: value_ty(&signature.result, &declared, &mut errors),
            };
            signatures.insert(signature.name.clone(), typed);
        }
        for (span, message) in errors {
            diagnostics.push(diagnostic(file, span.start, span.line, message));
        }
    }
    let mut text_concat = BTreeMap::new();
    let mut options = BTreeMap::new();
    for file in files {
        for decl in &file.decls {
            if let RecordDecl::Module(module) = decl {
                for item in &module.items {
                    if let ModuleItem::Option(option) = item {
                        let mut errors = Vec::new();
                        let ty = match &option.kind {
                            OptionKind::Enable(_) => Ty::Bool,
                            OptionKind::Type(ty) => option_ty(ty, &mut errors),
                        };
                        for (span, message) in errors {
                            diagnostics.push(diagnostic(file, span.start, span.line, message));
                        }
                        if let Some(path) = static_path(&option.path) {
                            options.insert(
                                path,
                                Declared {
                                    file: file.name.clone(),
                                    line: option.path.line,
                                    ty,
                                    description: option.fields.iter().find(|f| f.name == "description").and_then(|f| match &f.value.kind {
                                        ExprKind::Text(parts) => Some(parts.iter().map(|p| match p {
                                            TextPart::Text(t) => t.as_str(),
                                            TextPart::Interpolation(_) => "…",
                                        }).collect()),
                                        _ => None,
                                    }),
                                },
                            );
                        }
                    }
                }
            }
        }
    }
    let mut known: BTreeMap<Vec<String>, String> = BTreeMap::new();
    for path in schema.options.keys() {
        known.insert(path.clone(), schema.source.clone());
    }
    for (path, declared) in &options {
        known.insert(path.clone(), format!("{}:{}", declared.file, declared.line));
    }
    let mut typed = Vec::new();
    for file in files {
        let names: BTreeMap<String, Ty> = PRELUDE
            .iter()
            .map(|n| {
                let ty = match *n {
                    "first" | "lookup" => Ty::Fn(Box::new(Ty::Maybe(Box::new(Ty::Any)))),
                    _ => Ty::Any,
                };
                (n.to_string(), ty)
            })
            .collect();
        let mut checker = Checker {
            file,
            options: &options,
            known: &known,
            schema: !schema.options.is_empty(),
            signatures: &signatures,
            declared: &declared,
            quiet: false,
            scopes: vec![Frame { names, lines: BTreeMap::new(), open: false }],
            diagnostics: &mut diagnostics,
            typed: &mut typed,
            text_concat: &mut text_concat,
        };
        for decl in &file.decls {
            checker.decl(decl);
        }
        checker.facts();
        the_queries(file, &mut typed, &mut diagnostics);
        aggregate_values(file, &mut diagnostics);
    }
    (diagnostics, Index { options, typed, text_concat })
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
    known: &'a BTreeMap<Vec<String>, String>,
    schema: bool,
    signatures: &'a BTreeMap<String, Typed>,
    declared: &'a BTreeSet<String>,
    quiet: bool,
    scopes: Vec<Frame>,
    diagnostics: &'a mut Vec<Diagnostic>,
    typed: &'a mut Vec<Occurrence>,
    text_concat: &'a mut BTreeMap<String, BTreeSet<(usize, usize)>>,
}

impl Checker<'_> {
    fn decl(&mut self, decl: &RecordDecl) {
        match decl {
            RecordDecl::Module(module) => {
                self.push_nix();
                self.enter_arguments(&module.arguments, self.arguments_line(module.line));
                for item in &module.items {
                    self.item(item);
                }
                self.scopes.pop();
                self.scopes.pop();
            }
            RecordDecl::File(file) => {
                self.push_nix();
                match &file.arguments {
                    Some(arguments) => self.enter_arguments(arguments, self.arguments_line(file.line)),
                    None => self.push(false),
                }
                for binding in &file.lets {
                    let ty = self.expr(&binding.value);
                    self.bind_once(&binding.name, ty, binding.line);
                }
                match &file.value {
                    FileValue::Expr(expr) => {
                        self.expr(expr);
                    }
                    FileValue::Entries(entries) => self.entries(entries),
                }
                self.scopes.pop();
                self.scopes.pop();
            }
        }
    }

    fn arguments_line(&self, from: usize) -> usize {
        self.file
            .text
            .lines()
            .enumerate()
            .skip(from - 1)
            .find(|(_, row)| row.trim_start().starts_with("arguments:"))
            .map_or(from, |(index, _)| index + 1)
    }

    fn push_nix(&mut self) {
        self.push(false);
        for name in ["builtins", "import", "toString"] {
            self.bind(name, Ty::nix(Ty::Members(vec![name.into()])));
        }
    }

    fn enter_arguments(&mut self, arguments: &Arguments, line: usize) {
        self.push(false);
        for name in &arguments.names {
            let ty = if name == "config" {
                Ty::nix(Ty::Options(Vec::new()))
            } else {
                Ty::nix(Ty::Members(vec![name.clone()]))
            };
            self.bind_once(name, ty, line);
        }
    }

    fn item(&mut self, item: &ModuleItem) {
        match item {
            ModuleItem::Let(binding) => {
                let ty = self.expr(&binding.value);
                self.bind_once(&binding.name, ty, binding.line);
            }
            ModuleItem::Option(option) => {
                self.path(&option.path);
                if let OptionKind::Enable(description) = &option.kind {
                    let ty = self.expr(description);
                    if !ty.fits(&Ty::Text) {
                        self.report(description.span.start, description.span.line, format!("`enable` needs a Text expression, but `{}` is `{ty}`.", self.source(description)));
                    }
                }
                for field in &option.fields {
                    self.expr(&field.value);
                }
            }
            ModuleItem::Imports(expr) => {
                self.expr(expr);
            }
            ModuleItem::Config { when, entries, .. } => {
                if let Some(when) = when {
                    self.condition(when, "`when`");
                }
                self.entries(entries);
                self.option_paths(entries, &[]);
            }
            ModuleItem::For(block) => {
                self.for_block(block);
                self.option_paths(&block.entries, &[]);
            }
        }
    }

    fn entries(&mut self, entries: &[Entry]) {
        for entry in entries {
            match entry {
                Entry::Value { path, value, when } => {
                    self.path(path);
                    self.expr(value);
                    if let Some(when) = when {
                        self.condition(when, "`when`");
                    }
                }
                Entry::Nested {
                    path,
                    when,
                    entries,
                } => {
                    self.path(path);
                    if let Some(when) = when {
                        self.condition(when, "`when`");
                    }
                    self.entries(entries);
                }
                Entry::For(block) => self.for_block(block),
            }
        }
    }

    fn option_paths(&mut self, entries: &[Entry], prefix: &[String]) {
        for entry in entries {
            match entry {
                Entry::Value { path, .. } => {
                    self.option_path(path, prefix);
                }
                Entry::Nested { path, entries, .. } => {
                    if let Some(inner) = self.option_path(path, prefix) {
                        self.option_paths(entries, &inner);
                    }
                }
                Entry::For(block) => self.option_paths(&block.entries, prefix),
            }
        }
    }

    fn option_path(&mut self, path: &AttrPath, prefix: &[String]) -> Option<Vec<String>> {
        let mut at = prefix.to_vec();
        for segment in &path.segments {
            if self.known.contains_key(&at) {
                return None;
            }
            let (Segment::Name(name) | Segment::Text(name)) = segment else {
                return None;
            };
            let Some((message, near)) = self.unknown(&at, name) else {
                at.push(name.clone());
                continue;
            };
            let mut message = message;
            if let Some((near_path, source)) = near {
                message.push_str(&format!(" Did you mean `{}`? ({source})", near_path.join(".")));
            }
            let line = path.line;
            let offset = self.line_offset(line, name);
            self.report(offset, line, message);
            return None;
        }
        Some(at)
    }

    fn unknown(&self, at: &[String], name: &str) -> Option<(String, Option<(Vec<String>, String)>)> {
        let below = || {
            self.known
                .range(at.to_vec()..)
                .take_while(|(p, _)| p.starts_with(at))
                .filter(|(p, _)| p.len() > at.len())
        };
        let closed = if self.schema {
            below().next().is_some()
        } else {
            below().any(|(p, _)| p.len() == at.len() + 1 && self.options.contains_key(p))
        };
        let children: Vec<&str> = below().map(|(p, _)| p[at.len()].as_str()).collect();
        if !closed || children.contains(&name) {
            return None;
        }
        let message = format!(
            "`{name}` isn't an option of `{}`.",
            if at.is_empty() { "config".to_string() } else { at.join(".") }
        );
        let near = closest(name, children).map(|near| {
            let mut near_path = at.to_vec();
            near_path.push(near.to_string());
            let source = self
                .known
                .range(near_path.clone()..)
                .next()
                .map_or(String::new(), |(_, s)| s.clone());
            (near_path, source)
        });
        Some((message, near))
    }

    fn line_offset(&self, line: usize, name: &str) -> usize {
        let text = &self.file.text;
        let start: usize = text.split_inclusive('\n').take(line - 1).map(str::len).sum();
        let row = text[start..].lines().next().unwrap_or("");
        start + row.find(name).unwrap_or(row.len() - row.trim_start().len())
    }

    fn for_block(&mut self, block: &ForBlock) {
        self.expr(&block.source);
        self.push(false);
        for var in &block.vars {
            self.bind(&format!("?{var}"), Ty::Any);
        }
        self.entries(&block.entries);
        self.scopes.pop();
    }

    fn path(&mut self, path: &AttrPath) -> bool {
        let mut nix = false;
        for segment in &path.segments {
            if let Segment::Computed(expr) = segment {
                nix |= self.expr(expr).is_nix();
            }
        }
        nix
    }

    fn expr(&mut self, expr: &Expr) -> Ty {
        let ty = self.infer(expr);
        if !self.quiet && matches!(expr.kind, ExprKind::Name(_) | ExprKind::Field(..) | ExprKind::Var(_)) {
            let option = match &expr.kind {
                ExprKind::Field(..) => self.option_of(expr),
                _ => None,
            };
            self.typed.push(Occurrence {
                file: self.file.name.clone(),
                start: expr.span.start,
                end: expr.span.end,
                ty: ty.clone(),
                option,
            });
        }
        ty
    }

    fn option_of(&self, expr: &Expr) -> Option<Vec<String>> {
        let ExprKind::Field(base, name) = &expr.kind else {
            return None;
        };
        let prefix = self.prefix_of(base)?;
        let mut path = prefix;
        path.push(name.clone());
        self.options.contains_key(&path).then_some(path)
    }

    fn prefix_of(&self, expr: &Expr) -> Option<Vec<String>> {
        let typed = self.typed.iter().rev().find(|t| t.file == self.file.name && t.start == expr.span.start && t.end == expr.span.end)?;
        match &typed.ty {
            Ty::Nix(inner) => match inner.as_ref() {
                Ty::Options(prefix) => Some(prefix.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    fn infer(&mut self, expr: &Expr) -> Ty {
        match &expr.kind {
            ExprKind::Null => Ty::Null,
            ExprKind::Bool(_) => Ty::Bool,
            ExprKind::Int(_) => Ty::Int,
            ExprKind::Path(_) => Ty::Path,
            ExprKind::Text(parts) | ExprKind::TextBlock(parts) => {
                let mut nix = false;
                for part in parts {
                    if let TextPart::Interpolation(inner) = part {
                        nix |= self.expr(inner).is_nix();
                    }
                }
                staged(nix, Ty::Text)
            }
            ExprKind::Name(name) => self.lookup(name, name, &expr.span),
            ExprKind::Var(name) => self.lookup(&format!("?{name}"), &format!("?{name}"), &expr.span),
            ExprKind::List(items) => {
                let mut nix = false;
                let mut item_ty = None;
                for item in items {
                    let ty = self.expr(item);
                    nix |= ty.is_nix();
                    item_ty = match item_ty {
                        None => Some(ty.value().clone()),
                        Some(earlier) if earlier == *ty.value() => Some(earlier),
                        Some(_) => Some(Ty::Any),
                    };
                }
                staged(nix, Ty::List(Box::new(item_ty.unwrap_or(Ty::Any))))
            }
            ExprKind::Record(fields) => {
                let mut nix = false;
                for field in fields {
                    nix |= self.path(&field.path);
                    nix |= self.expr(&field.value).is_nix();
                }
                staged(nix, Ty::Attrs(Box::new(Ty::Any)))
            }
            ExprKind::Field(base, name) => {
                let base = self.expr(base);
                self.field(base, name, &expr.span)
            }
            ExprKind::Call(callee, args) => {
                let (callee_ty, arg_tys) = match &callee.kind {
                    ExprKind::Lambda(params, body) => {
                        let arg_tys: Vec<Ty> = args.iter().map(|a| self.expr(a)).collect();
                        (self.lambda(params, body, &arg_tys), arg_tys)
                    }
                    _ => {
                        let callee_ty = self.expr(callee);
                        (callee_ty, args.iter().map(|a| self.expr(a)).collect())
                    }
                };
                let nix = callee_ty.is_nix() || arg_tys.iter().any(Ty::is_nix);
                if let Ty::Fn(result) = callee_ty.unstaged() {
                    let result = result.as_ref().clone();
                    return staged(nix || result.is_nix(), result);
                }
                let Some(name) = self.signature_of(&callee_ty) else {
                    return staged(nix, Ty::Any);
                };
                let signature = &self.signatures[&name];
                if signature.params.len() != args.len() {
                    let message = format!(
                        "`{name}` takes {} argument{}, but this call passes {}.",
                        signature.params.len(),
                        if signature.params.len() == 1 { "" } else { "s" },
                        args.len()
                    );
                    self.report(expr.span.start, expr.span.line, message);
                } else {
                    let params = signature.params.clone();
                    for ((param, want), (arg, got)) in params.iter().zip(args.iter().zip(&arg_tys)) {
                        if !got.fits(want) {
                            let message = format!(
                                "`{name}` takes a `{want}` as `{param}`, but `{}` is `{got}`.",
                                self.source(arg)
                            );
                            self.report(arg.span.start, arg.span.line, message);
                        }
                    }
                }
                staged(nix, self.signatures[&name].result.clone())
            }
            ExprKind::Lambda(params, body) => self.lambda(params, body, &[]),
            ExprKind::If(cond, then, otherwise) => {
                let cond = self.condition(cond, "`if`");
                let then = self.expr(then);
                let otherwise = self.expr(otherwise);
                staged(
                    cond.is_nix() || then.is_nix() || otherwise.is_nix(),
                    then.join(&otherwise),
                )
            }
            ExprKind::Binary(op, lhs, rhs) => self.binary(*op, lhs, rhs),
            ExprKind::Not(inner) => {
                let ty = self.condition(inner, "`not`");
                staged(ty.is_nix(), Ty::Bool)
            }
            ExprKind::With(scope, body) => {
                let scope = self.expr(scope);
                self.push(true);
                let body = self.expr(body);
                self.scopes.pop();
                staged(scope.is_nix(), body)
            }
            ExprKind::Query(query) => {
                self.push(false);
                match query {
                    Query::All {
                        vars, sorted_by, ..
                    } => {
                        for var in vars {
                            self.bind(&format!("?{var}"), Ty::Any);
                        }
                        if let Some(key) = sorted_by {
                            self.expr(key);
                        }
                    }
                    Query::The { var, .. } => self.bind(&format!("?{var}"), Ty::Any),
                }
                self.scopes.pop();
                Ty::Any
            }
            ExprKind::NixModule(arguments, entries) => {
                self.enter_arguments(arguments, expr.span.line);
                if arguments.names.iter().any(|n| n == "config") {
                    self.bind("config", Ty::nix(Ty::Any));
                }
                self.entries(entries);
                self.scopes.pop();
                Ty::nix(Ty::Attrs(Box::new(Ty::Any)))
            }
        }
    }

    fn lambda(&mut self, params: &[Param], body: &Expr, args: &[Ty]) -> Ty {
        self.push(false);
        for (index, param) in params.iter().enumerate() {
            match param {
                Param::Name { name, ty } => {
                    let ty = match (ty, params.len() == args.len(), args.get(index)) {
                        (Some(ty), _, _) => {
                            let mut errors = Vec::new();
                            let ty = value_ty(ty, self.declared, &mut errors);
                            for (span, message) in errors {
                                self.report(span.start, span.line, message);
                            }
                            ty
                        }
                        (None, true, Some(arg)) => arg.clone(),
                        _ => Ty::Any,
                    };
                    self.bind(name, ty);
                }
                Param::Formals(arguments) => {
                    for name in &arguments.names {
                        self.bind(name, Ty::Any);
                    }
                }
            }
        }
        let result = self.expr(body);
        self.scopes.pop();
        Ty::Fn(Box::new(result))
    }

    fn binary(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr) -> Ty {
        let symbol = match op {
            BinOp::Or => "or",
            BinOp::And => "and",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::Update => "//",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Concat => "++",
        };
        let left = self.expr(lhs);
        let right = self.expr(rhs);
        let nix = left.is_nix() || right.is_nix();
        let result = match op {
            BinOp::Eq | BinOp::Ne => {
                for (side, ty, other) in [(lhs, &left, &right), (rhs, &right, &left)] {
                    if *other.value() == Ty::Null && !Ty::Null.fits(ty) {
                        let message = format!(
                            "`{}` is `{ty}`, which is never null, so `{symbol} null` always gives the same answer.",
                            self.source(side)
                        );
                        self.report(side.span.start, side.span.line, message);
                    }
                }
                Ty::Bool
            }
            BinOp::Or | BinOp::And => {
                self.operand(lhs, &left, symbol, &Ty::Bool);
                self.operand(rhs, &right, symbol, &Ty::Bool);
                Ty::Bool
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                self.operand(lhs, &left, symbol, &Ty::Int);
                self.operand(rhs, &right, symbol, &Ty::Int);
                Ty::Bool
            }
            BinOp::Add | BinOp::Sub | BinOp::Mul => {
                self.operand(lhs, &left, symbol, &Ty::Int);
                self.operand(rhs, &right, symbol, &Ty::Int);
                Ty::Int
            }
            BinOp::Concat | BinOp::Update => {
                if op == BinOp::Concat && (*left.value() == Ty::Text || *right.value() == Ty::Text) {
                    self.text_concat.entry(self.file.name.clone()).or_default().insert((lhs.span.start, rhs.span.end));
                }
                self.operand(lhs, &left, symbol, &Ty::Any);
                self.operand(rhs, &right, symbol, &Ty::Any);
                if left.value() == right.value() {
                    left.value().clone()
                } else {
                    Ty::Any
                }
            }
        };
        staged(nix, result)
    }

    fn operand(&mut self, expr: &Expr, ty: &Ty, symbol: &str, want: &Ty) {
        let message = match ty.value() {
            Ty::Null | Ty::Maybe(_) => format!(
                "`{}` is `{ty}`, so it may be null, and `{symbol}` can't use a null value.",
                self.source(expr)
            ),
            _ if !ty.fits(want) => format!(
                "`{symbol}` needs `{want}` here, but `{}` is `{ty}`.",
                self.source(expr)
            ),
            _ => return,
        };
        self.report(expr.span.start, expr.span.line, message);
    }

    fn condition(&mut self, expr: &Expr, place: &str) -> Ty {
        let ty = self.expr(expr);
        let message = match ty.value() {
            Ty::Null | Ty::Maybe(_) => format!(
                "{place} needs a `Bool`, but `{}` is `{ty}` and may be null.",
                self.source(expr)
            ),
            _ if !ty.fits(&Ty::Bool) => format!(
                "{place} needs a `Bool`, but `{}` is `{ty}`.",
                self.source(expr)
            ),
            _ => return ty,
        };
        self.report(expr.span.start, expr.span.line, message);
        ty
    }

    fn signature_of(&self, ty: &Ty) -> Option<String> {
        let Ty::Nix(inner) = ty else { return None };
        let Ty::Members(path) = inner.as_ref() else {
            return None;
        };
        let name = path.join(".");
        self.signatures.contains_key(&name).then_some(name)
    }

    fn source(&self, expr: &Expr) -> &str {
        if self.quiet {
            return "";
        }
        self.file.text[expr.span.start..expr.span.end].trim()
    }

    fn facts(&mut self) {
        let sentences = &self.file.sentences;
        let mut names = BTreeMap::new();
        for root in NIX_ROOTS {
            let ty = if *root == "config" {
                Ty::Options(Vec::new())
            } else {
                Ty::Members(vec![root.to_string()])
            };
            names.insert(root.to_string(), Ty::nix(ty));
        }
        self.scopes.push(Frame { names, lines: BTreeMap::new(), open: false });
        for fact in &sentences.facts {
            let shape = &sentences.vocabulary.shapes[fact.shape];
            for (arg, slot) in fact.args.iter().zip(&shape.slots) {
                if slot.ty.starts_with("nix ") || slot.ty == "OptionPath" {
                    continue;
                }
                let text = match arg {
                    Arg::Expr(text) => text.clone(),
                    Arg::Name(text) if text.contains('.') => text.clone(),
                    Arg::Text(_) => arg.to_string(),
                    _ => continue,
                };
                if self.stage_of(&text) {
                    let message = format!(
                        "`{text}` depends on Nix evaluation, so it can't fill the `{}` slot, which is `{}`. Muove can't decide it; only a slot typed `nix {}` stores a Nix value.",
                        slot.name, slot.ty, slot.ty
                    );
                    let line_start = self
                        .file
                        .text
                        .split_inclusive('\n')
                        .take(fact.line - 1)
                        .map(str::len)
                        .sum::<usize>();
                    let line_text = self.file.text[line_start..].lines().next().unwrap_or("");
                    let column = line_text
                        .find(text.as_str())
                        .unwrap_or(line_text.len() - line_text.trim_start().len());
                    self.report(line_start + column, fact.line, message);
                }
            }
        }
        self.scopes.pop();
    }

    fn stage_of(&mut self, text: &str) -> bool {
        let Ok(lines) = lex(text) else { return false };
        let Some(line) = lines.first() else {
            return false;
        };
        let Ok(expr) = parse_expr(&line.tokens, line) else {
            return false;
        };
        self.quiet = true;
        let nix = self.expr(&expr).is_nix();
        self.quiet = false;
        nix
    }

    fn lookup(&mut self, key: &str, shown: &str, span: &Span) -> Ty {
        if let Some(ty) = self.scopes.iter().rev().find_map(|f| f.names.get(key)) {
            return ty.clone();
        }
        if self.scopes.iter().any(|f| f.open) {
            return Ty::Any;
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
        Ty::Any
    }

    fn field(&mut self, base: Ty, name: &str, span: &Span) -> Ty {
        let nix = base.is_nix();
        let prefix = match base {
            Ty::Nix(inner) => match *inner {
                Ty::Members(mut path) => {
                    path.push(name.to_string());
                    return Ty::nix(Ty::Members(path));
                }
                Ty::Options(prefix) => prefix,
                _ => return Ty::nix(Ty::Any),
            },
            _ => return staged(nix, Ty::Any),
        };
        let mut path = prefix.clone();
        path.push(name.to_string());
        let Some((mut message, near)) = self.unknown(&prefix, name) else {
            return Ty::nix(match self.options.get(&path) {
                Some(declared) => declared.ty.clone(),
                None if self.known.contains_key(&path) => Ty::Any,
                None => Ty::Options(path),
            });
        };
        if let Some((near_path, source)) = near {
            message.push_str(&format!(" Did you mean `{}`? (declared at {source})", near_path[prefix.len()]));
        }
        self.report(span.end - name.len(), span.line, message);
        Ty::nix(Ty::Any)
    }

    fn report(&mut self, offset: usize, line: usize, message: String) {
        if self.quiet {
            return;
        }
        self.diagnostics.push(diagnostic(self.file, offset, line, message));
    }

    fn push(&mut self, open: bool) {
        self.scopes.push(Frame {
            names: BTreeMap::new(),
            lines: BTreeMap::new(),
            open,
        });
    }

    fn bind_once(&mut self, name: &str, ty: Ty, line: usize) {
        let first = self.scopes.last().and_then(|f| f.lines.get(name)).copied();
        if let Some(first) = first {
            let message = format!("`{name}` is already bound at line {first}; a block binds a name once.");
            let offset = self.line_offset(line, name);
            self.report(offset, line, message);
            return;
        }
        if let Some(frame) = self.scopes.last_mut() {
            frame.lines.insert(name.to_string(), line);
        }
        self.bind(name, ty);
    }

    fn bind(&mut self, name: &str, ty: Ty) {
        if let Some(frame) = self.scopes.last_mut() {
            frame.names.insert(name.to_string(), ty);
        }
    }
}

fn diagnostic(file: &SourceFile, offset: usize, line: usize, message: String) -> Diagnostic {
    let text = &file.text;
    let line_start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[offset..].find('\n').map_or(text.len(), |i| offset + i);
    Diagnostic {
        file: file.name.clone(),
        line,
        column: text[line_start..offset].chars().count() + 1,
        message,
        text: text[line_start..line_end].trim_end().to_string(),
    }
}

pub(crate) fn closest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (name.chars().count() / 3).max(1);
    candidates
        .into_iter()
        .map(|c| (distance(name, c), c))
        .filter(|(d, _)| *d <= limit)
        .min()
        .map(|(_, c)| c)
}

pub fn distance(a: &str, b: &str) -> usize {
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

struct Owner {
    outside: BTreeSet<String>,
    types: BTreeMap<String, Ty>,
}

fn slot_ty(text: &str) -> Ty {
    match text.split_once(' ') {
        Some(("maybe", rest)) => Ty::Maybe(Box::new(slot_ty(rest))),
        Some(("nix", rest)) => Ty::nix(slot_ty(rest)),
        _ => match text {
            "Text" => Ty::Text,
            "Int" | "Port" => Ty::Int,
            "Bool" => Ty::Bool,
            "Path" => Ty::Path,
            "Any" => Ty::Any,
            name => Ty::Declared(name.into()),
        },
    }
}

fn reading_types(vocabulary: &crate::Vocabulary, reading: &crate::Reading, types: &mut BTreeMap<String, Ty>) {
    let shape = &vocabulary.shapes[reading.shape];
    for (arg, slot) in reading.args.iter().zip(&shape.slots) {
        if let Arg::Var(name) = arg {
            types.entry(name.clone()).or_insert_with(|| slot_ty(&slot.ty));
        }
    }
}

fn literal_vars(literal: &crate::Literal, out: &mut BTreeSet<String>) {
    use crate::Literal as L;
    let reading = |r: &crate::Reading, out: &mut BTreeSet<String>| {
        for arg in &r.args {
            if let Arg::Var(name) = arg {
                out.insert(name.clone());
            }
        }
    };
    match literal {
        L::Sentence(r) | L::Not(r) => reading(r, out),
        L::Compare(_, a, b) => {
            expr_vars(a, out);
            expr_vars(b, out);
        }
        L::Holds(e) => expr_vars(e, out),
        L::Bind(v, e) | L::In(v, e) => {
            out.insert(v.clone());
            expr_vars(e, out);
        }
        L::IsA(v, _) => {
            out.insert(v.clone());
        }
        L::Aggregate { var, of, body, .. } => {
            out.insert(var.clone());
            if let Some(of) = of {
                expr_vars(of, out);
            }
            body.iter().for_each(|l| literal_vars(l, out));
        }
        L::First { var, of, by, body } => {
            out.insert(var.clone());
            out.insert(of.clone());
            expr_vars(by, out);
            body.iter().for_each(|l| literal_vars(l, out));
        }
    }
}

fn expr_vars(expr: &Expr, out: &mut BTreeSet<String>) {
    match &expr.kind {
        ExprKind::Var(name) => {
            out.insert(name.clone());
        }
        _ => sub_exprs(expr, &mut |e| expr_vars(e, out)),
    }
}

fn sub_exprs(expr: &Expr, each: &mut dyn FnMut(&Expr)) {
    match &expr.kind {
        ExprKind::Text(parts) | ExprKind::TextBlock(parts) => {
            for part in parts {
                if let TextPart::Interpolation(e) = part {
                    each(e);
                }
            }
        }
        ExprKind::List(items) => items.iter().for_each(|e| each(e)),
        ExprKind::Record(fields) => fields.iter().for_each(|f| each(&f.value)),
        ExprKind::Field(e, _) | ExprKind::Not(e) | ExprKind::Lambda(_, e) => each(e),
        ExprKind::Call(f, args) => {
            each(f);
            args.iter().for_each(|e| each(e));
        }
        ExprKind::If(a, b, c) => {
            each(a);
            each(b);
            each(c);
        }
        ExprKind::Binary(_, a, b) | ExprKind::With(a, b) => {
            each(a);
            each(b);
        }
        ExprKind::Query(Query::All { sorted_by: Some(key), .. }) => each(key),
        _ => {}
    }
}

fn literal_exprs<'e>(literal: &'e crate::Literal, out: &mut Vec<&'e Expr>) {
    use crate::Literal as L;
    match literal {
        L::Compare(_, a, b) => out.extend([a, b]),
        L::Holds(e) | L::Bind(_, e) | L::In(_, e) => out.push(e),
        L::Aggregate { of, body, .. } => {
            out.extend(of.iter());
            body.iter().for_each(|l| literal_exprs(l, out));
        }
        L::First { by, body, .. } => {
            out.push(by);
            body.iter().for_each(|l| literal_exprs(l, out));
        }
        L::Sentence(_) | L::Not(_) | L::IsA(..) => {}
    }
}

fn the_queries(file: &SourceFile, typed: &mut Vec<Occurrence>, diagnostics: &mut Vec<Diagnostic>) {
    let Ok(lines) = lex(&file.text) else { return };
    let lines: BTreeMap<usize, &Line> = lines.iter().map(|l| (l.no, l)).collect();
    let sentences = &file.sentences;
    let vocabulary = &sentences.vocabulary;
    let mut owners: Vec<(Owner, Vec<(usize, Expr)>)> = Vec::new();
    let body_owner = |literals: Vec<(usize, &crate::Literal)>, extra: &[&crate::Reading]| {
        let mut outside = BTreeSet::new();
        let mut types = BTreeMap::new();
        let mut exprs = Vec::new();
        for reading in extra {
            literal_vars(&crate::Literal::Sentence((*reading).clone()), &mut outside);
            reading_types(vocabulary, reading, &mut types);
        }
        for (line, literal) in &literals {
            literal_vars(literal, &mut outside);
            if let crate::Literal::Sentence(r) = literal {
                reading_types(vocabulary, r, &mut types);
            }
            let mut found = Vec::new();
            literal_exprs(literal, &mut found);
            exprs.extend(found.into_iter().map(|e| (*line, e.clone())));
        }
        (Owner { outside, types }, exprs)
    };
    for rule in &sentences.rules {
        owners.push(body_owner(rule.body.iter().map(|(l, x)| (*l, x)).collect(), &[&rule.head]));
    }
    for handler in &sentences.handlers {
        owners.push(body_owner(handler.when.iter().map(|(l, x)| (*l, x)).collect(), &[&handler.event]));
    }
    if let Ok(top) = blocks(&file.text) {
        for block in top.iter().filter(|b| word(&b.line.tokens, 0) == "view") {
            let tokens = &block.line.tokens;
            let mut types = BTreeMap::new();
            if tokens.len() > 4 && token_word(&tokens[2]) == "for" {
                for param in tokens[4..tokens.len() - 1].split(|t| token_word(t) == ",") {
                    if let [name, _colon, ty @ ..] = param {
                        let ty = ty.iter().map(token_word).collect::<Vec<_>>().join(" ");
                        types.insert(token_word(name).to_string(), slot_ty(&ty));
                    }
                }
            }
            let mut exprs = Vec::new();
            for child in &block.children {
                let line = &child.line;
                if let [_, colon, rest @ ..] = line.tokens.as_slice() {
                    if token_word(colon) == ":" && !rest.is_empty() {
                        if let Ok(e) = parse_expr(rest, line) {
                            exprs.push((line.no, e));
                        }
                    }
                }
            }
            owners.push((Owner { outside: BTreeSet::new(), types }, exprs));
        }
    }
    for (owner, exprs) in owners {
        for (line, expr) in exprs {
            let Some(source) = lines.get(&line) else { continue };
            visit_the(file, vocabulary, &owner, source, &expr, typed, diagnostics);
        }
    }
}

fn visit_the(
    file: &SourceFile,
    vocabulary: &crate::Vocabulary,
    owner: &Owner,
    line: &Line,
    expr: &Expr,
    typed: &mut Vec<Occurrence>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let ExprKind::Query(query @ Query::The { var, .. }) = &expr.kind {
        if let Ok(body) = vocabulary.read_query(line, query) {
            match the_type(vocabulary, owner, var, &body) {
                Some(ty) => typed.push(Occurrence {
                    file: file.name.clone(),
                    start: expr.span.start,
                    end: expr.span.end,
                    ty,
                    option: None,
                }),
                None => {
                    let shown = file.text[expr.span.start..expr.span.end].trim();
                    diagnostics.push(diagnostic(
                        file,
                        expr.span.start,
                        expr.span.line,
                        format!(
                            "`{shown}` may have several `?{var}`: no chain of `exactly one` or `at most one` modes fixes `?{var}` from constants and outside variables."
                        ),
                    ));
                }
            }
        }
        return;
    }
    sub_exprs(expr, &mut |e| visit_the(file, vocabulary, owner, line, e, typed, diagnostics));
}

fn the_type(vocabulary: &crate::Vocabulary, owner: &Owner, var: &str, body: &[crate::Literal]) -> Option<Ty> {
    use crate::Literal as L;
    let mut exact: BTreeMap<String, bool> = owner.outside.iter().map(|v| (v.clone(), true)).collect();
    let mut types = owner.types.clone();
    for literal in body {
        if let L::Sentence(r) = literal {
            reading_types(vocabulary, r, &mut types);
        }
    }
    let mut top = owner.outside.clone();
    for literal in body {
        if let L::Sentence(_) | L::Not(_) | L::Bind(..) | L::In(..) | L::IsA(..) | L::Compare(..) | L::Holds(_) = literal {
            literal_vars(literal, &mut top);
        } else if let L::Aggregate { var, .. } | L::First { var, .. } = literal {
            top.insert(var.clone());
        }
    }
    let reads = |vars: &BTreeSet<String>, exact: &BTreeMap<String, bool>| -> Option<bool> {
        vars.iter().try_fold(true, |all, v| exact.get(v).map(|e| all && *e))
    };
    loop {
        let mut changed = false;
        for literal in body {
            let found: Option<(String, bool)> = match literal {
                L::Sentence(reading) => {
                    let shape = &vocabulary.shapes[reading.shape];
                    shape.modes.iter().find_map(|mode| {
                        let Some(Arg::Var(value)) = reading.args.get(mode.value) else { return None };
                        if exact.contains_key(value) {
                            return None;
                        }
                        let mut keys = BTreeSet::new();
                        for key in &mode.keys {
                            if let Some(Arg::Var(k)) = reading.args.get(*key) {
                                keys.insert(k.clone());
                            }
                        }
                        reads(&keys, &exact).map(|e| (value.clone(), e && mode.exactly))
                    })
                }
                L::Bind(v, e) if !exact.contains_key(v) => {
                    let mut vars = BTreeSet::new();
                    expr_vars(e, &mut vars);
                    if let ExprKind::Var(other) = &e.kind {
                        if let Some(ty) = types.get(other).cloned() {
                            types.entry(v.clone()).or_insert(ty);
                        }
                    }
                    reads(&vars, &exact).map(|x| (v.clone(), x))
                }
                L::Aggregate { var: v, .. } | L::First { var: v, .. } if !exact.contains_key(v) => {
                    let mut vars = BTreeSet::new();
                    literal_vars(literal, &mut vars);
                    vars.remove(v);
                    let vars: BTreeSet<String> = vars.intersection(&top).cloned().collect();
                    reads(&vars, &exact).map(|x| (v.clone(), x))
                }
                _ => None,
            };
            if let Some((v, e)) = found {
                exact.insert(v, e);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let is_exact = *exact.get(var)?;
    let ty = types.get(var).cloned().unwrap_or(Ty::Any);
    Some(match ty {
        Ty::Maybe(_) => ty,
        _ if is_exact => ty,
        _ => Ty::Maybe(Box::new(ty)),
    })
}

fn aggregate_values(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    let sentences = &file.sentences;
    let bodies = sentences
        .rules
        .iter()
        .map(|r| (&r.head, &r.body))
        .chain(sentences.handlers.iter().map(|h| (&h.event, &h.when)));
    for (reading, body) in bodies {
        let mut types = BTreeMap::new();
        reading_types(&sentences.vocabulary, reading, &mut types);
        let literals: Vec<&crate::Literal> = body.iter().map(|(_, l)| l).collect();
        aggregate_body(file, &literals, &types, diagnostics);
    }
}

fn aggregate_body(
    file: &SourceFile,
    body: &[&crate::Literal],
    outer: &BTreeMap<String, Ty>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use crate::{Aggregate as A, Literal as L};
    let vocabulary = &file.sentences.vocabulary;
    let mut types = outer.clone();
    for literal in body {
        if let L::Sentence(r) = literal {
            reading_types(vocabulary, r, &mut types);
        }
    }
    for literal in body {
        let (value, want, inner): (Option<&Expr>, &str, &Vec<crate::Literal>) = match literal {
            L::Aggregate { op: A::Sum, of, body, .. } => (of.as_ref(), "sum", body),
            L::Aggregate { op: A::Min, of, body, .. } => (of.as_ref(), "min", body),
            L::Aggregate { op: A::Max, of, body, .. } => (of.as_ref(), "max", body),
            L::Aggregate { body, .. } => (None, "", body),
            L::First { by, body, .. } => (Some(by), "first … by", body),
            _ => continue,
        };
        let inner: Vec<&crate::Literal> = inner.iter().collect();
        if let Some(value) = value {
            let mut scope = types.clone();
            for l in &inner {
                if let L::Sentence(r) = l {
                    reading_types(vocabulary, r, &mut scope);
                }
            }
            let ty = match &value.kind {
                ExprKind::Var(name) => scope.get(name).cloned(),
                ExprKind::Int(_) => Some(Ty::Int),
                ExprKind::Text(_) | ExprKind::TextBlock(_) => Some(Ty::Text),
                _ => None,
            };
            let ty = match ty {
                Some(Ty::Maybe(inner)) => Some(*inner),
                other => other,
            };
            let takes = if want == "sum" { "`Int`" } else { "`Int` or `Text`" };
            let fits = match &ty {
                None | Some(Ty::Any) => true,
                Some(Ty::Int) => true,
                Some(Ty::Text) => want != "sum",
                Some(_) => false,
            };
            if !fits {
                let shown = file.text[value.span.start..value.span.end].trim();
                let ty = ty.map(|t| ty_name(&t));
                diagnostics.push(diagnostic(
                    file,
                    value.span.start,
                    value.span.line,
                    format!("`{want}` takes {takes}, but `{shown}` is `{}`.", ty.unwrap_or_default()),
                ));
            }
        }
        aggregate_body(file, &inner, &types, diagnostics);
    }
}

fn ty_name(ty: &Ty) -> String {
    match ty {
        Ty::Nix(inner) => format!("nix {}", ty_name(inner)),
        Ty::Maybe(inner) => format!("maybe {}", ty_name(inner)),
        Ty::Declared(name) => name.clone(),
        other => other.to_string(),
    }
}
