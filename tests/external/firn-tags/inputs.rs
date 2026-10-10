use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "fixture.rs"]
mod fixture;
use fixture::{firn_root, module_sources};

const TAGS: &str = include_str!("../../../lib/firn/tags.muove");

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
    let module = "nix module\n  arguments: config, lib, pkgs, ...\n  for ?name in all ?n where ?n flake input url ?_\n    myConfig.inputs.{?name}: true\n";
    std::fs::write(dir.join("inputs.muove"), format!("{TAGS}\n{facts}\n{module}")).unwrap();
    dir
}

fn muove(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_muove")).args(arguments).output().unwrap()
}

fn derived_pairs(dir: &Path) -> BTreeSet<(String, String)> {
    let out = muove(&["query", dir.to_str().unwrap(), "all ?n, ?u where ?n flake input url ?u", "--json"]);
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
fn flake_input_urls_equal_the_marker_sections_and_a_second_url_refuses_to_build() {
    let root = firn_root();
    let root = Path::new(&root);
    let sections = marker_sections(&std::fs::read_to_string(root.join("flake.bnix")).unwrap());
    assert_eq!(sections.len(), 6);
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
    let built = muove(&["build", dir.to_str().unwrap()]);
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    assert!(dir.join("inputs.nix").exists());

    let dir = scratch("flake-inputs-conflict", &(modules + "glide needs flake input \"nur\" from \"github:example/NUR\"\n"));
    let refused = muove(&["build", dir.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&refused.stderr);
    println!("{stderr}");
    assert!(!refused.status.success());
    assert!(!dir.join("inputs.nix").exists());
    assert!(stderr.starts_with("Fails: "), "{stderr}");
    for module in ["firefox needs flake input", "glide needs flake input"] {
        assert!(stderr.contains(module), "{module}: {stderr}");
    }
}
