use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::io::Write;
use std::process::{Command, Output, Stdio};

#[path = "fixture.rs"]
mod fixture;
use fixture::{firn_root, module_sources};

const TAGS: &str = include_str!("../../../lib/firn/tags.gal");

fn marker_sections(flake: &str) -> Vec<(String, Vec<String>)> {
    let mut sections = Vec::new();
    let mut open: Option<(String, Vec<String>)> = None;
    for line in flake.lines() {
        let content = line.trim();
        if let Some((_, rest)) = content.split_once(";; --- GENERATED ") {
            open = Some((rest.trim_end_matches(" (do not edit) ---").to_string(), Vec::new()));
        } else if content.contains(";; --- END GENERATED ") {
            sections.push(open.take().unwrap());
        } else if let Some((_, lines)) = open.as_mut() {
            lines.push(content.to_string());
        }
    }
    sections
}

fn scratch(name: &str, facts: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let module = include_str!("flake.gal");
    std::fs::write(dir.join("flake.gal"), format!("{TAGS}\n{facts}\n{module}")).unwrap();
    dir
}

fn galileo(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_galileo")).args(arguments).output().unwrap()
}

fn derived_pairs(dir: &Path) -> BTreeSet<(String, String)> {
    let out = galileo(&["query", dir.to_str().unwrap(), "all ?n, ?u where ?n flake input url ?u", "--json"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let json = String::from_utf8(out.stdout).unwrap();
    let strings: Vec<String> = json.split('"').skip(1).step_by(2).map(String::from).collect();
    strings
        .chunks(4)
        .map(|row| match row {
            [n, name, u, url] if n == "n" && u == "u" => (name.clone(), url.clone()),
            other => panic!("unexpected row {other:?} in {json}"),
        })
        .collect()
}

#[test]
fn flake_gal_builds_the_committed_flake_with_marker_section_inputs_and_a_second_url_refuses_to_build() {
    let root = firn_root();
    let root = Path::new(&root);
    let sections = marker_sections(&std::fs::read_to_string(root.join("flake.bnix")).unwrap());
    let expected: BTreeSet<(String, String)> = sections[0]
        .1
        .iter()
        .map(|line| {
            let name = line.split_whitespace().next().unwrap().trim_start_matches(':').to_string();
            let url = line.split(":url \"").nth(1).unwrap().split('"').next().unwrap().to_string();
            (name, url)
        })
        .collect();
    let names: BTreeSet<String> = expected.iter().map(|(name, _)| name.clone()).collect();
    for (marker, lines) in &sections[1..] {
        let section: BTreeSet<String> =
            lines.iter().map(|l| l.split_whitespace().next().unwrap().trim_start_matches(':').to_string()).collect();
        assert_eq!(section, names, "{marker}");
    }

    let modules = module_sources(root);
    let dir = scratch("flake-inputs", &modules);
    let derived = derived_pairs(&dir);
    println!("{} derived pairs, {} marker pairs", derived.len(), expected.len());
    assert_eq!(derived, expected);
    let built = galileo(&["build", dir.to_str().unwrap()]);
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let emitted = std::fs::read_to_string(dir.join("flake.nix")).unwrap();
    let current = std::fs::read_to_string(root.join("flake.nix")).unwrap();
    let first = format!("    {} = {{\n", expected.first().unwrap().0);
    let section = |flake: &str| {
        let start = flake.find(&first).unwrap();
        let end = start + flake[start..].find("\n  };\n").unwrap() + 1;
        flake[start..end].to_string()
    };
    let (emitted_section, current_section) = (section(&emitted), section(&current));
    println!(
        "{} input pairs, {} emitted bytes, {} marker bytes, byte cmp {}",
        derived.len(), emitted_section.len(), current_section.len(), if emitted_section == current_section { 0 } else { 1 }
    );
    assert_eq!(emitted_section, current_section);
    let parsed: Vec<Vec<u8>> = [&current, &emitted]
        .iter()
        .map(|text| {
            let mut child = Command::new("nix-instantiate")
                .args(["--parse", "-"])
                .current_dir(root)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            output.stdout
        })
        .collect();
    println!("flake.nix parse-identical: {}", parsed[0] == parsed[1]);
    assert!(parsed[0] == parsed[1], "flake.gal's flake.nix parses differently from {}", root.join("flake.nix").display());

    let names_in = |flake: &str| -> BTreeSet<String> {
        flake.split(|c: char| !(c.is_ascii_alphanumeric() || "_'-".contains(c))).map(String::from).collect()
    };
    assert!(!names_in(&emitted).contains("extra"));
    let dir = scratch("flake-inputs-extra", &(modules.clone() + "glide needs flake input \"extra\" from \"github:example/extra\"\n"));
    let built = galileo(&["build", dir.to_str().unwrap()]);
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let extra = std::fs::read_to_string(dir.join("flake.nix")).unwrap();
    let outputs = extra.lines().find(|line| line.trim_start().starts_with("outputs = ")).unwrap();
    let formals = &outputs[..outputs.find("}:").unwrap()];
    println!("extra formal: {}, extra = extra; records: {}", names_in(formals).contains("extra"), extra.matches("extra = extra;").count());
    assert!(names_in(formals).contains("extra"), "{formals}");
    assert_eq!(extra.matches("extra = extra;").count(), 4);

    let dir = scratch("flake-inputs-captured", &(modules.clone() + "glide needs flake input \"pkgs\" from \"github:example/pkgs\"\n"));
    let refused = galileo(&["build", dir.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&refused.stderr);
    println!("{stderr}");
    assert_eq!(refused.status.code(), Some(1));
    assert!(!dir.join("flake.nix").exists());
    let diagnostics: Vec<&str> = stderr.lines().filter(|line| line.contains("`pkgs`")).collect();
    assert_eq!(diagnostics.len(), 2, "{stderr}");

    let dir = scratch("flake-inputs-conflict", &(modules + "glide needs flake input \"nur\" from \"github:example/NUR\"\n"));
    let refused = galileo(&["build", dir.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&refused.stderr);
    println!("{stderr}");
    assert!(!refused.status.success());
    assert!(!dir.join("flake.nix").exists());
    assert!(stderr.starts_with("Fails: "), "{stderr}");
    for module in ["firefox needs flake input", "glide needs flake input"] {
        assert!(stderr.contains(module), "{module}: {stderr}");
    }
}
