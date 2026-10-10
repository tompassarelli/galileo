use std::path::Path;
use std::process::Command;

fn build(name: &str, source: &str) -> (std::process::Output, std::path::PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("probe.muove"), source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_muove")).arg("build").arg(dir.join("probe.muove")).output().unwrap();
    (out, dir.join("probe.nix"))
}

#[test]
fn text_concat_lowers_to_nix_string_addition_and_evaluates() {
    let source = "nix module\n  arguments: config, lib, ...\n  option name: text\n  option known: text\n  option staged: text\n  option list: list of int\n  config\n    known: \"a\" ++ \"b\"\n    staged: config.name ++ \"b\"\n    list: [1] ++ [2]\n";
    let (out, nix) = build("text-concat", source);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let expr = format!(
        "builtins.toJSON (import {} {{ lib.mkOption = x: x; lib.types = {{ str = \"str\"; int = \"int\"; listOf = t: t; }}; config.name = \"a\"; }}).config",
        nix.display()
    );
    let eval = Command::new("nix-instantiate").args(["--eval", "--strict", "--expr", &expr]).output().unwrap();
    assert!(eval.status.success(), "{}", String::from_utf8_lossy(&eval.stderr));
    assert_eq!(String::from_utf8_lossy(&eval.stdout).trim(), r#""{\"known\":\"ab\",\"list\":[1,2],\"staged\":\"ab\"}""#);
}

#[test]
fn nix_keyword_name_is_a_build_diagnostic_at_its_line() {
    let source = "nix module\n  arguments: lib, ...\n  config\n    value: ((`if`) => `if`)(1)\n";
    let (out, nix) = build("keyword-lambda", source);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("probe.muove:line 4:") && stderr.contains("`if` is a Nix keyword"), "{stderr}");
    assert!(!nix.exists());
}
