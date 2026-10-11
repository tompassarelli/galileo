use crate::lexer::{check_layout, tokenize};
use crate::Error;

enum Item {
    Blank,
    Comment(usize, String),
    Code {
        depth: usize,
        text: String,
        block: Option<Vec<String>>,
    },
}

fn error(no: usize, column: usize, raw: &str, message: &str) -> Error {
    Error {
        line: no,
        column,
        message: message.into(),
        text: Some(raw.into()),
    }
}

pub fn format(source: &str) -> Result<String, Error> {
    crate::blocks(source)?;
    let physical: Vec<&str> = source.split_terminator('\n').collect();
    let mut items = Vec::new();
    let mut index = 0;
    while index < physical.len() {
        let raw = physical[index];
        let no = index + 1;
        index += 1;
        check_layout(raw, no)?;
        let indent = raw.bytes().take_while(|c| *c == b' ').count();
        let (tokens, end, mut opening) = tokenize(raw, no, 0, indent)?;
        let comment = raw[end..].trim();
        if tokens.is_empty() && opening.is_none() {
            items.push(if comment.is_empty() {
                Item::Blank
            } else {
                Item::Comment(indent / 2, comment.into())
            });
            continue;
        }
        let depth = indent / 2;
        let mut text = String::new();
        let mut previous = None;
        for token in &tokens {
            if previous.is_some_and(|p| p < token.span.start) {
                text.push(' ');
            }
            text.push_str(&raw[token.span.start..token.span.end]);
            previous = Some(token.span.end);
        }
        if let Some(start) = opening {
            if previous.is_some_and(|p| p < start) {
                text.push(' ');
            }
            text.push_str("'''");
        }
        if !comment.is_empty() {
            text.push(' ');
            text.push_str(comment);
        }
        let mut block = Vec::new();
        while opening.is_some() {
            let close = physical[index..]
                .iter()
                .position(|line| line.trim_start_matches(' ').starts_with("'''"))
                .map(|at| index + at)
                .ok_or_else(|| error(no, indent + 1, raw, "unterminated text block"))?;
            let marker = physical[close].bytes().take_while(|c| *c == b' ').count();
            let mut content = Vec::new();
            for (at, line) in physical.iter().enumerate().take(close).skip(index) {
                check_layout(line, at + 1)?;
                let spaces = line.bytes().take_while(|c| *c == b' ').count();
                if spaces == line.len() {
                    content.push(line.get(marker..).map(|rest| (rest.len(), "")));
                } else if spaces < marker {
                    return Err(error(
                        at + 1,
                        spaces + 1,
                        line,
                        "text block content is left of its closing marker",
                    ));
                } else {
                    content.push(Some((0, &line[marker..])));
                }
            }
            index = close + 1;
            let column = 2 * depth + 2;
            block.extend(content.into_iter().map(|line| match line {
                Some((pad, rest)) if pad > 0 || !rest.is_empty() => {
                    format!("{}{}{rest}", " ".repeat(column), " ".repeat(pad))
                }
                _ => String::new(),
            }));
            let raw = physical[close];
            let (tokens, end, next) = tokenize(raw, close + 1, 0, marker + 3)?;
            let mut closer = format!("{}'''", " ".repeat(column));
            let mut previous = marker + 3;
            for token in tokens {
                if previous < token.span.start {
                    closer.push(' ');
                }
                closer.push_str(&raw[token.span.start..token.span.end]);
                previous = token.span.end;
            }
            if let Some(start) = next {
                if previous < start {
                    closer.push(' ');
                }
                closer.push_str("'''");
            }
            let comment = raw[end..].trim();
            if !comment.is_empty() {
                closer.push(' ');
                closer.push_str(comment);
            }
            block.push(closer);
            opening = next;
        }
        let block = (!block.is_empty()).then_some(block);
        items.push(Item::Code { depth, text, block });
    }
    let mut out = String::new();
    let mut blank = false;
    for (at, item) in items.iter().enumerate() {
        let (depth, text, block) = match item {
            Item::Blank => {
                blank = !out.is_empty();
                continue;
            }
            Item::Comment(own, text) => {
                let code = |item: &Item| match item {
                    Item::Code { depth, .. } => Some(*depth),
                    _ => None,
                };
                let previous = items[..at].iter().rev().find_map(code).unwrap_or(0);
                let next = items[at..].iter().find_map(code).unwrap_or(0);
                (
                    (*own).clamp(previous.min(next), previous.max(next)),
                    text,
                    None,
                )
            }
            Item::Code { depth, text, block } => (*depth, text, block.as_ref()),
        };
        if blank {
            out.push('\n');
            blank = false;
        }
        out.push_str(&" ".repeat(2 * depth));
        out.push_str(text);
        out.push('\n');
        if let Some(lines) = block {
            for line in lines {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    Ok(out)
}
