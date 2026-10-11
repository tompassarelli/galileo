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

struct Fact {
    shape: usize,
    key: String,
    value: String,
}

fn fact(text: &str, shapes: &[Shape]) -> Option<Fact> {
    let (key, rest) = text.split_once(' ')?;
    shapes.iter().enumerate().find_map(|(shape, s)| {
        rest.strip_prefix(&format!("{} ", s.words)).map(|value| Fact {
            shape,
            key: key.into(),
            value: value.into(),
        })
    })
}

fn kebab(ty: &str) -> String {
    ty.chars()
        .enumerate()
        .flat_map(|(at, c)| {
            let lower = c.to_ascii_lowercase();
            if at > 0 && c.is_ascii_uppercase() {
                vec!['-', lower]
            } else {
                vec![lower]
            }
        })
        .collect()
}

fn field_name(shape: &Shape) -> String {
    let prefix = format!("{}-", kebab(&shape.key_type));
    shape
        .name
        .strip_prefix(&prefix)
        .unwrap_or(&shape.name)
        .to_string()
}

fn substitute(text: &str, map: &BTreeMap<String, String>) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut quoted = false;
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if quoted && c == '\\' && at + 1 < chars.len() {
            out.push(c);
            out.push(chars[at + 1]);
            at += 2;
            continue;
        }
        if c == '"' {
            quoted = !quoted;
        }
        if c == '?' && !quoted {
            let mut end = at + 1;
            while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '-') {
                end += 1;
            }
            let name: String = chars[at..end].iter().collect();
            out.push_str(map.get(&name).unwrap_or(&name));
            at = end;
            continue;
        }
        out.push(c);
        at += 1;
    }
    out
}

fn slot_value(value: String) -> String {
    if value.starts_with('"') && value.ends_with('"') && split(&value, '+').len() == 1
        || !value.contains(' ')
        || value.starts_with('(') && value.ends_with(')')
    {
        value
    } else {
        format!("({value})")
    }
}

fn slot(name: &str, ty: &str, types: &[&str]) -> String {
    if types.contains(&ty) && kebab(ty) == name {
        ty.to_string()
    } else {
        format!("?{name}: {ty}")
    }
}

struct Clause<'a> {
    head: &'a str,
    handler: &'a str,
    conditions: Vec<&'a str>,
    withdrawals: Vec<Fact>,
    includes: Vec<Fact>,
    creates: Vec<String>,
}

struct Context<'a> {
    shapes: &'a [Shape],
    head: Vec<&'a str>,
    map: BTreeMap<String, String>,
    bound: std::collections::BTreeSet<String>,
}

impl Context<'_> {
    fn render(&self, text: &str) -> Result<String, String> {
        Ok(substitute(&expression(text)?, &self.map))
    }

    fn collect(&mut self, text: &str) {
        if let Some((_, tail)) = text.split_once(" where { ") {
            if let Some((body, _)) = tail.rsplit_once(" } as ") {
                for part in split(body, ';') {
                    self.collect(part);
                }
            }
            return;
        }
        let Some(read) = fact(text, self.shapes) else {
            return;
        };
        let shape = &self.shapes[read.shape];
        if shape.functional
            && read.value.starts_with('?')
            && !self.head.contains(&read.value.as_str())
            && !self.map.contains_key(&read.value)
        {
            self.map
                .insert(read.value, format!("{}.{}", read.key, field_name(shape)));
        }
    }

    fn condition(&mut self, text: &str) -> Result<Option<String>, String> {
        if text.starts_with("sum 1.0 given ") {
            let (_, tail) = text.split_once(" where { ").ok_or("unsupported menu sum")?;
            let (body, variable) = tail
                .rsplit_once(" } as ")
                .ok_or("unsupported menu sum binding")?;
            let mut parts = Vec::new();
            for part in split(body, ';') {
                parts.extend(self.condition(part)?);
            }
            return Ok(Some(format!("{variable} = sum 1 where {}", parts.join(" and "))));
        }
        let Some(read) = fact(text, self.shapes) else {
            return self.render(text).map(Some);
        };
        let shape = &self.shapes[read.shape];
        if !shape.functional {
            return Ok(Some(format!(
                "{} {} {}",
                read.key,
                shape.words,
                slot_value(self.render(&read.value)?)
            )));
        }
        let field = format!("{}.{}", read.key, field_name(shape));
        if self.map.get(&read.value) == Some(&field) && self.bound.insert(read.value.clone()) {
            return Ok(None);
        }
        Ok(Some(format!("{field} == {}", self.render(&read.value)?)))
    }
}

pub fn import(source: &str) -> Result<Import, String> {
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
    for name in ["host-effect", "effect-payload", "active-conversation"] {
        let marker = format!("\n{name}:\n");
        let (_, tail) = source
            .split_once(&marker)
            .ok_or_else(|| format!("missing {name}"))?;
        let end = tail
            .find("\n\n")
            .ok_or_else(|| format!("missing {name} terminator"))?;
        shapes.push(shape(&format!("{name}:\n{}", &tail[..end]))?);
    }
    let mut maybe = vec![false; shapes.len()];
    for (i, shape) in shapes.iter_mut().enumerate() {
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
        maybe[i] = mode == "maybe";
    }

    let (_, receipt) = source
        .split_once("\non clear-host-effect ")
        .ok_or("missing clear-host-effect receipt")?;
    let receipt = format!(
        "on clear-host-effect {}",
        receipt.split("\n\n").next().unwrap()
    );
    let mut clauses = Vec::new();
    for block in blocks
        .iter()
        .copied()
        .filter(|block| block.starts_with("on "))
        .chain(std::iter::once(receipt.as_str()))
    {
        let rows: Vec<&str> = block.lines().collect();
        let mut clause = Clause {
            head: rows[0],
            handler: rows[0].split_whitespace().nth(1).unwrap(),
            conditions: Vec::new(),
            withdrawals: Vec::new(),
            includes: Vec::new(),
            creates: Vec::new(),
        };
        let mut section = "";
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
                "when" => clause.conditions.push(text),
                "withdraw" => clause.withdrawals.push(
                    fact(text, &shapes).ok_or_else(|| format!("unknown withdrawal {text}"))?,
                ),
                "include" => clause.includes.push(
                    fact(text, &shapes).ok_or_else(|| format!("unknown inclusion {text}"))?,
                ),
                "create" => {
                    if rows.get(at + 1).map(|s| s.trim()) != Some("member of: MenuRow") {
                        return Err("unsupported menu creation".into());
                    }
                    clause.creates.push(text.to_string());
                    at += 1;
                }
                _ => return Err(format!("unsupported menu section {section}")),
            }
            at += 1;
        }
        clauses.push(clause);
    }

    let row_fields: Vec<usize> = (0..shapes.len())
        .filter(|&i| shapes[i].functional && shapes[i].key_type == "MenuRow")
        .collect();
    let mut creation = vec![false; shapes.len()];
    for &i in &row_fields {
        let mut created = false;
        let mut stated = false;
        let mut every_site = true;
        for clause in &clauses {
            let writes = clause.includes.iter().filter(|f| f.shape == i);
            let mut sets = false;
            for write in writes {
                if clause.creates.contains(&write.key) {
                    created = true;
                    sets = true;
                } else {
                    stated = true;
                }
            }
            if !clause.creates.is_empty() && !sets {
                every_site = false;
            }
        }
        creation[i] = created && !stated;
        if !every_site {
            maybe[i] = true;
        }
    }
    let field_type = |i: usize| {
        if maybe[i] {
            format!("maybe {}", shapes[i].value_type)
        } else {
            shapes[i].value_type.clone()
        }
    };

    let types = ["North", "MenuRow"];
    let mut out = String::from("type North\nnorth-main is a North\n\nstate entity MenuRow\n");
    for &i in row_fields.iter().filter(|&&i| creation[i]) {
        out.push_str(&format!("  {}: {}\n", field_name(&shapes[i]), field_type(i)));
    }
    for ty in types {
        out.push_str(&format!("\nstate for {ty}\n"));
        for (i, shape) in shapes.iter().enumerate() {
            if shape.functional && shape.key_type == ty && !creation[i] {
                out.push_str(&format!("  {}: {}\n", field_name(shape), field_type(i)));
            }
        }
    }
    out.push('\n');
    for shape in shapes.iter().filter(|s| !s.functional) {
        let name = if shape.words.replace(' ', "-") == shape.name {
            String::new()
        } else {
            format!("{}: ", shape.name)
        };
        out.push_str(&format!(
            "state {name}{} {} {}\n",
            slot(&shape.key, &shape.key_type, &types),
            shape.words,
            slot("value", &shape.value_type, &types)
        ));
    }
    out.push_str("\neffect menu-host-effect: North requests host effect ?effect: Text with payload ?payload: Text\n  at most one per step\n  settled by clear-host-effect\n");
    out.push_str("\ninitially\n");
    for line in source
        .lines()
        .filter(|line| line.starts_with("north-main "))
    {
        if let Some(initial) = fact(line, &shapes) {
            let shape = &shapes[initial.shape];
            let value = expression(&initial.value)?;
            if shape.functional {
                out.push_str(&format!("  north-main.{} = {value}\n", field_name(shape)));
            } else {
                out.push_str(&format!("  north-main {} {}\n", shape.words, slot_value(value)));
            }
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
        ("clear-host-effect", vec![("north", "North")]),
    ];
    for (name, slots) in signatures {
        out.push_str(&format!("\nevent {name}"));
        for (name, ty) in slots {
            out.push_str(&format!(" {}", slot(name, ty, &types)));
        }
        out.push('\n');
    }

    let mut count = 0;
    let mut maintenance = 0;
    let mut unpaired = Vec::new();
    for clause in &clauses {
        let handler = clause.handler;
        if handler != "clear-host-effect" {
            count += 1;
        }
        if matches!(handler, "filter-menu" | "count-menu") {
            maintenance += 1;
        }
        out.push_str(&format!("\n{}\n", clause.head));
        let mut context = Context {
            shapes: &shapes,
            head: clause.head.split_whitespace().skip(2).collect(),
            map: BTreeMap::new(),
            bound: Default::default(),
        };
        for text in &clause.conditions {
            context.collect(text);
        }
        let mut conditions = Vec::new();
        for text in &clause.conditions {
            conditions.extend(context.condition(text)?);
        }
        for (i, text) in conditions.iter().enumerate() {
            out.push_str(&format!(
                "  {} {text}\n",
                if i == 0 { "when" } else { "and" }
            ));
        }

        let teardown = clause.includes.is_empty() && clause.creates.is_empty();
        let torn: Vec<&String> = clause
            .withdrawals
            .iter()
            .map(|f| &f.key)
            .filter(|key| {
                teardown
                    && row_fields.iter().all(|&i| {
                        clause
                            .withdrawals
                            .iter()
                            .any(|f| f.shape == i && f.key == **key)
                    })
            })
            .fold(Vec::new(), |mut keys, key| {
                if !keys.contains(&key) {
                    keys.push(key);
                }
                keys
            });

        let mut posts = Vec::new();
        let mut records: BTreeMap<&str, Vec<(usize, String)>> = BTreeMap::new();
        let mut effect = None;
        let mut payload = None;
        for include in &clause.includes {
            let shape = &shapes[include.shape];
            let value = context.render(&include.value)?;
            let withdrawn = clause
                .withdrawals
                .iter()
                .any(|f| f.shape == include.shape && f.key == include.key);
            if shape.functional && clause.creates.contains(&include.key) {
                records
                    .entry(&include.key)
                    .or_default()
                    .push((include.shape, value.clone()));
            } else if shape.functional {
                if !withdrawn {
                    unpaired.push(format!("{handler} clause {count}: {}", shape.name));
                }
                posts.push(format!("{}.{} = {value}", include.key, field_name(shape)));
            } else {
                posts.push(format!(
                    "{} {} {}",
                    include.key,
                    shape.words,
                    slot_value(value.clone())
                ));
            }
            if shape.name == "host-effect" {
                effect = Some((include.key.clone(), slot_value(value)));
            } else if shape.name == "effect-payload" {
                payload = Some((include.key.clone(), slot_value(value)));
            }
        }
        if handler != "clear-host-effect" {
            match (effect, payload) {
                (Some((north, effect)), Some((payload_north, payload)))
                    if north == payload_north =>
                {
                    posts.push(format!(
                        "{north} requests host effect {effect} with payload {payload}"
                    ));
                }
                (None, None) => (),
                _ => return Err(format!("{handler}: incomplete host-effect mailbox write")),
            }
        }
        for key in &clause.creates {
            let mut fields = records.remove(key.as_str()).unwrap_or_default();
            fields.sort_by_key(|(shape, _)| *shape);
            let fields: Vec<String> = fields
                .into_iter()
                .map(|(shape, value)| format!("{}: {value}", field_name(&shapes[shape])))
                .collect();
            posts.push(if fields.is_empty() {
                format!("{key} is a new MenuRow")
            } else {
                format!("{key} is a new MenuRow {{{}}}", fields.join(", "))
            });
        }
        for key in &torn {
            posts.push(format!("not {key} is a MenuRow"));
        }
        for withdrawal in &clause.withdrawals {
            let shape = &shapes[withdrawal.shape];
            if torn.contains(&&withdrawal.key) || torn.contains(&&withdrawal.value) {
                continue;
            }
            if shape.functional {
                if clause
                    .includes
                    .iter()
                    .any(|f| f.shape == withdrawal.shape && f.key == withdrawal.key)
                {
                    continue;
                }
                return Err(format!("{handler}: withdrawal of {} without a new value", shape.name));
            }
            posts.push(format!(
                "not {} {} {}",
                withdrawal.key,
                shape.words,
                slot_value(context.render(&withdrawal.value)?)
            ));
        }
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
        clauses: count,
        maintenance,
        unpaired,
    })
}
