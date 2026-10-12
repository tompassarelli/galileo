use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Word(String),
    Variable(String),
    QuotedName(String),
    Computed(String),
    Integer(String),
    Text(String),
    TextBlock(String),
    Path(String),
    Symbol(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub no: usize,
    pub indent: usize,
    pub text: String,
    pub tokens: Vec<Token>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub text: Option<String>,
}

impl Error {
    pub fn at(line: &Line, message: impl Into<String>) -> Self {
        Self::source(line.no, line.indent + 1, &line.text, message)
    }

    pub fn bare(message: impl Into<String>) -> Self {
        Self {
            line: 0,
            column: 0,
            message: message.into(),
            text: None,
        }
    }

    fn source(no: usize, column: usize, text: &str, message: impl Into<String>) -> Self {
        Self {
            line: no,
            column,
            message: message.into(),
            text: Some(text.into()),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "line {}:{}: {}", self.line, self.column, self.message)?;
        } else {
            write!(f, "{}", self.message)?;
        }
        if let Some(text) = &self.text {
            write!(f, "\n    {text}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

pub fn lex(source: &str) -> Result<Vec<Line>, Error> {
    let physical: Vec<&str> = source.split_terminator('\n').collect();
    let mut out = Vec::new();
    let mut index = 0;
    let mut offset = 0;
    while index < physical.len() {
        let raw = physical[index];
        let no = index + 1;
        let mut opening_offset = offset;
        let mut opening_no = no;
        let mut opening_raw = raw;
        check_layout(raw, no)?;
        let indent = raw.bytes().take_while(|c| *c == b' ').count();
        let (mut tokens, end, mut opening) = tokenize(raw, no, offset, indent)?;
        let text = raw[indent..end].trim_end();
        if text.is_empty() {
            offset += raw.len() + 1;
            index += 1;
            continue;
        }
        if indent % 2 != 0 {
            return Err(Error::source(
                no,
                indent + 1,
                raw,
                "indentation must be a multiple of two spaces",
            ));
        }
        let mut line = Line {
            no,
            indent,
            text: text.into(),
            tokens: Vec::new(),
        };
        offset += raw.len() + 1;
        index += 1;
        while let Some(start) = opening {
            let content_start = index;
            let mut close = None;
            while index < physical.len() {
                let content = physical[index];
                check_layout(content, index + 1)?;
                let spaces = content.bytes().take_while(|c| *c == b' ').count();
                if content[spaces..].starts_with("'''") {
                    close = Some(spaces);
                    break;
                }
                if spaces < indent + 2 && spaces < content.len() {
                    return Err(Error::source(
                        index + 1,
                        spaces + 1,
                        content,
                        "text block content is left of its closing marker",
                    ));
                }
                offset += content.len() + 1;
                index += 1;
            }
            let closing_indent =
                close.ok_or_else(|| Error::at(&line, "unterminated text block"))?;
            if closing_indent != indent + 2 {
                return Err(Error::source(
                    index + 1,
                    closing_indent + 1,
                    physical[index],
                    "indent the closing text marker two spaces beneath its opening line",
                ));
            }
            let mut value = String::new();
            for content in physical.iter().take(index).skip(content_start) {
                value.push_str(&content[closing_indent.min(content.len())..]);
                value.push('\n');
            }
            tokens.push(Token {
                kind: TokenKind::TextBlock(value),
                span: Span {
                    start,
                    end: offset + closing_indent + 3,
                    line: opening_no,
                    column: opening_raw[..start - opening_offset].chars().count() + 1,
                },
            });
            opening_raw = physical[index];
            opening_no = index + 1;
            opening_offset = offset;
            let (continuation, end, next) =
                tokenize(opening_raw, opening_no, offset, closing_indent + 3)?;
            tokens.extend(continuation);
            line.text
                .push_str(opening_raw[closing_indent + 3..end].trim_end());
            opening = next;
            offset += physical[index].len() + 1;
            index += 1;
        }
        line.tokens = tokens;
        out.push(line);
    }
    Ok(out)
}

pub(crate) fn check_layout(raw: &str, no: usize) -> Result<(), Error> {
    for (index, c) in raw.char_indices() {
        let message = match c {
            '\t' => "tabs are not allowed; use spaces",
            '\r' => "use LF line endings",
            _ => continue,
        };
        return Err(Error::source(
            no,
            raw[..index].chars().count() + 1,
            raw,
            message,
        ));
    }
    Ok(())
}

pub(crate) fn tokenize(
    raw: &str,
    no: usize,
    offset: usize,
    indent: usize,
) -> Result<(Vec<Token>, usize, Option<usize>), Error> {
    let mut tokens = Vec::new();
    let mut at = indent;
    while at < raw.len() {
        let c = raw[at..].chars().next().unwrap();
        if c.is_whitespace() {
            at += c.len_utf8();
            continue;
        }
        if c == '#'
            && (at == indent
                || raw[..at]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace))
        {
            return Ok((tokens, at, None));
        }
        let start = at;
        if raw[at..].starts_with("'''") {
            let suffix = &raw[at + 3..];
            let tail = suffix.trim_start();
            let comment = suffix.starts_with(char::is_whitespace) && tail.starts_with('#');
            if !tail.is_empty() && !comment {
                return Err(Error::source(
                    no,
                    raw[..at].chars().count() + 1,
                    raw,
                    "a text block must open at the end of a line",
                ));
            }
            return Ok((tokens, at + 3, Some(offset + at)));
        }
        let kind = if c == '"' {
            at = text_end(raw, at, no)?;
            TokenKind::Text(raw[start + 1..at - 1].into())
        } else if c == '`' {
            let length = raw[at + 1..].find('`').ok_or_else(|| {
                Error::source(
                    no,
                    raw[..at].chars().count() + 1,
                    raw,
                    "unterminated quoted name",
                )
            })?;
            at += length + 2;
            let inner = &raw[start + 1..at - 1];
            if inner.starts_with('{') && closing_brace(inner) == Some(inner.len() - 1) {
                TokenKind::Computed(inner[1..inner.len() - 1].into())
            } else {
                TokenKind::QuotedName(inner.into())
            }
        } else if raw[at..].starts_with("./")
            || raw[at..].starts_with("../")
            || (c == '/'
                && !raw[at..].starts_with("//")
                && !raw[at + 1..].starts_with(char::is_whitespace)
                && at + 1 < raw.len())
        {
            at += c.len_utf8();
            while at < raw.len()
                && !raw[at..].starts_with(|c: char| c.is_whitespace() || ",()[]{}".contains(c))
            {
                at += raw[at..].chars().next().unwrap().len_utf8();
            }
            TokenKind::Path(raw[start..at].into())
        } else if let Some(op) = ["==", "!=", "<=", ">=", "++", "//", "=>", "..."]
            .into_iter()
            .find(|op| raw[at..].starts_with(op))
        {
            at += op.len();
            TokenKind::Symbol(op.into())
        } else if ":,.()[]{}=<>+*/|-".contains(c) {
            at += c.len_utf8();
            TokenKind::Symbol(c.to_string())
        } else if RESERVED.contains(c) && !(c == '?' && !raw[..at].ends_with(word_char)) {
            let message = if c == '!' {
                "`!` is reserved outside text, paths and comments; write `not x` or `a != b`".into()
            } else {
                format!("`{c}` is reserved outside text, paths and comments")
            };
            return Err(Error::source(
                no,
                raw[..at].chars().count() + 1,
                raw,
                message,
            ));
        } else {
            at += c.len_utf8();
            while at < raw.len() && raw[at..].starts_with(word_char) {
                at += raw[at..].chars().next().unwrap().len_utf8();
            }
            let word = &raw[start..at];
            if let Some(name) = word.strip_prefix('?') {
                TokenKind::Variable(name.into())
            } else if word.bytes().all(|c| c.is_ascii_digit()) {
                TokenKind::Integer(word.into())
            } else {
                TokenKind::Word(word.into())
            }
        };
        tokens.push(Token {
            kind,
            span: Span {
                start: offset + start,
                end: offset + at,
                line: no,
                column: raw[..start].chars().count() + 1,
            },
        });
    }
    Ok((tokens, raw.len(), None))
}

const RESERVED: &str = "!?%&^~@$;\\";

pub(crate) fn quoted_name(name: &str) -> Result<&str, String> {
    if !name.is_empty() && name.chars().all(word_char) {
        Ok(name)
    } else {
        Err(format!("backticks hold one bare name or one `{{e}}`, a computed name, not `{name}`"))
    }
}

fn closing_brace(inner: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (at, c) in inner.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => {}
        }
    }
    None
}

fn word_char(c: char) -> bool {
    !c.is_whitespace() && !":,.()[]{}=<>+*|/\"`".contains(c) && !RESERVED.contains(c)
}

fn text_end(raw: &str, start: usize, no: usize) -> Result<usize, Error> {
    let mut at = start + 1;
    let mut depth = 0;
    while at < raw.len() {
        let c = raw[at..].chars().next().unwrap();
        at += c.len_utf8();
        match c {
            '\\' => {
                let escaped = raw[at..].chars().next().ok_or_else(|| {
                    Error::source(
                        no,
                        raw[..start].chars().count() + 1,
                        raw,
                        "unterminated text literal",
                    )
                })?;
                if !"\"\\nt{".contains(escaped) {
                    return Err(Error::source(
                        no,
                        raw[..at].chars().count() + 1,
                        raw,
                        "invalid text escape",
                    ));
                }
                at += escaped.len_utf8();
            }
            '"' if depth == 0 => return Ok(at),
            '"' => at = text_end(raw, at - 1, no)?,
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ => {}
        }
    }
    Err(Error::source(
        no,
        raw[..start].chars().count() + 1,
        raw,
        "unterminated text literal",
    ))
}
