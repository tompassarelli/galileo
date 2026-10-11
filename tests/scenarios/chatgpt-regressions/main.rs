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

#[test]
fn r6_review_compares_complete_failure_instances_and_requires_both_explicit_flags() {
    use galileo_core::{Budget, Claim, Evidence, Instance, Requirement, StateClaim, Verdict};
    use galileo_review::{requirements, Edit, FailureChange, ReviewOptions, ReviewVerdict};
    let source = "\
type Node
type Backup

given {node: Node} has {backup: Backup}
derived {node: Node} is backed up

A is a Node
B is a Node
X is a Backup
Y is a Backup
B has X

rule supported: ?n is backed up
  when ?n has ?b

requirements
  every ?n where ?n is a Node: ?n is backed up
";
    let baseline = Baseline::freeze(source).unwrap();
    let removed = Baseline::freeze(&source.replace("B has X\n", "")).unwrap();
    let ordinary = ReviewOptions::default();
    let allow_existing = ReviewOptions { allow_baseline_failures: true, ..ordinary };
    let allow_both = ReviewOptions { allow_requirement_changes: true, ..allow_existing };
    let compare = |before, after| requirements(before, after, Budget::default()).unwrap();
    let introduced = compare(baseline.program(), removed.program());
    assert_eq!(introduced.failures, [
        FailureChange::PreExisting("A is a Node, but not A is backed up".into()),
        FailureChange::Introduced("B is a Node, but not B is backed up".into()),
    ]);
    assert_eq!(introduced.verdict(&[], None, allow_both), ReviewVerdict::Reject);
    let unchanged = compare(baseline.program(), baseline.program());
    assert_eq!(unchanged.failures, [introduced.failures[0].clone()]);
    assert_eq!(unchanged.verdict(&[], None, ordinary), ReviewVerdict::Reject);
    assert_eq!(unchanged.verdict(&[], None, allow_existing), ReviewVerdict::ApproveNoNewFailures);
    assert_eq!(unchanged.verdict(&[Verdict::Fails(Instance::new())], None, allow_both), ReviewVerdict::Reject);
    assert_eq!(unchanged.verdict(&[Verdict::Unknown(galileo_core::Reason::Unsupported { construct: "intended claim".into(), at: "scenario".into() })], None, allow_both), ReviewVerdict::Reject);

    let fixed = parse(&baseline, "patch fix\n  add\n    A has X\n").unwrap();
    let repaired = compare(baseline.program(), &fixed.program);
    assert_eq!(repaired.failures, [FailureChange::Fixed("A is a Node, but not A is backed up".into())]);
    assert_eq!(repaired.verdict(&[], Some(&fixed.edits), ordinary), ReviewVerdict::Approve);

    let renamed = Baseline::freeze(&source.replace("?n", "?node").replace("?b", "?backup")).unwrap();
    let renamed = compare(baseline.program(), renamed.program());
    assert!(renamed.requirement_changes.is_empty());
    assert_eq!(renamed.failures, unchanged.failures);
    let reordered = Baseline::freeze(&source.replace("given {node: Node} has", "given {node: Node} is extra\ngiven {node: Node} has")).unwrap();
    let reordered = compare(baseline.program(), reordered.program());
    assert!(reordered.requirement_changes.is_empty());
    assert_eq!(reordered.failures, unchanged.failures);

    let mut changed = fixed.program.clone();
    changed.requirements.clear();
    let altered = compare(baseline.program(), &changed);
    assert_eq!(altered.requirement_changes.len(), 1);
    assert_eq!(altered.verdict(&[], None, ordinary), ReviewVerdict::Unknown);
    assert_eq!(altered.verdict(&[], None, ReviewOptions { allow_requirement_changes: true, ..ordinary }), ReviewVerdict::Approve);
    assert_eq!(altered.verdict(&[], Some(&[Edit::ChangeRequirements { header: "requirements".into(), line: 1 }]), ordinary), ReviewVerdict::Approve);
    for (before, after) in [(baseline.program(), &changed), (&changed, baseline.program())] {
        let incomplete = requirements(before, after, Budget::work(0)).unwrap();
        assert!(!incomplete.incomplete.is_empty());
        assert!(incomplete.failures.is_empty());
        assert_eq!(incomplete.verdict(&[], None, allow_both), ReviewVerdict::Unknown);
    }

    let target = match &baseline.program().requirements[0].claim {
        Claim::State(StateClaim::Every { body, .. }) => match &body[0] {
            galileo_core::Literal::Positive(atom) => atom.clone(),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    let mut every = fixed.program.clone();
    every.requirements = vec![Requirement::new("each-supported", Claim::State(StateClaim::Every {
        variables: vec!["n".into()],
        domain: vec![target.clone().positive()],
        body: vec![target.clone().negative()],
    }))];
    let enumerated = compare(&every, &every);
    assert_eq!(enumerated.failures, [
        FailureChange::PreExisting("A is backed up, but not not A is backed up".into()),
        FailureChange::PreExisting("B is backed up, but not not B is backed up".into()),
    ]);
    let mut invariant = every.clone();
    invariant.requirements[0].invariant = true;
    assert_eq!(compare(&every, &invariant).requirement_changes.len(), 2);
    let mut different_vocabulary = every.clone();
    different_vocabulary.relations[target.relation.0].name = "another/holds".into();
    assert_eq!(compare(&every, &different_vocabulary).requirement_changes.len(), 2);
    let among = match &baseline.program().rules[0].body[0] {
        galileo_core::Literal::Positive(atom) => atom.clone(),
        other => panic!("{other:?}"),
    };
    let mut robust = fixed.program.clone();
    robust.requirements = vec![Requirement::new("robust", Claim::NoSinglePointOfFailure { claim: Box::new(Claim::State(StateClaim::Holds(target.clone()))), among: among.clone() })];
    let failures = galileo_core::claims::enumerate_claim_failures(&robust, Budget::default()).unwrap();
    assert_eq!(failures[0].instances.len(), 2);
    assert!(failures[0].instances.iter().all(|instance| instance.len() == 1));
    let mut requires = fixed.program.clone();
    let mut condition = among;
    condition.arguments[1] = galileo_core::Expr::value(galileo_core::Value::entity("Backup", "Y"));
    requires.requirements = vec![Requirement::new("requires", Claim::Requires { target: target.clone(), condition })];
    let failures = galileo_core::claims::enumerate_claim_failures(&requires, Budget::default()).unwrap();
    assert_eq!(failures[0].instances.len(), 2);
    let mut existential = fixed.program.clone();
    existential.requirements = vec![Requirement::new("none", Claim::State(StateClaim::No { variables: vec!["n".into()], body: vec![target.positive()] }))];
    let failures = galileo_core::claims::enumerate_claim_failures(&existential, Budget::default()).unwrap();
    assert_eq!(failures[0].instances, [Instance::new()]);
    assert!(matches!(failures[0].verdict, Verdict::Fails(_)));
    assert!(matches!(galileo_core::check_claims(baseline.program(), Budget::default()).unwrap()[0].verdict, Verdict::Fails(_)));
    assert!(matches!(galileo_core::claims::enumerate_claim_failures(&fixed.program, Budget::default()).unwrap()[0].verdict, Verdict::Holds(Evidence::Every { instances: 2 })));
}
