use muove_syntax::{blocks, fmt::format, Block, TokenKind};
use std::path::{Path, PathBuf};

fn corpus(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if path.is_dir() && !name.starts_with('.') && name != "target" {
            corpus(&path, out);
        } else if name.ends_with(".muove") {
            out.push(path);
        }
    }
}

#[derive(Debug, PartialEq)]
struct Tree(usize, Vec<TokenKind>, Vec<Tree>);

fn tree(block: &Block) -> Tree {
    Tree(
        block.line.indent,
        block.line.tokens.iter().map(|t| t.kind.clone()).collect(),
        block.children.iter().map(tree).collect(),
    )
}

#[test]
fn fmt_is_idempotent_and_keeps_the_parse_tree_on_every_repository_source() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    corpus(&root, &mut files);
    assert!(!files.is_empty());
    for path in &files {
        let source = std::fs::read_to_string(path).unwrap();
        let once = format(&source).unwrap();
        assert_eq!(format(&once).unwrap(), once, "{}", path.display());
        let parse = |text: &str| blocks(text).unwrap().iter().map(tree).collect::<Vec<_>>();
        assert_eq!(parse(&once), parse(&source), "{}", path.display());
        assert_eq!(
            once.split_whitespace().collect::<Vec<_>>(),
            source.split_whitespace().collect::<Vec<_>>(),
            "{}",
            path.display()
        );
    }
}

#[test]
fn a_comment_after_the_last_entry_of_a_nested_block_keeps_that_entry_depth() {
    let source = "a\n  b\n    c: 1\n    # d: 2\ne: 3\n";
    assert_eq!(format(source).unwrap(), source);
}
