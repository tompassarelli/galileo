#[derive(Clone, Debug, PartialEq)]
enum Form {
    Symbol(String),
    Text(String),
    Vector(Vec<Form>),
    Map(Vec<Form>),
    List(Vec<Form>),
}

const TEMPLATE_WORDS: [&str; 27] = [
    "a", "is", "tagged", "can", "opt", "into", "enables", "drops", "from", "adds", "to", "disables",
    "runs", "supports", "sets", "for", "enabled", "on", "by", "default", "through", "gets", "set",
    "allows", "every", "module", "supported",
];

fn name(word: &str) -> String {
    if TEMPLATE_WORDS.contains(&word) {
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
    Ok(out.join("\n") + "\n")
}

/// Platform facts: `linux allows every module`, and `X supports darwin` per entry of `config/darwin-modules.json`.
pub fn platform_facts(darwin_modules: &str) -> Result<String, String> {
    let list = darwin_modules.trim().strip_prefix('[').and_then(|t| t.strip_suffix(']'));
    let list = list.ok_or("darwin-modules.json: expected a JSON array")?;
    let mut out = String::from("linux allows every module\n");
    for entry in list.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let module = entry.strip_prefix('"').and_then(|e| e.strip_suffix('"'));
        let module = module.ok_or_else(|| format!("darwin-modules.json: expected a string, found {entry}"))?;
        out.push_str(&format!("{} supports darwin\n", name(module)));
    }
    Ok(out)
}

/// Facts for a module's `default.bnix`: `:tags`, `:tags-opt-in` and `:tag-overrides`.
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
    Ok(out.iter().map(|line| format!("{line}\n")).collect())
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
    let mut out = format!("{m} is a Module\n");
    for tag in tags {
        out.push_str(&format!("{m} is tagged {}\n", name(tag)));
    }
    for tag in opt_in {
        out.push_str(&format!("{m} can opt into {}\n", name(tag)));
    }
    Ok(out)
}

/// Declarations, once each, of every tag the module facts name and every `:platform` the host texts give.
pub fn catalog(module_facts: &str, host_texts: &[&str]) -> Result<String, String> {
    let mut tags = std::collections::BTreeSet::new();
    for line in module_facts.lines() {
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
    let tags = tags.iter().map(|t| format!("{t} is a Tag\n"));
    let platforms = platforms.iter().map(|p| format!("{p} is a Platform\n"));
    Ok(tags.chain(platforms).collect())
}
