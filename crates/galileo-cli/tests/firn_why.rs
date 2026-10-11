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
    let supported = program.relations.iter().position(|r| r.slots.len() == 2 && r.template.contains("is supported on")).map(RelationId).unwrap();
    let model = evaluate(&program, &[], Budget::default()).unwrap();
    let mut out = Attribution::new();
    for tuple in model.tuples(by.unwrap()) {
        if let [Value::Entity(m), Value::Entity(h), Value::Entity(t)] = tuple.as_slice() {
            if h.name == host {
                out.entry(m.name.clone()).or_default().insert(t.name.clone());
            }
        }
    }
    let raw_modules = out.len();
    let raw_pairs = out.values().map(BTreeSet::len).sum::<usize>();
    let supported_modules: BTreeSet<_> = model.tuples(supported).filter_map(|row| {
        match row.as_slice() {
            [Value::Entity(module), Value::Entity(h)] if h.name == host => Some(module.name.as_str()),
            _ => None,
        }
    }).collect();
    out.retain(|module, _| supported_modules.contains(module.as_str()));
    println!("{host}: raw attribution {raw_modules} modules, {raw_pairs} pairs; supported attribution {} modules, {} pairs", out.len(), out.values().map(BTreeSet::len).sum::<usize>());
    out
}

fn attribution_by_firn(root: &Path, scratch: &Path, host: &str) -> Attribution {
    let query_root = scratch.join("firn");
    std::fs::create_dir_all(&query_root).unwrap();
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == ".firn-build" { continue; }
        let link = query_root.join(entry.file_name());
        if !link.exists() {
            std::os::unix::fs::symlink(entry.path(), &link).unwrap();
        }
    }
    std::fs::create_dir_all(query_root.join(".firn-build")).unwrap();
    let output = Command::new(root.join("dotfiles/bin/firn")).args(["tag", "resolve", host]).env_remove("FIRN_TAG_EMIT").env("FIRN_REPO", &query_root).current_dir(&query_root).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    parse_resolve(&String::from_utf8(output.stdout).unwrap())
}

fn parse_resolve(text: &str) -> Attribution {
    let section = text.split("Per-tag contributions:").nth(1).unwrap().split("\nActive modules").next().unwrap();
    let mut out = Attribution::new();
    let mut tag = String::new();
    for line in section.lines() {
        if let Some(header) = line.strip_prefix("  ").filter(|l| !l.starts_with(' ')) {
            tag = header.split(" (").next().unwrap().to_string();
        } else if let Some(module) = line.strip_prefix("    ").filter(|l| !l.starts_with(' ')) {
            if module != "(none)" {
                out.entry(module.to_string()).or_default().insert(tag.clone());
            }
        }
    }
    out
}

fn proof_lines(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(|line| line.trim_start_matches(|c| "│├└─ ".contains(c)))
        .map(|line| line.split_once("    ").map_or((line.to_string(), String::new()), |(a, b)| (a.to_string(), b.to_string())))
        .collect()
}

fn proof_line<'a>(lines: &'a [(String, String)], sentence: &str) -> &'a str {
    let found: Vec<_> = lines.iter().filter(|(s, _)| s == sentence).map(|(_, r)| r.as_str()).collect();
    assert_eq!(found.len(), 1, "{sentence}: {lines:?}");
    found[0]
}

#[test]
fn firefox_proof_and_every_tag_attribution_match_firn_tag_resolve() {
    let root = firn_root();
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("firn-why");
    let full = std::env::var_os("GALILEO_FULL").is_some();
    if full {
        let host = "whiterabbit";
        let out = scratch.join(host);
        let paths = sources(&root, &out, host);
        let (code, proof) = galileo(&["why", "firefox", "is", "enabled", "on", "whiterabbit"], &paths, &out);
        println!("{proof}");
        assert_eq!(code, Some(0));
        let lines = proof_lines(&proof);
        assert_eq!(lines[0].0, "firefox is enabled on whiterabbit", "{proof}");
        assert!(lines[0].1.contains("firn/tags/enabled"), "{proof}");
        for (sentence, rule, file) in [
            ("firefox is enabled on whiterabbit by browsers", "firn/tags/default-membership-enables", ""),
            ("firefox is enabled on whiterabbit by default through browsers", "firn/tags/enabled-by-default", "lib/firn/tags.gal:"),
            ("firefox is supported on whiterabbit", "firn/tags/supported-everywhere", "lib/firn/tags.gal:"),
        ] {
            let at = proof_line(&lines, sentence);
            assert!(at.contains(rule) && at.contains(file), "{sentence}: {at}");
        }
        let (file, line) = proof_line(&lines, "whiterabbit enables browsers").rsplit_once(':').unwrap();
        assert_eq!(file, "hosts/whiterabbit/tags.gal", "{proof}");
        assert!(line.parse::<usize>().is_ok(), "{proof}");
        assert_eq!(proof_line(&lines, "firefox is tagged browsers").rsplit_once(':'), Some(("modules/firefox/default.gal", "4")), "{proof}");
        for absent in ["whiterabbit drops firefox from browsers", "whiterabbit disables firefox"] {
            assert!(lines.iter().any(|(s, checked)| s.starts_with("nothing states") && s.ends_with(absent) && checked.contains("checked")), "{absent}: {proof}");
        }
        let (code, near) = galileo(&["why", "not", "anytype", "is", "enabled", "on", "whiterabbit"], &paths, &out);
        println!("{near}");
        assert_eq!(code, Some(0));
        let blocker = "whiterabbit disables anytype";
        assert!(near.contains(blocker), "{near}");

        let (code, near) = galileo(&["why", "not", "anytype", "is", "enabled", "on", "whiterabbit", "--via", "firn/tags/enabled"], &paths, &out);
        assert_eq!(code, Some(0));
        assert!(near.lines().any(|line| line.contains(&format!("not {blocker}")) && line.contains("blocked")), "{near}");
        let rules: Vec<_> = near.lines().filter(|line| line.starts_with("  ") && !line.starts_with("   ") && line.ends_with("needs:")).collect();
        assert!(!rules.is_empty() && rules.iter().all(|line| line.trim_start().starts_with("firn/tags/enabled ")), "{near}");
    }

    let hosts: &[&str] = if full { &["whiterabbit", "ashashi"] } else { &["whiterabbit"] };
    for &host in hosts {
        let out = scratch.join(host);
        let paths = sources(&root, &out, host);
        let ours = attribution_by_galileo(&out, &paths, host);
        let firn = if full || std::env::var_os("FIRN_ROOT").is_some() {
            attribution_by_firn(&root, &scratch, host)
        } else {
            parse_resolve(include_str!("../../../tests/external/firn-tags/whiterabbit-tag-resolve-10740a8.txt"))
        };
        let pairs = |a: &Attribution| a.values().map(BTreeSet::len).sum::<usize>();
        let mut modules: BTreeSet<_> = std::fs::read_dir(out.join("modules")).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
        modules.extend(ours.keys().chain(firn.keys()).cloned());
        let mismatches = modules.iter().filter(|module| ours.get(*module) != firn.get(*module)).count();
        println!("{host}: galileo {} modules, {} module-tag pairs; firn {} modules, {} pairs; {} modules compared, {mismatches} mismatches", ours.len(), pairs(&ours), firn.len(), pairs(&firn), modules.len());
        assert_eq!(ours, firn, "{host}");
    }
}
