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

#[test]
fn r3_r4_review_separates_failure_strength_from_milestone_dependence() {
    use galileo_core::supports::{minimal_supports, Strength};
    use galileo_core::{Budget, EvalError, Verdict};
    use galileo_review::{milestones, support, MilestoneChange};

    let before = Baseline::freeze(&BASE.replace("derived {node: Node} holds", "derived milestone {node: Node} holds")).unwrap();
    let after = parse(&before, "patch bypass\n  add rule bypass\n    Goal holds\n    when Seed holds\n").unwrap();
    let goal = galileo_core::Value::Entity(before.program().entities.iter().find(|entity| entity.name == "Goal").unwrap().clone());
    let before_supports = minimal_supports(before.program(), &[], before.program().relation_id("holds").unwrap(), &[goal.clone()], Budget::default()).unwrap();
    let after_supports = minimal_supports(&after.program, &[], after.program.relation_id("holds").unwrap(), &[goal], Budget::default()).unwrap();
    let support_sentences = |program: &galileo_core::Program, supports: Verdict<Vec<galileo_core::supports::Support>, ()>| {
        let Verdict::Holds(sets) = supports else { panic!("supports must hold") };
        sets.into_iter().map(|set| {
            set.into_iter().map(|leaf| {
                let galileo_core::why::Proof::Stated { fact } = leaf else { panic!("a stated leaf") };
                galileo_core::why::sentence(program, fact.relation, &fact.values)
            }).collect::<Vec<_>>()
        }).collect::<Vec<_>>()
    };
    assert_eq!(support_sentences(before.program(), before_supports), [vec!["Seed is seeded"]]);
    assert_eq!(support_sentences(&after.program, after_supports), [vec!["Seed is seeded"]]);
    let changes = milestones(before.program(), &after.program, Budget::default()).unwrap();
    assert_eq!(changes, [MilestoneChange::Bypassed { fact: "Goal holds".into(), milestone: "Gate holds".into() }]);
    println!("{}", changes[0]);
    assert!(milestones(&after.program, before.program(), Budget::default()).unwrap().is_empty());

    let source = "\
type Node
given {node: Node} is available
derived milestone {node: Node} holds
A is a Node
B is a Node
C is a Node
D is a Node
Goal is a Node
A is available
B is available
C is available
D is available
rule via-a: Goal holds
  when A is available
rule via-b: Goal holds
  when B is available
requirements
  Goal holds
";
    let before = Baseline::freeze(source).unwrap();
    let proposal = source
        .replace("given {node: Node} is available", "given {node: Node} is unused\ngiven {node: Node} is available")
        .replace("when A is available", "when A is available\n  and C is available")
        .replace("when B is available", "when B is available\n  and C is available")
        .replace("requirements", "rule via-d: Goal holds\n  when D is available\n  and C is available\nrequirements");
    let after = Baseline::freeze(&proposal).unwrap();
    assert_ne!(before.program().relation_id("holds"), after.program().relation_id("holds"));
    let pattern = before.program().relations.iter().filter(|r| r.template.contains("is available")).map(|r| r.name.clone()).collect::<Vec<_>>();
    let views = support(before.program(), after.program(), &pattern, Budget::default()).unwrap();
    assert_eq!(views.len(), 1);
    let view = &views[0];
    assert_eq!(view.subject, "Goal holds");
    assert_eq!(view.before.as_ref().and_then(Strength::size), Some(2));
    assert_eq!(view.after.as_ref().and_then(Strength::size), Some(1));
    assert!(view.to_string().contains("The smallest failure set fell from 2 facts to 1."));
    println!("{view}");
    assert!(!view.to_string().contains("stronger"));
    for (program, strength, expected) in [
        (before.program(), view.before.as_ref().unwrap(), vec![vec!["A is available", "B is available"]]),
        (after.program(), view.after.as_ref().unwrap(), vec![vec!["C is available"]]),
    ] {
        let Strength::Smallest { sets, complete: true } = strength else { panic!("{strength:?}") };
        let mut actual = sets.iter().map(|set| {
            let mut facts = set.iter().map(|fact| galileo_core::why::sentence(program, fact.relation, &fact.values)).collect::<Vec<_>>();
            facts.sort();
            facts
        }).collect::<Vec<_>>();
        actual.sort();
        assert_eq!(actual, expected);
    }
    for (program, count) in [(before.program(), 2), (after.program(), 3)] {
        let relation = program.relation_id("holds").unwrap();
        let goal = program.entities.iter().find(|entity| entity.name == "Goal").unwrap().clone();
        let Verdict::Holds(sets) = minimal_supports(program, &[], relation, &[galileo_core::Value::Entity(goal)], Budget::default()).unwrap() else { panic!("supports unknown") };
        assert_eq!(sets.len(), count);
    }
    assert!(matches!(support(before.program(), after.program(), &pattern, Budget::work(0)), Err(EvalError::Unknown(_))));
    assert!(matches!(milestones(before.program(), after.program(), Budget::work(0)), Err(EvalError::Unknown(_))));
}

#[test]
fn review_names_the_rule_a_holding_requirement_reads_when_a_patch_redefines_it() {
    use galileo_core::Budget;
    use galileo_review::{definition_reads, requirements, ReviewOptions, ReviewVerdict};
    use std::collections::BTreeMap;
    let source = "\
type User

given {user: User} is registered
given {user: User} is vetted
derived {user: User} is approved
derived {user: User} has access
derived {user: User} is greeted

Ann is a User
Bob is a User
Ann is registered
Bob is registered
Ann is vetted

rule approved: ?u is approved
  when ?u is vetted

rule access: ?u has access
  when ?u is approved

rule greeting: ?u is greeted
  when ?u is registered

requirements
  ?u has access requires ?u is approved
";
    let baseline = Baseline::freeze(source).unwrap();
    let review = |proposal: &str| {
        let patch = parse(&baseline, proposal).unwrap();
        assert!(patch.diagnostics.is_empty(), "{:?}", patch.diagnostics);
        let comparison = requirements(baseline.program(), &patch.program, Budget::default()).unwrap();
        let verdict = comparison.verdict(&[], Some(&patch.edits), ReviewOptions::default());
        let sources = |file: &str, text: &str| BTreeMap::from([(file.to_string(), text.to_string())]);
        let reads = definition_reads(baseline.program(), &patch.program, &sources("baseline", baseline.source()), &sources("patch", &patch.source));
        (verdict, reads.iter().map(ToString::to_string).collect::<Vec<_>>())
    };

    let (verdict, reads) = review("patch everyone\n  replace rule approved\n    ?u is approved\n    when ?u is registered\n");
    assert_eq!(verdict, ReviewVerdict::Approve);
    assert_eq!(reads, ["\
`?u has access requires ?u is approved` reads rule `approved`, which this patch replaces
  before, baseline:15:
    rule approved: ?u is approved
      when ?u is vetted
  after, patch:24:
    rule approved: ?u is approved
      when ?u is registered"]);

    let (verdict, reads) = review("patch greet\n  replace rule greeting\n    ?u is greeted\n    when ?u is vetted\n");
    assert_eq!(verdict, ReviewVerdict::Approve);
    assert_eq!(reads, Vec::<String>::new());
}

#[test]
fn spec_8_1_identity_drops_local_names_and_sentence_order_but_keeps_nix_order() {
    let identity = |source: &str| Baseline::freeze(source).unwrap().identity();
    let base = identity(BASE);
    let local = |v: &str| format!("\
type Node
state Node is lit
event press: Node is pressed
Seed is a Node
on ?{v} is pressed
  when not ?{v} is lit
  then ?{v} is lit
requirements
  never ?{v} is lit
");
    assert_eq!(identity(&local("n")), identity(&local("k")));
    let gate = "rule make-gate: Gate holds\n  when Seed holds\n\n";
    let goal = "rule make-goal: Goal holds\n  when Gate holds\n\n";
    let swapped = BASE.replace(&format!("{gate}{goal}"), &format!("{goal}{gate}"));
    assert_ne!(swapped, BASE);
    assert_eq!(identity(&swapped), base);
    let module = |entries: &str| format!("{BASE}\nnix module\n  arguments: config, ...\n  config\n{entries}");
    assert_ne!(
        identity(&module("    a.enable: true\n    b.enable: false\n")),
        identity(&module("    b.enable: false\n    a.enable: true\n")),
    );
}
