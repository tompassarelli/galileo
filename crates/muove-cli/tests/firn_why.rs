use muove_core::{evaluate, Budget, RelationId, Value};
use muove_import::bnix_tags::{clause_facts, host_facts, module_facts};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

type Attribution = BTreeMap<String, BTreeSet<String>>;

fn firn_root() -> PathBuf {
    PathBuf::from(std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!("{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9", std::env::var("HOME").unwrap())
    }))
}

fn library() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib/firn/tags.muove")
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn sources(root: &Path, out: &Path, host: &str) -> Vec<PathBuf> {
    let _ = std::fs::remove_dir_all(out.join("modules"));
    let mut names: Vec<String> = std::fs::read_dir(root.join("modules"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    for name in &names {
        let directory = root.join("modules").join(name);
        let clause = directory.join("tags.clause");
        let beagle = directory.join("default.bnix");
        let facts = if clause.exists() {
            clause_facts(name, &std::fs::read_to_string(&clause).unwrap()).unwrap()
        } else if beagle.exists() {
            module_facts(name, &std::fs::read_to_string(&beagle).unwrap()).unwrap()
        } else {
            continue;
        };
        if !facts.is_empty() {
            write(&out.join(format!("modules/{name}/default.muove")), &facts);
        }
    }
    let tags = std::fs::read_to_string(root.join(format!("hosts/{host}/enabled-tags.bnix"))).unwrap();
    let host_file = out.join(format!("hosts/{host}/tags.muove"));
    write(&host_file, &host_facts(host, &tags).unwrap());
    vec![host_file, out.join("modules"), library()]
}

fn muove(arguments: &[&str], paths: &[PathBuf], cwd: &Path) -> (Option<i32>, String) {
    let relative = paths.iter().map(|p| p.strip_prefix(cwd).unwrap_or(p));
    let output = Command::new(env!("CARGO_BIN_EXE_muove")).args(arguments).args(relative).current_dir(cwd).output().unwrap();
    assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
    (output.status.code(), String::from_utf8_lossy(&output.stdout).into_owned())
}

fn attribution_by_muove(paths: &[PathBuf], host: &str) -> Attribution {
    let mut files = Vec::new();
    for path in paths {
        if path.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                files.push(entry.unwrap().path().join("default.muove"));
            }
        } else {
            files.push(path.clone());
        }
    }
    let texts: Vec<(String, String)> = files.iter().map(|f| (f.display().to_string(), std::fs::read_to_string(f).unwrap())).collect();
    let named: Vec<(&str, &str)> = texts.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let lowered = muove_syntax::lower::lower_files(&named).unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let program = lowered.program;
    let by = program.relations.iter().position(|r| r.slots.len() == 3 && r.template.contains("is enabled on") && !r.template.contains("default")).map(RelationId);
    let model = evaluate(&program, &[], Budget::default()).unwrap();
    let mut out = Attribution::new();
    for tuple in model.tuples(by.unwrap()) {
        if let [Value::Entity(m), Value::Entity(h), Value::Entity(t)] = tuple.as_slice() {
            if h.name == host {
                out.entry(m.name.clone()).or_default().insert(t.name.clone());
            }
        }
    }
    out
}

fn attribution_by_firn(root: &Path, scratch: &Path, host: &str) -> Attribution {
    let copy = scratch.join("firn");
    if !copy.exists() {
        assert!(Command::new("cp").arg("-r").arg(root).arg(&copy).status().unwrap().success());
    }
    let output = Command::new("firn").args(["tag", "resolve", host]).env("FIRN_REPO", &copy).current_dir(&copy).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8(output.stdout).unwrap();
    let section = text.split("Per-tag contributions:").nth(1).unwrap().split("\nActive modules").next().unwrap();
    let mut out = Attribution::new();
    let mut tag = String::new();
    for line in section.lines() {
        if let Some(header) = line.strip_prefix("  ").filter(|l| !l.starts_with(' ')) {
            tag = header.split(" (").next().unwrap().to_string();
        } else if let Some(module) = line.strip_prefix("    ").filter(|l| !l.starts_with(' ')) {
            out.entry(module.to_string()).or_default().insert(tag.clone());
        }
    }
    out
}

#[test]
fn firefox_proof_and_every_tag_attribution_match_firn_tag_resolve() {
    let root = firn_root();
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("firn-why");
    let host = "whiterabbit";
    let out = scratch.join(host);
    let paths = sources(&root, &out, host);
    let (code, proof) = muove(&["why", "firefox", "is", "enabled", "on", "whiterabbit"], &paths, &out);
    println!("{proof}");
    assert_eq!(code, Some(0));
    assert!(proof.starts_with("firefox is enabled on whiterabbit    ← enabled"), "{proof}");
    assert!(proof.contains("firefox is enabled on whiterabbit by browsers    ← default-membership-enables"));
    assert!(proof.contains("whiterabbit enables browsers    "));
    assert!(proof.contains("firefox is tagged browsers    "));
    assert!(proof.contains("nothing states: whiterabbit drops firefox from browsers    (checked: "));
    assert!(proof.contains("└─ nothing states: whiterabbit disables firefox    (checked: "));
    let (code, near) = muove(&["why", "not", "anytype", "is", "enabled", "on", "whiterabbit"], &paths, &out);
    println!("{near}");
    assert_eq!(code, Some(0));
    assert!(near.contains("whiterabbit disables anytype"), "{near}");

    for host in ["whiterabbit"] {
        let paths = sources(&root, &scratch.join(host), host);
        let ours = attribution_by_muove(&paths, host);
        let firn = attribution_by_firn(&root, &scratch, host);
        let pairs = |a: &Attribution| a.values().map(BTreeSet::len).sum::<usize>();
        println!("{host}: muove {} modules, {} module-tag pairs; firn {} modules, {} pairs", ours.len(), pairs(&ours), firn.len(), pairs(&firn));
        assert_eq!(ours, firn, "{host}");
    }
}
