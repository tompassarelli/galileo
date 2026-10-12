use crate::{value, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone)]
enum Json {
    Scalar(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

struct Parser<'a> {
    text: &'a str,
    at: usize,
}

impl Parser<'_> {
    fn space(&mut self) {
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.at += 1;
        }
    }

    fn take(&mut self, byte: u8) -> bool {
        self.space();
        if self.text.as_bytes().get(self.at) == Some(&byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn string(&mut self) -> Result<String> {
        self.space();
        let start = self.at;
        if !self.take(b'"') {
            return Err("expected JSON string".into());
        }
        while let Some(byte) = self.text.as_bytes().get(self.at) {
            self.at += 1;
            match byte {
                b'\\' => self.at += 1,
                b'"' => return Ok(self.text[start..self.at].to_owned()),
                _ => {}
            }
        }
        Err("unterminated JSON string".into())
    }

    fn parse(&mut self) -> Result<Json> {
        self.space();
        if self.text.as_bytes().get(self.at) == Some(&b'"') {
            return Ok(Json::Scalar(self.string()?));
        }
        if self.take(b'{') {
            let mut fields = BTreeMap::new();
            if !self.take(b'}') {
                loop {
                    let quoted = self.string()?;
                    let key = quoted[1..quoted.len() - 1].to_owned();
                    if !self.take(b':') {
                        return Err("expected JSON colon".into());
                    }
                    fields.insert(key, self.parse()?);
                    if self.take(b'}') {
                        break;
                    }
                    if !self.take(b',') {
                        return Err("expected JSON object comma".into());
                    }
                }
            }
            return Ok(Json::Object(fields));
        }
        if self.take(b'[') {
            let mut items = Vec::new();
            if !self.take(b']') {
                loop {
                    items.push(self.parse()?);
                    if self.take(b']') {
                        break;
                    }
                    if !self.take(b',') {
                        return Err("expected JSON array comma".into());
                    }
                }
            }
            return Ok(Json::Array(items));
        }
        let start = self.at;
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b',' | b']' | b'}'))
        {
            self.at += 1;
        }
        if start == self.at {
            return Err("expected JSON value".into());
        }
        Ok(Json::Scalar(self.text[start..self.at].to_owned()))
    }
}

impl Json {
    fn read(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        let mut parser = Parser { text: &text, at: 0 };
        let result = parser.parse()?;
        parser.space();
        if parser.at != text.len() {
            return Err("trailing JSON input".into());
        }
        Ok(result)
    }

    fn object(&self) -> Result<&BTreeMap<String, Self>> {
        match self {
            Self::Object(fields) => Ok(fields),
            _ => Err("expected JSON object".into()),
        }
    }

    fn object_mut(&mut self) -> Result<&mut BTreeMap<String, Self>> {
        match self {
            Self::Object(fields) => Ok(fields),
            _ => Err("expected JSON object".into()),
        }
    }

    fn field_mut(&mut self, key: &str) -> Result<&mut Self> {
        self.object_mut()?
            .get_mut(key)
            .ok_or_else(|| format!("missing JSON field {key}").into())
    }

    fn print(&self) -> String {
        match self {
            Self::Scalar(text) => text.clone(),
            Self::Array(items) => format!(
                "[{}]",
                items.iter().map(Self::print).collect::<Vec<_>>().join(",")
            ),
            Self::Object(fields) => format!(
                "{{{}}}",
                fields
                    .iter()
                    .map(|(key, item)| format!("\"{key}\":{}", item.print()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }

    fn write(&self, path: &Path) -> Result<()> {
        fs::write(path, self.print() + "\n")?;
        Ok(())
    }
}

fn alias(name: &str, clone: usize) -> String {
    format!("{name}_{clone:03}")
}

fn identities(fields: &mut BTreeMap<String, Json>, names: &[String], clone: usize) {
    for name in names {
        if let Some(value) = fields.remove(name) {
            fields.insert(alias(name, clone), value);
        }
    }
}

fn module_options(value: &mut Json, names: &[String], clone: usize) -> Result<()> {
    match value {
        Json::Object(fields) => {
            if let Some(Json::Object(config)) = fields.get_mut("myConfig") {
                if let Some(modules) = config.get_mut("modules") {
                    identities(modules.object_mut()?, names, clone);
                }
            }
            for (key, child) in fields {
                if key != "$ref" {
                    module_options(child, names, clone)?;
                }
            }
        }
        Json::Array(items) => {
            for item in items {
                module_options(item, names, clone)?;
            }
        }
        Json::Scalar(_) => {}
    }
    Ok(())
}

fn expand(value: &Json, names: &[String], count: usize, options: bool) -> Result<Json> {
    let source = value.object()?;
    let mut fields: BTreeMap<_, _> = source
        .iter()
        .filter(|(key, _)| !names.contains(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    for clone in 1..=count {
        for name in names {
            if let Some(value) = source.get(name) {
                let mut value = value.clone();
                if options {
                    module_options(&mut value, names, clone)?;
                }
                fields.insert(alias(name, clone), value);
            }
        }
    }
    Ok(Json::Object(fields))
}

fn contract(mut source: Json, names: &[String], count: usize) -> Result<Json> {
    let modules = source.field_mut("modules")?;
    *modules = expand(modules, names, count, true)?;
    let settings = source
        .field_mut("settings")?
        .field_mut("myConfig")?
        .field_mut("modules")?;
    *settings = expand(settings, names, count, false)?;
    let Json::Array(enabled) = source.field_mut("enabled")? else {
        return Err("expected enabled array".into());
    };
    let original = std::mem::take(enabled);
    for clone in 1..=count {
        for value in &original {
            let Json::Scalar(text) = value else {
                return Err("expected enabled module name".into());
            };
            let name = text
                .strip_prefix('"')
                .and_then(|text| text.strip_suffix('"'))
                .ok_or("expected enabled module string")?;
            enabled.push(Json::Scalar(format!("\"{}\"", alias(name, clone))));
        }
    }
    Ok(source)
}

fn identifier(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn nix(source: &str, names: &[String], clone: usize) -> String {
    let mut output = String::new();
    let mut at = 0;
    while at < source.len() {
        let rest = &source[at..];
        if rest.starts_with("/*") {
            let end = rest.find("*/").map_or(rest.len(), |end| end + 2);
            output.push_str(&rest[..end]);
            at += end;
            continue;
        }
        if let Some(prefix) = [
            "myConfig.modules.",
            "\"myConfig\".modules.",
            "myConfig.\"modules\".",
            "\"myConfig\".\"modules\".",
        ]
        .iter()
        .find(|prefix| rest.starts_with(**prefix))
        {
            let tail = &rest[prefix.len()..];
            let quoted = tail.starts_with('"');
            let name_start = usize::from(quoted);
            let length = tail.as_bytes()[name_start..]
                .iter()
                .take_while(|byte| identifier(**byte))
                .count();
            let name = &tail[name_start..name_start + length];
            if (!quoted || tail.as_bytes().get(name_start + length) == Some(&b'"'))
                && names.iter().any(|candidate| candidate == name)
            {
                output.push_str(prefix);
                if quoted {
                    output.push('"');
                }
                output.push_str(&alias(name, clone));
                if quoted {
                    output.push('"');
                }
                at += prefix.len() + name_start + length + usize::from(quoted);
                continue;
            }
        }
        if rest.starts_with('"') || rest.starts_with("''") {
            let delimiter = if rest.starts_with('"') { "\"" } else { "''" };
            let start = at;
            at += delimiter.len();
            while at < source.len() {
                if delimiter == "\"" && source.as_bytes()[at] == b'\\' {
                    at += 2;
                } else if source[at..].starts_with(delimiter) {
                    at += delimiter.len();
                    break;
                } else {
                    at += source[at..].chars().next().unwrap().len_utf8();
                }
            }
            output.push_str(&source[start..at]);
            continue;
        }
        if rest.starts_with('#') {
            let end = rest.find('\n').unwrap_or(rest.len());
            output.push_str(&rest[..end]);
            at += end;
            continue;
        }
        if let Some(name) = names
            .iter()
            .find(|name| rest.starts_with(&format!("./{name}.nix")))
        {
            output.push_str(&format!("./{}.nix", alias(name, clone)));
            at += name.len() + "./.nix".len();
            continue;
        }
        let ch = rest.chars().next().unwrap();
        output.push(ch);
        at += ch.len_utf8();
    }
    output
}

fn host(source: &str, names: &[String], count: usize) -> String {
    let lines: Vec<_> = source.split_inclusive('\n').collect();
    let mut output = String::new();
    let mut at = 0;
    while at < lines.len() {
        let sample = lines[at]
            .strip_prefix("  myConfig.modules.")
            .is_some_and(|tail| {
                let tail = tail.trim_start_matches('"');
                let length = tail
                    .as_bytes()
                    .iter()
                    .take_while(|byte| identifier(**byte))
                    .count();
                names.iter().any(|name| name == &tail[..length])
            });
        if sample {
            let mut end = at + 1;
            while end < lines.len()
                && !lines[end].starts_with("  myConfig.modules.")
                && !lines[end].starts_with('}')
            {
                end += 1;
            }
            let statement = lines[at..end].concat();
            for clone in 1..=count {
                output.push_str(&nix(&statement, names, clone));
            }
            at = end;
        } else {
            output.push_str(lines[at]);
            at += 1;
        }
    }
    output
}

fn expected(source: &Path, output: &Path, names: &[String], count: usize) -> Result<()> {
    fs::create_dir(output)?;
    fs::create_dir(output.join("modules"))?;
    for clone in 1..=count {
        for name in names {
            let target = output.join("modules").join(alias(name, clone));
            fs::create_dir(&target)?;
            for entry in fs::read_dir(source.join("modules").join(name))? {
                let path = entry?.path();
                if path.extension().is_some_and(|ext| ext == "nix") {
                    let filename = if path.file_stem().is_some_and(|stem| stem == name.as_str()) {
                        format!("{}.nix", alias(name, clone))
                    } else {
                        path.file_name()
                            .ok_or("missing output filename")?
                            .to_str()
                            .ok_or("non-UTF-8 output filename")?
                            .to_owned()
                    };
                    fs::write(
                        target.join(filename),
                        nix(&fs::read_to_string(path)?, names, clone),
                    )?;
                }
            }
        }
    }
    fs::create_dir(output.join("hosts"))?;
    for name in ["ashashi", "whiterabbit"] {
        let target = output.join("hosts").join(name);
        fs::create_dir(&target)?;
        for entry in fs::read_dir(source.join("hosts").join(name))? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "nix") {
                fs::write(
                    target.join(path.file_name().ok_or("missing host filename")?),
                    host(&fs::read_to_string(path)?, names, count),
                )?;
            }
        }
    }
    Ok(())
}

pub fn run(args: Vec<String>) -> Result<()> {
    let mut count = None;
    let mut out = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--help" | "-h" => {
                println!("Usage: compare scale COUNT --out NEW_DIRECTORY\n\nRun from the repository root after bench/contract/make-reference.sh and\nbench/galileo/make.sh plus check.sh. Write the shared scaled context.json,\nprivate.json, contract/<host>.json, and galileo-expected/ from those verified\nsample outputs. Module identities and myConfig.modules paths gain _001, etc.;\nNixOS options, references, strings, schema keys, tags and rules stay unchanged.\nThe output directory must not exist. It contains private runtime values;\nuse the ignored bench/scaled directory. Each port generates one program separately.");
                return Ok(());
            }
            _ if count.is_none() && !arg.starts_with('-') => count = Some(arg.parse::<usize>()?),
            _ => return Err(format!("unknown option: {arg}").into()),
        }
    }
    let count = count
        .filter(|count| *count > 0)
        .ok_or("scale needs a positive COUNT")?;
    let out = out.ok_or("scale requires --out")?;
    if out.exists() {
        return Err("scaled output directory already exists".into());
    }
    let sample = Path::new("bench/galileo/out");
    let checked = Command::new("target/debug/galileo")
        .args(["build", "--check", "bench/galileo/out"])
        .output()?;
    if !checked.status.success() {
        return Err("sample build is not current; run bench/galileo/make.sh and check.sh".into());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(sample.join("modules"))? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            names.push(
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| "non-UTF-8 module name")?,
            );
        }
    }
    names.sort();
    if names.is_empty() {
        return Err("sample has no modules".into());
    }
    let context = expand(
        &Json::read(Path::new("bench/context.json"))?,
        &names,
        count,
        false,
    )?;
    let mut private = Json::read(Path::new("bench/contract/out/private.json"))?;
    for settings in private.object_mut()?.values_mut() {
        *settings = expand(settings, &names, count, false)?;
    }
    let mut contracts = Vec::new();
    for name in ["ashashi", "whiterabbit"] {
        let source = Json::read(&Path::new("bench/contract/out").join(format!("{name}.json")))?;
        contracts.push((name, contract(source, &names, count)?));
    }
    fs::create_dir(&out)?;
    context.write(&out.join("context.json"))?;
    private.write(&out.join("private.json"))?;
    fs::create_dir(out.join("contract"))?;
    for (name, value) in contracts {
        value.write(&out.join("contract").join(format!("{name}.json")))?;
    }
    expected(sample, &out.join("galileo-expected"), &names, count)?;
    println!(
        "clones\tmodules\n{count}\t{}",
        count
            .checked_mul(names.len())
            .ok_or("module count overflow")?
    );
    Ok(())
}
