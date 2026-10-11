use std::path::Path;
use std::process::Command;

fn build(name: &str, source: &str) -> (std::process::Output, std::path::PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("probe.gal"), source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).arg("build").arg(dir.join("probe.gal")).output().unwrap();
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
fn spec_9_4_brace_lambdas_lower_defaults_and_report_invalid_names() {
    let source = "nix file\n  value\n    makeHost: ({hostname, system = \"x86_64-linux\", extraModules = [], ...}) => hostname\n";
    let (out, nix) = build("lambda-defaults", source);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(std::fs::read_to_string(&nix).unwrap(), "{\n  makeHost = ({ hostname, system ? \"x86_64-linux\", extraModules ? [ ], ... }: hostname);\n}\n");
    let parsed = Command::new("nix-instantiate").arg("--parse").arg(&nix).output().unwrap();
    assert!(parsed.status.success(), "{}", String::from_utf8_lossy(&parsed.stderr));

    let source = "nix file\n  value: ({hostname, system = hostname}) => system\n";
    let (out, nix) = build("lambda-default-scope", source);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("probe.gal:2:") && stderr.contains("formal `hostname`"), "{stderr}");
    assert!(!nix.exists());

    let source = "nix file\n  let x = \"outer\"\n  value: ({x, y = x}) => y\n";
    let (out, nix) = build("lambda-shadowed-default", source);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("probe.gal:3:") && stderr.contains("formal `x`"), "{stderr}");
    assert!(!nix.exists());

    let source = "nix file\n  let defaultSystem = \"x86_64-linux\"\n  value: ({system = defaultSystem}) => system\n";
    let (out, nix) = build("lambda-outer-default", source);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let eval = Command::new("nix-instantiate").args(["--eval", "--strict", "--expr", &format!("(import {}) {{}}", nix.display())]).output().unwrap();
    assert!(eval.status.success(), "{}", String::from_utf8_lossy(&eval.stderr));
    assert_eq!(String::from_utf8_lossy(&eval.stdout).trim(), "\"x86_64-linux\"");

    let source = "nix module\n  arguments: lib, ...\n  config\n    value: ((`if`) => `if`)(1)\n";
    let (out, nix) = build("keyword-lambda", source);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("probe.gal:line 4:") && stderr.contains("`if` is a Nix keyword"), "{stderr}");
    assert!(!nix.exists());
}

#[test]
fn flake_root_let_is_refused_at_its_line() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flake-root-let");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = "nix file\n  let marker = 42\n  value\n    description: \"d\"\n    outputs: ({self}) => { result: marker }\n";
    std::fs::write(dir.join("flake.gal"), source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).arg("build").arg(dir.join("flake.gal")).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("flake.gal:line 2:") && stderr.contains("`outputs`"), "{stderr}");
    assert!(!dir.join("flake.nix").exists());
}

#[test]
fn d_59_linked_build_accepts_module_and_submodule_scaffolds() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("d-59-linked-build");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert!(Command::new("git").args(["init", "--quiet"]).arg(&dir).status().unwrap().success());
    let module = dir.join("modules/sample-tool/default.gal");
    std::fs::create_dir_all(module.parent().unwrap()).unwrap();
    let prefix = "use firn/tags\n\nsample-tool is a Module\n\n";
    let sources = [
        "nix module\n  arguments: config, lib, ...\n  option myConfig.modules.sample-tool.enable: enable \"Enable sample-tool\"\n  config when config.myConfig.modules.sample-tool.enable\n    sample: true\n",
        "nix file\n  arguments: config, lib, ...\n  value\n    options.myConfig.modules.sample-tool\n      enable: lib.mkEnableOption(\"sample-tool configuration\")\n      extraConfig: lib.mkOption({type: lib.types.lines, default: \"\", description: \"Extra config text.\"})\n    config when config.myConfig.modules.sample-tool.enable\n      sample: true\n",
    ];
    for (i, source) in sources.iter().enumerate() {
        std::fs::write(&module, format!("{prefix}{source}")).unwrap();
        for command in ["check", "build"] {
            let out = Command::new(env!("CARGO_BIN_EXE_galileo")).current_dir(&dir).args([command, "modules/sample-tool/default.gal"]).output().unwrap();
            assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
            assert!(out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));
        }
        let built = std::fs::read_to_string(module.with_extension("nix")).unwrap();
        assert!(built.contains("sample = true;"), "{built}");
        if i == 1 {
            assert_eq!(built.matches("type = lib.types.lines;").count(), 1, "{built}");
        }
    }
    std::fs::write(&module, "nix file\n  arguments: lib\n  value: lib.mkEnableOption(1)\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).current_dir(&dir).args(["build", "modules/sample-tool/default.gal"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(stderr.lines().filter(|line| line.starts_with("modules/sample-tool/default.gal:")).count(), 1, "{stderr}");
    assert!(stderr.contains("Text"), "{stderr}");
}
