use std::fmt;

use crate::expr::{is_symbol, parse_attrpath, parse_expr, parse_line_expr, word, KEYWORDS};
pub use crate::expr::{BinOp, Expr, ExprKind, Field, Param, Query, TextPart};
use crate::types::{parse_type, Type};
use crate::{blocks, Block, Error, Token, TokenKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Segment {
    Name(String),
    Text(String),
    Computed(Expr),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttrPath {
    pub segments: Vec<Segment>,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arguments {
    pub names: Vec<String>,
    pub open: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Let {
    pub name: String,
    pub value: Expr,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionField {
    pub name: String,
    pub value: Expr,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionDecl {
    pub path: AttrPath,
    pub kind: OptionKind,
    pub fields: Vec<OptionField>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OptionKind {
    Type(Type),
    Enable(Expr),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForHead {
    pub vars: Vec<String>,
    pub source: Expr,
    pub query: bool,
    pub offset: usize,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForBlock {
    pub head: ForHead,
    pub entries: Vec<Entry>,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Value {
        path: AttrPath,
        value: Expr,
        when: Option<Expr>,
    },
    Nested {
        path: AttrPath,
        when: Option<Expr>,
        entries: Vec<Entry>,
    },
    For(ForBlock),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleItem {
    Let(Let),
    Option(OptionDecl),
    OptionGroup {
        path: AttrPath,
        options: Vec<OptionDecl>,
    },
    Imports(Expr),
    Config {
        when: Option<Expr>,
        entries: Vec<Entry>,
        line: usize,
    },
    For(ForBlock),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NixModule {
    pub arguments: Arguments,
    pub items: Vec<ModuleItem>,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileValue {
    Expr(Expr),
    Entries(Vec<Entry>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NixFile {
    pub arguments: Option<Arguments>,
    pub lets: Vec<Let>,
    pub value: FileValue,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordDecl {
    Module(NixModule),
    File(NixFile),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileError {
    pub file: String,
    pub error: Error,
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.file, self.error.line, self.error.column, self.error.message
        )?;
        if let Some(text) = &self.error.text {
            write!(f, "\n    {text}")?;
        }
        Ok(())
    }
}

impl std::error::Error for FileError {}

const ENTRY: &str = "expected an entry: `name: expr [when expr]`, `name [when expr]` with indented entries, or `for ?x in expr` with indented entries";
const MODULE_ITEM: &str = "expected a module item: `let name = expr`, `option path: type`, `imports: expr`, `config [when expr]` with indented entries, or a `for` block";

pub fn parse_record_file(file: &str, source: &str) -> Result<Vec<RecordDecl>, FileError> {
    let located = |error| FileError {
        file: file.into(),
        error,
    };
    let mut out = Vec::new();
    for block in blocks(source).map_err(located)? {
        match record_decl(&block) {
            Some(decl) => out.push(decl.map_err(located)?),
            None => {
                return Err(located(Error::at(
                    &block.line,
                    "expected `nix module` or `nix file`",
                )))
            }
        }
    }
    Ok(out)
}

pub fn record_decl(block: &Block) -> Option<Result<RecordDecl, Error>> {
    let words: Vec<&str> = block.line.tokens.iter().map(word).collect();
    match words.as_slice() {
        ["nix", "module"] => Some(nix_module(block).map(RecordDecl::Module)),
        ["nix", "file"] => Some(nix_file(block).map(RecordDecl::File)),
        _ => None,
    }
}

fn nix_module(block: &Block) -> Result<NixModule, Error> {
    let first = block.children.first().ok_or_else(|| {
        Error::at(
            &block.line,
            "`nix module` needs an indented `arguments:` line",
        )
    })?;
    let arguments = arguments(first).ok_or_else(|| {
        Error::at(
            &first.line,
            "expected `arguments: name, name, ...` first in a `nix module`",
        )
    })??;
    let mut items = Vec::new();
    for child in &block.children[1..] {
        items.push(module_item(child)?);
    }
    Ok(NixModule {
        arguments,
        items,
        line: block.line.no,
    })
}

fn nix_file(block: &Block) -> Result<NixFile, Error> {
    let mut children = block.children.iter().peekable();
    let arguments = match children.peek().and_then(|c| arguments(c)) {
        Some(args) => {
            children.next();
            Some(args?)
        }
        None => None,
    };
    let mut lets = Vec::new();
    while let Some(child) = children.peek() {
        if word(&child.line.tokens[0]) != "let" {
            break;
        }
        lets.push(let_binding(child)?);
        children.next();
    }
    let child = children.next().ok_or_else(|| {
        Error::at(
            &block.line,
            "`nix file` needs a `value: expr` or `value` block",
        )
    })?;
    let tokens = &child.line.tokens;
    let value = match tokens.iter().map(word).collect::<Vec<_>>().as_slice() {
        ["value"] if !child.children.is_empty() => FileValue::Entries(entries(&child.children)?),
        ["value", ":", rest @ ..] if !rest.is_empty() => {
            FileValue::Expr(line_expr(child, &tokens[2..], "`value: expr`")?)
        }
        _ => {
            return Err(Error::at(
                &child.line,
                "expected `value: expr` or `value` with indented entries",
            ))
        }
    };
    if let Some(extra) = children.next() {
        return Err(Error::at(
            &extra.line,
            "nothing may follow a `nix file`'s value",
        ));
    }
    Ok(NixFile {
        arguments,
        lets,
        value,
        line: block.line.no,
    })
}

fn arguments(block: &Block) -> Option<Result<Arguments, Error>> {
    let tokens = &block.line.tokens;
    if tokens.len() < 2 || word(&tokens[0]) != "arguments" || word(&tokens[1]) != ":" {
        return None;
    }
    let expected = "expected `arguments: name, name, ...`";
    Some((|| {
        no_children(block, "`arguments:`")?;
        argument_list(&tokens[2..]).ok_or_else(|| Error::at(&block.line, expected))
    })())
}

pub(crate) fn argument_list(tokens: &[Token]) -> Option<Arguments> {
    let mut names = Vec::new();
    let mut open = false;
    for (index, part) in tokens.split(|t| is_symbol(t, ",")).enumerate() {
        match part {
            [t] if is_symbol(t, "...") && !open && index > 0 => open = true,
            [Token {
                kind: TokenKind::Word(name),
                ..
            }] if !open => names.push(name.clone()),
            _ => return None,
        }
    }
    if names.is_empty() {
        return None;
    }
    Some(Arguments { names, open })
}

fn module_item(block: &Block) -> Result<ModuleItem, Error> {
    let tokens = &block.line.tokens;
    match word(&tokens[0]) {
        "let" => Ok(ModuleItem::Let(let_binding(block)?)),
        "for" => Ok(ModuleItem::For(for_block(block)?)),
        "imports" if tokens.len() > 2 && is_symbol(&tokens[1], ":") => Ok(ModuleItem::Imports(
            line_expr(block, &tokens[2..], "`imports: expr`")?,
        )),
        "config" if tokens.len() == 1 || word(&tokens[1]) == "when" => {
            let when = match &tokens[1..] {
                [] => None,
                [_, rest @ ..] if !rest.is_empty() => Some(parse_expr(rest, &block.line)?),
                _ => return Err(Error::at(&block.line, "expected `config when expr`")),
            };
            if block.children.is_empty() {
                return Err(Error::at(&block.line, "`config` needs indented entries"));
            }
            Ok(ModuleItem::Config {
                when,
                entries: entries(&block.children)?,
                line: block.line.no,
            })
        }
        "option" => option(block),
        _ => Err(Error::at(&block.line, MODULE_ITEM)),
    }
}

fn let_binding(block: &Block) -> Result<Let, Error> {
    match &block.line.tokens[..] {
        [_, Token {
            kind: TokenKind::Word(name),
            ..
        }, eq, rest @ ..]
            if is_symbol(eq, "=") && !rest.is_empty() =>
        {
            Ok(Let {
                name: name.clone(),
                value: line_expr(block, rest, "`let name = expr`")?,
                line: block.line.no,
            })
        }
        [_, Token { kind: TokenKind::Computed(raw), .. }, ..] => Err(Error::at(&block.line, crate::expr::misplaced_computed(raw))),
        _ => Err(Error::at(&block.line, "expected `let name = expr`")),
    }
}

fn option(block: &Block) -> Result<ModuleItem, Error> {
    let expected = "expected `option path: type`";
    let tokens = &block.line.tokens[1..];
    let (path, rest) =
        parse_attrpath(tokens, &block.line)?.ok_or_else(|| Error::at(&block.line, expected))?;
    if !rest.is_empty() {
        return Ok(ModuleItem::Option(option_decl(block, path, rest)?));
    }
    if block.children.is_empty() {
        return Err(Error::at(
            &block.line,
            "`option path` with no `:` opens a group and needs indented declarations `name: type`",
        ));
    }
    let mut options = Vec::new();
    for child in &block.children {
        let malformed = || Error::at(&child.line, "a line in an option group is a declaration `name: type` or `name: enable expr`");
        let (child_path, rest) = parse_attrpath(&child.line.tokens, &child.line)?.ok_or_else(malformed)?;
        if rest.is_empty() {
            return Err(malformed());
        }
        options.push(option_decl(child, child_path, rest)?);
    }
    Ok(ModuleItem::OptionGroup { path, options })
}

fn option_decl(block: &Block, path: AttrPath, rest: &[Token]) -> Result<OptionDecl, Error> {
    if rest.len() < 2 || !is_symbol(&rest[0], ":") {
        return Err(Error::at(&block.line, "expected `option path: type`"));
    }
    let kind = if word(&rest[1]) == "enable" {
        OptionKind::Enable(parse_expr(&rest[2..], &block.line)?)
    } else {
        OptionKind::Type(parse_type(&rest[1..], &block.line)?)
    };
    let mut fields = Vec::new();
    for child in &block.children {
        let field_tokens = &child.line.tokens;
        no_children(child, "`field: expr`")?;
        match &field_tokens[..] {
            [Token {
                kind: TokenKind::Word(name),
                ..
            }, colon, value @ ..]
                if is_symbol(colon, ":") && !value.is_empty() =>
            {
                fields.push(OptionField {
                    name: name.clone(),
                    value: parse_expr(value, &child.line)?,
                    line: child.line.no,
                })
            }
            _ => {
                return Err(Error::at(
                    &child.line,
                    "expected an option field `name: expr`",
                ))
            }
        }
    }
    Ok(OptionDecl {
        path,
        kind,
        fields,
    })
}

/// Every option an item declares, each with its full path at its own line.
pub fn item_declarations(item: &ModuleItem) -> Vec<OptionDecl> {
    match item {
        ModuleItem::Option(option) => vec![option.clone()],
        ModuleItem::OptionGroup { path, options } => options
            .iter()
            .map(|option| OptionDecl {
                path: AttrPath {
                    segments: path.segments.iter().chain(&option.path.segments).cloned().collect(),
                    line: option.path.line,
                },
                ..option.clone()
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub fn declarations(items: &[ModuleItem]) -> Vec<OptionDecl> {
    items.iter().flat_map(item_declarations).collect()
}

fn for_block(block: &Block) -> Result<ForBlock, Error> {
    let head = for_head(&block.line.tokens, &block.line)?;
    if block.children.is_empty() {
        return Err(Error::at(&block.line, "expected `for ?x in expr` or `for ?x, ?y where …` with indented entries"));
    }
    Ok(ForBlock {
        head,
        entries: entries(&block.children)?,
        line: block.line.no,
    })
}

pub(crate) fn for_head(tokens: &[Token], line: &crate::Line) -> Result<ForHead, Error> {
    let (vars, source, query) = for_header(tokens, line)?;
    Ok(ForHead { vars, source, query, offset: tokens[0].span.start, line: tokens[0].span.line })
}

/// Reads `for ?x in e` or `for ?a, ?b where C [, sorted by k]`; the second yields the query `all ?a, ?b where C`.
pub(crate) fn for_header(tokens: &[Token], line: &crate::Line) -> Result<(Vec<String>, Expr, bool), Error> {
    let expected = "expected `for ?x in expr` or `for ?x, ?y where …` with indented entries";
    let mut vars = Vec::new();
    let mut at = 1;
    loop {
        match tokens.get(at).map(|t| &t.kind) {
            Some(TokenKind::Variable(name)) if !name.is_empty() => vars.push(name.clone()),
            _ => return Err(Error::at(line, expected)),
        }
        at += 1;
        if !tokens.get(at).is_some_and(|t| is_symbol(t, ",")) {
            break;
        }
        at += 1;
    }
    let listed = vars.iter().map(|v| format!("?{v}")).collect::<Vec<_>>().join(", ");
    match tokens.get(at).map(word) {
        Some("where") => {
            let mut query = tokens.to_vec();
            query[0].kind = TokenKind::Word("all".into());
            Ok((vars, parse_expr(&query, line)?, true))
        }
        Some("in") if at + 1 < tokens.len() => {
            let source = parse_expr(&tokens[at + 1..], line)?;
            if let ExprKind::Query(Query::All { vars: names, head: None, .. }) = &source.kind {
                let names = names.iter().map(|v| format!("?{v}")).collect::<Vec<_>>().join(", ");
                return Err(Error::at(
                    line,
                    &format!("a `for` binds its query's variables in place: write `for {names} where …`, not `for {listed} in all {names} where …`"),
                ));
            }
            if vars.len() > 1 {
                return Err(Error::at(
                    line,
                    &format!("a `for` over a list binds one variable: write `for ?{} in …` and read its fields as `?{}.f`", vars[0], vars[0]),
                ));
            }
            Ok((vars, source, false))
        }
        _ => Err(Error::at(line, expected)),
    }
}

pub(crate) fn entries(children: &[Block]) -> Result<Vec<Entry>, Error> {
    children.iter().map(entry).collect()
}

fn entry(block: &Block) -> Result<Entry, Error> {
    let tokens = &block.line.tokens;
    if word(&tokens[0]) == "for" {
        return Ok(Entry::For(for_block(block)?));
    }
    let malformed = || Error::at(&block.line, ENTRY);
    let (path, rest) = parse_attrpath(tokens, &block.line)?.ok_or_else(malformed)?;
    match rest {
        [colon, value @ ..] if is_symbol(colon, ":") => {
            let (value, when) = when_suffix(value).ok_or_else(malformed)?;
            let (value, used) = parse_line_expr(value, block)?;
            if !used && !block.children.is_empty() {
                return Err(Error::at(
                    &block.children[0].line,
                    "an entry with a value takes no indented entries; drop the `:` to nest",
                ));
            }
            let when = when.map(|c| parse_expr(c, &block.line)).transpose()?;
            Ok(Entry::Value { path, value, when })
        }
        _ if block.children.is_empty() => Err(malformed()),
        [] => Ok(Entry::Nested {
            path,
            when: None,
            entries: entries(&block.children)?,
        }),
        [when, cond @ ..] if word(when) == "when" && !cond.is_empty() => Ok(Entry::Nested {
            path,
            when: Some(parse_expr(cond, &block.line)?),
            entries: entries(&block.children)?,
        }),
        _ => Err(malformed()),
    }
}

fn when_suffix(tokens: &[Token]) -> Option<(&[Token], Option<&[Token]>)> {
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate() {
        match word(token) {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "when" if depth == 0 => {
                let cond = &tokens[index + 1..];
                if cond.is_empty() || index == 0 {
                    return None;
                }
                return Some((&tokens[..index], Some(cond)));
            }
            _ => {}
        }
    }
    if tokens.is_empty() {
        return None;
    }
    Some((tokens, None))
}

fn line_expr(block: &Block, tokens: &[Token], form: &str) -> Result<Expr, Error> {
    let (expr, used) = parse_line_expr(tokens, block)?;
    if !used {
        no_children(block, form)?;
    }
    Ok(expr)
}

fn no_children(block: &Block, form: &str) -> Result<(), Error> {
    match block.children.first() {
        Some(child) => Err(Error::at(
            &child.line,
            format!("{form} takes no indented lines"),
        )),
        None => Ok(()),
    }
}

fn print_option(out: &mut String, option: &OptionDecl, lead: &str, depth: usize) {
    let kind = match &option.kind {
        OptionKind::Type(ty) => print_type(ty),
        OptionKind::Enable(value) => format!("enable {}", print_expr(value, depth, 0)),
    };
    print_line(out, depth, format!("{lead}{}: {kind}", print_path(&option.path)));
    for field in &option.fields {
        print_line(
            out,
            depth + 1,
            format!("{}: {}", field.name, print_expr(&field.value, depth + 1, 0)),
        );
    }
}

pub fn print(decl: &RecordDecl) -> String {
    let mut out = String::new();
    match decl {
        RecordDecl::Module(module) => {
            out.push_str("nix module\n");
            print_line(
                &mut out,
                1,
                format!("arguments: {}", print_arguments(&module.arguments)),
            );
            for item in &module.items {
                match item {
                    ModuleItem::Let(binding) => print_let(&mut out, binding),
                    ModuleItem::Option(option) => print_option(&mut out, option, "option ", 1),
                    ModuleItem::OptionGroup { path, options } => {
                        print_line(&mut out, 1, format!("option {}", print_path(path)));
                        for option in options {
                            print_option(&mut out, option, "", 2);
                        }
                    }
                    ModuleItem::Imports(value) => {
                        print_line(&mut out, 1, format!("imports: {}", print_expr(value, 1, 0)))
                    }
                    ModuleItem::Config { when, entries, .. } => {
                        print_line(&mut out, 1, format!("config{}", print_when(when, 1)));
                        print_entries(&mut out, entries, 2);
                    }
                    ModuleItem::For(block) => print_for(&mut out, block, 1),
                }
            }
        }
        RecordDecl::File(file) => {
            out.push_str("nix file\n");
            if let Some(arguments) = &file.arguments {
                print_line(
                    &mut out,
                    1,
                    format!("arguments: {}", print_arguments(arguments)),
                );
            }
            for binding in &file.lets {
                print_let(&mut out, binding);
            }
            match &file.value {
                FileValue::Expr(value) => {
                    print_line(&mut out, 1, format!("value: {}", print_expr(value, 1, 0)))
                }
                FileValue::Entries(entries) => {
                    print_line(&mut out, 1, "value".into());
                    print_entries(&mut out, entries, 2);
                }
            }
        }
    }
    out
}

fn print_line(out: &mut String, depth: usize, text: String) {
    out.push_str(&"  ".repeat(depth));
    out.push_str(&text);
    out.push('\n');
}

fn print_arguments(arguments: &Arguments) -> String {
    let mut names = arguments.names.join(", ");
    if arguments.open {
        names.push_str(", ...");
    }
    names
}

fn print_let(out: &mut String, binding: &Let) {
    print_line(
        out,
        1,
        format!(
            "let {} = {}",
            binding.name,
            print_expr(&binding.value, 1, 0)
        ),
    );
}

fn print_entries(out: &mut String, entries: &[Entry], depth: usize) {
    for entry in entries {
        match entry {
            Entry::Value { path, value, when } => {
                let mut value = print_expr(value, depth, 0);
                value.push_str(&print_when(when, depth));
                print_line(out, depth, format!("{}: {value}", print_path(path)));
            }
            Entry::Nested {
                path,
                when,
                entries,
            } => {
                print_line(
                    out,
                    depth,
                    format!("{}{}", print_path(path), print_when(when, depth)),
                );
                print_entries(out, entries, depth + 1);
            }
            Entry::For(block) => print_for(out, block, depth),
        }
    }
}

fn print_for(out: &mut String, block: &ForBlock, depth: usize) {
    print_line(out, depth, print_head(&block.head, depth));
    print_entries(out, &block.entries, depth + 1);
}

fn print_head(head: &ForHead, depth: usize) -> String {
    let vars = head
        .vars
        .iter()
        .map(|var| format!("?{var}"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = print_expr(&head.source, depth, 0);
    match source.strip_prefix("all ") {
        Some(query) if head.query => format!("for {query}"),
        _ => format!("for {vars} in {source}"),
    }
}

fn print_each(each: &[ForHead], depth: usize) -> String {
    each.iter().map(|head| format!("{}: ", print_head(head, depth))).collect()
}

fn print_when(when: &Option<Expr>, depth: usize) -> String {
    when.as_ref().map_or_else(String::new, |expr| {
        format!(" when {}", print_expr(expr, depth, 0))
    })
}

fn print_path(path: &AttrPath) -> String {
    path.segments
        .iter()
        .map(print_segment)
        .collect::<Vec<_>>()
        .join(".")
}

fn print_segment(segment: &Segment) -> String {
    match segment {
        Segment::Name(name) => print_name(name),
        Segment::Text(text) => print_text(&[TextPart::Text(text.clone())], 0, false),
        Segment::Computed(expr) if matches!(expr.kind, ExprKind::Text(_)) => print_expr(expr, 0, 0),
        Segment::Computed(expr) => format!("{{{}}}", print_expr(expr, 0, 0)),
    }
}

fn print_name(name: &str) -> String {
    let bare = crate::lex(name).is_ok_and(|lines| matches!(lines.as_slice(), [line] if matches!(line.tokens.as_slice(), [Token { kind: TokenKind::Word(word), .. }] if word == name)));
    if bare && !KEYWORDS.contains(&name) && !matches!(name, "for" | "nix") {
        name.into()
    } else {
        format!("`{name}`")
    }
}

fn print_type(ty: &Type) -> String {
    match ty {
        Type::Maybe(inner) => format!("maybe {}", print_type(inner)),
        Type::Nix(inner) => format!("nix {}", print_type(inner)),
        Type::Named { name, args, .. } => {
            let mut out = name.clone();
            for arg in args {
                out.push(' ');
                match arg {
                    Type::Named { args, .. } if args.is_empty() => out.push_str(&print_type(arg)),
                    _ => out.push_str(&format!("({})", print_type(arg))),
                }
            }
            out
        }
    }
}

fn print_text(parts: &[TextPart], depth: usize, block: bool) -> String {
    let mut out = String::new();
    for part in parts {
        match part {
            TextPart::Text(text) if block => out.push_str(text),
            TextPart::Text(text) => {
                for c in text.chars() {
                    match c {
                        '\\' => out.push_str("\\\\"),
                        '"' => out.push_str("\\\""),
                        '{' => out.push_str("\\{"),
                        '\n' => out.push_str("\\n"),
                        '\t' => out.push_str("\\t"),
                        _ => out.push(c),
                    }
                }
            }
            TextPart::Interpolation(expr) => {
                out.push_str(if block { "{{" } else { "{" });
                out.push_str(&print_expr(expr, depth, 0));
                out.push_str(if block { "}}" } else { "}" });
            }
        }
    }
    if block {
        let mut text = String::from("'''\n");
        for line in out.split_terminator('\n') {
            if !line.is_empty() {
                text.push_str(&"  ".repeat(depth + 1));
                text.push_str(line);
            }
            text.push('\n');
        }
        text.push_str(&"  ".repeat(depth + 1));
        text.push_str("'''");
        text
    } else {
        format!("\"{out}\"")
    }
}

pub(crate) fn print_expr(expr: &Expr, depth: usize, min: u8) -> String {
    let (text, precedence) = match &expr.kind {
        ExprKind::Null => ("null".into(), 10),
        ExprKind::Bool(value) => (value.to_string(), 10),
        ExprKind::Int(value) => (value.to_string(), if *value < 0 { 8 } else { 10 }),
        ExprKind::Text(parts) => (print_text(parts, depth, false), 10),
        ExprKind::TextBlock(parts) => (print_text(parts, depth, true), 10),
        ExprKind::Path(path) => (path.clone(), 10),
        ExprKind::Name(name) => (print_name(name), 10),
        ExprKind::Var(var) => (format!("?{var}"), 10),
        ExprKind::Computed(name) => (format!("`{{{}}}`", print_expr(name, depth, 0)), 10),
        ExprKind::List(values) => (
            format!(
                "[{}]",
                values
                    .iter()
                    .map(|value| print_expr(value, depth, 0))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            10,
        ),
        ExprKind::Record(fields) => (
            format!(
                "{{{}}}",
                fields
                    .iter()
                    .map(|field| format!(
                        "{}{}: {}",
                        print_each(&field.each, depth),
                        print_path(&field.path),
                        print_expr(&field.value, depth, 0)
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            10,
        ),
        ExprKind::Field(value, name) if value.grouped() => (
            format!("({}).{}", print_expr(value, depth, 0), print_name(name)),
            9,
        ),
        ExprKind::Field(value, name) => (
            format!("{}.{}", print_expr(value, depth, 9), print_name(name)),
            9,
        ),
        ExprKind::Select(value, segment) if value.grouped() => (
            format!("({}).{}", print_expr(value, depth, 0), print_segment(segment)),
            9,
        ),
        ExprKind::Select(value, segment) => (
            format!("{}.{}", print_expr(value, depth, 9), print_segment(segment)),
            9,
        ),
        ExprKind::Call(function, args) => (
            format!(
                "{}({})",
                print_expr(function, depth, 9),
                args.iter()
                    .map(|arg| print_expr(arg, depth, 0))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            9,
        ),
        ExprKind::Lambda(params, body) => {
            let params = params
                .iter()
                .map(|param| match param {
                    Param::Name { name, ty } => format!(
                        "{}{}",
                        print_name(name),
                        ty.as_ref()
                            .map_or_else(String::new, |ty| format!(": {}", print_type(ty)))
                    ),
                    Param::Formals { fields, open } => {
                        let mut fields = fields
                            .iter()
                            .map(|field| {
                                format!(
                                    "{}{}{}",
                                    print_each(&field.each, depth),
                                    field.computed.as_ref().map_or_else(|| print_name(&field.name), |name| format!("`{{{}}}`", print_expr(name, depth, 0))),
                                    field.default.as_ref().map_or_else(
                                        String::new,
                                        |value| format!(" = {}", print_expr(value, depth, 0))
                                    )
                                )
                            })
                            .collect::<Vec<_>>();
                        if *open {
                            fields.push("...".into());
                        }
                        format!("{{{}}}", fields.join(", "))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            (format!("({params}) => {}", print_expr(body, depth, 0)), 0)
        }
        ExprKind::If(cond, yes, no) => (
            format!(
                "if {} then {} else {}",
                print_expr(cond, depth, 0),
                print_expr(yes, depth, 0),
                print_expr(no, depth, 0)
            ),
            0,
        ),
        ExprKind::With(scope, body) => (
            format!(
                "with {}: {}",
                print_expr(scope, depth, 0),
                print_expr(body, depth, 0)
            ),
            0,
        ),
        ExprKind::Binary(op, lhs, rhs) => {
            let (symbol, precedence, right) = match op {
                BinOp::Or => ("or", 1, false),
                BinOp::And => ("and", 2, false),
                BinOp::Eq => ("==", 4, false),
                BinOp::Ne => ("!=", 4, false),
                BinOp::Lt => ("<", 4, false),
                BinOp::Le => ("<=", 4, false),
                BinOp::Gt => (">", 4, false),
                BinOp::Ge => (">=", 4, false),
                BinOp::In => ("in", 4, false),
                BinOp::Update => ("//", 5, true),
                BinOp::Add => ("+", 6, false),
                BinOp::Sub => ("-", 6, false),
                BinOp::Mul => ("*", 7, false),
                BinOp::Concat => ("++", 8, true),
            };
            let left_min = precedence + u8::from(right || precedence == 4);
            let right_min = precedence + u8::from(!right);
            (
                format!(
                    "{} {symbol} {}",
                    print_expr(lhs, depth, left_min),
                    print_expr(rhs, depth, right_min)
                ),
                precedence,
            )
        }
        ExprKind::Not(value) => (format!("not {}", print_expr(value, depth, 3)), 3),
        ExprKind::Neg(value) => (format!("-{}", print_expr(value, depth, 9)), 8),
        ExprKind::Query(query) => {
            let (head, vars, conj, sorted) = match query {
                Query::All {
                    vars,
                    conj,
                    sorted_by,
                    ..
                } => ("all", vars.as_slice(), conj, sorted_by.as_deref()),
                Query::The { var, conj } => ("the", std::slice::from_ref(var), conj, None),
            };
            let vars = vars
                .iter()
                .map(|var| format!("?{var}"))
                .collect::<Vec<_>>()
                .join(", ");
            let vars = match query {
                Query::All { head: Some(value), .. } => print_expr(value, depth, 0),
                _ => vars,
            };
            let conj = conj
                .iter()
                .map(|token| match &token.kind {
                    TokenKind::Word(word)
                    | TokenKind::Integer(word)
                    | TokenKind::Path(word)
                    | TokenKind::Symbol(word) => word.clone(),
                    TokenKind::Variable(var) => format!("?{var}"),
                    TokenKind::QuotedName(name) => format!("`{name}`"),
                    TokenKind::Computed(e) => format!("`{{{e}}}`"),
                    TokenKind::Text(raw) => format!("\"{raw}\""),
                    TokenKind::TextBlock(raw) => {
                        print_text(&[TextPart::Text(raw.clone())], depth, true)
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            (
                format!(
                    "{head} {vars} where {conj}{}",
                    sorted.map_or_else(String::new, |value| format!(
                        ", sorted by {}",
                        print_expr(value, depth, 0)
                    ))
                ),
                0,
            )
        }
        ExprKind::NixModule(arguments, entries) => {
            let mut out = format!("nix module({})\n", print_arguments(arguments));
            print_entries(&mut out, entries, depth + 1);
            out.pop();
            (out, 10)
        }
    };
    if precedence < min {
        format!("({text})")
    } else {
        text
    }
}
