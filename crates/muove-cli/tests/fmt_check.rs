use std::path::Path;
use std::process::{Command, Output};

const SOURCE: &str = include_str!("../../../tests/external/kanata/kanata.muove");

fn fmt(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_muove")).arg("fmt").args(arguments).output().unwrap()
}

fn scratch(name: &str, text: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("muove-fmt-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("kanata.muove");
    std::fs::write(&file, text).unwrap();
    file
}

fn clean(file: &Path) {
    std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
}

#[test]
fn fmt_check_exits_1_on_a_doubled_space_and_fmt_restores_the_original() {
    let messy = format!("{}\n\n", SOURCE.replacen("let cfg", "let  cfg", 1));
    let file = scratch("messy", &messy);
    let path = file.to_str().unwrap();
    assert_eq!(fmt(&["--check", path]).status.code(), Some(1));
    assert_eq!(fmt(&[path]).status.code(), Some(0));
    assert_eq!(fmt(&["--check", path]).status.code(), Some(0));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), SOURCE);
    clean(&file);
}

#[test]
fn fmt_refuses_a_line_indented_3_or_6_spaces_and_leaves_the_file_unchanged() {
    for spaces in ["   ", "      "] {
        let bad = SOURCE.replacen("\n  let cfg", &format!("\n{spaces}let cfg"), 1);
        let file = scratch(&spaces.len().to_string(), &bad);
        let path = file.to_str().unwrap();
        for arguments in [vec!["--check", path], vec![path]] {
            let output = fmt(&arguments);
            assert_eq!(output.status.code(), Some(2));
            assert!(String::from_utf8_lossy(&output.stderr).contains(&format!("{path}:line 3:")));
            assert_eq!(std::fs::read_to_string(&file).unwrap(), bad);
        }
        clean(&file);
    }
}
