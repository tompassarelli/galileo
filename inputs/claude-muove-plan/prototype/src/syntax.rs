//! Lines, indentation blocks, and source errors.
//!
//! Source is line-oriented. A line's indentation nests it under the nearest
//! less-indented line above it. Blank lines and lines starting with `#` are
//! ignored.

use std::fmt;

#[derive(Clone, Debug)]
pub struct Line {
    pub no: usize,
    pub indent: usize,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub line: Line,
    pub children: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub line: usize,
    pub message: String,
    pub text: Option<String>,
}

impl Error {
    pub fn at(line: &Line, message: impl Into<String>) -> Self {
        Error {
            line: line.no,
            message: message.into(),
            text: Some(line.text.clone()),
        }
    }

    pub fn bare(message: impl Into<String>) -> Self {
        Error {
            line: 0,
            message: message.into(),
            text: None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "line {}: {}", self.line, self.message)?;
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

/// Split source into top-level blocks.
pub fn blocks(source: &str) -> Result<Vec<Block>, Error> {
    let mut lines = Vec::new();
    for (index, raw) in source.lines().enumerate() {
        let no = index + 1;
        // `#` starts a comment, at the start of a line or after a space.
        let uncommented = match raw.find(" #") {
            Some(at) => &raw[..at],
            None => raw,
        };
        let trimmed = uncommented.trim_end();
        let text = trimmed.trim_start();
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        let indent_text = &trimmed[..trimmed.len() - text.len()];
        if indent_text.contains('\t') {
            return Err(Error {
                line: no,
                message: "indent with spaces, not tabs".into(),
                text: Some(text.into()),
            });
        }
        lines.push(Line {
            no,
            indent: indent_text.len(),
            text: text.to_string(),
        });
    }

    let mut index = 0;
    let mut out = Vec::new();
    while index < lines.len() {
        if lines[index].indent != 0 {
            return Err(Error::at(&lines[index], "unexpected indentation"));
        }
        out.push(block(&lines, &mut index)?);
    }
    Ok(out)
}

fn block(lines: &[Line], index: &mut usize) -> Result<Block, Error> {
    let line = lines[*index].clone();
    *index += 1;
    let mut children = Vec::new();
    if *index < lines.len() && lines[*index].indent > line.indent {
        let child_indent = lines[*index].indent;
        while *index < lines.len() && lines[*index].indent > line.indent {
            if lines[*index].indent != child_indent {
                return Err(Error::at(
                    &lines[*index],
                    "indentation doesn't line up with the lines above it",
                ));
            }
            children.push(block(lines, index)?);
        }
    }
    Ok(Block { line, children })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nests_by_indentation_and_skips_comments() {
        let parsed = blocks("a\n  b\n    c\n  # note\n  d\ne\n").unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].children.len(), 2);
        assert_eq!(parsed[0].children[0].children[0].line.text, "c");
        assert_eq!(parsed[1].line.text, "e");
    }

    #[test]
    fn rejects_ragged_indentation() {
        assert!(blocks("a\n    b\n  c\n").is_err());
    }
}
