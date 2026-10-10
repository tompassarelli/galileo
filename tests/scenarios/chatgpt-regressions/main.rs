use galileo_review::{parse, Baseline};

const BASE: &str = "\
type Node

given {node: Node} is seeded
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

    let error = parse(&baseline, "patch stated\n  add\n    Goal holds\n").unwrap_err();
    assert_eq!(error.line, 3, "{error}");

    let stated_source = BASE
        .replace("derived {node: Node} holds", "given {node: Node} holds")
        .replace("rule make-seed: ?n holds\n  when ?n is seeded\n", "")
        .replace("rule make-gate: Gate holds\n  when Seed holds\n", "")
        .replace("rule make-goal: Goal holds\n  when Gate holds\n", "")
        .replace("Seed is seeded", "Seed is seeded\nSeed holds\nGate holds\nGoal holds");
    let stated = Baseline::freeze(&stated_source).unwrap();
    let changes = galileo_review::truth(
        baseline.program(),
        stated.program(),
        galileo_core::Budget::default(),
    )
    .unwrap();
    assert_eq!(changes, [
        galileo_review::TruthChange::NowStated("Gate holds".into()),
        galileo_review::TruthChange::NowStated("Goal holds".into()),
        galileo_review::TruthChange::NowStated("Seed holds".into()),
    ]);
    assert_eq!(changes.iter().map(ToString::to_string).collect::<Vec<_>>(), [
        "Gate holds: now stated",
        "Goal holds: now stated",
        "Seed holds: now stated",
    ]);
    let changes = galileo_review::truth(
        stated.program(),
        baseline.program(),
        galileo_core::Budget::default(),
    )
    .unwrap();
    assert_eq!(changes, [
        galileo_review::TruthChange::NowDerived("Gate holds".into()),
        galileo_review::TruthChange::NowDerived("Goal holds".into()),
        galileo_review::TruthChange::NowDerived("Seed holds".into()),
    ]);

    let patch = parse(&baseline, "patch seeded\n  add\n    Goal is seeded\n  remove make-gate\n").unwrap();
    assert!(patch.diagnostics.is_empty(), "{:?}", patch.diagnostics);
    assert_eq!(patch.edit_sentences, [
        "Added: Goal is seeded    patch:3",
        "Removed rule make-gate    patch:4",
    ]);
    let changes = galileo_review::truth(
        baseline.program(),
        &patch.program,
        galileo_core::Budget::default(),
    )
    .unwrap();
    assert_eq!(changes, [galileo_review::TruthChange::Lost("Gate holds".into())]);
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
