use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaOption {
    pub path: String,
    pub ty: String,
    pub values: Vec<String>,
    pub declarations: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Schema {
    pub source: String,
    pub options: BTreeMap<Vec<String>, SchemaOption>,
}

impl Schema {
    pub fn parse(source: &str, text: &str) -> Result<Self, String> {
        let located = |message: String| format!("{source}: {message}");
        let Json::Array(items) = Json::parse(text).map_err(located)? else {
            return Err(located("the option schema is a JSON array".into()));
        };
        let mut options = BTreeMap::new();
        for item in &items {
            let path = item.text("name").ok_or_else(|| located("an option without a `name`".into()))?;
            options.insert(
                path.split('.').map(str::to_string).collect(),
                SchemaOption {
                    path: path.to_string(),
                    ty: describe(item),
                    values: item.texts("enum"),
                    declarations: item.texts("declarations"),
                },
            );
        }
        Ok(Self {
            source: source.into(),
            options,
        })
    }

    pub fn get(&self, path: &str) -> Option<&SchemaOption> {
        self.options.get(&path.split('.').map(str::to_string).collect::<Vec<_>>())
    }
}

fn describe(ty: &Json) -> String {
    let name = ty.text("t").unwrap_or("?");
    match ty.field("inner") {
        Some(inner) => format!("{name} ({})", describe(inner)),
        None => name.to_string(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(String),
    Text(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut reader = Reader { text: text.as_bytes(), at: 0 };
        let value = reader.value()?;
        reader.space();
        if reader.at != reader.text.len() {
            return Err(format!("trailing data at byte {}", reader.at));
        }
        Ok(value)
    }

    pub fn field(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn text(&self, key: &str) -> Option<&str> {
        match self.field(key) {
            Some(Json::Text(text)) => Some(text),
            _ => None,
        }
    }

    fn texts(&self, key: &str) -> Vec<String> {
        match self.field(key) {
            Some(Json::Array(items)) => items
                .iter()
                .map(|item| match item {
                    Json::Text(text) => text.clone(),
                    Json::Number(n) => n.clone(),
                    Json::Bool(b) => b.to_string(),
                    _ => String::new(),
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}

struct Reader<'a> {
    text: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn space(&mut self) {
        while self.text.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        self.space();
        if self.text.get(self.at) == Some(&byte) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("expected `{}` at byte {}", byte as char, self.at))
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.space();
        self.text.get(self.at).copied()
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some(b'{') => {
                self.at += 1;
                let mut fields = Vec::new();
                if self.peek() == Some(b'}') {
                    self.at += 1;
                    return Ok(Json::Object(fields));
                }
                loop {
                    self.space();
                    let key = self.string()?;
                    self.expect(b':')?;
                    fields.push((key, self.value()?));
                    match self.peek() {
                        Some(b',') => self.at += 1,
                        Some(b'}') => {
                            self.at += 1;
                            return Ok(Json::Object(fields));
                        }
                        _ => return Err(format!("expected `,` or `}}` at byte {}", self.at)),
                    }
                }
            }
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                if self.peek() == Some(b']') {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                loop {
                    items.push(self.value()?);
                    match self.peek() {
                        Some(b',') => self.at += 1,
                        Some(b']') => {
                            self.at += 1;
                            return Ok(Json::Array(items));
                        }
                        _ => return Err(format!("expected `,` or `]` at byte {}", self.at)),
                    }
                }
            }
            Some(b'"') => self.string().map(Json::Text),
            Some(_) => {
                let start = self.at;
                while self
                    .text
                    .get(self.at)
                    .is_some_and(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'+' | b'.'))
                {
                    self.at += 1;
                }
                match &self.text[start..self.at] {
                    b"null" => Ok(Json::Null),
                    b"true" => Ok(Json::Bool(true)),
                    b"false" => Ok(Json::Bool(false)),
                    n if !n.is_empty() && (n[0].is_ascii_digit() || n[0] == b'-') => {
                        Ok(Json::Number(String::from_utf8_lossy(n).into_owned()))
                    }
                    _ => Err(format!("unexpected input at byte {start}")),
                }
            }
            None => Err("unexpected end of input".into()),
        }
    }

    fn string(&mut self) -> Result<String, String> {
        if self.text.get(self.at) != Some(&b'"') {
            return Err(format!("expected a string at byte {}", self.at));
        }
        self.at += 1;
        let mut out = Vec::new();
        loop {
            let byte = *self.text.get(self.at).ok_or("unterminated string")?;
            self.at += 1;
            match byte {
                b'"' => return String::from_utf8(out).map_err(|e| e.to_string()),
                b'\\' => {
                    let escape = *self.text.get(self.at).ok_or("unterminated string")?;
                    self.at += 1;
                    let ch = match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let high = self.hex()?;
                            let code = if (0xD800..0xDC00).contains(&high)
                                && self.text.get(self.at..self.at + 2) == Some(b"\\u")
                            {
                                self.at += 2;
                                let low = self.hex()?;
                                0x10000 + ((high - 0xD800) << 10) + (low.wrapping_sub(0xDC00) & 0x3FF)
                            } else {
                                high
                            };
                            char::from_u32(code).unwrap_or('\u{FFFD}')
                        }
                        _ => return Err(format!("bad escape at byte {}", self.at)),
                    };
                    let mut buffer = [0; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buffer).as_bytes());
                }
                _ => out.push(byte),
            }
        }
    }

    fn hex(&mut self) -> Result<u32, String> {
        let digits = self.text.get(self.at..self.at + 4).ok_or("short \\u escape")?;
        self.at += 4;
        u32::from_str_radix(&String::from_utf8_lossy(digits), 16).map_err(|e| e.to_string())
    }
}
