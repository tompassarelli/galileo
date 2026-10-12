use std::collections::{BTreeMap, BTreeSet};

pub struct Import {
    pub source: String,
    pub clauses: usize,
    pub maintenance: usize,
    pub unpaired: Vec<String>,
}

const MAINTENANCE: [&str; 8] = [
    "filter-menu",
    "count-menu",
    "count-usage",
    "present-usage",
    "filter-transcript",
    "focus-prompts",
    "present-prompt-questions",
    "clear-resolved-answers",
];

const BUILTIN: [&str; 4] = ["Text", "Bool", "F64", "Int"];

struct Shape {
    name: String,
    line: usize,
    key: String,
    key_type: String,
    words: String,
    value: String,
    value_type: String,
    functional: bool,
    maybe: bool,
}

struct Item<'a> {
    line: usize,
    rows: Vec<&'a str>,
}

fn items(source: &str) -> Vec<Item<'_>> {
    let mut out: Vec<Item> = Vec::new();
    let mut open = false;
    for (at, row) in source.lines().enumerate() {
        if row.trim().is_empty() {
            open = false;
        } else if row.starts_with(' ') && (open || !out.is_empty()) {
            out.last_mut().unwrap().rows.push(row);
        } else {
            open = true;
            out.push(Item { line: at + 1, rows: vec![row] });
        }
    }
    out
}

struct Template {
    name: String,
    line: usize,
    slots: Vec<(String, String)>,
    sentence: String,
    given: Vec<String>,
    yields: String,
    define: Option<String>,
}

enum Relation {
    Keyed(Shape),
    Template(Template),
}

fn relation(item: &Item, modes: &BTreeMap<String, String>) -> Option<Result<Relation, String>> {
    let name = item.rows[0].strip_suffix(':')?;
    if item.rows.get(1) != Some(&"  ?example:") || name.contains(' ') {
        return None;
    }
    Some((|| {
        let mut slots = BTreeMap::new();
        let mut ordered = Vec::new();
        let mut sentence = None;
        let mut keyed = None;
        let mut at = 2;
        while at < item.rows.len() {
            let row = item.rows[at].trim();
            if let Some(key) = row.strip_prefix('?').and_then(|r| r.strip_suffix(':')) {
                let (words, value) = item
                    .rows
                    .get(at + 1)
                    .and_then(|r| r.trim().split_once(": ?"))
                    .ok_or_else(|| format!("{name}: unsupported relation template"))?;
                keyed = Some((key.to_string(), words.to_string(), value.to_string()));
                at += 2;
                continue;
            }
            match row.split_once(": ") {
                Some((slot, ty)) if !slot.contains([' ', '?']) && sentence.is_none() => {
                    slots.insert(slot.to_string(), ty.to_string());
                    ordered.push((slot.to_string(), ty.replace("F64", "Int")));
                }
                _ if sentence.is_none() && keyed.is_none() => sentence = Some(row.to_string()),
                _ => return Err(format!("{name}: unsupported relation template")),
            }
            at += 1;
        }
        if let Some(sentence) = sentence {
            return Ok(Relation::Template(Template {
                name: name.to_string(),
                line: item.line,
                slots: ordered,
                sentence,
                given: Vec::new(),
                yields: String::new(),
                define: None,
            }));
        }
        let (key, words, value) = keyed.ok_or_else(|| format!("{name}: unsupported relation"))?;
        let mode = modes
            .get(name)
            .ok_or_else(|| format!("missing {name} mode"))?;
        let mode = mode
            .strip_prefix(&format!("{key} yields {value}: "))
            .ok_or_else(|| format!("unsupported {name} mode {mode}"))?;
        let functional = match mode {
            "one" | "maybe" => true,
            "many" => false,
            _ => return Err(format!("unsupported {name} mode {mode}")),
        };
        let slot = |s: &str| slots.get(s).cloned().ok_or_else(|| format!("{name}: untyped slot {s}"));
        Ok(Relation::Keyed(Shape {
            name: name.to_string(),
            line: item.line,
            key_type: slot(&key)?,
            value_type: slot(&value)?.replace("F64", "Int"),
            key,
            words,
            value,
            functional,
            maybe: mode == "maybe",
        }))
    })())
}

fn partition(slices: &str, total: usize) -> Result<(Vec<String>, Vec<usize>), String> {
    let mut modules = Vec::new();
    let mut owner = vec![usize::MAX; total + 1];
    for row in slices.lines().filter(|r| !r.trim().is_empty()) {
        let mut words = row.split_whitespace();
        let module = words.next().unwrap().to_string();
        let index = modules.len();
        for range in words {
            let bounds = range
                .strip_prefix('L')
                .ok_or_else(|| format!("{module}: bad range {range}"))?;
            let (from, to) = bounds.split_once('-').unwrap_or((bounds, bounds));
            let parse = |n: &str| n.parse::<usize>().map_err(|_| format!("{module}: bad range {range}"));
            for line in parse(from)?..=parse(to)? {
                let slot = owner
                    .get_mut(line)
                    .filter(|_| line > 0)
                    .ok_or_else(|| format!("{module}: line {line} is past the end of the file"))?;
                if *slot != usize::MAX {
                    return Err(format!("line {line} is in both {} and {module}", modules[*slot]));
                }
                *slot = index;
            }
        }
        modules.push(module);
    }
    if let Some(line) = (1..=total).find(|&line| owner[line] == usize::MAX) {
        return Err(format!("line {line} is in no slice"));
    }
    Ok((modules, owner))
}

fn split<'t>(text: &'t str, delimiter: &str) -> Vec<&'t str> {
    let mut quoted = false;
    let mut escaped = false;
    let mut depth = 0;
    let mut start = 0;
    let mut parts = Vec::new();
    for (at, c) in text.char_indices() {
        if at < start {
            continue;
        }
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
            _ if depth == 0 && text[at..].starts_with(delimiter) => {
                parts.push(text[start..at].trim());
                start = at + delimiter.len();
            }
            _ => (),
        }
    }
    parts.push(text[start..].trim());
    parts
}

fn expression(text: &str) -> Result<String, String> {
    chain(text.trim(), true)
}

fn chain(text: &str, bare: bool) -> Result<String, String> {
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
            let args = split(&inner, ",");
            let [condition, yes, no] = args.as_slice() else {
                return Err("Clause if needs three arguments".into());
            };
            let condition = condition.strip_suffix(" = true").unwrap_or(condition);
            let before = chars[..at].iter().rev().find(|c| !c.is_whitespace());
            let after = chars[end + 1..].iter().find(|c| !c.is_whitespace());
            let argument = matches!(before, Some('(' | ',')) && matches!(after, Some(')' | ','));
            let whole = argument || (bare && at == 0 && end + 1 == chars.len());
            let inner = if *yes == "true" {
                format!("{} or {}", chain(condition, false)?, chain(no, true)?)
            } else {
                format!(
                    "if {} then {} else {}",
                    chain(condition, false)?,
                    chain(yes, false)?,
                    chain(no, true)?
                )
            };
            result.push_str(&if whole { inner } else { format!("({inner})") });
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
                return Err("nonintegral Clause number".into());
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
    shapes
        .iter()
        .enumerate()
        .filter_map(|(shape, s)| {
            rest.strip_prefix(&format!("{} ", s.words)).map(|value| Fact {
                shape,
                key: key.into(),
                value: value.into(),
            })
        })
        .max_by_key(|f| shapes[f.shape].words.len())
}

fn term(text: &str) -> &str {
    if text.starts_with('"') {
        let mut escaped = false;
        for (at, c) in text.char_indices().skip(1) {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                return &text[..at + 1];
            }
        }
        text
    } else {
        text.split(' ').next().unwrap_or_default()
    }
}

fn instance(text: &str, sentence: &str) -> Option<BTreeMap<String, String>> {
    let words: Vec<&str> = sentence.split(' ').collect();
    let mut rest = text.trim();
    let mut out = BTreeMap::new();
    for (at, word) in words.iter().enumerate() {
        if at > 0 {
            rest = rest.strip_prefix(' ')?;
        }
        if let Some(slot) = word.strip_prefix('?') {
            let value = if at + 1 == words.len() { rest } else { term(rest) };
            if value.is_empty() {
                return None;
            }
            out.insert(slot.to_string(), value.to_string());
            rest = &rest[value.len()..];
        } else {
            rest = rest.strip_prefix(word)?;
        }
    }
    rest.is_empty().then_some(out)
}

fn template_read(text: &str, templates: &[Template]) -> Option<(usize, BTreeMap<String, String>)> {
    templates.iter().enumerate().find_map(|(t, template)| {
        let binding = instance(text, &template.sentence)?;
        binding
            .values()
            .all(|value| term(value) == value.as_str())
            .then_some((t, binding))
    })
}

fn template_reads(text: &str, templates: &[Template], out: &mut Vec<(usize, BTreeMap<String, String>)>) {
    if let Some((_, tail)) = text.split_once(" where { ") {
        if let Some((body, _)) = tail.rsplit_once(" } as ") {
            for part in split(body, ";") {
                template_reads(part, templates, out);
            }
        }
        return;
    }
    out.extend(template_read(text, templates));
}

fn law(item: &Item, templates: &[Template], derived: &BTreeSet<&str>) -> Result<(usize, String), String> {
    let name = item.rows[0].strip_prefix("law ").unwrap_or_default();
    let unsupported = || format!("unsupported law {name}");
    let rows: Vec<&str> = item.rows.iter().map(|r| r.trim()).collect();
    let then = rows.iter().position(|r| *r == "then").ok_or_else(unsupported)?;
    if rows.get(1) != Some(&"if") || then + 2 != rows.len() || !derived.contains(name) {
        return Err(unsupported());
    }
    let mut bound = BTreeSet::new();
    for row in &rows[2..then] {
        match row.split_once(" = ") {
            Some((a, b)) if a == b && a.starts_with('?') && tokens(a).len() == 1 => {
                bound.insert(a[1..].to_string());
            }
            _ => return Err(unsupported()),
        }
    }
    let (t, binding) = templates
        .iter()
        .enumerate()
        .find_map(|(t, template)| Some((t, instance(rows[then + 1], &template.sentence)?)))
        .ok_or_else(unsupported)?;
    let template = &templates[t];
    let given: BTreeSet<String> = template.given.iter().cloned().collect();
    let last = template.sentence.rsplit(' ').next() == Some(&format!("?{}", template.yields));
    if template.yields.is_empty()
        || !last
        || bound != given
        || template.given.iter().any(|g| binding.get(g) != Some(&format!("?{g}")))
    {
        return Err(unsupported());
    }
    let map: BTreeMap<String, String> = template.given.iter().map(|g| (format!("?{g}"), g.clone())).collect();
    let body = membership(interpolate(&substitute(&expression(&binding[&template.yields])?, &map)));
    let mut cases: Vec<String> = split(&body, " else if ").into_iter().map(str::to_string).collect();
    if cases.len() > 1 {
        let tail = cases.pop().unwrap();
        match split(&tail, " else ").as_slice() {
            [case, otherwise] => cases.extend([case.to_string(), format!("else {otherwise}")]),
            _ => cases.push(tail.clone()),
        }
    }
    let body = cases
        .iter()
        .enumerate()
        .map(|(at, case)| match at {
            0 => case.clone(),
            _ if case.starts_with("else ") => case.clone(),
            _ => format!("else if {case}"),
        })
        .collect::<Vec<_>>()
        .join("\n  ");
    let slot_type = |slot: &str| {
        template
            .slots
            .iter()
            .find(|(s, _)| s == slot)
            .map(|(_, ty)| ty.clone())
            .ok_or_else(unsupported)
    };
    let params = template
        .given
        .iter()
        .map(|g| Ok(format!("{g}: {}", slot_type(g)?)))
        .collect::<Result<Vec<_>, String>>()?;
    Ok((
        t,
        format!(
            "define {}({}): {}\n  {body}\n",
            template.name,
            params.join(", "),
            slot_type(&template.yields)?
        ),
    ))
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

fn tokens(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let start = at;
        if chars[at] == '"' {
            at += 1;
            while at < chars.len() && chars[at] != '"' {
                at += if chars[at] == '\\' { 2 } else { 1 };
            }
            at += 1;
        } else if chars[at] == '?' {
            at += 1;
            while at < chars.len() && (chars[at].is_ascii_alphanumeric() || matches!(chars[at], '-' | '.')) {
                at += 1;
            }
        } else {
            at += 1;
        }
        out.push(chars[start..at.min(chars.len())].iter().collect());
    }
    out
}

fn interpolate(text: &str) -> String {
    let tokens = tokens(text);
    let atom = |t: &String| {
        t.starts_with('?') || t.len() > 1 && t.starts_with('"') && !t.contains(['{', '}'])
    };
    let joint = |at: usize| {
        tokens.get(at..at + 4).is_some_and(|t| t.concat() == " ++ ")
            && tokens.get(at + 4).is_some_and(atom)
    };
    let mut out = String::new();
    let mut at = 0;
    while at < tokens.len() {
        if atom(&tokens[at]) && joint(at + 1) {
            let mut parts = vec![&tokens[at]];
            at += 1;
            while joint(at) {
                parts.push(&tokens[at + 4]);
                at += 5;
            }
            out.push('"');
            for part in parts {
                match part.strip_prefix('"') {
                    Some(inner) => out.push_str(&inner[..inner.len() - 1]),
                    None => out.push_str(&format!("{{{part}}}")),
                }
            }
            out.push('"');
        } else {
            out.push_str(&tokens[at]);
            at += 1;
        }
    }
    out
}

fn membership(text: String) -> String {
    let parts = split(&text, " or ");
    let tests: Vec<_> = parts
        .iter()
        .filter_map(|part| part.split_once(" == "))
        .filter(|(_, literal)| tokens(literal).len() == 1 && literal.starts_with('"'))
        .collect();
    if parts.len() > 1 && tests.len() == parts.len() && tests.iter().all(|(e, _)| *e == tests[0].0) {
        let literals: Vec<_> = tests.iter().map(|(_, literal)| *literal).collect();
        format!("{} in [{}]", tests[0].0, literals.join(", "))
    } else {
        text
    }
}

fn mentions(text: &str, variable: &str) -> bool {
    tokens(text).iter().any(|t| t == variable || t.strip_prefix(variable).is_some_and(|f| f.starts_with('.')))
}

fn slot_value(value: String) -> String {
    if value.starts_with('"') && value.ends_with('"') && split(&value, "+").len() == 1
        || !value.contains(' ')
        || value.starts_with('(') && value.ends_with(')')
    {
        value
    } else {
        format!("({value})")
    }
}

fn slot(name: &str, ty: &str, types: &[String]) -> String {
    if types.iter().any(|t| t == ty) && kebab(ty) == name {
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
    creates: Vec<(String, String)>,
    line: usize,
}

struct Context<'a> {
    shapes: &'a [Shape],
    templates: &'a [Template],
    calls: BTreeSet<usize>,
    head: Vec<&'a str>,
    map: BTreeMap<String, String>,
    bound: BTreeSet<String>,
    inline: BTreeSet<String>,
    used: BTreeSet<usize>,
    grounded: BTreeSet<String>,
    declared: BTreeSet<String>,
    created: BTreeSet<String>,
}

impl Context<'_> {
    fn render(&self, text: &str) -> Result<String, String> {
        Ok(membership(interpolate(&substitute(&expression(text)?, &self.map))))
    }

    fn call(&self, t: usize, binding: &BTreeMap<String, String>) -> String {
        let template = &self.templates[t];
        let args: Vec<String> = template.given.iter().map(|g| substitute(&binding[g], &self.map)).collect();
        format!("{}({})", template.name, args.join(", "))
    }

    fn collect(&mut self, text: &str) {
        if let Some((_, tail)) = text.split_once(" where { ") {
            if let Some((body, _)) = tail.rsplit_once(" } as ") {
                for part in split(body, ";") {
                    self.collect(part);
                }
            }
            return;
        }
        if let Some((t, binding)) = template_read(text, self.templates) {
            self.calls.insert(t);
            let value = &binding[&self.templates[t].yields];
            if value.starts_with('?') && !self.head.contains(&value.as_str()) && !self.map.contains_key(value) {
                self.map.insert(value.clone(), self.call(t, &binding));
            }
            return;
        }
        let Some(read) = fact(text, self.shapes) else {
            return;
        };
        self.used.insert(read.shape);
        let shape = &self.shapes[read.shape];
        if !shape.functional {
            self.grounded.insert(read.key.clone());
            self.grounded.insert(read.value.clone());
        }
        if shape.functional
            && read.value.starts_with('?')
            && !self.head.contains(&read.value.as_str())
            && !self.map.contains_key(&read.value)
        {
            self.map
                .insert(read.value, format!("{}.{}", read.key, field_name(shape)));
        }
    }

    fn literals(&mut self, text: &str) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        if let Some(read) = fact(text, self.shapes) {
            let shape = &self.shapes[read.shape];
            if shape.functional
                && read.key.starts_with('?')
                && !self.head.contains(&read.key.as_str())
                && !self.grounded.contains(&read.key)
                && !self.map.contains_key(&read.key)
                && self.declared.insert(read.key.clone())
            {
                out.push(format!("{} is a {}", read.key, shape.key_type));
            }
        }
        out.extend(self.condition(text)?);
        Ok(out)
    }

    fn condition(&mut self, text: &str) -> Result<Option<String>, String> {
        if text.starts_with("sum 1.0 given ") {
            let (head, tail) = text.split_once(" where { ").ok_or("unsupported sum")?;
            let (body, variable) = tail
                .rsplit_once(" } as ")
                .ok_or("unsupported sum binding")?;
            let fresh: BTreeMap<String, String> = self
                .created
                .iter()
                .map(|v| (v.clone(), format!("?existing-{}", &v[1..])))
                .collect();
            let body = substitute(body, &fresh);
            let body = body.as_str();
            let given: Vec<&str> = head["sum 1.0 given ".len()..].split(' ').collect();
            let reads: Vec<Fact> = split(body, ";").into_iter().filter_map(|part| fact(part, self.shapes)).collect();
            let mut parts: Vec<(String, bool)> = Vec::new();
            for read in &reads {
                let shape = &self.shapes[read.shape];
                let grounded = given.contains(&read.key.as_str())
                    || reads.iter().any(|other| !self.shapes[other.shape].functional && (other.key == read.key || other.value == read.key));
                if shape.functional && read.key.starts_with('?') && !grounded && !parts.iter().any(|(text, _)| text.starts_with(&format!("{} is a ", read.key))) {
                    parts.push((format!("{} is a {}", read.key, shape.key_type), true));
                }
            }
            for part in split(body, ";") {
                let sentence = fact(part, self.shapes).is_some_and(|read| !self.shapes[read.shape].functional);
                parts.extend(self.condition(part)?.map(|text| (text, sentence)));
            }
            let mut joined = Vec::new();
            let mut at = 0;
            while at < parts.len() {
                let run = parts[at..].iter().take_while(|(_, sentence)| !sentence).count();
                let last = at + run == parts.len();
                if run >= 2 && (!last || run >= 3) {
                    let group: Vec<&str> = parts[at..at + run].iter().map(|(text, _)| text.as_str()).collect();
                    joined.push(format!("({})", group.join(" and ")));
                    at += run;
                } else {
                    joined.push(parts[at].0.clone());
                    at += 1;
                }
            }
            let aggregate = format!("count where {}", joined.join(" and "));
            if self.inline.contains(variable) {
                self.map.insert(variable.to_string(), aggregate);
                return Ok(None);
            }
            return Ok(Some(format!("{variable} = {aggregate}")));
        }
        if let Some((t, binding)) = template_read(text, self.templates) {
            self.calls.insert(t);
            let call = self.call(t, &binding);
            let value = &binding[&self.templates[t].yields];
            if self.map.get(value) == Some(&call) && self.bound.insert(value.clone()) {
                return Ok(None);
            }
            if value == "true" {
                return Ok(Some(call));
            }
            return Ok(Some(format!("{call} == {}", self.render(value)?)));
        }
        if let Some(test) = text.strip_suffix(" = true") {
            return self.render(test).map(Some);
        }
        let Some(read) = fact(text, self.shapes) else {
            return self.render(text).map(Some);
        };
        self.used.insert(read.shape);
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
        if read.value == "true" {
            return Ok(Some(field));
        }
        Ok(Some(format!("{field} == {}", self.render(&read.value)?)))
    }
}


const TEXT_FUNCTIONS: [(&str, &str); 6] = [
    ("contains-text", "Bool"),
    ("starts-with", "Bool"),
    ("first-word", "Text"),
    ("remaining-words", "Text"),
    ("lowercase", "Text"),
    ("trim", "Text"),
];

fn lexemes(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        let start = at;
        if c.is_whitespace() {
            at += 1;
            continue;
        }
        if c == '"' {
            at += 1;
            while at < chars.len() && chars[at] != '"' {
                at += if chars[at] == '\\' { 2 } else { 1 };
            }
            at += 1;
        } else if c == '?' || c.is_ascii_alphabetic() {
            at += 1;
            while at < chars.len() && (chars[at].is_ascii_alphanumeric() || chars[at] == '-') {
                at += 1;
            }
        } else if c.is_ascii_digit() {
            while at < chars.len() && (chars[at].is_ascii_digit() || chars[at] == '.') {
                at += 1;
            }
        } else if matches!(c, '<' | '>' | '!') && chars.get(at + 1) == Some(&'=')
            || c == '+' && chars.get(at + 1) == Some(&'+')
        {
            at += 2;
        } else {
            at += 1;
        }
        out.push(chars[start..at.min(chars.len())].iter().collect());
    }
    out
}

fn operand_type(lex: &[String], at: usize, types: &BTreeMap<String, String>) -> Option<String> {
    let token = lex.get(at)?;
    if lex.get(at + 1).is_some_and(|t| t == "(") {
        return TEXT_FUNCTIONS
            .iter()
            .find(|(name, _)| name == token)
            .map(|(_, ty)| ty.to_string());
    }
    if token.starts_with('"') {
        Some("Text".into())
    } else if token == "true" || token == "false" {
        Some("Bool".into())
    } else if token.starts_with(|c: char| c.is_ascii_digit()) {
        Some("Int".into())
    } else {
        types.get(token).cloned()
    }
}

fn compared(text: &str, variable: &str, types: &BTreeMap<String, String>) -> Option<String> {
    let lex = lexemes(text);
    if lex.len() == 1 && lex[0] == variable {
        return Some("Bool".into());
    }
    let operator = |t: &str| matches!(t, "=" | "!=" | "<" | ">" | "<=" | ">=" | "+" | "++" | "-" | "*" | "/");
    for at in (0..lex.len()).filter(|&at| lex[at] == variable) {
        if lex.get(at + 1).is_some_and(|t| operator(t)) {
            if let Some(ty) = operand_type(&lex, at + 2, types) {
                return Some(ty);
            }
        }
        if at >= 2 && operator(&lex[at - 1]) {
            let mut from = at - 2;
            if lex[from] == ")" {
                let mut depth = 0;
                while from > 0 {
                    depth += match lex[from].as_str() {
                        ")" => 1,
                        "(" => -1,
                        _ => 0,
                    };
                    if depth == 0 {
                        break;
                    }
                    from -= 1;
                }
                from = from.saturating_sub(1);
            }
            if let Some(ty) = operand_type(&lex, from, types) {
                return Some(ty);
            }
        }
        let mut depth = 0;
        let mut argument = 0;
        let mut open = None;
        for back in (0..at).rev() {
            match lex[back].as_str() {
                ")" => depth += 1,
                "(" if depth == 0 => {
                    open = Some(back);
                    break;
                }
                "(" => depth -= 1,
                "," if depth == 0 => argument += 1,
                _ => (),
            }
        }
        let Some(name) = open.and_then(|open| open.checked_sub(1)).map(|at| lex[at].as_str()) else {
            continue;
        };
        if TEXT_FUNCTIONS.iter().any(|(f, _)| *f == name) {
            return Some("Text".into());
        }
        if name == "if" && argument == 0 && lex.get(at + 1).is_some_and(|t| t == ",") {
            return Some("Bool".into());
        }
    }
    None
}

struct Event {
    line: usize,
    slots: Vec<(String, Option<String>)>,
}

fn reads<'t>(text: &'t str, shapes: &[Shape], out: &mut Vec<(&'t str, Fact)>) {
    if let Some((_, tail)) = text.split_once(" where { ") {
        if let Some((body, _)) = tail.rsplit_once(" } as ") {
            for part in split(body, ";") {
                reads(part, shapes, out);
            }
        }
        return;
    }
    if let Some(read) = fact(text, shapes) {
        out.push((text, read));
    }
}

fn infer(clauses: &[Clause], shapes: &[Shape], templates: &[Template]) -> Result<BTreeMap<String, Event>, String> {
    let mut events: BTreeMap<String, Event> = BTreeMap::new();
    for clause in clauses {
        let args: Vec<&str> = clause.head.split_whitespace().skip(2).collect();
        let event = events.entry(clause.handler.to_string()).or_insert_with(|| Event {
            line: clause.line,
            slots: args
                .iter()
                .enumerate()
                .map(|(at, a)| (a.trim_start_matches('?').to_string(), (at == 0).then(|| "North".to_string())))
                .collect(),
        });
        if event.slots.len() != args.len() {
            return Err(format!("line {}: {} takes {} arguments at line {}", clause.line, clause.handler, event.slots.len(), event.line));
        }
    }
    let texts = |clause: &'_ Clause<'_>| -> Vec<String> {
        let mut texts: Vec<String> = clause.conditions.iter().map(|t| t.to_string()).collect();
        for f in clause.includes.iter().chain(&clause.withdrawals) {
            texts.push(f.value.clone());
        }
        texts
    };
    let filled = |clause: &Clause, variable: &str| -> Option<String> {
        let mut found = Vec::new();
        for text in &clause.conditions {
            reads(text, shapes, &mut found);
        }
        for f in clause.includes.iter().chain(&clause.withdrawals) {
            if f.key == variable {
                return Some(shapes[f.shape].key_type.clone());
            }
            if f.value == variable {
                return Some(shapes[f.shape].value_type.clone());
            }
        }
        let mut calls = Vec::new();
        for text in &clause.conditions {
            template_reads(text, templates, &mut calls);
        }
        found
            .iter()
            .find_map(|(_, f)| {
                if f.key == variable {
                    Some(shapes[f.shape].key_type.clone())
                } else if f.value == variable {
                    Some(shapes[f.shape].value_type.clone())
                } else {
                    None
                }
            })
            .or_else(|| {
                calls.iter().find_map(|(t, binding)| {
                    templates[*t]
                        .slots
                        .iter()
                        .find(|(slot, _)| binding.get(slot).is_some_and(|v| v == variable))
                        .map(|(_, ty)| ty.clone())
                })
            })
    };
    for clause in clauses {
        let event = events.get_mut(clause.handler).unwrap();
        for (at, variable) in clause.head.split_whitespace().skip(2).enumerate() {
            if event.slots[at].1.is_none() {
                event.slots[at].1 = filled(clause, variable);
            }
        }
    }
    loop {
        let mut changed = false;
        for clause in clauses {
            let args: Vec<&str> = clause.head.split_whitespace().skip(2).collect();
            let mut types: BTreeMap<String, String> = BTreeMap::new();
            let mut found = Vec::new();
            for text in &clause.conditions {
                reads(text, shapes, &mut found);
            }
            for f in found.iter().map(|(_, f)| f).chain(clause.includes.iter()).chain(&clause.withdrawals) {
                let shape = &shapes[f.shape];
                types.entry(f.key.clone()).or_insert(shape.key_type.clone());
                if f.value.starts_with('?') && lexemes(&f.value).len() == 1 {
                    types.entry(f.value.clone()).or_insert(shape.value_type.clone());
                }
            }
            let mut calls = Vec::new();
            for text in &clause.conditions {
                template_reads(text, templates, &mut calls);
            }
            for (t, binding) in &calls {
                for (slot, ty) in &templates[*t].slots {
                    if binding[slot].starts_with('?') {
                        types.entry(binding[slot].clone()).or_insert(ty.clone());
                    }
                }
            }
            let event = events.get_mut(clause.handler).unwrap();
            for (at, variable) in args.iter().enumerate() {
                if let Some(ty) = &event.slots[at].1 {
                    types.insert(variable.to_string(), ty.clone());
                }
            }
            for (at, variable) in args.iter().enumerate() {
                if event.slots[at].1.is_some() {
                    continue;
                }
                if let Some(ty) = texts(clause).iter().find_map(|text| compared(text, variable, &types)) {
                    event.slots[at].1 = Some(ty.clone());
                    types.insert(variable.to_string(), ty);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let read = |clause: &Clause, variable: &str| -> bool {
        let mut rows: Vec<String> = clause.conditions.iter().map(|t| t.to_string()).collect();
        for f in clause.includes.iter().chain(&clause.withdrawals) {
            rows.extend([f.key.clone(), f.value.clone()]);
        }
        rows.extend(clause.creates.iter().map(|(text, _)| text.clone()));
        rows.iter().any(|row| lexemes(row).iter().any(|l| l == variable))
    };
    let mut unread: BTreeMap<(String, usize), bool> = BTreeMap::new();
    for clause in clauses {
        for (at, variable) in clause.head.split_whitespace().skip(2).enumerate() {
            *unread.entry((clause.handler.to_string(), at)).or_insert(true) &= !read(clause, variable);
        }
    }
    for ((handler, at), unread) in unread {
        let slot = &mut events.get_mut(&handler).unwrap().slots[at];
        if unread && slot.1.is_none() {
            slot.1 = Some("Text".to_string());
        }
    }
    Ok(events)
}

fn declaration<'a>(item: &Item<'a>) -> Option<(&'a str, &'a str)> {
    match item.rows.as_slice() {
        [name, member] if !name.contains(' ') && !name.starts_with(' ') => {
            Some((name, member.trim().strip_prefix("member of: ")?))
        }
        _ => None,
    }
}

fn parse_clause<'a>(item: &Item<'a>, shapes: &[Shape]) -> Result<Clause<'a>, String> {
    let rows = &item.rows;
    let mut clause = Clause {
        head: rows[0],
        handler: rows[0].split_whitespace().nth(1).ok_or("unsupported clause head")?,
        conditions: Vec::new(),
        withdrawals: Vec::new(),
        includes: Vec::new(),
        creates: Vec::new(),
        line: item.line,
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
            "withdraw" => clause
                .withdrawals
                .push(fact(text, shapes).ok_or_else(|| format!("unknown withdrawal {text}"))?),
            "include" => clause
                .includes
                .push(fact(text, shapes).ok_or_else(|| format!("unknown inclusion {text}"))?),
            "create" => {
                let ty = rows
                    .get(at + 1)
                    .and_then(|s| s.trim().strip_prefix("member of: "))
                    .ok_or("unsupported creation")?;
                clause.creates.push((text.to_string(), ty.to_string()));
                at += 1;
            }
            _ => return Err(format!("unsupported section {section}")),
        }
        at += 1;
    }
    Ok(clause)
}

pub fn import(source: &str, slices: &str, slice: &str) -> Result<Import, String> {
    let (modules, owner) = partition(slices, source.lines().count())?;
    let me = modules
        .iter()
        .position(|m| m == slice)
        .ok_or_else(|| format!("{slice} is not in the slices file"))?;
    let items = items(source);
    let modes: BTreeMap<String, String> = source
        .lines()
        .filter_map(|line| line.strip_prefix("mode ")?.split_once(" given "))
        .map(|(name, rest)| (name.to_string(), rest.to_string()))
        .collect();
    let mut shapes = Vec::new();
    let mut templates: Vec<Template> = Vec::new();
    let mut broken = BTreeMap::new();
    for item in &items {
        match relation(item, &modes) {
            Some(Ok(Relation::Keyed(shape))) => shapes.push(shape),
            Some(Ok(Relation::Template(mut template))) => {
                if let Some((given, yields)) = modes
                    .get(&template.name)
                    .and_then(|mode| mode.split_once(" yields "))
                    .and_then(|(given, yields)| Some((given, yields.split_once(": ")?.0)))
                {
                    template.given = given.split(' ').map(str::to_string).collect();
                    template.yields = yields.to_string();
                }
                templates.push(template);
            }
            Some(Err(message)) => {
                broken.insert(item.line, message);
            }
            None => (),
        }
    }
    let derived: BTreeSet<&str> = items
        .iter()
        .filter_map(|item| item.rows[0].strip_prefix("derive "))
        .collect();
    let mut laws: BTreeMap<usize, String> = BTreeMap::new();
    let mut law_errors: BTreeMap<usize, String> = BTreeMap::new();
    for item in items.iter().filter(|item| item.rows[0].starts_with("law ")) {
        match law(item, &templates, &derived) {
            Ok((t, define)) if templates[t].define.is_none() => {
                templates[t].define = Some(define);
                laws.insert(item.line, item.rows[0][4..].to_string());
            }
            Ok(_) => {
                law_errors.insert(item.line, format!("unsupported law {}", &item.rows[0][4..]));
            }
            Err(message) => {
                law_errors.insert(item.line, message);
            }
        }
    }

    let mut types: Vec<String> = Vec::new();
    let mut type_owner: BTreeMap<String, usize> = BTreeMap::new();
    let mut created: BTreeSet<String> = BTreeSet::new();
    for item in &items {
        let head = item.rows[0];
        if item.rows.len() == 1 && !head.contains(' ') && head.starts_with(|c: char| c.is_ascii_uppercase()) {
            types.push(head.to_string());
        } else if let Some((_, ty)) = declaration(item) {
            types.push(ty.to_string());
            type_owner.entry(ty.to_string()).or_insert(owner[item.line]);
        } else if head.starts_with("on ") {
            created.extend(
                item.rows
                    .iter()
                    .filter_map(|row| row.trim().strip_prefix("member of: "))
                    .map(str::to_string),
            );
        }
    }
    for shape in &shapes {
        types.push(shape.key_type.clone());
        type_owner.entry(shape.key_type.clone()).or_insert(owner[shape.line]);
    }
    let given: BTreeSet<(String, usize)> = items
        .iter()
        .filter(|item| item.rows.len() == 1)
        .filter_map(|item| fact(item.rows[0], &shapes))
        .map(|f| (f.key, f.shape))
        .collect();
    let declared: Vec<(&str, &str)> = items
        .iter()
        .filter_map(declaration)
        .filter(|(_, ty)| created.contains(*ty))
        .collect();
    let mut seen = BTreeSet::new();
    types.retain(|ty| !BUILTIN.contains(&ty.as_str()) && seen.insert(ty.clone()));
    let fields_of = |ty: &str| -> Vec<usize> {
        (0..shapes.len())
            .filter(|&i| shapes[i].functional && shapes[i].key_type == ty)
            .collect()
    };

    let mut parse_errors: BTreeMap<usize, String> = BTreeMap::new();
    let mut all: Vec<Clause> = Vec::new();
    for item in items.iter().filter(|item| item.rows[0].starts_with("on ")) {
        match parse_clause(item, &shapes) {
            Ok(clause) => all.push(clause),
            Err(message) => {
                parse_errors.insert(item.line, message);
            }
        }
    }
    let events = infer(&all, &shapes, &templates)?;
    let mut handled: BTreeSet<usize> = BTreeSet::new();
    let mut errors: BTreeMap<usize, String> = BTreeMap::new();
    let mut own: Vec<usize> = Vec::new();
    let mut declarations = Vec::new();
    let mut initial = Vec::new();
    let mut clauses: Vec<&Clause> = Vec::new();
    let mut used: BTreeSet<usize> = BTreeSet::new();
    let mut named: BTreeSet<String> = BTreeSet::new();
    let mut defines: Vec<&str> = Vec::new();
    let mut calls: BTreeSet<usize> = BTreeSet::new();
    for item in items.iter().filter(|item| owner[item.line] == me) {
        let head = item.rows[0];
        let result: Result<(), String> = (|| {
            if let Some(message) = broken.get(&item.line) {
                return Err(message.clone());
            }
            if let Some(template) = templates.iter().find(|t| t.line == item.line) {
                let define = template
                    .define
                    .as_deref()
                    .ok_or_else(|| format!("unsupported relation template {}", template.name))?;
                defines.push(define);
                return Ok(());
            }
            if let Some(i) = shapes.iter().position(|s| s.line == item.line) {
                own.push(i);
                named.extend([shapes[i].key_type.clone(), shapes[i].value_type.clone()]);
                return Ok(());
            }
            if head.starts_with("mode ") {
                return Ok(());
            }
            if let Some(message) = law_errors.get(&item.line) {
                return Err(message.clone());
            }
            if laws.contains_key(&item.line) {
                return Ok(());
            }
            if let Some(name) = head.strip_prefix("derive ") {
                return if laws.values().any(|law| law == name) {
                    Ok(())
                } else {
                    Err(format!("unsupported derive {name}"))
                };
            }
            if head.starts_with("on ") {
                if let Some(message) = parse_errors.get(&item.line) {
                    return Err(message.clone());
                }
                let clause = all.iter().find(|c| c.line == item.line).unwrap();
                let event = &events[clause.handler];
                if event.line == item.line {
                    for (name, ty) in &event.slots {
                        let ty = ty
                            .as_ref()
                            .ok_or_else(|| format!("event {}: no type for argument ?{name}", clause.handler))?;
                        named.insert(ty.clone());
                    }
                } else {
                    handled.insert(owner[event.line]);
                }
                named.extend(clause.creates.iter().map(|(_, ty)| ty.clone()));
                used.extend(clause.includes.iter().chain(&clause.withdrawals).map(|f| f.shape));
                clauses.push(clause);
                return Ok(());
            }
            if let Some((name, ty)) = declaration(item) {
                if created.contains(ty) {
                    let at = initial
                        .iter()
                        .position(|line: &String| {
                            line.strip_prefix("  ")
                                .and_then(|l| l.strip_prefix(name))
                                .is_some_and(|l| l.starts_with(['.', ' ']))
                        })
                        .unwrap_or(initial.len());
                    initial.insert(at, format!("  {name} is a {ty}\n"));
                } else {
                    declarations.push((name, ty));
                }
                named.insert(ty.to_string());
                return Ok(());
            }
            if item.rows.len() == 1 && !head.contains(' ') && head.starts_with(|c: char| c.is_ascii_uppercase()) {
                return Ok(());
            }
            let read = (item.rows.len() == 1)
                .then(|| fact(head, &shapes))
                .flatten()
                .ok_or_else(|| format!("unsupported form `{}`", head.trim()))?;
            let shape = &shapes[read.shape];
            let value = expression(&read.value)?;
            used.insert(read.shape);
            initial.push(if shape.functional {
                format!("  {}.{} = {value}\n", read.key, field_name(shape))
            } else {
                format!("  {} {} {}\n", read.key, shape.words, slot_value(value))
            });
            Ok(())
        })();
        if let Err(message) = result {
            errors.insert(item.line, message);
        }
    }

    let mut maybe: Vec<bool> = shapes.iter().map(|s| s.maybe).collect();
    let mut creation = vec![false; shapes.len()];
    let made: BTreeSet<&str> = all
        .iter()
        .flat_map(|c| c.creates.iter().map(|(_, ty)| ty.as_str()))
        .collect();
    for ty in &made {
        for i in fields_of(ty) {
            let mut written = false;
            let mut stated = false;
            let mut every_site = true;
            for clause in &all {
                let mut sets = false;
                for write in clause.includes.iter().filter(|f| f.shape == i) {
                    if clause.creates.iter().any(|(key, t)| key == &write.key && t == ty) {
                        written = true;
                        sets = true;
                    } else {
                        stated = true;
                    }
                }
                if clause.creates.iter().any(|(_, t)| t == ty) && !sets {
                    every_site = false;
                }
            }
            if declared.iter().any(|(name, t)| t == ty && !given.contains(&(name.to_string(), i))) {
                every_site = false;
            }
            creation[i] = written && !stated;
            if !every_site {
                maybe[i] = true;
            }
        }
    }
    let field_type = |i: usize| {
        if maybe[i] {
            format!("maybe {}", shapes[i].value_type)
        } else {
            shapes[i].value_type.clone()
        }
    };

    let mut count = 0;
    let mut maintenance = 0;
    let mut unpaired = Vec::new();
    let mut rendered = String::new();
    for clause in &clauses {
        count += 1;
        if MAINTENANCE.contains(&clause.handler) {
            maintenance += 1;
        }
        match render_clause(clause, count, &shapes, &templates, &created, &fields_of, &mut unpaired) {
            Ok((text, reads, read_calls)) => {
                rendered.push_str(&text);
                used.extend(reads);
                calls.extend(read_calls);
            }
            Err(message) => {
                errors.insert(clause.line, message);
            }
        }
    }
    if let Some((line, message)) = errors.into_iter().next() {
        return Err(format!("line {line}: {message}"));
    }

    for &i in &used {
        let shape = &shapes[i];
        named.extend([shape.key_type.clone(), shape.value_type.clone()]);
    }
    let declarer = |i: usize| {
        if creation[i] {
            type_owner[&shapes[i].key_type]
        } else {
            owner[shapes[i].line]
        }
    };
    let mut uses: BTreeSet<usize> = used.iter().map(|&i| declarer(i)).collect();
    uses.extend(handled);
    uses.extend(calls.iter().map(|&t| owner[templates[t].line]));
    uses.extend(named.iter().filter_map(|ty| type_owner.get(ty)));
    uses.remove(&me);

    let mut out = String::new();
    for &module in &uses {
        out.push_str(&format!("use {}\n", modules[module]));
    }
    if !uses.is_empty() {
        out.push('\n');
    }
    for ty in types.iter().filter(|ty| type_owner.get(*ty) == Some(&me)) {
        if created.contains(ty) {
            out.push_str(&format!("state entity {ty}\n"));
            for i in (0..shapes.len()).filter(|&i| shapes[i].functional && shapes[i].key_type == *ty && creation[i]) {
                out.push_str(&format!("  {}: {}\n", field_name(&shapes[i]), field_type(i)));
            }
        } else {
            out.push_str(&format!("type {ty}\n"));
            for (name, _) in declarations.iter().filter(|(_, t)| t == ty) {
                out.push_str(&format!("{name} is a {ty}\n"));
            }
        }
        out.push('\n');
    }
    for (name, ty) in declarations.iter().filter(|(_, t)| type_owner.get(*t) != Some(&me)) {
        out.push_str(&format!("{name} is a {ty}\n\n"));
    }
    for ty in &types {
        let fields: Vec<usize> = own
            .iter()
            .copied()
            .filter(|&i| shapes[i].functional && shapes[i].key_type == *ty && !creation[i])
            .collect();
        if fields.is_empty() {
            continue;
        }
        out.push_str(&format!("state for {ty}\n"));
        for i in fields {
            out.push_str(&format!("  {}: {}\n", field_name(&shapes[i]), field_type(i)));
        }
        out.push('\n');
    }
    for shape in own.iter().map(|&i| &shapes[i]).filter(|s| !s.functional) {
        let name = if shape.words.replace(' ', "-") == shape.name {
            String::new()
        } else {
            format!("{}: ", shape.name)
        };
        out.push_str(&format!(
            "state {name}{} {} {}\n",
            slot(&shape.key, &shape.key_type, &types),
            shape.words,
            slot(&shape.value, &shape.value_type, &types)
        ));
    }
    if own.iter().any(|&i| shapes[i].name == "host-effect") {
        out.push_str("\neffect menu-host-effect: North requests host effect ?effect: Text with payload ?payload: Text\n  at most one per step\n  settled by clear-host-effect\n");
    }
    for define in &defines {
        out.push('\n');
        out.push_str(define);
    }
    if !initial.is_empty() {
        out.push_str("\ninitially\n");
        out.extend(initial);
    }
    for clause in clauses.iter().filter(|c| events[c.handler].line == c.line) {
        out.push_str(&format!("\nevent {}", clause.handler));
        for (name, ty) in &events[clause.handler].slots {
            out.push_str(&format!(" {}", slot(name, ty.as_deref().unwrap_or_default(), &types)));
        }
        out.push('\n');
    }
    out.push_str(&rendered);
    Ok(Import {
        source: out,
        clauses: count,
        maintenance,
        unpaired,
    })
}

fn render_clause(
    clause: &Clause,
    count: usize,
    shapes: &[Shape],
    templates: &[Template],
    created: &BTreeSet<String>,
    fields_of: &dyn Fn(&str) -> Vec<usize>,
    unpaired: &mut Vec<String>,
) -> Result<(String, BTreeSet<usize>, BTreeSet<usize>), String> {
    let handler = clause.handler;
    let mut out = format!("\n{}\n", clause.head);
    let mut context = Context {
        shapes,
        templates,
        calls: Default::default(),
        head: clause.head.split_whitespace().skip(2).collect(),
        map: BTreeMap::new(),
        bound: Default::default(),
        used: Default::default(),
        grounded: Default::default(),
        declared: Default::default(),
        created: clause.creates.iter().map(|(key, _)| key.clone()).collect(),
        inline: clause
            .includes
            .iter()
            .filter(|f| f.value.starts_with('?') && tokens(&f.value).len() == 1)
            .filter(|f| clause.includes.iter().filter(|g| mentions(&g.value, &f.value)).count() == 1)
            .map(|f| f.value.clone())
            .collect(),
    };
    for text in &clause.conditions {
        context.collect(text);
    }
    let mut conditions = Vec::new();
    for text in &clause.conditions {
        conditions.extend(context.literals(text)?);
    }

    let teardown = clause.includes.is_empty() && clause.creates.is_empty();
    let mut torn: Vec<(&String, &str)> = Vec::new();
    if teardown {
        for key in clause.withdrawals.iter().map(|f| &f.key) {
            if torn.iter().any(|(k, _)| *k == key) {
                continue;
            }
            let ty = created.iter().find(|ty| {
                let fields = fields_of(ty);
                !fields.is_empty()
                    && fields
                        .iter()
                        .all(|&i| clause.withdrawals.iter().any(|f| f.shape == i && f.key == *key))
            });
            if let Some(ty) = ty {
                torn.push((key, ty));
            }
        }
    }

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
        if shape.functional && clause.creates.iter().any(|(key, _)| *key == include.key) {
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
    if let Some((_, value)) = effect.as_mut().filter(|(_, value)| value.contains(' ')) {
        let bound = "?effect";
        if conditions.iter().chain(&posts).any(|text| mentions(text, bound)) {
            return Err(format!("{handler}: {bound} already in use"));
        }
        let value = std::mem::replace(value, bound.to_string());
        let value = value
            .strip_prefix('(')
            .and_then(|v| v.strip_suffix(')'))
            .unwrap_or(&value)
            .to_string();
        for post in posts.iter_mut() {
            if let Some(head) = post.strip_suffix(&value) {
                *post = format!("{head}{bound}");
            }
        }
        conditions.push(format!("{bound} = {value}"));
    }
    for (i, text) in conditions.iter().enumerate() {
        out.push_str(&format!("  {} {text}\n", if i == 0 { "when" } else { "and" }));
    }
    if handler != "clear-host-effect" {
        match (effect, payload) {
            (Some((north, effect)), Some((payload_north, payload))) if north == payload_north => {
                posts.push(format!(
                    "{north} requests host effect {effect} with payload {payload}"
                ));
            }
            (None, None) => (),
            _ => return Err(format!("{handler}: incomplete host-effect mailbox write")),
        }
    }
    for (key, ty) in &clause.creates {
        let mut fields = records.remove(key.as_str()).unwrap_or_default();
        fields.sort_by_key(|(shape, _)| *shape);
        let fields: Vec<String> = fields
            .into_iter()
            .map(|(shape, value)| {
                let name = field_name(&shapes[shape]);
                let own = value.strip_prefix('?').is_some_and(|v| {
                    tokens(&value).len() == 1
                        && (v == name || v.rsplit_once('.').is_some_and(|(_, f)| f == name))
                });
                if own { value } else { format!("{name}: {value}") }
            })
            .collect();
        posts.push(if fields.is_empty() {
            format!("{key} is a new {ty}")
        } else {
            format!("{key} is a new {ty} {{{}}}", fields.join(", "))
        });
    }
    for (key, ty) in &torn {
        posts.push(format!("not {key} is a {ty}"));
    }
    for withdrawal in &clause.withdrawals {
        let shape = &shapes[withdrawal.shape];
        if torn.iter().any(|(key, _)| **key == withdrawal.key || **key == withdrawal.value) {
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
        out.push_str(&format!("  {} {text}\n", if i == 0 { "then" } else { "and" }));
    }
    out.push_str("  may skip\n");
    Ok((out, context.used, context.calls))
}
