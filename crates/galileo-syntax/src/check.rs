use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::expr::{parse_expr, BinOp, Expr, ExprKind, Param, Query, TextPart};
use crate::record::{
    record_decl, Arguments, AttrPath, Entry, FileError, FileValue, ForBlock, ForHead, ModuleItem,
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

pub(crate) const FACT_ROOTS: &[&str] = &["config", "lib", "pkgs", "inputs", "builtins", "import", "toString"];

pub(crate) const PRELUDE: &[&str] = &[
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
    has_body: bool,
}

struct Typed {
    params: Vec<(String, Ty)>,
    result: Ty,
}

const TYPE_NAMES: &[&str] = &["Any", "Bool", "Int", "Text", "Path", "Port", "OptionPath", "List", "Attrs"];

impl SourceFile {
    pub fn parse(name: &str, text: &str) -> Result<Self, FileError> {
        Self::linked(name, text, None)
    }

    pub fn linked(name: &str, text: &str, sentences: Option<Sentences>) -> Result<Self, FileError> {
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
            } else if word(&block.line.tokens, 0) == "define" {
                let mut definition = signature(&block.line).map_err(located)?;
                definition.has_body = !block.children.is_empty();
                signatures.push(definition);
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
            sentences: match sentences {
                Some(sentences) => sentences,
                None => read_sentences(text).map_err(located)?,
            },
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

pub(crate) fn signature(line: &Line) -> Result<Signature, Error> {
    const FORM: &str = "a definition signature is `define name(param: Type, ...): Type`";
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
        has_body: false,
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
    let embedded = SourceFile::parse(
        "lib/firn/nix-lib.gal",
        include_str!("../../../lib/firn/nix-lib.gal"),
    ).expect("the embedded Nix signatures parse");
    for file in std::iter::once(&embedded).chain(files) {
        let mut errors = Vec::new();
        for signature in &file.signatures {
            let before = errors.len();
            let typed = Typed {
                params: signature
                    .params
                    .iter()
                    .map(|(name, ty)| (name.clone(), value_ty(ty, &declared, &mut errors)))
                    .collect(),
                result: value_ty(&signature.result, &declared, &mut errors),
            };
            let message = if std::ptr::eq(file, &embedded) {
                let parts: Vec<&str> = signature.name.split('.').collect();
                if signature.has_body {
                    Some("a Nix signature has no body".to_string())
                } else if !matches!(parts.as_slice(), ["lib" | "builtins", _, ..])
                    && !(parts.len() == 1 && !PRELUDE.contains(&signature.name.as_str()))
                    || parts.iter().any(|part| part.is_empty())
                {
                    Some("a Nix signature names a `lib` or `builtins` member, or a bare global that is not a prelude name".to_string())
                } else if signature.params.iter().any(|(_, ty)| contains_nix(ty))
                    || contains_nix(&signature.result)
                {
                    Some("a Nix signature writes its types without `nix`".to_string())
                } else if signatures.contains_key(&signature.name) {
                    Some(format!("`{}` already has a Nix signature", signature.name))
                } else {
                    if errors.len() == before {
                        signatures.insert(signature.name.clone(), typed);
                    }
                    None
                }
            } else if !signature.has_body && errors.len() == before {
                Some(format!("`define {}` needs a body; only the embedded Nix library declares a signature without one", signature.name))
            } else {
                None
            };
            if let Some(message) = message {
                let start = file.text.split_inclusive('\n').take(signature.line - 1).map(str::len).sum();
                diagnostics.push(diagnostic(file, start, signature.line, message));
            }
        }
        for (span, message) in errors {
            diagnostics.push(diagnostic(file, span.start, span.line, message));
        }
    }
    let mut text_concat = BTreeMap::new();
    let mut options = BTreeMap::new();
    let mut read = Vec::new();
    for file in files {
        for decl in &file.decls {
            if let RecordDecl::Module(module) = decl {
                for option in &crate::record::declarations(&module.items) {
                    let mut errors = Vec::new();
                    let ty = match &option.kind {
                        OptionKind::Enable(_) => Ty::Bool,
                        OptionKind::Type(ty) => option_ty(ty, &mut errors),
                    };
                    for (span, message) in errors {
                        diagnostics.push(diagnostic(file, span.start, span.line, message));
                    }
                    if let Some(path) = static_path(&option.path) {
                        let line = option.path.line;
                        let start = file.text.split_inclusive('\n').take(line - 1).map(str::len).sum::<usize>();
                        let start = start + file.text[start..].len() - file.text[start..].trim_start_matches([' ', '\t']).len();
                        read.push((path.clone(), file, start, line));
                        if options.contains_key(&path) {
                            continue;
                        }
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
    let mut seen = BTreeSet::new();
    for (path, file, start, line) in &read {
        let message = if !seen.insert(path) {
            let first = &options[path];
            Some(format!("`{}` is already declared at {}:{}", path.join("."), first.file, first.line))
        } else {
            (1..path.len()).find_map(|n| options.get(&path[..n]).map(|q| {
                format!(
                    "`{}` lies under the option `{}`, declared at {}:{}; past an option the path is a key into its value",
                    path.join("."), path[..n].join("."), q.file, q.line,
                )
            }))
        };
        if let Some(message) = message {
            diagnostics.push(diagnostic(file, *start, *line, message));
        }
    }
    let mut open = BTreeSet::new();
    for file in files {
        for decl in &file.decls {
            let RecordDecl::Module(module) = decl else { continue };
            if !opaque_imports(&module.items) {
                continue;
            }
            for option in &crate::record::declarations(&module.items) {
                if let Some(path) = static_path(&option.path) {
                    open.insert(path[..path.len().saturating_sub(1)].to_vec());
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
                    "length" => Ty::Fn(Box::new(Ty::Int)),
                    "contains" | "starts-with" => Ty::Fn(Box::new(Ty::Bool)),
                    "lowercase" | "trim" | "first-word" | "rest-words" => Ty::Fn(Box::new(Ty::Text)),
                    _ => Ty::Any,
                };
                (n.to_string(), ty)
            })
            .collect();
        let mut checker = Checker {
            file,
            options: &options,
            known: &known,
            schema: !schema.source.is_empty() || !schema.options.is_empty(),
            open: &open,
            signatures: &signatures,
            declared: &declared,
            quiet: false,
            scopes: vec![Frame { names, lines: BTreeMap::new(), open: false }],
            diagnostics: &mut diagnostics,
            typed: &mut typed,
            text_concat: &mut text_concat,
            narrowed: Vec::new(),
        };
        for decl in &file.decls {
            checker.decl(decl);
        }
        checker.facts();
        for (reading, body) in file.sentences.rules.iter().map(|rule| (&rule.head, &rule.body))
            .chain(file.sentences.handlers.iter().map(|handler| (&handler.event, &handler.when)))
        {
            let mut types = BTreeMap::new();
            reading_types(&file.sentences.vocabulary, reading, &mut types);
            for (_, literal) in body {
                if let crate::Literal::Sentence(reading) = literal {
                    reading_types(&file.sentences.vocabulary, reading, &mut types);
                }
            }
            checker.scopes.push(Frame {
                names: types.into_iter().map(|(name, ty)| (format!("?{name}"), ty)).collect(),
                lines: BTreeMap::new(), open: true,
            });
            let mut expressions = Vec::new();
            for (_, literal) in body {
                literal_exprs(literal, &mut expressions);
            }
            while let Some(expr) = expressions.pop() {
                if matches!(expr.kind, ExprKind::Binary(BinOp::In, _, _)) {
                    checker.expr(expr);
                } else {
                    sub_exprs(expr, &mut |inner| expressions.push(inner));
                }
            }
            checker.scopes.pop();
        }
        the_queries(file, &mut typed, &mut diagnostics);
        aggregate_values(file, &mut diagnostics);
    }
    (diagnostics, Index { options, typed, text_concat })
}

fn contains_nix(ty: &Type) -> bool {
    match ty {
        Type::Nix(_) => true,
        Type::Maybe(inner) => contains_nix(inner),
        Type::Named { args, .. } => args.iter().any(contains_nix),
    }
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

fn opaque_imports(items: &[ModuleItem]) -> bool {
    items.iter().any(|item| matches!(item, ModuleItem::Imports(value) if !matches!(&value.kind, ExprKind::List(paths) if paths.iter().all(|path| matches!(path.kind, ExprKind::Path(_))))))
}

struct Checker<'a> {
    file: &'a SourceFile,
    options: &'a BTreeMap<Vec<String>, Declared>,
    known: &'a BTreeMap<Vec<String>, String>,
    schema: bool,
    open: &'a BTreeSet<Vec<String>>,
    signatures: &'a BTreeMap<String, Typed>,
    declared: &'a BTreeSet<String>,
    quiet: bool,
    scopes: Vec<Frame>,
    diagnostics: &'a mut Vec<Diagnostic>,
    typed: &'a mut Vec<Occurrence>,
    text_concat: &'a mut BTreeMap<String, BTreeSet<(usize, usize)>>,
    narrowed: Vec<(Guard, String, usize)>,
}

#[derive(Clone, Copy, PartialEq)]
enum Guard {
    NotNull,
    NotZero,
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
            ModuleItem::Option(_) | ModuleItem::OptionGroup { .. } => {
                for option in crate::record::item_declarations(item) {
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
            }
            ModuleItem::Imports(expr) => {
                self.expr(expr);
            }
            ModuleItem::Config { when, entries, .. } => {
                if let Some(when) = when {
                    self.condition(when, "`when`");
                }
                let mark = self.narrow(when.as_ref(), BinOp::Ne);
                self.entries(entries);
                self.narrowed.truncate(mark);
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
                    let mark = self.narrow(when.as_ref(), BinOp::Ne);
                    self.expr(value);
                    self.narrowed.truncate(mark);
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
                    let mark = self.narrow(when.as_ref(), BinOp::Ne);
                    self.entries(entries);
                    self.narrowed.truncate(mark);
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
        if !self.schema && self.open.contains(at) {
            return None;
        }
        let closed = if self.schema {
            at.is_empty() || below().next().is_some()
        } else {
            below().any(|(p, _)| p.len() == at.len() + 1 && self.options.contains_key(p))
        };
        let children: Vec<&str> = below().map(|(p, _)| p[at.len()].as_str()).collect();
        if !closed || children.contains(&name) {
            return None;
        }
        let mut message = format!(
            "`{name}` isn't an option of `{}`.",
            if at.is_empty() { "config".to_string() } else { at.join(".") }
        );
        let mut names = children.clone();
        names.dedup();
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
        if near.is_none() && !names.is_empty() {
            let shown: Vec<String> = names.iter().take(6).map(|n| format!("`{n}`")).collect();
            let list = match (shown.split_last(), names.len().saturating_sub(6)) {
                (Some((last, [])), 0) => last.clone(),
                (Some((last, rest)), 0) => format!("{} and {last}", rest.join(", ")),
                (_, more) => format!("{} and {more} more", shown.join(", ")),
            };
            message.pop();
            message.push_str(&format!(", which has {list}."));
        }
        Some((message, near))
    }

    fn line_offset(&self, line: usize, name: &str) -> usize {
        let text = &self.file.text;
        let start: usize = text.split_inclusive('\n').take(line - 1).map(str::len).sum();
        let row = text[start..].lines().next().unwrap_or("");
        start + row.find(name).unwrap_or(row.len() - row.trim_start().len())
    }

    fn for_block(&mut self, block: &ForBlock) {
        self.for_head(&block.head);
        self.entries(&block.entries);
        self.scopes.pop();
    }

    fn for_head(&mut self, head: &ForHead) {
        if self.expr(&head.source).is_nix() {
            let message = format!(
                "`{}` depends on Nix evaluation, but `for` unrolls when Galileo builds, so its source must be known.",
                self.source(&head.source)
            );
            self.report(head.source.span.start, head.source.span.line, message);
        }
        self.push(false);
        for var in &head.vars {
            self.bind(&format!("?{var}"), Ty::Any);
        }
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

    fn narrow_key(&self, expr: &Expr) -> Option<(String, usize)> {
        match &expr.kind {
            ExprKind::Name(name) => {
                let frame = self.scopes.iter().rposition(|f| f.names.contains_key(name))?;
                Some((name.clone(), frame))
            }
            ExprKind::Field(base, name) => {
                let (key, frame) = self.narrow_key(base)?;
                Some((format!("{key}.{name}"), frame))
            }
            _ => None,
        }
    }

    fn narrow(&mut self, cond: Option<&Expr>, op: BinOp) -> usize {
        let mark = self.narrowed.len();
        let mut pending: Vec<&Expr> = cond.into_iter().collect();
        while let Some(expr) = pending.pop() {
            let ExprKind::Binary(found, lhs, rhs) = &expr.kind else { continue };
            if *found == BinOp::And && op == BinOp::Ne {
                pending.extend([lhs.as_ref(), rhs.as_ref()]);
            } else if *found == op {
                let tested = match (&lhs.kind, &rhs.kind) {
                    (_, ExprKind::Null) => lhs,
                    (ExprKind::Null, _) => rhs,
                    _ => continue,
                };
                if let Some((key, frame)) = self.narrow_key(tested) {
                    self.narrowed.push((Guard::NotNull, key, frame));
                }
            }
        }
        for tested in cond.map(|cond| nonzero_tested(cond, op == BinOp::Ne)).unwrap_or_default() {
            if let Some((key, frame)) = self.narrow_key(tested) {
                self.narrowed.push((Guard::NotZero, key, frame));
            }
        }
        mark
    }

    fn infer(&mut self, expr: &Expr) -> Ty {
        let ty = self.infer_unnarrowed(expr);
        if !matches!(expr.kind, ExprKind::Name(_) | ExprKind::Field(..)) || self.narrowed.is_empty() {
            return ty;
        }
        match self.narrow_key(expr) {
            Some((key, frame)) if self.narrowed.iter().any(|(guard, k, f)| *guard == Guard::NotNull && *k == key && *f == frame) => {
                let nix = ty.is_nix();
                match ty.value() {
                    Ty::Maybe(inner) => staged(nix, inner.as_ref().clone()),
                    _ => ty,
                }
            }
            _ => ty,
        }
    }

    fn infer_unnarrowed(&mut self, expr: &Expr) -> Ty {
        match &expr.kind {
            ExprKind::Null => Ty::Null,
            ExprKind::Bool(_) => Ty::Bool,
            ExprKind::Int(_) => Ty::Int,
            ExprKind::Path(_) => Ty::Path,
            ExprKind::Text(parts) | ExprKind::TextBlock(parts) => {
                let mut nix = false;
                for part in parts {
                    if let TextPart::Interpolation(inner) = part {
                        let ty = self.expr(inner);
                        nix |= ty.is_nix() || matches!(ty.value(), Ty::Path);
                        if !ty.is_nix() && !matches!(ty.value(), Ty::Text | Ty::Int | Ty::Path | Ty::Any) {
                            self.report(inner.span.start, inner.span.line, format!("text interpolation needs Text, Int or a Nix value, but this is `{ty}`."));
                        }
                    }
                }
                staged(nix, Ty::Text)
            }
            ExprKind::Name(name) => self.lookup(name, name, &expr.span),
            ExprKind::Var(name) => self.lookup(&format!("?{name}"), &format!("?{name}"), &expr.span),
            ExprKind::Computed(inner) => {
                self.computed(inner);
                Ty::nix(Ty::Any)
            }
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
                    field.each.iter().for_each(|head| self.for_head(head));
                    nix |= self.path(&field.path);
                    nix |= self.expr(&field.value).is_nix();
                    self.scopes.truncate(self.scopes.len() - field.each.len());
                }
                staged(nix, Ty::Attrs(Box::new(Ty::Any)))
            }
            ExprKind::Field(base, name) => {
                let base = self.expr(base);
                self.field(base, name, &expr.span)
            }
            ExprKind::Select(base, segment) => {
                let base_ty = self.expr(base);
                if let Segment::Computed(key) = segment.as_ref() {
                    self.expr(key);
                }
                self.select(base, base_ty, &expr.span)
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
                if let ExprKind::Name(name) = &callee.kind {
                    if PRELUDE.contains(&name.as_str()) && self.scopes.iter().skip(1).all(|frame| !frame.names.contains_key(name)) {
                        if arg_tys.iter().any(Ty::is_nix) {
                            self.report(expr.span.start, expr.span.line, format!("`{name}` needs known arguments; this depends on Nix evaluation, so use `builtins.{name}` on the Nix side."));
                        }
                        return match &callee_ty {
                            Ty::Fn(result) => result.as_ref().clone(),
                            _ => Ty::Any,
                        };
                    }
                }
                if let Ty::Fn(result) = callee_ty.unstaged() {
                    let result = result.as_ref().clone();
                    return staged(nix || result.is_nix(), result);
                }
                let Some(name) = self.member_of(&callee_ty) else {
                    return staged(nix, Ty::Any);
                };
                let Some(signature) = self.signatures.get(&name) else {
                    if let Some((parent, member)) = name.rsplit_once('.') {
                        if matches!(name.split('.').next(), Some("lib" | "builtins")) {
                            let mut message = format!("`{name}` has no Nix signature.");
                            let candidates = self.signatures.keys().filter_map(|candidate| {
                                let (prefix, member) = candidate.rsplit_once('.')?;
                                (prefix == parent).then_some(member)
                            });
                            if let Some(near) = closest(member, candidates) {
                                message.push_str(&format!(" Did you mean `{parent}.{near}`?"));
                            }
                            self.report(callee.span.start, callee.span.line, message);
                        }
                    }
                    return staged(nix, Ty::Any);
                };
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
                        if !got.fits(want) && !(*want == Ty::Path && *got.value() == Ty::Text) {
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
            ExprKind::If(cond_expr, then, otherwise) => {
                let cond = self.condition(cond_expr, "`if`");
                let mark = self.narrow(Some(cond_expr), BinOp::Ne);
                let then = self.expr(then);
                self.narrowed.truncate(mark);
                self.narrow(Some(cond_expr), BinOp::Eq);
                let otherwise = self.expr(otherwise);
                self.narrowed.truncate(mark);
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
            ExprKind::Neg(inner) => {
                let ty = self.expr(inner);
                self.operand(inner, &ty, "-", &Ty::Int);
                staged(ty.is_nix(), Ty::Int)
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
                        vars, head, sorted_by, ..
                    } => {
                        for var in vars {
                            self.bind(&format!("?{var}"), Ty::Any);
                        }
                        if let Some(head) = head {
                            self.galileo_value(head, "A query head");
                        }
                        if let Some(key) = sorted_by { self.expr(key); }
                    }
                    Query::The { var, .. } => self.bind(&format!("?{var}"), Ty::Any),
                    Query::Aggregate { op, of, value, .. } => {
                        if let Some(of) = of { self.bind(&format!("?{of}"), Ty::Any); }
                        let ty = value.as_ref().map(|value| self.galileo_value(value, "An aggregate's value"));
                        self.scopes.pop();
                        return match (op, of, ty) {
                            (crate::Aggregate::Min | crate::Aggregate::Max, None, Some(ty)) => Ty::Maybe(Box::new(ty.value().clone())),
                            (crate::Aggregate::Min | crate::Aggregate::Max, _, _) => Ty::Maybe(Box::new(Ty::Any)),
                            _ => Ty::Int,
                        };
                    }
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

    fn galileo_value(&mut self, value: &Expr, what: &str) -> Ty {
        let mut variables = BTreeSet::new();
        let mut operands = Vec::new();
        let mut stack = vec![value];
        while let Some(value) = stack.pop() {
            if let ExprKind::Var(name) = &value.kind { variables.insert(name.clone()); }
            if matches!(value.kind, ExprKind::Name(_) | ExprKind::Field(..) | ExprKind::Var(_)) { operands.push(value); }
            if !matches!(value.kind, ExprKind::Query(_)) { sub_exprs(value, &mut |child| stack.push(child)); }
        }
        for variable in variables {
            let name = format!("?{variable}");
            if !self.scopes.iter().any(|scope| scope.names.contains_key(&name)) { self.bind(&name, Ty::Any); }
        }
        let typed_start = self.typed.len();
        let ty = self.expr(value);
        let operand = operands.into_iter().filter(|value| {
            self.typed[typed_start..].iter().any(|occurrence| {
                occurrence.start == value.span.start && occurrence.end == value.span.end && occurrence.ty.is_nix()
            })
        }).min_by_key(|value| (value.span.start, std::cmp::Reverse(value.span.end)));
        if let Some(operand) = operand.or_else(|| ty.is_nix().then_some(value)) {
            self.report(value.span.start, value.span.line, format!("{what} is computed by Galileo; `{}` is a Nix-stage operand.", self.source(operand)));
        }
        ty
    }

    fn computed(&mut self, name: &Expr) {
        let ty = self.expr(name);
        if ty.is_nix() || !matches!(ty.value(), Ty::Text | Ty::Any) {
            let message = format!("The computed name `{{{}}}` needs a known `Text`, but `{}` is `{ty}`; Nix has no computed variables.", self.source(name), self.source(name));
            self.report(name.span.start, name.span.line, message);
        }
    }

    fn lambda(&mut self, params: &[Param], body: &Expr, args: &[Ty]) -> Ty {
        let mut names = BTreeSet::new();
        for param in params {
            match param {
                Param::Name { name, .. } => { names.insert(name.as_str()); }
                Param::Formals { fields, .. } => names.extend(fields.iter().filter(|field| field.computed.is_none()).map(|field| field.name.as_str())),
            }
        }
        for param in params {
            if let Param::Formals { fields, .. } = param {
                for field in fields {
                    field.each.iter().for_each(|head| self.for_head(head));
                    if let Some(name) = &field.computed {
                        self.computed(name);
                    }
                    if let Some(default) = &field.default {
                        let mut expressions = vec![default];
                        while let Some(expr) = expressions.pop() {
                            if let ExprKind::Name(name) = &expr.kind {
                                if names.contains(name.as_str()) {
                                    self.report(expr.span.start, expr.span.line, format!("A default may not mention formal `{name}` of the same lambda."));
                                }
                            }
                            sub_exprs(expr, &mut |e| expressions.push(e));
                        }
                        self.expr(default);
                    }
                    self.scopes.truncate(self.scopes.len() - field.each.len());
                }
            }
        }
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
                Param::Formals { fields, .. } => {
                    for field in fields.iter().filter(|field| field.computed.is_none()) {
                        self.bind(&field.name, Ty::Any);
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
            BinOp::In => "in",
            BinOp::Update => "//",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Concat => "++",
        };
        let left = self.expr(lhs);
        let mark = if op == BinOp::And { self.narrow(Some(lhs), BinOp::Ne) } else { self.narrowed.len() };
        let right = self.expr(rhs);
        self.narrowed.truncate(mark);
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
            BinOp::In => {
                if nix {
                    let side = if left.is_nix() { lhs } else { rhs };
                    self.report(side.span.start, side.span.line, in_stage_message(&self.source(side)));
                } else if let Ty::List(item) = right.value() {
                    if !left.fits(item) {
                        self.report(lhs.span.start, lhs.span.line, format!(
                            "`in` needs `{item}` here, but `{}` is `{left}`.", self.source(lhs)
                        ));
                    }
                } else {
                    self.operand(rhs, &right, symbol, &Ty::List(Box::new(Ty::Any)));
                }
                Ty::Bool
            }
            BinOp::Or | BinOp::And => {
                self.operand(lhs, &left, symbol, &Ty::Bool);
                self.operand(rhs, &right, symbol, &Ty::Bool);
                Ty::Bool
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let text = *left.value() == Ty::Text || *right.value() == Ty::Text;
                let want = if text { Ty::Text } else { Ty::Int };
                self.operand(lhs, &left, symbol, &want);
                self.operand(rhs, &right, symbol, &want);
                Ty::Bool
            }
            BinOp::Div => {
                self.operand(lhs, &left, symbol, &Ty::Int);
                self.operand(rhs, &right, symbol, &Ty::Int);
                if nix {
                    let side = if left.is_nix() { lhs } else { rhs };
                    self.report(side.span.start, side.span.line, div_stage_message(&self.source(side)));
                }
                let proven: Vec<(String, usize)> = self.narrowed.iter().filter(|(guard, ..)| *guard == Guard::NotZero).map(|(_, key, frame)| (key.clone(), *frame)).collect();
                if let ExprKind::Int(0) = rhs.kind {
                    let message = format!("`{}` divides by 0, so it is always `null`; write `null`.", self.source(lhs).to_string() + " / 0");
                    self.report(rhs.span.start, rhs.span.line, message);
                    Ty::Maybe(Box::new(Ty::Int))
                } else if divisor_nonzero(rhs, &proven, |e| self.narrow_key(e)) {
                    Ty::Int
                } else {
                    Ty::Maybe(Box::new(Ty::Int))
                }
            }
            BinOp::Add | BinOp::Sub | BinOp::Mul => {
                self.operand(lhs, &left, symbol, &Ty::Int);
                self.operand(rhs, &right, symbol, &Ty::Int);
                Ty::Int
            }
            BinOp::Concat | BinOp::Update => {
                if op == BinOp::Concat && *left.value() == Ty::Any && *right.value() == Ty::Any {
                    let offset = lhs.span.end + self.file.text[lhs.span.end..rhs.span.start].find("++").unwrap_or(0);
                    self.report(offset, lhs.span.line, "`++` needs a typed operand to choose text or list concatenation.".into());
                }
                let text = op == BinOp::Concat && (*left.value() == Ty::Text || *right.value() == Ty::Text);
                if text {
                    self.text_concat.entry(self.file.name.clone()).or_default().insert((lhs.span.start, rhs.span.end));
                }
                self.operand(lhs, &left, symbol, &Ty::Any);
                self.operand(rhs, &right, symbol, &Ty::Any);
                if text {
                    Ty::Text
                } else if left.value() == right.value() {
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
            Ty::Maybe(_) if matches!(expr.kind, ExprKind::Binary(BinOp::Div, ..)) => {
                let ExprKind::Binary(_, _, divisor) = &expr.kind else { unreachable!() };
                let divisor = self.source(divisor);
                format!("`{}` is `null` when `{divisor}` is 0, so `{symbol}` can't use it; guard it: `if {divisor} != 0 then … else …`.", self.source(expr))
            }
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

    fn member_of(&self, ty: &Ty) -> Option<String> {
        let Ty::Nix(inner) = ty else { return None };
        let Ty::Members(path) = inner.as_ref() else {
            return None;
        };
        Some(path.join("."))
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
        let lines = lex(&self.file.text).unwrap_or_default();
        for root in FACT_ROOTS {
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
                if slot.ty.starts_with("nix ") {
                    self.payload(arg, fact.line, &lines, &mut 0);
                    continue;
                }
                if slot.ty == "OptionPath" {
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
                        "`{text}` depends on Nix evaluation, so it can't fill the `{}` slot, which is `{}`. Galileo can't decide it; only a slot typed `nix {}` stores a Nix value.",
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

    fn payload(&mut self, arg: &Arg, line: usize, lines: &[Line], from: &mut usize) {
        let text = match arg {
            Arg::List(items) => {
                for item in items {
                    self.payload(item, line, lines, from);
                }
                return;
            }
            Arg::Name(text) | Arg::Expr(text) => text.clone(),
            Arg::Text(_) => arg.to_string(),
            _ => return,
        };
        let Some(row) = lines.iter().find(|row| row.no == line) else { return };
        let line_start = self.file.text.split_inclusive('\n').take(line - 1).map(str::len).sum::<usize>();
        let Some(at) = self.file.text[line_start + *from..].find(text.as_str()) else { return };
        let start = line_start + *from + at;
        let end = start + text.len();
        *from += at + text.len();
        let tokens: Vec<Token> = row.tokens.iter().filter(|t| t.span.start >= start && t.span.end <= end).cloned().collect();
        if let Ok(expr) = parse_expr(&tokens, row) {
            self.expr(&expr);
        }
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
        if let Ty::Declared(ty) = &base {
            let vocabulary = &self.file.sentences.vocabulary;
            let key = (vocabulary.canonical(ty), name.to_string());
            if let Some(ty) = vocabulary.fields.get(&key) {
                return slot_ty(ty);
            }
        }
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

    fn select(&mut self, base: &Expr, ty: Ty, span: &Span) -> Ty {
        if let Some(parent) = self.member_of(&ty).filter(|p| matches!(p.split('.').next(), Some("lib" | "builtins"))) {
            let message = format!(
                "`{parent}` members are found by name, so a quoted or computed key can't select from `{parent}`; write the member's name."
            );
            self.report(span.start, span.line, message);
        } else if !ty.is_nix() && *ty.value() != Ty::Any {
            let message = format!(
                "`{}` is known, so a quoted or computed key can't select from it; `lookup` reads a key and returns `maybe`.",
                self.source(base)
            );
            self.report(span.start, span.line, message);
        }
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
    let mut older = row.clone();
    for (i, ca) in a.iter().enumerate() {
        let previous = row.clone();
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let mut next = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(diagonal + usize::from(ca != cb));
            if i > 0 && j > 0 && *ca == b[j - 1] && a[i - 1] == *cb {
                next = next.min(older[j - 1] + 1);
            }
            diagonal = row[j + 1];
            row[j + 1] = next;
        }
        older = previous;
    }
    row[b.len()]
}

pub(crate) struct Owner {
    pub(crate) outside: BTreeSet<String>,
    pub(crate) types: BTreeMap<String, Ty>,
}

fn slot_ty(text: &str) -> Ty {
    match text.split_once(' ') {
        Some(("maybe", rest)) => Ty::Maybe(Box::new(slot_ty(rest))),
        Some(("nix", rest)) => Ty::nix(slot_ty(rest)),
        Some(("List", _)) => Ty::List(Box::new(slot_ty(crate::sentences::list_item(text).unwrap_or(text)))),
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
        L::Before(inner) => literal_vars(inner, out),
        L::Compare(_, a, b) => {
            expr_vars(a, out);
            expr_vars(b, out);
        }
        L::Holds(e) => expr_vars(e, out),
        L::Bind(v, e) | L::In(v, e) => {
            out.insert(v.clone());
            expr_vars(e, out);
        }
        L::Aggregate { var, of, body, .. } => {
            out.insert(var.clone());
            if let Some(of) = of {
                expr_vars(of, out);
            }
            body.iter().for_each(|l| literal_vars(l, out));
        }
        L::ArgMin { var, of, by, body, .. } => {
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

pub(crate) fn sub_exprs<'e>(expr: &'e Expr, each: &mut dyn FnMut(&'e Expr)) {
    match &expr.kind {
        ExprKind::Text(parts) | ExprKind::TextBlock(parts) => {
            for part in parts {
                if let TextPart::Interpolation(e) = part {
                    each(e);
                }
            }
        }
        ExprKind::List(items) => items.iter().for_each(|e| each(e)),
        ExprKind::Record(fields) => fields.iter().for_each(|f| {
            f.each.iter().for_each(|head| each(&head.source));
            each(&f.value)
        }),
        ExprKind::Select(e, segment) => {
            each(e);
            if let Segment::Computed(key) = segment.as_ref() {
                each(key);
            }
        }
        ExprKind::Field(e, _) | ExprKind::Not(e) | ExprKind::Neg(e) | ExprKind::Computed(e) => each(e),
        ExprKind::Lambda(params, body) => {
            for param in params {
                if let Param::Formals { fields, .. } = param {
                    for field in fields {
                        field.each.iter().for_each(|head| each(&head.source));
                        if let Some(name) = &field.computed {
                            each(name);
                        }
                        if let Some(default) = &field.default {
                            each(default);
                        }
                    }
                }
            }
            each(body);
        }
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
        ExprKind::Query(Query::All { head, sorted_by, .. }) => {
            if let Some(head) = head { each(head); }
            if let Some(key) = sorted_by { each(key); }
        }
        ExprKind::Query(Query::Aggregate { value: Some(value), .. }) => each(value),
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
        L::ArgMin { by, body, .. } => {
            out.push(by);
            body.iter().for_each(|l| literal_exprs(l, out));
        }
        L::Before(inner) => literal_exprs(inner, out),
        L::Sentence(_) | L::Not(_) => {}
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
            if tokens.len() == 4 && token_word(&tokens[2]) == "for" {
                let written = token_word(&tokens[3]);
                if let Ok(ty) = vocabulary.resolve_type(&block.line, written) {
                    let name = written.chars().enumerate().flat_map(|(at, c)| {
                        let prefix = if at > 0 && c.is_uppercase() { "-" } else { "" };
                        prefix.chars().chain(c.to_lowercase())
                    }).collect();
                    types.insert(name, slot_ty(&ty));
                }
            } else if tokens.len() >= 6 && token_word(&tokens[2]) == "for" {
                if let TokenKind::Variable(name) = &tokens[3].kind {
                    if token_word(&tokens[4]) == ":" {
                        let ty = tokens[5..].iter().map(token_word).collect::<Vec<_>>().join(" ");
                        types.insert(name.clone(), slot_ty(&ty));
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
            owners.push((Owner { outside: types.keys().cloned().collect(), types }, exprs));
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
    if let ExprKind::Field(base, name) = &expr.kind {
        if let ExprKind::Var(var) = &base.kind {
            if let Some(Ty::Declared(ty)) = owner.types.get(var) {
                let key = (vocabulary.canonical(ty), name.clone());
                if let Some(ty) = vocabulary.fields.get(&key) {
                    typed.push(Occurrence {
                        file: file.name.clone(), start: expr.span.start, end: expr.span.end,
                        ty: slot_ty(ty), option: None,
                    });
                } else if vocabulary.fields.keys().any(|(t, _)| *t == key.0) {
                    let mut message = format!("`{}` has no field `{name}`", vocabulary.written(ty));
                    if let Some(near) = closest(name, vocabulary.fields.keys().filter(|(t, _)| *t == key.0).map(|(_, f)| f.as_str())) {
                        message.push_str(&format!("; did you mean `{near}`?"));
                    }
                    diagnostics.push(diagnostic(file, expr.span.start, expr.span.line, message));
                }
            }
        }
    }
    if let ExprKind::Query(query @ (Query::All { .. } | Query::Aggregate { .. })) = &expr.kind {
        if let Ok(body) = vocabulary.read_query(line, query) {
            let mut inner = Owner { outside: owner.outside.clone(), types: owner.types.clone() };
            for literal in &body {
                if let crate::Literal::Sentence(r) = literal {
                    reading_types(vocabulary, r, &mut inner.types);
                }
            }
            sub_exprs(expr, &mut |e| visit_the(file, vocabulary, &inner, line, e, typed, diagnostics));
            return;
        }
    }
    if let ExprKind::Query(Query::Aggregate { op, .. }) = &expr.kind {
        let ty = if matches!(op, crate::Aggregate::Min | crate::Aggregate::Max) { Ty::Maybe(Box::new(Ty::Any)) } else { Ty::Int };
        typed.push(Occurrence { file: file.name.clone(), start: expr.span.start, end: expr.span.end, ty, option: None });
    }
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

pub(crate) fn the_type(vocabulary: &crate::Vocabulary, owner: &Owner, var: &str, body: &[crate::Literal]) -> Option<Ty> {
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
        if let L::Sentence(_) | L::Not(_) | L::Bind(..) | L::In(..) | L::Compare(..) | L::Holds(_) = literal {
            literal_vars(literal, &mut top);
        } else if let L::Aggregate { var, .. } | L::ArgMin { var, .. } = literal {
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
                L::Aggregate { var: v, .. } | L::ArgMin { var: v, .. } if !exact.contains_key(v) => {
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
            L::ArgMin { direction, by, body, .. } => (
                Some(by),
                match direction {
                    galileo_core::ArgMinDirection::Min => "min … by",
                    galileo_core::ArgMinDirection::Max => "max … by",
                },
                body,
            ),
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

pub(crate) struct NixScope {
    frames: Vec<Frame>,
}

impl NixScope {
    pub(crate) fn new() -> Self {
        let prelude = PRELUDE.iter().map(|name| (name.to_string(), Ty::Any)).collect();
        let mut scope = NixScope { frames: vec![Frame { names: prelude, lines: BTreeMap::new(), open: false }] };
        scope.push();
        for name in ["builtins", "import", "toString"] {
            scope.frames.last_mut().expect("a frame").names.insert(name.into(), Ty::Any);
        }
        scope
    }

    pub(crate) fn push(&mut self) {
        self.frames.push(Frame { names: BTreeMap::new(), lines: BTreeMap::new(), open: false });
    }

    pub(crate) fn pop(&mut self) {
        self.frames.pop();
    }

    pub(crate) fn bind(&mut self, name: &str, line: usize) {
        let frame = self.frames.last_mut().expect("a frame");
        frame.names.insert(name.into(), Ty::Any);
        frame.lines.insert(name.into(), line);
    }

    pub(crate) fn formal(&self, name: &str, others: &[String]) -> Result<(), String> {
        nix_identifier(name)?;
        if others.iter().any(|other| other == name) {
            return Err(format!("the computed formal `{name}` repeats a formal its lambda already has"));
        }
        if ["true", "false", "null"].contains(&name) {
            return Err(format!("the computed formal `{name}` would shadow Nix's `{name}`, which Galileo's literal `{name}` emits"));
        }
        match self.frames.iter().enumerate().rev().find(|(_, frame)| frame.names.contains_key(name)) {
            None => Ok(()),
            Some((0, _)) => Err(format!("the computed formal `{name}` would shadow the prelude function `{name}`")),
            Some((_, frame)) => match frame.lines.get(name) {
                Some(line) => Err(format!("the computed formal `{name}` would shadow `{name}`, {}", bound_at(*line))),
                None => Err(format!("the computed formal `{name}` would shadow `{name}`, which the Nix target binds")),
            },
        }
    }

    pub(crate) fn reference(&self, name: &str) -> Result<(), String> {
        nix_identifier(name)?;
        let binders: Vec<&Frame> = self.frames.iter().skip(1).rev().filter(|frame| frame.names.contains_key(name)).collect();
        match binders.as_slice() {
            [] => {
                let mut message = format!("the computed reference `{name}` reads no Nix binding");
                let candidates = self.frames.iter().skip(1).flat_map(|frame| frame.names.keys().map(String::as_str));
                if let Some(near) = closest(name, candidates) {
                    message.push_str(&format!("; did you mean `{near}`?"));
                }
                Err(message)
            }
            [_] => Ok(()),
            [nearer, ..] => Err(match nearer.lines.get(name) {
                Some(line) => format!("the computed reference `{name}` would be captured by the nearer `{name}`, {}", bound_at(*line)),
                None => format!("the computed reference `{name}` would be captured by the nearer `{name}`, which the Nix target binds"),
            }),
        }
    }
}

fn bound_at(line: usize) -> String {
    if line == 0 { "bound by `arguments:`".into() } else { format!("bound at line {line}") }
}

fn nix_identifier(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let starts = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if !starts || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '\'' || c == '-') {
        return Err(format!("the computed name `{name}` isn't a Nix identifier, which matches `[A-Za-z_][A-Za-z0-9_'-]*`"));
    }
    if galileo_nix::KEYWORDS.contains(&name) {
        return Err(format!("the computed name `{name}` is a Nix keyword, so it can't name a Nix binding or reference"));
    }
    Ok(())
}

pub(crate) fn nonzero_tested(cond: &Expr, guarded: bool) -> Vec<&Expr> {
    let zero = |side: &Expr| matches!(side.kind, ExprKind::Int(0));
    let mut out = Vec::new();
    let mut pending = vec![cond];
    while let Some(expr) = pending.pop() {
        let ExprKind::Binary(op, lhs, rhs) = &expr.kind else { continue };
        let compares = if guarded { matches!(op, BinOp::Ne | BinOp::Gt | BinOp::Lt) } else { *op == BinOp::Eq };
        if guarded && *op == BinOp::And {
            pending.extend([lhs.as_ref(), rhs.as_ref()]);
        } else if compares && zero(lhs) != zero(rhs) {
            out.push(if zero(lhs) { rhs.as_ref() } else { lhs.as_ref() });
        }
    }
    out
}

pub(crate) fn divisor_nonzero<K: PartialEq>(divisor: &Expr, proven: &[K], key: impl Fn(&Expr) -> Option<K>) -> bool {
    match divisor.kind {
        ExprKind::Int(value) => value != 0,
        _ => key(divisor).is_some_and(|key| proven.contains(&key)),
    }
}

pub(crate) fn divisor_key(expr: &Expr) -> Option<String> {
    match &expr.kind {
        ExprKind::Name(name) => Some(name.clone()),
        ExprKind::Var(var) => Some(format!("?{var}")),
        ExprKind::Field(base, name) => Some(format!("{}.{name}", divisor_key(base)?)),
        _ => None,
    }
}

pub(crate) fn div_stage_message(operand: &str) -> String {
    format!("`{operand}` depends on Nix evaluation, so Galileo cannot compute `/`; use `builtins.div` in the Nix body.")
}

pub(crate) fn in_stage_message(operand: &str) -> String {
    format!("`{operand}` depends on Nix evaluation, so Galileo cannot compute `in`; use `builtins.elem` in the Nix body.")
}
