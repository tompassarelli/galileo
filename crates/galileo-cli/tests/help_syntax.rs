use galileo_syntax::check::{check, SourceFile};
use galileo_syntax::schema::Schema;
use std::collections::BTreeSet;
use std::process::Command;

fn productions() -> BTreeSet<String> {
    let spec = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../SPEC.md")).unwrap();
    let section = spec.split("### 9.4").nth(1).unwrap();
    let grammar = section.split("```text").nth(1).unwrap().split("```").next().unwrap();
    let ident = |s: &str| {
        let name: String = s.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        let rest = s[name.len()..].trim_start();
        (!name.is_empty() && rest.starts_with('=') && !rest.starts_with("==")).then_some(name)
    };
    let mut out = BTreeSet::new();
    for line in grammar.lines() {
        out.extend(ident(line));
        out.extend(line.split("   ").skip(1).map(str::trim_start).filter_map(ident));
    }
    out
}

fn card() -> Vec<(String, Vec<(String, String)>)> {
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["help", "syntax"]).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    let mut examples: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("== ") {
            examples.push((name.trim().into(), vec![("example".into(), String::new())]));
        } else if line.starts_with("## ") {
            continue;
        } else if let Some((_, modules)) = examples.last_mut() {
            if let Some(module) = line.strip_prefix("# file: ") {
                if modules.len() == 1 && modules[0].1.trim().is_empty() {
                    modules.clear();
                }
                modules.push((module.trim().into(), String::new()));
            } else {
                let source = &mut modules.last_mut().unwrap().1;
                source.push_str(line);
                source.push('\n');
            }
        }
    }
    examples
}

fn diagnostics(modules: &[(String, String)]) -> Vec<String> {
    if modules.len() > 1 {
        let sources: Vec<(&str, &str)> = modules.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
        return match galileo_syntax::read_program(&sources) {
            Ok(read) => read.values().flat_map(|s| &s.warnings).map(ToString::to_string).collect(),
            Err((module, error)) => vec![format!("{module}: {error}")],
        };
    }
    let (name, text) = &modules[0];
    let mut out: Vec<String> = match galileo_syntax::lower::lower(text, name) {
        Ok(lowered) => lowered.diagnostics.iter().map(ToString::to_string).collect(),
        Err(error) => vec![error.to_string()],
    };
    let parsed = SourceFile::parse(name, text).map(|file| vec![file]);
    match parsed {
        Err(error) => out.push(error.to_string()),
        Ok(files) => out.extend(check(&files, &Schema::default()).iter().map(ToString::to_string)),
    }
    out
}

fn lowered_nix(name: &str, modules: &[(String, String)]) -> Vec<String> {
    let records = |text: &str| text.lines().any(|l| l == "nix module" || l == "nix file");
    if !modules.iter().any(|(_, text)| records(text)) {
        return Vec::new();
    }
    let dir = std::env::temp_dir().join(format!("galileo-help-syntax-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (module, text) in modules {
        std::fs::write(dir.join(format!("{module}.gal")), text).unwrap();
    }
    let mut out = Vec::new();
    for (module, _) in modules.iter().filter(|(_, text)| records(text)) {
        let file = dir.join(format!("{module}.gal"));
        let built = Command::new(env!("CARGO_BIN_EXE_galileo")).current_dir(&dir).args(["build", "--stdout"]).arg(&file).output().unwrap();
        if !built.status.success() {
            out.push(format!("{module}: build: {}", String::from_utf8_lossy(&built.stderr).trim()));
            continue;
        }
        if let Err(error) = galileo_import::nix::parse(&String::from_utf8(built.stdout).unwrap()) {
            out.push(format!("{module}: emitted Nix does not parse: {error}"));
        }
    }
    std::fs::remove_dir_all(&dir).unwrap();
    out
}

#[test]
fn help_syntax_prints_one_compiling_example_per_spec_9_4_production() {
    let examples = card();
    let printed: Vec<&str> = examples.iter().map(|(name, _)| name.as_str()).collect();
    let unique: BTreeSet<String> = printed.iter().map(|s| s.to_string()).collect();
    assert_eq!(unique.len(), printed.len(), "a production is printed twice");
    assert_eq!(unique, productions());
    let failures: Vec<String> = examples
        .iter()
        .flat_map(|(name, modules)| {
            diagnostics(modules).into_iter().chain(lowered_nix(name, modules)).map(move |d| format!("{name}: {d}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
