use muove_core::{evaluate, Budget, RelationId, Value};
use muove_import::bnix_tags::host_facts;
use std::collections::BTreeSet;
use std::path::Path;

#[path = "fixture.rs"]
mod fixture;
use fixture::{firn_root, module_sources};

const TAGS: &str = include_str!("../../../lib/firn/tags.muove");

fn generated(root: &Path, host: &str) -> BTreeSet<String> {
    std::fs::read_to_string(root.join(format!("hosts/{host}/_generated-enables.bnix")))
        .unwrap()
        .split_whitespace()
        .filter_map(|word| word.trim_start_matches('{').strip_prefix(":myConfig.modules.")?.strip_suffix(".enable"))
        .map(String::from)
        .collect()
}

fn enabled(modules: &str, root: &Path, host: &str) -> BTreeSet<String> {
    let facts = "use firn/tags\n\n".to_string()
        + modules
        + &host_facts(host, &std::fs::read_to_string(root.join(format!("hosts/{host}/enabled-tags.bnix"))).unwrap())
            .unwrap();
    let lowered = muove_syntax::lower::lower_program(&[("firn/tags", TAGS), ("facts", &facts)]).unwrap();
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
