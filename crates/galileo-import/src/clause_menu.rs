use std::collections::BTreeMap;

pub struct Import {
    pub source: String,
    pub clauses: usize,
    pub maintenance: usize,
    pub unpaired: Vec<String>,
}

struct Shape {
    name: String,
    key: String,
    key_type: String,
    words: String,
    value_type: String,
    functional: bool,
}

fn shape(block: &str) -> Result<Shape, String> {
    let rows: Vec<_> = block.lines().collect();
    let name = rows[0].trim_end_matches(':').to_string();
    let (key, key_type) = rows
        .get(2)
        .and_then(|s| s.trim().split_once(": "))
        .ok_or_else(|| format!("{name}: unsupported relation"))?;
    let value_type = rows
        .get(3)
        .and_then(|s| s.trim().strip_prefix("value: "))
        .ok_or_else(|| format!("{name}: unsupported value"))?;
    let words = rows
        .get(5)
        .and_then(|s| s.trim().strip_suffix(": ?value"))
        .ok_or_else(|| format!("{name}: unsupported template"))?;
    Ok(Shape {
        name,
        key: key.into(),
        key_type: key_type.into(),
        words: words.into(),
        value_type: value_type.replace("F64", "Int"),
        functional: false,
    })
}

fn split(text: &str, delimiter: char) -> Vec<&str> {
    let mut quoted = false;
    let mut escaped = false;
    let mut depth = 0;
    let mut start = 0;
    let mut parts = Vec::new();
    for (at, c) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quoted && c == '\\' {
            escaped = true;
            continue;
        }
        if c == '"' {
            quoted = !quoted;
        }
        if quoted {
            continue;
        }
        match c {
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth -= 1,
            c if c == delimiter && depth == 0 => {
                parts.push(text[start..at].trim());
                start = at + c.len_utf8();
            }
            _ => (),
        }
    }
    parts.push(text[start..].trim());
    parts
}

fn expression(text: &str) -> Result<String, String> {
    let mut result = String::new();
    let mut at = 0;
    let chars: Vec<char> = text.chars().collect();
    while at < chars.len() {
        if chars[at] == '"' {
            result.push('"');
            at += 1;
            while at < chars.len() {
                let c = chars[at];
                result.push(c);
                at += 1;
                if c == '\\' && at < chars.len() {
                    result.push(chars[at]);
                    at += 1;
                } else if c == '"' {
                    break;
                }
            }
        } else if chars[at..].starts_with(&['i', 'f', '(']) {
            let start = at + 3;
            let mut end = start;
            let mut depth = 1;
            let mut quote = false;
            let mut escape = false;
            while end < chars.len() {
                let c = chars[end];
                if escape {
                    escape = false;
                } else if quote && c == '\\' {
                    escape = true;
                } else if c == '"' {
                    quote = !quote;
                } else if !quote {
                    if c == '(' {
                        depth += 1;
                    }
                    if c == ')' {
                        depth -= 1;
                    }
                }
                if depth == 0 {
                    break;
                }
                end += 1;
            }
            if depth != 0 {
                return Err("unterminated Clause if".into());
            }
            let inner: String = chars[start..end].iter().collect();
            let args = split(&inner, ',');
            let [condition, yes, no] = args.as_slice() else {
                return Err("Clause if needs three arguments".into());
            };
            result.push_str(&format!(
                "(if {} then {} else {})",
                expression(condition)?,
                expression(yes)?,
                expression(no)?
            ));
            at = end + 1;
        } else if chars[at] == '=' && (at == 0 || !matches!(chars[at - 1], '<' | '>' | '!' | '=')) {
            result.push_str("==");
            at += 1;
        } else if chars[at].is_ascii_digit() {
            while at < chars.len() && chars[at].is_ascii_digit() {
                result.push(chars[at]);
                at += 1;
            }
            if at + 1 < chars.len() && chars[at] == '.' && chars[at + 1] == '0' {
                at += 2;
            }
            if at < chars.len() && chars[at] == '.' {
                return Err("nonintegral Clause number in menu".into());
            }
        } else {
            result.push(chars[at]);
            at += 1;
        }
    }
    Ok(result
        .replace("contains-text(", "contains(")
        .replace("remaining-words(", "rest-words("))
}

fn sentence(text: &str, shapes: &[Shape]) -> Result<Option<(String, String)>, String> {
    let Some((key, rest)) = text.split_once(' ') else {
        return Ok(None);
    };
    for shape in shapes {
        if let Some(value) = rest.strip_prefix(&format!("{} ", shape.words)) {
            let value = expression(value)?;
            let value =
                if value.starts_with('"') && value.ends_with('"') && split(&value, '+').len() == 1
                    || !value.contains(' ')
                    || value.starts_with('(') && value.ends_with(')')
                {
                    value
                } else {
                    format!("({value})")
                };
            return Ok(Some((
                format!("{key} {} {value}", shape.words),
                format!("{}:{key}", shape.name),
            )));
        }
    }
    Ok(None)
}

fn condition(text: &str, shapes: &[Shape]) -> Result<String, String> {
    if text.starts_with("sum 1.0 given ") {
        let (_, tail) = text.split_once(" where { ").ok_or("unsupported menu sum")?;
        let (body, variable) = tail
            .rsplit_once(" } as ")
            .ok_or("unsupported menu sum binding")?;
        let body = split(body, ';')
            .into_iter()
            .map(|s| condition(s, shapes))
            .collect::<Result<Vec<_>, _>>()?
            .join(" and ");
        return Ok(format!("{variable} = sum 1 where {body}"));
    }
    Ok(match sentence(text, shapes)? {
        Some((text, _)) => text,
        None => expression(text)?,
    })
}

pub fn import(source: &str) -> Result<Import, String> {
    if source.contains("    ?north host effect ?effect") {
        return Err("DECISION 8.6: mailbox readers in accept-menu work, history/archive and models/efforts/config clauses need the pending-effect mapping".into());
    }
    let (_, tail) = source
        .split_once("\nmenu-kind:\n")
        .ok_or("expected north menu-kind slice")?;
    let (menu, _) = tail
        .split_once("\nknown-reference:\n")
        .ok_or("expected known-reference after menu slice")?;
    let menu = format!("menu-kind:\n{menu}");
    let blocks: Vec<&str> = menu
        .split("\n\n")
        .filter(|s| !s.trim().is_empty())
        .collect();
    let mut shapes = Vec::new();
    let rows: Vec<_> = menu.lines().collect();
    for (at, row) in rows.iter().enumerate() {
        if row.ends_with(':') && !row.starts_with(' ') {
            let block = rows
                .get(at..at + 6)
                .ok_or("incomplete menu relation")?
                .join("\n");
            shapes.push(shape(&block)?);
        }
    }
    for name in ["active-conversation"] {
        let marker = format!("\n{name}:\n");
        let (_, tail) = source
            .split_once(&marker)
            .ok_or_else(|| format!("missing {name}"))?;
        let end = tail
            .find("\n\n")
            .ok_or_else(|| format!("missing {name} terminator"))?;
        shapes.push(shape(&format!("{name}:\n{}", &tail[..end]))?);
    }
    for shape in &mut shapes {
        let mode = format!("mode {} given {} yields value: ", shape.name, shape.key);
        let mode = source
            .lines()
            .find_map(|line| line.strip_prefix(&mode))
            .ok_or_else(|| format!("missing {} mode", shape.name))?;
        shape.functional = match mode {
            "one" | "maybe" => true,
            "many" => false,
            _ => return Err(format!("unsupported {} mode {mode}", shape.name)),
        };
    }
    let mut out = String::from("type North\nstate entity MenuRow\nnorth-main is a North\n\n");
    for shape in &shapes {
        out.push_str(&format!(
            "state {}: {{{}: {}}} {} {{value: {}}}\n",
            shape.name, shape.key, shape.key_type, shape.words, shape.value_type
        ));
        if shape.functional {
            out.push_str(&format!(
                "  {} one value per {}\n",
                if shape.key_type == "North" {
                    "exactly"
                } else {
                    "at most"
                },
                shape.key
            ));
        }
    }
    out.push_str("\ninitially\n");
    for line in source
        .lines()
        .filter(|line| line.starts_with("north-main "))
    {
        if let Some((text, _)) = sentence(line, &shapes)? {
            out.push_str(&format!("  {text}\n"));
        }
    }
    let signatures = [
        ("clear-menu-rows", vec![("north", "North")]),
        (
            "open-history-menu",
            vec![("north", "North"), ("kind", "Text")],
        ),
        (
            "open-settings-menu",
            vec![("north", "North"), ("kind", "Text"), ("title", "Text")],
        ),
        (
            "offer-work-row",
            vec![
                ("north", "North"),
                ("reference", "Text"),
                ("title", "Text"),
                ("state", "Text"),
                ("holder", "Text"),
                ("clock", "Text"),
                ("eta", "Text"),
                ("blocked", "Bool"),
            ],
        ),
        ("accept-menu", vec![("north", "North")]),
        (
            "offer-settings-row",
            vec![
                ("north", "North"),
                ("key", "Text"),
                ("label", "Text"),
                ("description", "Text"),
                ("annotation", "Text"),
            ],
        ),
        (
            "offer-history-row",
            vec![
                ("north", "North"),
                ("key", "Text"),
                ("label", "Text"),
                ("description", "Text"),
            ],
        ),
        ("query-menu", vec![("north", "North"), ("query", "Text")]),
        ("filter-menu", vec![("north", "North")]),
        ("count-menu", vec![("north", "North")]),
        ("move-menu", vec![("north", "North"), ("delta", "Int")]),
        ("close-menu", vec![("north", "North")]),
    ];
    for (name, slots) in signatures {
        out.push_str(&format!("\nevent {name}: {name}"));
        for (name, ty) in slots {
            out.push_str(&format!(" {{{name}: {ty}}}"));
        }
        out.push('\n');
    }
    let mut clauses = 0;
    let mut maintenance = 0;
    let mut unpaired = Vec::new();
    for block in blocks.iter().filter(|block| block.starts_with("on ")) {
        let rows: Vec<&str> = block.lines().collect();
        clauses += 1;
        let handler = rows[0].split_whitespace().nth(1).unwrap();
        if matches!(handler, "filter-menu" | "count-menu") {
            maintenance += 1;
        }
        out.push_str(&format!("\n{}\n", rows[0]));
        let mut section = "";
        let mut conditions = Vec::new();
        let mut withdrawals = BTreeMap::new();
        let mut inclusions = Vec::new();
        let mut creates = Vec::new();
        let mut at = 1;
        while at < rows.len() {
            let row = rows[at];
            if row.starts_with("  ") && !row.starts_with("    ") {
                section = row.trim();
                at += 1;
                continue;
            }
            let text = row.trim();
            match section {
                "when" => conditions.push(condition(text, &shapes)?),
                "withdraw" => {
                    let (text, key) = sentence(text, &shapes)?
                        .ok_or_else(|| format!("unknown withdrawal {text}"))?;
                    withdrawals.insert(key, text);
                }
                "include" => {
                    let (text, key) = sentence(text, &shapes)?
                        .ok_or_else(|| format!("unknown inclusion {text}"))?;
                    inclusions.push((text, key));
                }
                "create" => {
                    if rows.get(at + 1).map(|s| s.trim()) != Some("member of: MenuRow") {
                        return Err("unsupported menu creation".into());
                    }
                    creates.push(format!("{text} is a new MenuRow"));
                    at += 1;
                }
                _ => return Err(format!("unsupported menu section {section}")),
            }
            at += 1;
        }
        for (i, text) in conditions.iter().enumerate() {
            out.push_str(&format!(
                "  {} {text}\n",
                if i == 0 { "when" } else { "and" }
            ));
        }
        let mut posts = Vec::new();
        for (text, key) in inclusions {
            let shape = shapes
                .iter()
                .find(|s| key.starts_with(&format!("{}:", s.name)))
                .unwrap();
            if shape.functional
                && withdrawals.remove(&key).is_none()
                && !creates.iter().any(|create| {
                    key.ends_with(&format!(":{}", create.split_whitespace().next().unwrap()))
                })
            {
                unpaired.push(format!("{handler} clause {clauses}: {}", shape.name));
            }
            posts.push(text);
        }
        posts.extend(creates);
        posts.extend(withdrawals.into_values().map(|text| format!("not {text}")));
        for (i, text) in posts.iter().enumerate() {
            out.push_str(&format!(
                "  {} {text}\n",
                if i == 0 { "then" } else { "and" }
            ));
        }
        out.push_str("  may skip\n");
    }
    Ok(Import {
        source: out,
        clauses,
        maintenance,
        unpaired,
    })
}
