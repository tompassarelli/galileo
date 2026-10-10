use std::fmt;

use crate::{blocks, Block, Error, Line, Token, TokenKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub tokens: Vec<Token>,
}

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
    pub ty: Expr,
    pub fields: Vec<OptionField>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForBlock {
    pub vars: Vec<String>,
    pub source: Expr,
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
            no_children(child, "`value: expr`")?;
            FileValue::Expr(expr(&tokens[2..]))
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
        let mut names = Vec::new();
        let mut open = false;
        for (index, part) in tokens[2..].split(|t| is_symbol(t, ",")).enumerate() {
            match part {
                [t] if is_symbol(t, "...") && !open && index > 0 => open = true,
                [Token {
                    kind: TokenKind::Word(name),
                    ..
                }] if !open => names.push(name.clone()),
                _ => return Err(Error::at(&block.line, expected)),
            }
        }
        if names.is_empty() {
            return Err(Error::at(&block.line, expected));
        }
        Ok(Arguments { names, open })
    })())
}

fn module_item(block: &Block) -> Result<ModuleItem, Error> {
    let tokens = &block.line.tokens;
    match word(&tokens[0]) {
        "let" => Ok(ModuleItem::Let(let_binding(block)?)),
        "for" => Ok(ModuleItem::For(for_block(block)?)),
        "imports" if tokens.len() > 2 && is_symbol(&tokens[1], ":") => {
            no_children(block, "`imports: expr`")?;
            Ok(ModuleItem::Imports(expr(&tokens[2..])))
        }
        "config" if tokens.len() == 1 || word(&tokens[1]) == "when" => {
            let when = match &tokens[1..] {
                [] => None,
                [_, rest @ ..] if !rest.is_empty() => Some(expr(rest)),
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
    no_children(block, "`let name = expr`")?;
    match &block.line.tokens[..] {
        [_, Token {
            kind: TokenKind::Word(name),
            ..
        }, eq, rest @ ..]
            if is_symbol(eq, "=") && !rest.is_empty() =>
        {
            Ok(Let {
                name: name.clone(),
                value: expr(rest),
                line: block.line.no,
            })
        }
        _ => Err(Error::at(&block.line, "expected `let name = expr`")),
    }
}

fn option(block: &Block) -> Result<ModuleItem, Error> {
    let expected = "expected `option path: type`";
    let tokens = &block.line.tokens[1..];
    let (path, rest) =
        attrpath(tokens, &block.line).ok_or_else(|| Error::at(&block.line, expected))?;
    if rest.len() < 2 || !is_symbol(&rest[0], ":") {
        return Err(Error::at(&block.line, expected));
    }
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
                    value: expr(value),
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
    Ok(ModuleItem::Option(OptionDecl {
        path,
        ty: expr(&rest[1..]),
        fields,
    }))
}

fn for_block(block: &Block) -> Result<ForBlock, Error> {
    let expected = "expected `for ?x[, ?y] in expr` with indented entries";
    let tokens = &block.line.tokens;
    let split = tokens
        .iter()
        .position(|t| word(t) == "in")
        .ok_or_else(|| Error::at(&block.line, expected))?;
    let mut vars = Vec::new();
    for part in tokens[1..split].split(|t| is_symbol(t, ",")) {
        match part {
            [Token {
                kind: TokenKind::Variable(name),
                ..
            }] => vars.push(name.clone()),
            _ => return Err(Error::at(&block.line, expected)),
        }
    }
    if split + 1 == tokens.len() || block.children.is_empty() {
        return Err(Error::at(&block.line, expected));
    }
    Ok(ForBlock {
        vars,
        source: expr(&tokens[split + 1..]),
        entries: entries(&block.children)?,
        line: block.line.no,
    })
}

fn entries(children: &[Block]) -> Result<Vec<Entry>, Error> {
    children.iter().map(entry).collect()
}

fn entry(block: &Block) -> Result<Entry, Error> {
    let tokens = &block.line.tokens;
    if word(&tokens[0]) == "for" {
        return Ok(Entry::For(for_block(block)?));
    }
    let malformed = || Error::at(&block.line, ENTRY);
    let (path, rest) = attrpath(tokens, &block.line).ok_or_else(malformed)?;
    match rest {
        [colon, value @ ..] if is_symbol(colon, ":") => {
            if !block.children.is_empty() {
                return Err(Error::at(
                    &block.children[0].line,
                    "an entry with a value takes no indented entries; drop the `:` to nest",
                ));
            }
            let (value, when) = when_suffix(value).ok_or_else(malformed)?;
            if value.tokens.is_empty() {
                return Err(malformed());
            }
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
            when: Some(expr(cond)),
            entries: entries(&block.children)?,
        }),
        _ => Err(malformed()),
    }
}

fn when_suffix(tokens: &[Token]) -> Option<(Expr, Option<Expr>)> {
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
                return Some((expr(&tokens[..index]), Some(expr(cond))));
            }
            _ => {}
        }
    }
    Some((expr(tokens), None))
}

fn attrpath<'a>(tokens: &'a [Token], line: &Line) -> Option<(AttrPath, &'a [Token])> {
    let mut segments = Vec::new();
    let mut at = 0;
    loop {
        let token = tokens.get(at)?;
        match &token.kind {
            TokenKind::Word(name) if name != "when" && name != "for" => {
                segments.push(Segment::Name(name.clone()))
            }
            TokenKind::QuotedName(name) => segments.push(Segment::Name(name.clone())),
            TokenKind::Text(text) => segments.push(Segment::Text(text.clone())),
            TokenKind::Symbol(s) if s == "{" => {
                let close = matching(tokens, at)?;
                if close == at + 1 {
                    return None;
                }
                segments.push(Segment::Computed(expr(&tokens[at + 1..close])));
                at = close;
            }
            _ => return None,
        }
        at += 1;
        match tokens.get(at) {
            Some(dot) if is_symbol(dot, ".") => at += 1,
            _ => break,
        }
    }
    Some((
        AttrPath {
            segments,
            line: line.no,
        },
        &tokens[at..],
    ))
}

fn matching(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match word(token) {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
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

fn expr(tokens: &[Token]) -> Expr {
    Expr {
        tokens: tokens.to_vec(),
    }
}

fn word(token: &Token) -> &str {
    match &token.kind {
        TokenKind::Word(w) | TokenKind::Symbol(w) => w,
        _ => "",
    }
}

fn is_symbol(token: &Token, symbol: &str) -> bool {
    matches!(&token.kind, TokenKind::Symbol(s) if s == symbol)
}
