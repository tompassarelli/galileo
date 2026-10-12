use crate::{value, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

enum Json {
    String(String),
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
        if !self.take(b'"') {
            return Err(format!("expected JSON string at byte {}", self.at).into());
        }
        let mut out = String::new();
        loop {
            let ch = self.text[self.at..]
                .chars()
                .next()
                .ok_or("unterminated JSON string")?;
            self.at += ch.len_utf8();
            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let escaped = *self
                        .text
                        .as_bytes()
                        .get(self.at)
                        .ok_or("unterminated JSON escape")?;
                    self.at += 1;
                    out.push(match escaped {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{0008}',
                        b'f' => '\u{000c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let end = self.at + 4;
                            let digits = self
                                .text
                                .get(self.at..end)
                                .ok_or("short JSON Unicode escape")?;
                            let mut code = u32::from_str_radix(digits, 16)?;
                            self.at = end;
                            if (0xd800..=0xdbff).contains(&code) {
                                if !self.text[self.at..].starts_with("\\u") {
                                    return Err("missing JSON low surrogate".into());
                                }
                                self.at += 2;
                                let end = self.at + 4;
                                let low = u32::from_str_radix(
                                    self.text
                                        .get(self.at..end)
                                        .ok_or("short JSON low surrogate")?,
                                    16,
                                )?;
                                if !(0xdc00..=0xdfff).contains(&low) {
                                    return Err("invalid JSON low surrogate".into());
                                }
                                self.at = end;
                                code = 0x10000 + ((code - 0xd800) << 10) + low - 0xdc00;
                            }
                            char::from_u32(code).ok_or("invalid JSON Unicode scalar")?
                        }
                        _ => return Err("invalid JSON escape".into()),
                    });
                }
                ch if ch.is_control() => return Err("control character in JSON string".into()),
                _ => out.push(ch),
            }
        }
    }

    fn parse(&mut self) -> Result<Json> {
        self.space();
        if self.text.as_bytes().get(self.at) == Some(&b'"') {
            return Ok(Json::String(self.string()?));
        }
        if self.take(b'[') {
            let mut values = Vec::new();
            if !self.take(b']') {
                loop {
                    values.push(self.parse()?);
                    if self.take(b']') {
                        break;
                    }
                    if !self.take(b',') {
                        return Err("expected comma in JSON array".into());
                    }
                }
            }
            return Ok(Json::Array(values));
        }
        if self.take(b'{') {
            let mut fields = BTreeMap::new();
            if !self.take(b'}') {
                loop {
                    let name = self.string()?;
                    if !self.take(b':') {
                        return Err("expected colon in JSON object".into());
                    }
                    if fields.insert(name, self.parse()?).is_some() {
                        return Err("duplicate JSON field".into());
                    }
                    if self.take(b'}') {
                        break;
                    }
                    if !self.take(b',') {
                        return Err("expected comma in JSON object".into());
                    }
                }
            }
            return Ok(Json::Object(fields));
        }
        Err(format!("unsupported schema JSON value at byte {}", self.at).into())
    }
}

impl Json {
    fn field(&self, name: &str) -> Option<&Json> {
        match self {
            Self::Object(fields) => fields.get(name),
            _ => None,
        }
    }

    fn text(&self, name: &str) -> Result<&str> {
        match self.field(name) {
            Some(Self::String(text)) => Ok(text),
            _ => Err(format!("schema record needs string {name}").into()),
        }
    }
}

fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn print_type(record: &Json, language: usize) -> Result<String> {
    let any = ["Dyn", "_", "JSON", "any"];
    let scalar = match record.text("t")? {
        "bool" => Some(["Bool", "bool", "Bool", "bool"]),
        "str" | "separatedString" => Some(["String", "string", "Text", "str"]),
        "unknown" | "coercedTo" => Some(any),
        "path" => Some(["Path", "#Path", "Path", "Path"]),
        "package" => Some(["Package", "#Package", "Package", "Package"]),
        "unsignedInt16" => Some(["Port", "(int & >=0 & <=65535)", "Natural", "int"]),
        "submodule" => Some(["{_ : Dyn}", "{...}", "Map JSON", "{str:any}"]),
        "listOf" | "attrsOf" | "nullOr" => None,
        kind => {
            return Err(format!(
                "unsupported native schema type {kind}; request a representation ruling"
            )
            .into())
        }
    };
    if let Some(types) = scalar {
        return Ok(types[language].to_owned());
    }
    let inner = match record.field("inner") {
        Some(inner) => print_type(inner, language)?,
        None if record.text("t")? == "attrsOf" => any[language].to_owned(),
        None => {
            return Err(format!("{} needs its native inner descriptor", record.text("t")?).into())
        }
    };
    Ok(match (record.text("t")?, language) {
        ("listOf", 0) => format!("Array ({inner})"),
        ("listOf", 1) => format!("[...{inner}]"),
        ("listOf", 2) => format!("List ({inner})"),
        ("listOf", 3) => format!("[{inner}]"),
        ("attrsOf", 0) => format!("{{_ : {inner}}}"),
        ("attrsOf", 1) => format!("{{[string]: {inner}}}"),
        ("attrsOf", 2) => format!("Map ({inner})"),
        ("attrsOf", 3) => format!("{{str:{inner}}}"),
        ("nullOr", 0) => format!(
            "std.contract.any_of [std.contract.from_predicate (fun x => x == null), {inner}]"
        ),
        ("nullOr", 1) => format!("null | {inner}"),
        ("nullOr", 2) => format!("Optional ({inner})"),
        ("nullOr", 3) => inner,
        _ => unreachable!(),
    })
}

pub fn run(args: Vec<String>) -> Result<()> {
    let mut schema = PathBuf::from("bench/schema/sample.json");
    let mut out = PathBuf::from("bench/schema");
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--schema" => schema = PathBuf::from(value(&mut args, &arg)?),
            "--out" => out = PathBuf::from(value(&mut args, &arg)?),
            "--help" | "-h" => {
                println!("Usage: compare types [--schema SAMPLE.json] [--out DIRECTORY]\n\nPrint the frozen option descriptors into nickel/types.ncl, cue/types.cue,\ndhall/types.dhall and kcl/types.k beneath DIRECTORY (default bench/schema).\nNative container-element descriptors remain container descriptors.\nUnknown and opaque coercedTo types stay unchecked in all four languages.");
                return Ok(());
            }
            _ => return Err(format!("unknown option: {arg}").into()),
        }
    }
    let text = fs::read_to_string(&schema)?;
    let mut parser = Parser { text: &text, at: 0 };
    let Json::Array(records) = parser.parse()? else {
        return Err("schema must be an array".into());
    };
    parser.space();
    if parser.at != text.len() {
        return Err("trailing schema JSON".into());
    }
    let mut options = BTreeMap::new();
    for record in &records {
        let name = record.text("name")?;
        if options.insert(name, record).is_some() {
            return Err(format!("duplicate schema option {name}").into());
        }
    }
    let headers = [
        "let Ref = {\"$ref\" | std.contract.any_of [String, Array String]} in\nlet Call = {\"$call\" | Dyn} in\nlet Package = Ref in\nlet Path = std.contract.any_of [String, Ref, Call] in\nlet Port = std.contract.from_predicate (fun x => std.is_number x && std.number.is_integer x && x >= 0 && x <= 65535) in\n{\n",
        "package schema\n\n#Ref: {\"$ref\": string | [...string]}\n#Call: {\"$call\": _}\n#Package: #Ref\n#Path: string | #Ref | #Call\n\n#Options: {\n",
        "let JSON = https://prelude.dhall-lang.org/JSON/Type sha256:40edbc9371979426df63e064333b02689b969c4cfbbccfa481216d2d1a6e9759\nlet Map = \\(T : Type) -> List { mapKey : Text, mapValue : T }\nlet Ref = { `$ref` : < Name : Text | Parts : List Text > }\nlet Call = { `$call` : JSON }\nlet Package = Ref\nlet Path = < String : Text | Ref : Ref | Call : Call >\nlet validPort = \\(port : Optional Natural) -> merge { None = True, Some = \\(n : Natural) -> Natural/isZero (Natural/subtract 65535 n) } port\nin { validPort = validPort, types = {\n",
        "schema Package:\n    \"$ref\": str | [str]\n\nschema Call:\n    \"$call\": any\n\ntype Path = str | Package | Call\n\n",
    ];
    let mut generated: Vec<String> = headers.into_iter().map(str::to_owned).collect();
    for (index, (name, record)) in options.iter().enumerate() {
        for (language, output) in generated.iter_mut().enumerate() {
            let ty = print_type(record, language).map_err(|error| format!("{name}: {error}"))?;
            match language {
                0 => output.push_str(&format!("  {} = {ty},\n", quoted(name))),
                1 => output.push_str(&format!("  {}: {ty}\n", quoted(name))),
                2 => {
                    if name.contains('`') {
                        return Err("Dhall option names cannot contain backticks".into());
                    }
                    output.push_str(&format!(
                        "  {} `{name}` = {ty}\n",
                        if index == 0 { "" } else { "," }
                    ));
                }
                3 => {
                    let optional_port = record.text("t")? == "nullOr"
                        && record.field("inner").is_some_and(|inner| {
                            inner.text("t").is_ok_and(|ty| ty == "unsignedInt16")
                        });
                    let ty = if optional_port { "int".to_owned() } else { ty };
                    output.push_str(&format!(
                        "schema Option{index:03}:\n    name: str = {}\n    value{}: {ty}\n",
                        quoted(name),
                        if matches!(record.text("t")?, "nullOr" | "unknown" | "coercedTo") {
                            "?"
                        } else {
                            ""
                        }
                    ));
                    if optional_port {
                        output.push_str("    check:\n        value >= 0 if value != None\n        value <= 65535 if value != None\n");
                    }
                    output.push('\n');
                }
                _ => unreachable!(),
            }
        }
    }
    generated[0].push_str("}\n");
    generated[1].push_str("}\n");
    generated[2].push_str("} }\n");
    generated[3].pop();
    for ((language, filename), source) in [
        ("nickel", "types.ncl"),
        ("cue", "types.cue"),
        ("dhall", "types.dhall"),
        ("kcl", "types.k"),
    ]
    .into_iter()
    .zip(generated)
    {
        let directory = out.join(language);
        fs::create_dir_all(&directory)?;
        let path = directory.join(filename);
        fs::write(&path, source)?;
        println!("{}", path.display());
    }
    Ok(())
}
