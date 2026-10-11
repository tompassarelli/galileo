use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification as _,
    PublishDiagnostics,
};
use lsp_types::request::{CodeLensRequest, GotoDefinition, HoverRequest, References, Request as _};
use lsp_types::{
    CodeLens, CodeLensOptions, CodeLensParams, Command, Diagnostic, DiagnosticSeverity, GotoDefinitionParams, GotoDefinitionResponse, Hover,
    HoverContents, HoverParams, HoverProviderCapability, InitializeParams, Location,
    MarkupContent, MarkupKind, OneOf, Position, PublishDiagnosticsParams, Range,
    ReferenceParams, ServerCapabilities, TextDocumentPositionParams, TextDocumentSyncCapability,
    TextDocumentSyncKind, Url,
};
use galileo_syntax::check::{analyze, Index, SourceFile, Occurrence, Ty};
use galileo_syntax::lower::{lower, Role};
use galileo_syntax::record::{ModuleItem, RecordDecl};
use galileo_syntax::{blocks, lex, Kind, TokenKind};

struct Analysis {
    files: Vec<SourceFile>,
    diagnostics: BTreeMap<String, Vec<Diagnostic>>,
    index: Index,
    enabled: BTreeMap<String, String>,
}

struct Server {
    root: PathBuf,
    library: Vec<(String, String)>,
    texts: BTreeMap<String, String>,
    memo: Option<Analysis>,
    published: Vec<String>,
}

pub fn run(library: &[(&str, &str)]) -> Result<(), String> {
    let (connection, threads) = Connection::stdio();
    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        code_lens_provider: Some(CodeLensOptions { resolve_provider: Some(false) }),
        ..Default::default()
    };
    let params = handshake(&connection, capabilities)?;
    #[allow(deprecated)]
    let root = params
        .workspace_folders
        .as_ref()
        .and_then(|f| f.first())
        .map(|f| f.uri.clone())
        .or(params.root_uri)
        .and_then(|u| u.to_file_path().ok())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let mut server = Server {
        root,
        library: library.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect(),
        texts: BTreeMap::new(),
        memo: None,
        published: Vec::new(),
    };
    server.load_workspace();
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request).map_err(|e| e.to_string())? {
                    break;
                }
                let response = server.request(request);
                connection.sender.send(response.into()).map_err(|e| e.to_string())?;
            }
            Message::Notification(notification) => {
                if server.notification(notification) {
                    for message in server.publish() {
                        connection.sender.send(message.into()).map_err(|e| e.to_string())?;
                    }
                }
            }
            Message::Response(_) => {}
        }
    }
    drop(connection);
    threads.join().map_err(|e| e.to_string())
}

fn handshake(connection: &Connection, capabilities: ServerCapabilities) -> Result<InitializeParams, String> {
    let request = match connection.receiver.recv().map_err(|e| e.to_string())? {
        Message::Request(request) => request,
        other => return Err(format!("expected initialize, got {other:?}")),
    };
    let (id, params) = request
        .extract::<InitializeParams>(lsp_types::request::Initialize::METHOD)
        .map_err(|e| e.to_string())?;
    let result = lsp_types::InitializeResult {
        capabilities,
        server_info: Some(lsp_types::ServerInfo {
            name: "galileo".into(),
            version: None,
        }),
    };
    connection.sender.send(Response::new_ok(id, result).into()).map_err(|e| e.to_string())?;
    match connection.receiver.recv().map_err(|e| e.to_string())? {
        Message::Notification(n) if n.method == lsp_types::notification::Initialized::METHOD => Ok(params),
        other => Err(format!("expected initialized, got {other:?}")),
    }
}

impl Server {
    fn load_workspace(&mut self) {
        let mut found = Vec::new();
        collect(&self.root, &mut found);
        for path in found {
            if let Ok(text) = std::fs::read_to_string(&path) {
                self.texts.insert(self.name_of(&path), text);
            }
        }
    }

    fn name_of(&self, path: &Path) -> String {
        path.strip_prefix(&self.root).unwrap_or(path).display().to_string()
    }

    fn name_of_uri(&self, uri: &Url) -> Option<String> {
        uri.to_file_path().ok().map(|p| self.name_of(&p))
    }

    fn uri_of(&self, name: &str) -> Option<Url> {
        Url::from_file_path(self.root.join(name)).ok()
    }

    fn analysis(&mut self) -> &Analysis {
        if self.memo.is_none() {
            let mut files = Vec::new();
            let mut diagnostics: BTreeMap<String, Vec<Diagnostic>> =
                self.texts.keys().map(|n| (n.clone(), Vec::new())).collect();
            for (name, text) in &self.texts {
                match SourceFile::parse(&name, &text) {
                    Ok(file) => files.push(file),
                    Err(error) => diagnostics.entry(name.clone()).or_default().push(diagnostic(
                        &text,
                        error.error.line,
                        error.error.column,
                        error.error.message,
                    )),
                }
            }
            let (found, index) = analyze(&files, &galileo_syntax::schema::Schema::default());
            for d in found {
                let text = &files.iter().find(|f| f.name == d.file).expect("checked file").text;
                diagnostics.entry(d.file.clone()).or_default().push(diagnostic(text, d.line, d.column, d.message));
            }
            let enabled = enabled_lenses(&self.texts, &self.library);
            self.memo = Some(Analysis {
                files,
                diagnostics,
                index,
                enabled,
            });
        }
        self.memo.as_ref().expect("memo")
    }

    fn notification(&mut self, notification: Notification) -> bool {
        let notification = match notification.extract::<lsp_types::DidOpenTextDocumentParams>(DidOpenTextDocument::METHOD) {
            Ok(params) => {
                if let Some(name) = self.name_of_uri(&params.text_document.uri) {
                    self.texts.insert(name, params.text_document.text);
                }
                self.memo = None;
                return true;
            }
            Err(lsp_server::ExtractError::MethodMismatch(n)) => n,
            Err(_) => return false,
        };
        let notification = match notification.extract::<lsp_types::DidChangeTextDocumentParams>(DidChangeTextDocument::METHOD) {
            Ok(params) => {
                if let (Some(name), Some(change)) = (self.name_of_uri(&params.text_document.uri), params.content_changes.into_iter().last()) {
                    self.texts.insert(name, change.text);
                }
                self.memo = None;
                return true;
            }
            Err(lsp_server::ExtractError::MethodMismatch(n)) => n,
            Err(_) => return false,
        };
        if let Ok(params) = notification.extract::<lsp_types::DidCloseTextDocumentParams>(DidCloseTextDocument::METHOD) {
            if let Some(path) = params.text_document.uri.to_file_path().ok() {
                let name = self.name_of(&path);
                match std::fs::read_to_string(&path) {
                    Ok(text) => self.texts.insert(name, text),
                    Err(_) => self.texts.remove(&name),
                };
                self.memo = None;
                return true;
            }
        }
        false
    }

    fn publish(&mut self) -> Vec<Notification> {
        let diagnostics = self.analysis().diagnostics.clone();
        let mut out = Vec::new();
        let mut published = Vec::new();
        for name in self.published.iter().chain(diagnostics.keys()) {
            if published.contains(name) {
                continue;
            }
            let Some(uri) = self.uri_of(name) else { continue };
            let items = diagnostics.get(name).cloned().unwrap_or_default();
            out.push(Notification::new(
                PublishDiagnostics::METHOD.into(),
                PublishDiagnosticsParams::new(uri, items, None),
            ));
            published.push(name.clone());
        }
        self.published = diagnostics.into_iter().filter(|(_, d)| !d.is_empty()).map(|(n, _)| n).collect();
        out
    }

    fn request(&mut self, request: Request) -> Response {
        let id = request.id.clone();
        let request = match request.extract::<HoverParams>(HoverRequest::METHOD) {
            Ok((id, params)) => return Response::new_ok(id, self.hover(&params.text_document_position_params)),
            Err(lsp_server::ExtractError::MethodMismatch(r)) => r,
            Err(e) => return Response::new_err(id, lsp_server::ErrorCode::InvalidParams as i32, e.to_string()),
        };
        let request = match request.extract::<GotoDefinitionParams>(GotoDefinition::METHOD) {
            Ok((id, params)) => {
                let found = self.definition(&params.text_document_position_params);
                return Response::new_ok(id, found.map(GotoDefinitionResponse::Scalar));
            }
            Err(lsp_server::ExtractError::MethodMismatch(r)) => r,
            Err(e) => return Response::new_err(id, lsp_server::ErrorCode::InvalidParams as i32, e.to_string()),
        };
        let request = match request.extract::<CodeLensParams>(CodeLensRequest::METHOD) {
            Ok((id, params)) => return Response::new_ok(id, self.lenses(&params.text_document.uri)),
            Err(lsp_server::ExtractError::MethodMismatch(r)) => r,
            Err(e) => return Response::new_err(id, lsp_server::ErrorCode::InvalidParams as i32, e.to_string()),
        };
        match request.extract::<ReferenceParams>(References::METHOD) {
            Ok((id, params)) => {
                let found = self.references(&params.text_document_position, params.context.include_declaration);
                Response::new_ok(id, found)
            }
            Err(lsp_server::ExtractError::MethodMismatch(r)) => {
                Response::new_err(r.id, lsp_server::ErrorCode::MethodNotFound as i32, format!("unhandled {}", r.method))
            }
            Err(e) => Response::new_err(id, lsp_server::ErrorCode::InvalidParams as i32, e.to_string()),
        }
    }

    fn lenses(&mut self, uri: &Url) -> Vec<CodeLens> {
        let Some(name) = self.name_of_uri(uri) else { return Vec::new() };
        let Some(module) = name.strip_prefix("modules/").and_then(|rest| rest.split_once('/')).map(|(m, _)| m.to_string()) else {
            return Vec::new();
        };
        let Some(title) = self.analysis().enabled.get(&module).cloned() else { return Vec::new() };
        vec![CodeLens {
            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
            command: Some(Command { title, command: String::new(), arguments: None }),
            data: None,
        }]
    }

    fn at(&mut self, position: &TextDocumentPositionParams) -> Option<(String, usize)> {
        let name = self.name_of_uri(&position.text_document.uri)?;
        let text = self.texts.get(&name)?;
        Some((name.clone(), offset(text, position.position)))
    }

    fn hover(&mut self, position: &TextDocumentPositionParams) -> Option<Hover> {
        let (name, offset) = self.at(position)?;
        let analysis = self.analysis();
        let file = analysis.files.iter().find(|f| f.name == name)?;
        let value = match analysis.index.at(&name, offset) {
            Some(typed) => typed_hover(&file.text, typed, &analysis.index),
            None => sentence_hover(file, line_of(&file.text, offset))?,
        };
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: None,
        })
    }

    fn definition(&mut self, position: &TextDocumentPositionParams) -> Option<Location> {
        let (name, offset) = self.at(position)?;
        if let Some(path) = self.option_at(&name, offset) {
            return self.declaration(&path);
        }
        let analysis = self.analysis();
        let file = analysis.files.iter().find(|f| f.name == name)?;
        let occurrence = analysis.index.at(&name, offset)?;
        let symbol = &file.text[occurrence.start..occurrence.end];
        let line = line_of(&file.text, offset);
        let top = blocks(&file.text).ok()?;
        let block = top.iter().rev().find(|b| b.line.no <= line)?;
        let declaration = file.decls.iter().find(|d| match d {
            RecordDecl::Module(module) => module.line == block.line.no,
            RecordDecl::File(file) => file.line == block.line.no,
        })?;
        let binding = match declaration {
            RecordDecl::Module(module) => module.items.iter().find_map(|item| match item {
                ModuleItem::Let(binding) if binding.name == symbol && binding.line < line => Some(binding),
                _ => None,
            }),
            RecordDecl::File(file) => file.lets.iter().find(|binding| binding.name == symbol && binding.line < line),
        }?;
        let declared_line = binding.line.saturating_sub(1) as u32;
        Some(Location::new(self.uri_of(&name)?, Range::new(Position::new(declared_line, 0), Position::new(declared_line + 1, 0))))
    }

    fn option_at(&mut self, name: &str, offset: usize) -> Option<Vec<String>> {
        let analysis = self.analysis();
        if let Some(path) = analysis.index.at(name, offset).and_then(|t| t.option.clone()) {
            return Some(path);
        }
        let file = analysis.files.iter().find(|f| f.name == name)?;
        let line = line_of(&file.text, offset);
        let (path, _) = analysis.index.options.iter().find(|(_, d)| d.file == name && d.line == line)?;
        let row = lex(&file.text).ok()?.into_iter().find(|r| r.no == line)?;
        let start = row.tokens.get(1)?.span.start;
        let end = row.tokens.iter().find(|t| matches!(&t.kind, TokenKind::Symbol(s) if s == ":"))?.span.start;
        (start <= offset && offset < end).then(|| path.clone())
    }

    fn declaration(&mut self, path: &[String]) -> Option<Location> {
        let declared = self.analysis().index.options.get(path)?.clone();
        let line = declared.line.saturating_sub(1) as u32;
        Some(Location::new(self.uri_of(&declared.file)?, Range::new(Position::new(line, 0), Position::new(line + 1, 0))))
    }

    fn references(&mut self, position: &TextDocumentPositionParams, declaration: bool) -> Option<Vec<Location>> {
        let (name, offset) = self.at(position)?;
        let path = self.option_at(&name, offset)?;
        let mut out: Vec<Location> = declaration.then(|| self.declaration(&path)).flatten().into_iter().collect();
        let analysis = self.analysis();
        let uses: Vec<(String, Range)> = analysis
            .index
            .uses(&path)
            .filter_map(|t| {
                let text = &analysis.files.iter().find(|f| f.name == t.file)?.text;
                Some((t.file.clone(), Range::new(position_of(text, t.start), position_of(text, t.end))))
            })
            .collect();
        for (file, range) in uses {
            if let Some(uri) = self.uri_of(&file) {
                out.push(Location::new(uri, range));
            }
        }
        Some(out)
    }
}

fn enabled_lenses(texts: &BTreeMap<String, String>, library: &[(String, String)]) -> BTreeMap<String, String> {
    use galileo_core::{evaluate, Budget, RelationId, RelationKind, Value};
    let mut sources: Vec<(String, &str)> = texts
        .iter()
        .map(|(name, text)| (name.strip_suffix(".gal").unwrap_or(name).to_string(), text.as_str()))
        .collect();
    for (name, text) in library {
        let used = sources.iter().any(|(_, t)| t.lines().any(|l| l.strip_prefix("use ").map(str::trim) == Some(name.as_str())));
        if used && !sources.iter().any(|(n, _)| n == name) {
            sources.push((name.clone(), text.as_str()));
        }
    }
    let named: Vec<(&str, &str)> = sources.iter().map(|(n, t)| (n.as_str(), *t)).collect();
    let Ok(lowered) = galileo_syntax::lower::lower_program(&named) else { return BTreeMap::new() };
    if !lowered.diagnostics.is_empty() {
        return BTreeMap::new();
    }
    let program = lowered.program;
    let find = |template: &str| program.relations.iter().position(|r| r.template == template).map(RelationId);
    let (Some(on), Some(by), Some(tagged), Some(turned)) = (
        find("{module} is enabled on {host}").filter(|id| program.relations[id.0].kind == RelationKind::Derived),
        find("{module} is enabled on {host} by {tag}"),
        find("{module} is tag-enabled on {host}"),
        find("{host} turns on {module}"),
    ) else {
        return BTreeMap::new();
    };
    let Ok(model) = evaluate(&program, &[], Budget::default()) else { return BTreeMap::new() };
    let names = |row: &Vec<Value>| -> Vec<String> {
        row.iter().map(|v| match v { Value::Entity(e) => e.name.clone(), other => format!("{other:?}") }).collect()
    };
    let mut hosts: BTreeMap<String, BTreeMap<String, (BTreeSet<String>, bool)>> = BTreeMap::new();
    for row in model.tuples(on).map(names) {
        hosts.entry(row[0].clone()).or_default().entry(row[1].clone()).or_default();
    }
    let tag_enabled: BTreeSet<Vec<String>> = model.tuples(tagged).map(names).collect();
    for row in model.tuples(by).map(names) {
        if tag_enabled.contains(&row[..2].to_vec()) {
            if let Some(entry) = hosts.get_mut(&row[0]).and_then(|h| h.get_mut(&row[1])) {
                entry.0.insert(row[2].clone());
            }
        }
    }
    for row in model.tuples(turned).map(names) {
        if let Some(entry) = hosts.get_mut(&row[1]).and_then(|h| h.get_mut(&row[0])) {
            entry.1 = true;
        }
    }
    let mut out: BTreeMap<String, String> = texts
        .keys()
        .filter_map(|n| n.strip_prefix("modules/")?.split_once('/').map(|(m, _)| (m.to_string(), "enabled on no host".to_string())))
        .collect();
    for (module, on) in hosts {
        let parts: Vec<String> = on
            .into_iter()
            .map(|(host, (tags, direct))| {
                let mut why = Vec::new();
                if !tags.is_empty() {
                    why.push(format!("via {}", tags.into_iter().collect::<Vec<_>>().join(", ")));
                }
                if direct {
                    why.push("turned on".to_string());
                }
                format!("{host} ({})", why.join("; "))
            })
            .collect();
        out.insert(module, format!("enabled on {}", parts.join(", ")));
    }
    out
}

fn typed_hover(text: &str, typed: &Occurrence, index: &Index) -> String {
    let source = &text[typed.start..typed.end];
    let level = if matches!(typed.ty, Ty::Nix(_)) { "staged" } else { "total" };
    let (reading, ty) = match (&typed.option, &typed.ty) {
        (Some(path), _) => {
            let declared = &index.options[path];
            let mut reading = format!("`{source}` reads the option `{}`", path.join("."));
            if let Some(description) = &declared.description {
                reading.push_str(&format!(": {description}"));
            }
            (reading, typed.ty.to_string())
        }
        (None, Ty::Nix(inner)) => match inner.as_ref() {
            Ty::Options(prefix) if !prefix.is_empty() => (
                format!("`{source}` is the options under `{}`", prefix.join(".")),
                format!("config.{}", prefix.join(".")),
            ),
            Ty::Members(path) => (format!("`{source}` is the Nix value `{}`", path.join(".")), "nix Any".into()),
            _ => (format!("`{source}`"), typed.ty.to_string()),
        },
        _ => (format!("`{source}`"), typed.ty.to_string()),
    };
    format!("{reading}\n\ntype: `{ty}`\n\neffect level: {level}")
}

fn sentence_hover(file: &SourceFile, line: usize) -> Option<String> {
    let lowered = lower(&file.text, &file.name).ok()?;
    let reading = lowered.readings.iter().find(|r| r.line == line)?;
    let slots: Vec<String> = reading
        .slots
        .iter()
        .map(|s| format!("{} = {}", s.name, s.filler.as_deref().unwrap_or("_")))
        .collect();
    let types: Vec<String> = reading.slots.iter().map(|s| format!("{}: {}", s.name, s.ty)).collect();
    let level = match &reading.role {
        Role::Event | Role::Handler { .. } => "transition",
        Role::Head { rule } | Role::Condition { rule, .. }
            if lowered.readings.iter().any(|condition| {
                matches!(&condition.role, Role::Condition { rule: owner, .. } if owner == rule)
                    && file.sentences.vocabulary.shapes.iter().any(|shape| shape.name == condition.shape && shape.kind == Kind::State)
            }) => "reads state",
        _ => "total",
    };
    Some(format!(
        "{} ({})\n\ntype: `{}({})`\n\neffect level: {level}",
        reading.template,
        slots.join(", "),
        reading.shape,
        types.join(", ")
    ))
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for path in entries {
        let hidden = path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.') || n == "target");
        if hidden {
            continue;
        }
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "gal") {
            out.push(path);
        }
    }
}

fn diagnostic(text: &str, line: usize, column: usize, message: String) -> Diagnostic {
    let line0 = line.saturating_sub(1);
    let line_text = text.lines().nth(line0).unwrap_or("");
    let start: u32 = line_text.chars().take(column.saturating_sub(1)).map(|c| c.len_utf16() as u32).sum();
    let end: u32 = line_text.chars().map(|c| c.len_utf16() as u32).sum();
    Diagnostic {
        range: Range::new(Position::new(line0 as u32, start), Position::new(line0 as u32, end.max(start))),
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("galileo".into()),
        message,
        ..Default::default()
    }
}

fn line_start(text: &str, line: usize) -> usize {
    text.split_inclusive('\n').take(line).map(str::len).sum()
}

fn offset(text: &str, position: Position) -> usize {
    let start = line_start(text, position.line as usize);
    let mut units = 0;
    for (i, c) in text[start..].char_indices() {
        if units >= position.character as usize || c == '\n' {
            return start + i;
        }
        units += c.len_utf16();
    }
    text.len()
}

fn position_of(text: &str, offset: usize) -> Position {
    let line = text[..offset].matches('\n').count();
    let start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
    let character: usize = text[start..offset].chars().map(char::len_utf16).sum();
    Position::new(line as u32, character as u32)
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset].matches('\n').count() + 1
}
