#[derive(Clone, Debug, PartialEq)]
enum Form {
    Symbol(String),
    Text(String),
    Vector(Vec<Form>),
    Map(Vec<Form>),
    List(Vec<Form>),
}

fn name(word: &str) -> String {
    if galileo_syntax::expr::KEYWORDS.contains(&word) {
        format!("`{word}`")
    } else {
        word.to_string()
    }
}

fn read(text: &str) -> Result<Vec<Form>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0;
    let mut stack: Vec<(char, Vec<Form>)> = vec![(' ', Vec::new())];
    while at < chars.len() {
        let c = chars[at];
        match c {
            ';' => {
                while at < chars.len() && chars[at] != '\n' {
                    at += 1;
                }
            }
            '#' if at == 0 || chars[at - 1] == '\n' => {
                while at < chars.len() && chars[at] != '\n' {
                    at += 1;
                }
            }
            c if c.is_whitespace() || c == ',' => at += 1,
            '(' | '[' | '{' => {
                stack.push((c, Vec::new()));
                at += 1;
            }
            ')' | ']' | '}' => {
                let (open, items) = stack.pop().ok_or("unbalanced close")?;
                let form = match (open, c) {
                    ('(', ')') => Form::List(items),
                    ('[', ']') => Form::Vector(items),
                    ('{', '}') => Form::Map(items),
                    _ => return Err(format!("`{open}` closed by `{c}`")),
                };
                stack.last_mut().ok_or("unbalanced close")?.1.push(form);
                at += 1;
            }
            '"' => {
                let mut value = String::new();
                at += 1;
                while at < chars.len() && chars[at] != '"' {
                    if chars[at] == '\\' && at + 1 < chars.len() {
                        at += 1;
                        value.push(match chars[at] {
                            'n' => '\n',
                            't' => '\t',
                            other => other,
                        });
                    } else {
                        value.push(chars[at]);
                    }
                    at += 1;
                }
                if at >= chars.len() {
                    return Err("unterminated string".into());
                }
                at += 1;
                stack.last_mut().ok_or("unbalanced")?.1.push(Form::Text(value));
            }
            _ => {
                let start = at;
                while at < chars.len()
                    && !chars[at].is_whitespace()
                    && !"()[]{}\";,".contains(chars[at])
                {
                    at += 1;
                }
                let word: String = chars[start..at].iter().collect();
                stack.last_mut().ok_or("unbalanced")?.1.push(Form::Symbol(word));
            }
        }
    }
    match stack.pop() {
        Some((' ', forms)) if stack.is_empty() => Ok(forms),
        _ => Err("unclosed form".into()),
    }
}

fn field<'a>(forms: &'a [Form], key: &str) -> Option<&'a Form> {
    for form in forms {
        let items = match form {
            Form::Vector(items) | Form::List(items) => items,
            Form::Map(items) => {
                for pair in items.chunks(2) {
                    if let [Form::Symbol(k), value] = pair {
                        if k == key {
                            return Some(value);
                        }
                    }
                }
                items
            }
            _ => continue,
        };
        if let Some(found) = field(items, key) {
            return Some(found);
        }
    }
    None
}

fn symbols(form: &Form, what: &str) -> Result<Vec<String>, String> {
    match form {
        Form::Vector(items) => items
            .iter()
            .map(|item| match item {
                Form::Symbol(s) => Ok(s.clone()),
                other => Err(format!("{what}: expected a symbol, found {other:?}")),
            })
            .collect(),
        other => Err(format!("{what}: expected a vector, found {other:?}")),
    }
}

fn text_literal(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '{' => out.push_str("\\{"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn value(form: &Form) -> Result<String, String> {
    match form {
        Form::Text(t) => Ok(text_literal(t)),
        Form::Symbol(s) if s == "true" || s == "false" || s.parse::<i64>().is_ok() => Ok(s.clone()),
        other => Err(format!(":tag-overrides: unsupported value {other:?}")),
    }
}

fn once(lines: &[String]) -> String {
    let facts: Vec<(&str, &str)> = lines.iter().map(|l| l.split_once(' ').unwrap_or((l, ""))).collect();
    let mut out = String::new();
    let mut at = 0;
    while at < facts.len() {
        let subject = facts[at].0;
        let run = facts[at..].iter().take_while(|(s, _)| *s == subject).count();
        if run == 1 {
            out.push_str(&format!("{subject} {}\n", facts[at].1));
        } else {
            out.push_str(&format!("{subject}\n"));
            for (_, predicate) in &facts[at..at + run] {
                out.push_str(&format!("  {predicate}\n"));
            }
        }
        at += run;
    }
    out
}

fn roll_call(names: &[String], predicate: &str) -> String {
    match names {
        [] => String::new(),
        [one] => format!("{one} {predicate}\n"),
        many => format!("[{}] {predicate}\n", many.join(", ")),
    }
}

fn unfold(text: &str) -> Vec<String> {
    let mut subject = String::new();
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.starts_with(' ') {
            lines.push(format!("{subject} {}", line.trim()));
        } else if !line.contains(' ') {
            subject = line.to_string();
        } else {
            lines.push(line.to_string());
        }
    }
    lines
}

/// Facts for `hosts/<host>/enabled-tags.bnix`: `:platform`, `:enabled` (with `-m`/`+m` edits) and `:disabled`.
pub fn host_facts(host: &str, text: &str) -> Result<String, String> {
    let forms = read(text)?;
    let h = name(host);
    let mut out = vec![format!("{h} is a Host")];
    if let Some(Form::Symbol(platform)) = field(&forms, ":platform") {
        out.push(format!("{h} runs {}", name(platform)));
    }
    let enabled = match field(&forms, ":enabled") {
        Some(Form::Vector(items)) => items.clone(),
        Some(other) => return Err(format!(":enabled: expected a vector, found {other:?}")),
        None => Vec::new(),
    };
    for entry in &enabled {
        match entry {
            Form::Symbol(tag) => out.push(format!("{h} enables {}", name(tag))),
            Form::Vector(_) => {
                let words = symbols(entry, ":enabled")?;
                let (tag, edits) = words.split_first().ok_or(":enabled: empty tag vector")?;
                let tag = name(tag);
                out.push(format!("{h} enables {tag}"));
                for edit in edits {
                    match edit.split_at(1) {
                        ("-", m) if !m.is_empty() => out.push(format!("{h} drops {} from {tag}", name(m))),
                        ("+", m) if !m.is_empty() => out.push(format!("{h} adds {} to {tag}", name(m))),
                        _ => return Err(format!(":enabled: edit `{edit}` is not -module or +module")),
                    }
                }
            }
            other => return Err(format!(":enabled: unexpected {other:?}")),
        }
    }
    if let Some(disabled) = field(&forms, ":disabled") {
        for m in symbols(disabled, ":disabled")? {
            out.push(format!("{h} disables {}", name(&m)));
        }
    }
    Ok(once(&out))
}

/// Platform facts: `linux allows every module`, and with `config/darwin-modules.json`'s entries `darwin` declared beside `linux` and one `[…] supports darwin` line.
pub fn platform_facts(darwin_modules: &str) -> Result<String, String> {
    let list = darwin_modules.trim().strip_prefix('[').and_then(|t| t.strip_suffix(']'));
    let list = list.ok_or("darwin-modules.json: expected a JSON array")?;
    let mut supported = Vec::new();
    for entry in list.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let module = entry.strip_prefix('"').and_then(|e| e.strip_suffix('"'));
        let module = module.ok_or_else(|| format!("darwin-modules.json: expected a string, found {entry}"))?;
        supported.push(name(module));
    }
    let mut platforms = vec!["linux".to_string()];
    if !supported.is_empty() {
        platforms.push("darwin".to_string());
    }
    Ok(roll_call(&platforms, "is a Platform") + "linux allows every module\n" + &roll_call(&supported, "supports darwin"))
}

/// Facts for a module's `default.bnix`: `:tags`, `:tags-opt-in`, `:tag-overrides` and each `:flake-inputs` url.
pub fn module_facts(module: &str, text: &str) -> Result<String, String> {
    let forms = read(text)?;
    let m = name(module);
    let mut out = vec![format!("{m} is a Module")];
    if let Some(tags) = field(&forms, ":tags") {
        for tag in symbols(tags, ":tags")? {
            out.push(format!("{m} is tagged {}", name(&tag)));
        }
    }
    if let Some(tags) = field(&forms, ":tags-opt-in") {
        for tag in symbols(tags, ":tags-opt-in")? {
            out.push(format!("{m} can opt into {}", name(&tag)));
        }
    }
    match field(&forms, ":tag-overrides") {
        None => {}
        Some(Form::Map(items)) => {
            for pair in items.chunks(2) {
                let [Form::Symbol(tag), Form::Map(settings)] = pair else {
                    return Err(":tag-overrides: expected {tag {:path value ...}}".into());
                };
                for setting in settings.chunks(2) {
                    let [Form::Symbol(path), v] = setting else {
                        return Err(":tag-overrides: expected :path value".into());
                    };
                    let path = path.strip_prefix(':').ok_or(":tag-overrides: a path is a keyword")?;
                    out.push(format!("{} sets {path} to {} for {m}", name(tag), value(v)?));
                }
            }
        }
        Some(other) => return Err(format!(":tag-overrides: expected a map, found {other:?}")),
    }
    match field(&forms, ":flake-inputs") {
        None => {}
        Some(Form::Map(items)) => {
            for pair in items.chunks(2) {
                let [Form::Symbol(input), Form::Map(attributes)] = pair else {
                    return Err(":flake-inputs: expected {:name {:url ...}}".into());
                };
                let input = input.strip_prefix(':').ok_or(":flake-inputs: a name is a keyword")?;
                let attributes = [Form::Map(attributes.clone())];
                let Some(Form::Text(url)) = field(&attributes, ":url") else {
                    return Err(format!(":flake-inputs: {input} has no :url"));
                };
                out.push(format!("{m} needs flake input {} from {}", text_literal(input), text_literal(url)));
            }
        }
        Some(other) => return Err(format!(":flake-inputs: expected a map, found {other:?}")),
    }
    Ok(once(&out))
}

/// Facts for a module's `tags.clause`: `module-metadata([tags...], [opt-in...])`.
pub fn clause_facts(module: &str, text: &str) -> Result<String, String> {
    let start = text.find("module-metadata(").ok_or("tags.clause: no module-metadata call")?;
    let forms = read(&text[start + "module-metadata".len()..])?;
    let Some(Form::List(arguments)) = forms.first() else {
        return Err("tags.clause: malformed module-metadata call".into());
    };
    let lists: Vec<Vec<String>> = arguments
        .iter()
        .map(|argument| match argument {
            Form::Vector(items) => items
                .iter()
                .map(|item| match item {
                    Form::Text(t) => Ok(t.clone()),
                    other => Err(format!("tags.clause: expected a string, found {other:?}")),
                })
                .collect(),
            other => Err(format!("tags.clause: expected a list, found {other:?}")),
        })
        .collect::<Result<_, _>>()?;
    let [tags, opt_in] = lists.as_slice() else {
        return Err("tags.clause: module-metadata takes two lists".into());
    };
    let m = name(module);
    let mut out = vec![format!("{m} is a Module")];
    out.extend(tags.iter().map(|tag| format!("{m} is tagged {}", name(tag))));
    out.extend(opt_in.iter().map(|tag| format!("{m} can opt into {}", name(tag))));
    Ok(once(&out))
}

/// A Clause module's `.gal`: its `tags.clause` facts above the imported `default.nix` module.
pub fn clause_module(module: &str, tags_clause: &str, nix_module: &str) -> Result<String, String> {
    let facts = clause_facts(module, tags_clause)?;
    let m = name(module);
    let tags_itself = unfold(&facts).iter().any(|l| *l == format!("{m} is tagged {m}") || *l == format!("{m} can opt into {m}"));
    let own_tag = if tags_itself { format!("{m} is a Tag\n") } else { String::new() };
    Ok(format!("use firn/tags\n\n{own_tag}{facts}\n{nix_module}"))
}

/// One roll-call line each for the tags the module facts name and the `:platform`s the host texts give, less `linux`, which `platform_facts` declares.
pub fn catalog(module_facts: &str, host_texts: &[&str]) -> Result<String, String> {
    let mut tags = std::collections::BTreeSet::new();
    for line in unfold(module_facts) {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            [_, "is", "tagged", tag] | [_, "can", "opt", "into", tag] | [tag, "sets", _, "to", .., "for", _] => {
                tags.insert(tag.to_string());
            }
            _ => {}
        }
    }
    let mut platforms = std::collections::BTreeSet::new();
    for text in host_texts {
        if let Some(Form::Symbol(platform)) = field(&read(text)?, ":platform") {
            platforms.insert(name(platform));
        }
    }
    platforms.remove("linux");
    let tags: Vec<String> = tags.into_iter().collect();
    let platforms: Vec<String> = platforms.into_iter().collect();
    Ok(roll_call(&tags, "is a Tag") + &roll_call(&platforms, "is a Platform"))
}

fn names(path: &galileo_syntax::record::AttrPath) -> Option<Vec<&str>> {
    use galileo_syntax::record::Segment;
    path.segments.iter().map(|s| if let Segment::Name(n) = s { Some(n.as_str()) } else { None }).collect()
}

fn literal_enable(entry: &galileo_syntax::record::Entry, prefix: &[&str]) -> Option<bool> {
    use galileo_syntax::record::{Entry, ExprKind};
    let Entry::Value { path, value, when: None } = entry else { return None };
    let ExprKind::Bool(on) = value.kind else { return None };
    (names(path)? == [prefix, &["enable"]].concat()).then_some(on)
}

fn module_record(path: &galileo_syntax::record::AttrPath) -> Option<String> {
    match names(path)?.as_slice() {
        ["myConfig", "modules", m] => Some(m.to_string()),
        _ => None,
    }
}

/// A host's `.gal` for `hosts/<host>/configuration.nix`: its literal `myConfig.modules.<m>.enable` entries become `turns on`/`turns off` facts, emitted by one `for` each.
pub fn host_configuration(host: &str, file: &galileo_syntax::record::NixFile) -> Result<String, String> {
    use galileo_syntax::record::{print, Entry, FileValue, RecordDecl};
    let plain = || print(&RecordDecl::File(file.clone()));
    let FileValue::Entries(entries) = &file.value else { return Ok(plain()) };
    let h = name(host);
    let mut facts = Vec::new();
    let mut kept = Vec::new();
    let mut place = None;
    for entry in entries {
        let mut hits = Vec::new();
        let mut keep = Some(entry.clone());
        match entry {
            Entry::Value { path, .. } => {
                if let Some(["myConfig", "modules", m, "enable"]) = names(path).as_deref() {
                    if let Some(on) = literal_enable(entry, &["myConfig", "modules", m]) {
                        hits.push((m.to_string(), on));
                        keep = None;
                    }
                }
            }
            Entry::Nested { path, when: None, entries: inner } => {
                if let Some(m) = module_record(path) {
                    let mut rest = Vec::new();
                    for item in inner {
                        match literal_enable(item, &[]) {
                            Some(on) => hits.push((m.clone(), on)),
                            None => rest.push(item.clone()),
                        }
                    }
                    keep = (!rest.is_empty()).then(|| Entry::Nested { path: path.clone(), when: None, entries: rest });
                }
            }
            _ => {}
        }
        if !hits.is_empty() {
            place.get_or_insert(kept.len());
        }
        for (m, on) in hits {
            facts.push(format!("{h} turns {} {}", if on { "on" } else { "off" }, name(&m)));
        }
        kept.extend(keep);
    }
    let Some(place) = place else { return Ok(plain()) };
    let mut fors = Vec::new();
    for (word, value) in [("on", "true"), ("off", "false")] {
        if facts.iter().any(|f| f.starts_with(&format!("{h} turns {word} "))) {
            let snippet = format!("nix file\n  value\n    for ?m where {h} turns {word} ?m\n      myConfig.modules.{{?m}}.enable: {value}\n");
            let decls = galileo_syntax::record::parse_record_file("<host>", &snippet).map_err(|e| e.to_string())?;
            let [RecordDecl::File(galileo_syntax::record::NixFile { value: FileValue::Entries(made), .. })] = decls.as_slice() else {
                return Err("host configuration: malformed for block".into());
            };
            fors.extend(made.iter().cloned());
        }
    }
    kept.splice(place..place, fors);
    let lifted = RecordDecl::File(galileo_syntax::record::NixFile { value: FileValue::Entries(kept), ..file.clone() });
    Ok(format!("use firn/tags\n\n{}\n\n{}", facts.join("\n"), print(&lifted)))
}
