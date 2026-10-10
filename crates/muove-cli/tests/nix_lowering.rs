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
    let source = "nix module\n  arguments: config, lib, ...\n  option name: Text\n  option known: Text\n  option staged: Text\n  option list: List Int\n  config\n    known: \"a\" ++ \"b\"\n    staged: config.name ++ \"b\"\n    list: [1] ++ [2]\n";
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

#[test]
fn flake_root_let_is_refused_at_its_line() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flake-root-let");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = "nix file\n  let marker = 42\n  value\n    description: \"d\"\n    outputs: ({self}) => { result: marker }\n";
    std::fs::write(dir.join("flake.muove"), source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_muove")).arg("build").arg(dir.join("flake.muove")).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("flake.muove:line 2:") && stderr.contains("`outputs`"), "{stderr}");
    assert!(!dir.join("flake.nix").exists());
}
