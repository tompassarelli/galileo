use galileo_core::why::{render, why, why_all, Proof};
use galileo_core::*;

fn leaves<'a>(proof: &'a Proof, out: &mut Vec<&'a Proof>) {
    match proof {
        Proof::Derived { premises, .. } => premises.iter().for_each(|p| leaves(p, out)),
        leaf => out.push(leaf),
    }
}

#[test]
fn r10_reconstruction_never_invents_a_stated_leaf() {
    let program = examples::gate();
    let holds = program.relation_id("w/holds").unwrap();
    let goal = [Value::entity("Node", "Goal")];
    let mut outcomes = (0, 0);
    for work in 0..=60 {
        match why(&program, &[], holds, &goal, Budget::work(work)).unwrap() {
            Verdict::Holds(proof) => {
                outcomes.0 += 1;
                let mut found = Vec::new();
                leaves(&proof, &mut found);
                for leaf in found {
                    let Proof::Stated { fact } = leaf else { continue };
                    assert!(program.facts.contains(fact), "invented leaf at budget {work}");
                }
                assert!(render(&program, holds, &goal, &Verdict::Holds(proof), Budget::work(work))
                    .contains("└─ Gate holds    ← w/make-gate\n   └─ Seed holds    stated without a source"));
            }
            Verdict::Unknown(_) => outcomes.1 += 1,
            Verdict::Fails(()) => panic!("Goal holds"),
        }
    }
    assert!(outcomes.0 > 0 && outcomes.1 > 0, "{outcomes:?}");
}

#[test]
fn a_not_proof_shows_its_absence_leaf_and_scope() {
    let mut program = examples::tags();
    for (line, fact) in program.facts.iter_mut().enumerate() {
        let host = fact.values.iter().any(|v| *v == Value::entity("Host", "whiterabbit"));
        let file = if host { "hosts/whiterabbit/tags.gal" } else { "modules/default.gal" };
        *fact = fact.clone().at(SourceSpan {
            file: file.into(),
            start: 0,
            end: 0,
            line: line + 1,
            column: 1,
        });
    }
    let enabled = program.relation_id("firn/enabled").unwrap();
    let asked = [Value::entity("Module", "firefox"), Value::entity("Host", "whiterabbit")];
    let verdict = why(&program, &[], enabled, &asked, Budget::default()).unwrap();
    let text = render(&program, enabled, &asked, &verdict, Budget::default());
    println!("{text}");
    assert!(text.starts_with("firefox is enabled on whiterabbit    ← enabled\n"));
    assert!(text.contains("nothing states: whiterabbit drops firefox from browsers    (checked: hosts/whiterabbit/tags.gal)"));
    assert!(text.contains("└─ nothing states: whiterabbit disables firefox    (checked: hosts/whiterabbit/tags.gal)"));
    assert!(text.contains("whiterabbit enables browsers    hosts/whiterabbit/tags.gal:"));

    let disables = program.relation_id("firn/disables").unwrap();
    let moved = program.facts.iter_mut().find(|fact| fact.relation == disables).unwrap();
    moved.origin = FactOrigin::Foreign {
        source: "hosts/whiterabbit/disables.gal".into(),
        revision: "recorded".into(),
        location: None,
    };
    let mut duplicate = moved.clone();
    duplicate.origin = FactOrigin::Stated(Some(SourceSpan {
        file: "hosts/whiterabbit/tags.gal".into(), start: 0, end: 0, line: 1, column: 1,
    }));
    program.facts.push(duplicate);
    program.entities.push(Entity::named("Host", "unrelated-host"));
    program.facts.push(Fact::new(disables, [Value::entity("Host", "unrelated-host"), Value::entity("Module", "anytype")])
        .at(SourceSpan { file: "hosts/unrelated/disables.gal".into(), start: 0, end: 0, line: 1, column: 1 }));
    program.entities.push(Entity::named("Module", "kanata"));
    let tagged = program.relation_id("firn/is-tagged").unwrap();
    program.facts.push(Fact::new(tagged, [Value::entity("Module", "kanata"), Value::entity("Tag", "browsers")]));
    let asked = [Value::entity("Module", "kanata"), Value::entity("Host", "whiterabbit")];
    let verdict = why(&program, &[], enabled, &asked, Budget::default()).unwrap();
    let text = render(&program, enabled, &asked, &verdict, Budget::default());
    assert!(text.contains("nothing states: whiterabbit disables kanata    (checked: hosts/whiterabbit/disables.gal, hosts/whiterabbit/tags.gal)"));
    assert!(!text.contains("hosts/unrelated/disables.gal"));

    let empty_host = Value::entity("Host", "empty-host");
    program.entities.push(Entity::named("Host", "empty-host"));
    let enables = program.relation_id("firn/enables").unwrap();
    program.facts.push(Fact::new(enables, [empty_host.clone(), Value::entity("Tag", "browsers")]));
    let runs = program.relation_id("firn/runs").unwrap();
    let platform = program.facts.iter().find(|fact| fact.relation == runs).unwrap().values[1].clone();
    program.facts.push(Fact::new(runs, [empty_host.clone(), platform]));
    let asked = [Value::entity("Module", "firefox"), empty_host];
    let verdict = why(&program, &[], enabled, &asked, Budget::default()).unwrap();
    let text = render(&program, enabled, &asked, &verdict, Budget::default());
    assert!(text.contains("nothing states: empty-host disables firefox    (checked: every stated fact of the program)"));

    let mut builder = Program::builder();
    let seed = builder.relation(Relation::new("seed", RelationKind::Static, []));
    let pending = builder.relation(Relation::new("pending", RelationKind::Derived, []).template("pending"));
    let active = builder.relation(Relation::new("active", RelationKind::State, []).template("active"));
    let goal = builder.relation(Relation::new("goal", RelationKind::Derived, []));
    for (name, file, line) in [("pending-first", "first.gal", 3), ("pending-second", "second.gal", 7)] {
        builder.rule(Rule::new(name, Atom::new(pending, []))
            .when([Atom::new(seed, []).positive()])
            .at(SourceSpan { file: file.into(), start: 0, end: 0, line, column: 1 }));
    }
    builder.rule(Rule::new("goal", Atom::new(goal, []))
        .when([Atom::new(pending, []).negative(), Atom::new(active, []).negative()]));
    let program = builder.build();
    let verdict = why(&program, &[], goal, &[], Budget::default()).unwrap();
    let text = render(&program, goal, &[], &verdict, Budget::default());
    assert!(text.contains("nothing derives: pending    (checked: pending-first (first.gal:3), pending-second (second.gal:7))"));
    assert!(text.contains("nothing states: active    (checked: current state)"));
}

#[test]
fn r13_why_gives_one_proof_and_why_all_runs_out_of_work() {
    let program = examples::r13();
    let goal = program.relation_id("r13/goal").unwrap();
    let budget = if std::env::var_os("GALILEO_FULL").is_some() { Budget::default() } else { Budget::work(200_000) };
    let Verdict::Holds(proof) = why(&program, &[], goal, &[], budget).unwrap() else {
        panic!("the goal holds");
    };
    let mut found = Vec::new();
    leaves(&proof, &mut found);
    assert_eq!(found.len(), 20);
    let all = why_all(&program, &[], goal, &[], budget).unwrap();
    let Verdict::Unknown(Reason::OutOfBudget { resource: Resource::Work, .. }) = &all else {
        panic!("expected Unknown(work), got {all:?}");
    };
    println!("{}", why::render_all(&program, goal, &[], &all, budget));
}

fn height(proof: &Proof) -> usize {
    match proof {
        Proof::Derived { premises, .. } => 1 + premises.iter().map(height).max().unwrap_or(0),
        _ => 0,
    }
}

#[test]
fn why_returns_a_proof_of_minimal_height_through_a_recursive_group() {
    let mut builder = Program::builder();
    let seed = builder.relation(Relation::new("seed", RelationKind::Static, []));
    builder.fact(Fact::new(seed, []));
    let mut previous = seed;
    for n in 1..=5 {
        let next = builder.relation(Relation::new(format!("long-{n}"), RelationKind::Derived, []));
        builder.rule(Rule::new(format!("long-{n}"), Atom::new(next, [])).when([Atom::new(previous, []).positive()]));
        previous = next;
    }
    let bridge = builder.relation(Relation::new("bridge", RelationKind::Derived, []));
    let goal = builder.relation(Relation::new("goal", RelationKind::Derived, []));
    builder.rule(Rule::new("bridge-from-seed", Atom::new(bridge, [])).when([Atom::new(seed, []).positive()]));
    builder.rule(Rule::new("long-goal", Atom::new(goal, [])).when([Atom::new(previous, []).positive()]));
    builder.rule(Rule::new("short-goal", Atom::new(goal, [])).when([Atom::new(bridge, []).positive()]));
    builder.rule(Rule::new("bridge-from-goal", Atom::new(bridge, [])).when([Atom::new(goal, []).positive()]));
    let program = builder.build();
    let budget = if std::env::var_os("GALILEO_FULL").is_some() { Budget::default() } else { Budget::work(200_000) };
    let Verdict::Holds(proof) = why(&program, &[], goal, &[], budget).unwrap() else {
        panic!("goal holds through seed and bridge");
    };
    assert_eq!(height(&proof), 2, "{proof:?}");
}
