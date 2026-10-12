use crate::{value, Result};
use std::collections::BTreeSet;
use std::path::PathBuf;

pub fn run(args: Vec<String>) -> Result<()> {
    let mut physical = false;
    let mut lang = String::from("unspecified");
    let mut part = String::from("modules");
    let mut files = BTreeSet::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--physical" => physical = true,
            "--lang" => lang = value(&mut args, &arg)?,
            "--part" => part = value(&mut args, &arg)?,
            "--help" | "-h" => {
                println!("Usage: compare size [--physical] [--lang COLUMN] [--part PART] FILE...\n\nPrint one TSV row: column, part, files, lines. Default excludes blank and\ncomment-only lines; --physical counts newline bytes exactly as wc -l.\nInputs must already be formatted with the column's own formatter.");
                return Ok(());
            }
            "--" => {
                files.extend(args.map(PathBuf::from));
                break;
            }
            _ if arg.starts_with('-') => return Err(format!("unknown option: {arg}").into()),
            _ => {
                files.insert(PathBuf::from(arg));
            }
        }
    }
    if files.is_empty() {
        return Err("size needs at least one FILE".into());
    }
    let mut lines = 0;
    for file in &files {
        let source = std::fs::read(file).map_err(|error| format!("{}: {error}", file.display()))?;
        lines += if physical {
            source.iter().filter(|&&byte| byte == b'\n').count()
        } else {
            let source = std::str::from_utf8(&source)?;
            let extension = file.extension().and_then(|ext| ext.to_str()).unwrap_or("");
            logical_lines(source, extension)
        };
    }
    println!(
        "column\tpart\tfiles\tlines\n{lang}\t{part}\t{}\t{lines}",
        files.len()
    );
    Ok(())
}

fn logical_lines(source: &str, extension: &str) -> usize {
    let line_comments: &[&str] = match extension {
        "bnix" | "bjs" => &[";"],
        "dhall" => &["--"],
        "clause" => &["//", "#"],
        "cue" | "rs" => &["//"],
        _ => &["#"],
    };
    let block = if extension == "dhall" {
        ("{-", "-}")
    } else {
        ("/*", "*/")
    };
    let mut depth = 0;
    let mut quote: Option<&str> = None;
    let mut count = 0;
    for line in source.lines() {
        let mut code = quote.is_some();
        let mut at = 0;
        while at < line.len() {
            let rest = &line[at..];
            if depth > 0 {
                if rest.starts_with(block.1) {
                    depth -= 1;
                    at += 2;
                } else if rest.starts_with(block.0) {
                    depth += 1;
                    at += 2;
                } else {
                    at += rest.chars().next().unwrap().len_utf8();
                }
            } else if let Some(delimiter) = quote {
                if rest.starts_with(delimiter) {
                    quote = None;
                    at += delimiter.len();
                } else if rest.starts_with('\\') && delimiter.len() == 1 {
                    at += 1;
                    if at < line.len() {
                        at += line[at..].chars().next().unwrap().len_utf8();
                    }
                } else {
                    at += rest.chars().next().unwrap().len_utf8();
                }
            } else if rest.starts_with(block.0) {
                depth = 1;
                at += 2;
            } else if line_comments.iter().any(|prefix| rest.starts_with(prefix)) {
                break;
            } else if rest.starts_with("\"\"\"") {
                code = true;
                quote = Some("\"\"\"");
                at += 3;
            } else if rest.starts_with("'''") {
                code = true;
                quote = Some("'''");
                at += 3;
            } else if rest.starts_with("''") {
                code = true;
                quote = Some("''");
                at += 2;
            } else if rest.starts_with('"') {
                code = true;
                quote = Some("\"");
                at += 1;
            } else if rest.starts_with('\'') && matches!(extension, "k" | "cue") {
                code = true;
                quote = Some("'");
                at += 1;
            } else {
                let character = rest.chars().next().unwrap();
                code |= !character.is_whitespace();
                at += character.len_utf8();
            }
        }
        count += usize::from(code && !line.trim().is_empty());
    }
    count
}
