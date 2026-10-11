use galileo_core::{evaluate, Budget, RelationId, Value};
use galileo_import::bnix_tags::{catalog, clause_facts, host_facts, module_facts, platform_facts};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

type Attribution = BTreeMap<String, BTreeSet<String>>;

fn firn_root() -> PathBuf {
    PathBuf::from(std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!("{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9", std::env::var("HOME").unwrap())
    }))
}

const TAGS: &str = include_str!("../../../lib/firn/tags.gal");

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("use firn/tags\n\n{text}")).unwrap();
}

fn sources(root: &Path, out: &Path, host: &str) -> Vec<PathBuf> {
    let _ = std::fs::remove_dir_all(out.join("modules"));
    std::fs::create_dir_all(out.join(".git")).unwrap();
    let mut names: Vec<String> = std::fs::read_dir(root.join("modules"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    let mut all = String::new();
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
        write(&out.join(format!("modules/{name}/default.gal")), &facts);
        all += &facts;
    }
    let hosts = ["whiterabbit", "ashashi"]
        .map(|h| std::fs::read_to_string(root.join(format!("hosts/{h}/enabled-tags.bnix"))).unwrap());
    let tags_file = out.join("config/tags.gal");
    write(&tags_file, &catalog(&all, &hosts.each_ref().map(String::as_str)).unwrap());
    let tags = std::fs::read_to_string(root.join(format!("hosts/{host}/enabled-tags.bnix"))).unwrap();
    let host_file = out.join(format!("hosts/{host}/tags.gal"));
    write(&host_file, &host_facts(host, &tags).unwrap());
    let platforms = out.join("config/platforms.gal");
    write(&platforms, &platform_facts(&std::fs::read_to_string(root.join("config/darwin-modules.json")).unwrap()).unwrap());
    vec![host_file, tags_file, platforms, out.join("modules")]
}

fn galileo(arguments: &[&str], paths: &[PathBuf], cwd: &Path) -> (Option<i32>, String) {
    let relative = paths.iter().map(|p| p.strip_prefix(cwd).unwrap_or(p));
    let output = Command::new(env!("CARGO_BIN_EXE_galileo")).args(arguments).args(relative).current_dir(cwd).output().unwrap();
    assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
    (output.status.code(), String::from_utf8_lossy(&output.stdout).into_owned())
}

fn attribution_by_galileo(out: &Path, paths: &[PathBuf], host: &str) -> Attribution {
    let mut files = Vec::new();
    for path in paths {
        if path.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                files.push(entry.unwrap().path().join("default.gal"));
            }
        } else {
            files.push(path.clone());
        }
    }
    let module = |f: &PathBuf| f.strip_prefix(out).unwrap().with_extension("").display().to_string();
    let texts: Vec<(String, String)> = files.iter().map(|f| (module(f), std::fs::read_to_string(f).unwrap())).collect();
    let mut named: Vec<(&str, &str)> = texts.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    named.push(("firn/tags", TAGS));
    let lowered = galileo_syntax::lower::lower_program(&named).unwrap();
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
    let output = Command::new(root.join("dotfiles/bin/firn")).args(["tag", "resolve", host]).env("FIRN_REPO", &copy).current_dir(&copy).output().unwrap();
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
    let (code, proof) = galileo(&["why", "firefox", "is", "enabled", "on", "whiterabbit"], &paths, &out);
    println!("{proof}");
    assert_eq!(code, Some(0));
    assert!(proof.starts_with("firefox is enabled on whiterabbit    ← firn/tags/enabled"), "{proof}");
    assert!(proof.contains("firefox is enabled on whiterabbit by browsers    ← firn/tags/default-membership-enables"));
    assert!(proof.contains("whiterabbit enables browsers    "));
    assert!(proof.contains("firefox is tagged browsers    "));
    assert!(proof.contains("nothing states: whiterabbit drops firefox from browsers    (checked: "));
    assert!(proof.contains("└─ nothing states: whiterabbit disables firefox    (checked: "));
    let (code, near) = galileo(&["why", "not", "anytype", "is", "enabled", "on", "whiterabbit"], &paths, &out);
    println!("{near}");
    assert_eq!(code, Some(0));
    assert!(near.contains("whiterabbit disables anytype"), "{near}");

    for host in ["whiterabbit"] {
        let out = scratch.join(host);
        let paths = sources(&root, &out, host);
        let ours = attribution_by_galileo(&out, &paths, host);
        let firn = attribution_by_firn(&root, &scratch, host);
        let pairs = |a: &Attribution| a.values().map(BTreeSet::len).sum::<usize>();
        println!("{host}: galileo {} modules, {} module-tag pairs; firn {} modules, {} pairs", ours.len(), pairs(&ours), firn.len(), pairs(&firn));
        assert_eq!(ours, firn, "{host}");
    }
}
