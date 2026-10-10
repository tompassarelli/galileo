use muove_core::{evaluate, Budget, RelationId, Value};
use muove_import::bnix_tags::{clause_facts, host_facts, module_facts};
use std::collections::BTreeSet;
use std::path::Path;

const TAGS: &str = include_str!("../../../lib/firn/tags.muove");

fn firn_root() -> String {
    std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!("{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9", std::env::var("HOME").unwrap())
    })
}

fn module_sources(root: &Path) -> String {
    let mut names: Vec<String> = std::fs::read_dir(root.join("modules"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    let mut facts = String::new();
    for name in &names {
        let directory = root.join("modules").join(name);
        let clause = directory.join("tags.clause");
        let beagle = directory.join("default.bnix");
        if clause.exists() {
            facts += &clause_facts(name, &std::fs::read_to_string(&clause).unwrap()).unwrap();
        } else if beagle.exists() {
            facts += &module_facts(name, &std::fs::read_to_string(&beagle).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", beagle.display()));
        }
    }
    facts
}

fn generated(root: &Path, host: &str) -> BTreeSet<String> {
    std::fs::read_to_string(root.join(format!("hosts/{host}/_generated-enables.bnix")))
        .unwrap()
        .split_whitespace()
        .filter_map(|word| word.trim_start_matches('{').strip_prefix(":myConfig.modules.")?.strip_suffix(".enable"))
        .map(String::from)
        .collect()
}

fn enabled(modules: &str, root: &Path, host: &str) -> BTreeSet<String> {
    let facts = modules.to_string()
        + &host_facts(host, &std::fs::read_to_string(root.join(format!("hosts/{host}/enabled-tags.bnix"))).unwrap())
            .unwrap();
    let lowered = muove_syntax::lower::lower_files(&[("lib/firn/tags.muove", TAGS), ("facts.muove", &facts)]).unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let program = lowered.program;
    let relation = program
        .relations
        .iter()
        .position(|r| r.slots.len() == 2 && r.template.contains("is enabled on"))
        .map(RelationId)
        .unwrap();
    let model = evaluate(&program, &[], Budget::default()).unwrap();
    model
        .tuples(relation)
        .filter_map(|tuple| match tuple.as_slice() {
            [Value::Entity(m), Value::Entity(h)] if h.name == host => Some(m.name.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn all_enabled_modules_equal_generated_enables_on_whiterabbit() {
    let root = firn_root();
    let root = Path::new(&root);
    let modules = module_sources(root);
    for host in ["whiterabbit"] {
        let expected = generated(root, host);
        let actual = enabled(&modules, root, host);
        println!("{host}: {} enabled, {} generated", actual.len(), expected.len());
        assert!(!expected.is_empty());
        assert_eq!(actual, expected, "{host}");
    }
}
