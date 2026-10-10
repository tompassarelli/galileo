use crate::{lex, Error, Line};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub line: Line,
    pub children: Vec<Block>,
}

pub fn blocks(source: &str) -> Result<Vec<Block>, Error> {
    let lines = lex(source)?;
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
    while *index < lines.len() && lines[*index].indent > line.indent {
        if lines[*index].indent != line.indent + 2 {
            return Err(Error::at(
                &lines[*index],
                "each indentation level must be two spaces",
            ));
        }
        children.push(block(lines, index)?);
    }
    Ok(Block { line, children })
}
