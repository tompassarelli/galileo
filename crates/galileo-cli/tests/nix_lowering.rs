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

    let source = "nix file\n  let a = \"abc\"\n  value\n    x: length(\"abc\")\n    scoped: length(a)\n    text: \"x{1}\"\n    computed: \"x{1 + 1}\"\n    member: \"a\" in [\"a\", \"b\"]\n";
    let (out, nix) = build("known-computation", source);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let built = std::fs::read_to_string(&nix).unwrap();
    for expected in ["x = 3;", "scoped = 3;", "text = \"x1\";", "computed = \"x2\";", "member = true;"] {
        assert!(built.contains(expected), "{built}");
    }

    for (name, source, line, column, offer) in [
        ("staged-prelude", "nix module\n  arguments: config, lib\n  option extraArgs: Text\n  option x: Int\n  let cfg = config\n  config\n    x: length(cfg.extraArgs)\n", 7, 8, "builtins.length"),
        ("staged-in", "nix module\n  arguments: config, lib\n  option extraArgs: List Text\n  option x: Bool\n  let cfg = config\n  config\n    x: \"a\" in cfg.extraArgs\n", 7, 15, "builtins.elem"),
        ("bool-interpolation", "nix file\n  value: \"x{true}\"\n", 2, 13, "Text, Int"),
        ("untyped-concat", "nix file\n  arguments: a, b\n  value: a ++ b\n", 3, 12, "typed operand"),
        ("duplicate-let", "nix module\n  arguments: lib\n  option value: Int\n  let a = 1\n  config\n    value: a\n  let a = 2\n", 7, 7, "already bound"),
    ] {
        let (out, nix) = build(name, source);
        assert!(!out.status.success());
        let stderr = String::from_utf8_lossy(&out.stderr);
        let diagnostics: Vec<_> = stderr.lines().filter(|line| line.contains("probe.gal:")).collect();
        assert_eq!(diagnostics.len(), 1, "{stderr}");
        assert!(diagnostics[0].contains(&format!("probe.gal:{line}:{column}:")), "{stderr}");
        assert!(stderr.contains(offer), "{stderr}");
        assert!(!nix.exists());
    }
}

#[test]
fn spec_9_4_lambda_default_goldens_evaluate_in_nix() {
    let golden = |name: &str| std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/lowering").join(format!("{name}.gal"))).unwrap();
    let (out, nix) = build("lambda-defaults", &golden("lambda-defaults"));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let expr = format!("let f = (import {}).makeHost; in builtins.toJSON [ (builtins.functionArgs f) (f {{ hostname = \"h\"; other = 1; }}) ]", nix.display());
    let eval = Command::new("nix-instantiate").args(["--eval", "--strict", "--expr", &expr]).output().unwrap();
    assert!(eval.status.success(), "{}", String::from_utf8_lossy(&eval.stderr));
    assert_eq!(String::from_utf8_lossy(&eval.stdout).trim(), r#""[{\"extraModules\":true,\"hostname\":false,\"system\":true},\"h\"]""#);

    let (out, nix) = build("lambda-outer-default", &golden("lambda-outer-default"));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let eval = Command::new("nix-instantiate").args(["--eval", "--strict", "--expr", &format!("(import {}) {{}}", nix.display())]).output().unwrap();
    assert!(eval.status.success(), "{}", String::from_utf8_lossy(&eval.stderr));
    assert_eq!(String::from_utf8_lossy(&eval.stdout).trim(), "\"x86_64-linux\"");
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
