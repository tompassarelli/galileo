use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::expr::{parse_expr, BinOp, Expr, ExprKind, Param, Query, TextPart};
use crate::record::{
    record_decl, Arguments, AttrPath, Entry, FileError, FileValue, ForBlock, ModuleItem,
    RecordDecl, Segment,
};
use crate::schema::Schema;
use crate::types::{parse_type, OptionType, Type};
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

const TYPE_NAMES: &[&str] = &["Any", "Bool", "Int", "Text", "Path", "OptionPath", "List", "Attrs"];

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
    let close = (open..tokens.len()).find(|&i| word(tokens, i) == ")").ok_or_else(form)?;
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

fn option_ty(ty: &OptionType) -> Ty {
    match ty {
        OptionType::Enable(_) | OptionType::Bool => Ty::Bool,
        OptionType::Int | OptionType::Port => Ty::Int,
        OptionType::Text | OptionType::Lines | OptionType::OneOf(_) => Ty::Text,
        OptionType::Path => Ty::Path,
        OptionType::Package => Ty::Any,
        OptionType::ListOf(inner) => Ty::List(Box::new(option_ty(inner))),
        OptionType::AttrsOf(inner) => Ty::Attrs(Box::new(option_ty(inner))),
        OptionType::Maybe(inner) => Ty::Maybe(Box::new(option_ty(inner))),
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
        "Int" => Ty::Int,
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
                                    ty: option_ty(&option.ty),
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
    if !schema.options.is_empty() {
        for path in schema.options.keys() {
            known.insert(path.clone(), schema.source.clone());
        }
        for (path, declared) in &options {
            known.insert(path.clone(), format!("{}:{}", declared.file, declared.line));
        }
    }
    let mut typed = Vec::new();
    for file in files {
        let mut names: BTreeMap<String, Ty> = PRELUDE
            .iter()
            .map(|n| {
                let ty = match *n {
                    "first" | "lookup" => Ty::Fn(Box::new(Ty::Maybe(Box::new(Ty::Any)))),
                    _ => Ty::Any,
                };
                (n.to_string(), ty)
            })
            .collect();
        names.insert("builtins".into(), Ty::nix(Ty::Members(vec!["builtins".into()])));
        let mut checker = Checker {
            file,
            options: &options,
            known: &known,
            signatures: &signatures,
            declared: &declared,
            quiet: false,
            scopes: vec![Frame { names, open: false }],
            diagnostics: &mut diagnostics,
            typed: &mut typed,
        };
        for decl in &file.decls {
            checker.decl(decl);
        }
        checker.facts();
    }
    (diagnostics, Index { options, typed })
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
    signatures: &'a BTreeMap<String, Typed>,
    declared: &'a BTreeSet<String>,
    quiet: bool,
    scopes: Vec<Frame>,
    diagnostics: &'a mut Vec<Diagnostic>,
    typed: &'a mut Vec<Occurrence>,
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
                Ty::nix(Ty::Options(Vec::new()))
            } else {
                Ty::nix(Ty::Members(vec![name.clone()]))
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
        if self.known.is_empty() {
            return;
        }
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
            let children: Vec<&str> = self
                .known
                .range(at.clone()..)
                .take_while(|(p, _)| p.starts_with(&at))
                .filter(|(p, _)| p.len() > at.len())
                .map(|(p, _)| p[at.len()].as_str())
                .collect();
            if children.contains(&name.as_str()) {
                at.push(name.clone());
                continue;
            }
            let mut message = format!(
                "`{name}` isn't an option of `{}`.",
                if at.is_empty() { "config".to_string() } else { at.join(".") }
            );
            if let Some(near) = closest(name, children) {
                let mut near_path = at.clone();
                near_path.push(near.to_string());
                let source = self
                    .known
                    .range(near_path.clone()..)
                    .next()
                    .map_or("", |(_, s)| s.as_str());
                message.push_str(&format!(" Did you mean `{}`? ({source})", near_path.join(".")));
            }
            let line = path.line;
            let offset = self.line_offset(line, name);
            self.report(offset, line, message);
            return None;
        }
        Some(at)
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
                self.enter_arguments(arguments);
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
        self.scopes.push(Frame { names, open: false });
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
        let children: Vec<&str> = self
            .options
            .keys()
            .filter(|p| p.len() > prefix.len() && p.starts_with(&prefix))
            .map(|p| p[prefix.len()].as_str())
            .collect();
        if children.is_empty() || children.contains(&name) {
            return Ty::nix(match self.options.get(&path) {
                Some(declared) => declared.ty.clone(),
                None => Ty::Options(path),
            });
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
            open,
        });
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
