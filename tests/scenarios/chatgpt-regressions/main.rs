use galileo_review::{parse, Baseline};

const BASE: &str = "\
type Node

static {node: Node} is seeded
derived {node: Node} holds

Seed is a Node
Gate is a Node
Goal is a Node
Seed is seeded

rule make-seed: ?n holds
  when ?n is seeded

rule make-gate: Gate holds
  when Seed holds

rule make-goal: Goal holds
  when Gate holds

requirements
  Goal holds requires Gate holds
";

#[test]
fn r9_a_rule_outside_the_patch_cannot_enter_the_baseline() {
    let baseline = Baseline::freeze(BASE).unwrap();
    let frozen = baseline.identity();
    let proposal = "\
patch unchanged
  add
    Goal is seeded

rule outside-revision: Goal holds
  when Seed holds
";
    let error = parse(&baseline, proposal).unwrap_err();
    assert_eq!(error.line, 5, "{error}");
    assert!(error.to_string().contains("outside the patch"), "{error}");
    assert_eq!(baseline.identity(), frozen);
    assert!(baseline.program().rules.iter().all(|r| r.name != "outside-revision"));

    let inside = "\
patch unchanged
  add
    Goal is seeded
  add rule bypass
    Goal holds
    when Seed holds
  add
    rule smuggled
";
    let error = parse(&baseline, inside).unwrap_err();
    assert_eq!(error.line, 8, "{error}");

    let stated = parse(&baseline, "patch stated\n  add\n    Goal holds\n").unwrap();
    assert!(stated.diagnostics.is_empty(), "{:?}", stated.diagnostics);
    assert_eq!(stated.edit_sentences, ["Added: Goal holds    patch:3"]);
    let changes = galileo_review::truth(
        baseline.program(),
        &stated.program,
        galileo_core::Budget::default(),
    )
    .unwrap();
    assert_eq!(changes, [galileo_review::TruthChange::NowStated("Goal holds".into())]);
    assert_eq!(changes[0].to_string(), "Goal holds: now stated");
    assert_eq!(baseline.identity(), frozen);
}

#[test]
fn identity_keeps_entity_names_and_drops_rule_variables_and_formatting() {
    let base = Baseline::freeze(BASE).unwrap().identity();
    let renamed_variable = BASE.replace("?n", "?node");
    let reformatted = format!("# a comment\n\n{}", BASE.replace("rule make-gate", "\n\nrule make-gate"));
    let renamed_entity = BASE.replace("Seed", "Sprout");
    assert_eq!(Baseline::freeze(&renamed_variable).unwrap().identity(), base);
    assert_eq!(Baseline::freeze(&reformatted).unwrap().identity(), base);
    assert_ne!(Baseline::freeze(&renamed_entity).unwrap().identity(), base);
}
