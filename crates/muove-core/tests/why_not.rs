use muove_core::why_not::{render, why_not, Via};
use muove_core::*;

fn ask(program: &Program, relation: &str, values: &[Value], via: Option<&Via>) -> (Verdict<(), Vec<why_not::Attempt>>, String) {
    let relation = program.relation_id(relation).unwrap();
    let verdict = why_not(program, &[], relation, values, via, Budget::default()).unwrap();
    let text = render(program, relation, values, via, &verdict, Budget::default());
    println!("{text}\n");
    (verdict, text)
}

#[test]
fn why_not_matches_the_prototype_readme_example() {
    let program = examples::dungeon_static();
    let ash = Value::entity("Hero", "Ash");
    let (_, key) = ask(&program, "level/holds", &[ash.clone(), Value::entity("Key", "Silver-Key")], None);
    assert!(key.contains(
        "  level/pick-up needs:\n    ✗ Silver-Key lies in ?room   (nothing states this; checked: every stated fact of the program)\n    · Ash can reach ?room"
    ));
    let (_, shrine) = ask(&program, "level/reach", &[ash, Value::entity("Room", "Shrine")], None);
    assert!(shrine.contains(
        "  level/unlock needs:\n    ✓ Ash can reach Crypt\n    ✓ Shrine-Door leads from Crypt to Shrine\n    ✓ Shrine-Door is locked by Silver-Key\n    ✗ Ash holds Silver-Key\n        level/pick-up needs:\n          ✗ Silver-Key lies in ?room   (nothing states this; checked: every stated fact of the program)\n          · Ash can reach ?room"
    ));
}

#[test]
fn a_failing_not_names_its_blocker_under_via() {
    let mut program = examples::tags();
    for (line, fact) in program.facts.iter_mut().enumerate() {
        *fact = fact.clone().at(SourceSpan { file: "tags.muove".into(), start: 0, end: 0, line: line + 1, column: 1 });
    }
    let via = Via { rule: "enabled".into(), bindings: vec![] };
    let asked = [Value::entity("Module", "anytype"), Value::entity("Host", "whiterabbit")];
    let (_, text) = ask(&program, "firn/enabled", &asked, Some(&via));
    assert!(text.contains("The closest binding of rule enabled"));
    assert!(text.contains("✗ not whiterabbit disables anytype   (blocked by: whiterabbit disables anytype    tags.muove:"));
}
