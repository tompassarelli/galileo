use galileo_core::examples::dungeon_static;
use galileo_core::supports::{achieve, prevent, AchieveMode, Intervention};
use galileo_core::why::sentence;
use galileo_core::*;
use std::collections::BTreeSet;

fn placements(program: &Program, sets: &[Vec<Fact>]) -> BTreeSet<String> {
    sets.iter()
        .map(|set| {
            assert_eq!(set.len(), 1, "{set:?}");
            sentence(program, set[0].relation, &set[0].values)
        })
        .collect()
}

#[test]
fn r17_achieve_acceptable_excludes_the_placements_that_break_never() {
    let program = dungeon_static();
    let reach = program.relation_id("level/reach").unwrap();
    let lies = program.relation_id("level/lies-in").unwrap();
    let shrine = [Value::entity("Hero", "Ash"), Value::entity("Room", "Shrine")];
    let budget = Budget::default();
    let six: BTreeSet<String> = ["Entrance", "Hall", "Library", "Armory", "Crypt", "Vault"]
        .iter()
        .map(|room| format!("Silver-Key lies in {room}"))
        .collect();

    let Intervention::Found { sets, excluded } =
        achieve(&program, &[], &said(reach, &shrine), &[lies], AchieveMode::Possible, budget).unwrap()
    else {
        panic!("expected placements")
    };
    assert_eq!(placements(&program, &sets), six);
    assert!(excluded.is_empty());

    let Intervention::Found { sets, excluded } =
        achieve(&program, &[], &said(reach, &shrine), &[lies], AchieveMode::Acceptable, budget).unwrap()
    else {
        panic!("expected placements")
    };
    assert!(sets.is_empty(), "{sets:?}");
    let broken: Vec<Vec<Fact>> = excluded.iter().map(|(set, _)| set.clone()).collect();
    assert_eq!(placements(&program, &broken), six);
    assert!(excluded.iter().all(|(_, names)| names == &["never-shrine".to_string()]));

    let mut permissive = program.clone();
    permissive.requirements.retain(|r| r.name != "never-shrine");
    let Intervention::Found { sets, excluded } =
        achieve(&permissive, &[], &said(reach, &shrine), &[lies], AchieveMode::Acceptable, budget).unwrap()
    else {
        panic!("expected placements")
    };
    assert_eq!(placements(&permissive, &sets), six);
    assert!(excluded.is_empty());

    let starved = achieve(&program, &[], &said(reach, &shrine), &[lies], AchieveMode::Acceptable, Budget::work(500)).unwrap();
    assert!(matches!(starved, Intervention::Unknown(_)), "{starved:?}");
}

#[test]
fn spec_6_1_achieve_charges_each_value_in_candidate_tuple_prefixes() {
    let mut builder = Program::builder();
    builder.entity_type("E");
    builder.entity(Entity::named("E", "e"));
    let target = builder.relation(Relation::new("target", RelationKind::Derived, []));
    let candidates = builder.relation(Relation::new(
        "candidates",
        RelationKind::Static,
        (0..40).map(|i| Slot::new(format!("s{i}"), Type::entity("E"))),
    ));
    let program = builder.build();

    for work in [0, 200] {
        let result = achieve(&program, &[], &said(target, &[]), &[candidates], AchieveMode::Possible, Budget::work(work)).unwrap();
        assert!(
            matches!(result, Intervention::Unknown(Reason::OutOfBudget { resource: Resource::Work, ref at }) if at == "intervention candidates"),
            "{result:?}",
        );
    }
    assert_eq!(
        achieve(&program, &[], &said(target, &[]), &[candidates], AchieveMode::Possible, Budget::work(2_000)).unwrap(),
        Intervention::Impossible,
    );
}

#[test]
fn spec_7_4_acceptable_minimizes_among_sets_that_hold_every_requirement() {
    let mut builder = Program::builder();
    let a = builder.relation(Relation::new("a", RelationKind::Static, []));
    let b = builder.relation(Relation::new("b", RelationKind::Static, []));
    let goal = builder.relation(Relation::new("goal", RelationKind::Derived, []));
    let alarm = builder.relation(Relation::new("alarm", RelationKind::Derived, []));
    builder
        .rule(Rule::new("goal-when-a", Atom::new(goal, [])).when([Atom::new(a, []).positive()]))
        .rule(Rule::new("alarm-when-a-and-not-b", Atom::new(alarm, [])).when([
            Atom::new(a, []).positive(),
            Atom::new(b, []).negative(),
        ]))
        .requirement(Requirement::new("never alarm", Claim::Never(StateClaim::Holds(Atom::new(alarm, [])))));
    let mut program = builder.build();
    let a_fact = Fact::new(a, []);
    let b_fact = Fact::new(b, []);
    assert_eq!(
        achieve(&program, &[], &said(goal, &[]), &[a, b], AchieveMode::Acceptable, Budget::default()).unwrap(),
        Intervention::Found {
            sets: vec![vec![a_fact.clone(), b_fact.clone()]],
            excluded: vec![(vec![a_fact.clone()], vec!["never alarm".to_string()])],
        }
    );
    assert_eq!(
        achieve(&program, &[], &said(goal, &[]), &[a, b], AchieveMode::Possible, Budget::default()).unwrap(),
        Intervention::Found { sets: vec![vec![a_fact.clone()]], excluded: Vec::new() }
    );
    program.facts.push(a_fact);
    assert_eq!(
        achieve(&program, &[], &said(goal, &[]), &[a, b], AchieveMode::Acceptable, Budget::default()).unwrap(),
        Intervention::Found {
            sets: vec![vec![b_fact]],
            excluded: vec![(Vec::new(), vec!["never alarm".to_string()])],
        }
    );
    assert_eq!(
        achieve(&program, &[], &said(goal, &[]), &[a, b], AchieveMode::Possible, Budget::default()).unwrap(),
        Intervention::Already
    );
}

fn said(relation: RelationId, values: &[Value]) -> Claim {
    Claim::State(StateClaim::Holds(Fact::new(relation, values.iter().cloned()).atom()))
}

fn atom(program: &Program, relation: &str, terms: &[Expr]) -> Atom {
    Atom::new(program.relation_id(relation).unwrap(), terms.iter().cloned())
}

fn ash() -> Expr {
    Expr::value(Value::entity("Hero", "Ash"))
}

fn sentences(program: &Program, sets: &[Vec<Fact>]) -> BTreeSet<BTreeSet<String>> {
    sets.iter().map(|set| set.iter().map(|f| sentence(program, f.relation, &f.values)).collect()).collect()
}

fn holds_after(program: &Program, added: &[Fact], removed: &[Fact], relation: &str, values: &[Value]) -> bool {
    let mut changed = program.clone();
    changed.facts.retain(|fact| !removed.contains(fact));
    changed.facts.extend(added.iter().cloned());
    let model = evaluate(&changed, &[], Budget::default()).unwrap();
    model.contains(changed.relation_id(relation).unwrap(), values)
}

#[test]
fn spec_7_4_a_some_claim_gets_verified_minimal_withdrawal_sets() {
    let program = dungeon_static();
    let some_key = Claim::State(StateClaim::Some {
        variables: vec![Variable::new("k")],
        body: vec![atom(&program, "level/holds", &[ash(), Expr::var("k")]).positive()],
    });
    let held = |removed: &[Fact]| {
        ["Bronze-Key", "Iron-Key", "Silver-Key"]
            .iter()
            .any(|key| holds_after(&program, &[], removed, "level/holds", &[Value::entity("Hero", "Ash"), Value::entity("Key", *key)]))
    };
    let expected = [
        ("level/lies-in", vec![vec!["Bronze-Key lies in Library", "Iron-Key lies in Crypt"]]),
        ("level/unlocked", vec![vec!["Front-Door is unlocked"], vec!["Library-Arch is unlocked"]]),
    ];
    for (using, want) in expected {
        let using = program.relation_id(using).unwrap();
        let Intervention::Found { sets, excluded } = prevent(&program, &[], &some_key, &[using], Budget::default()).unwrap() else {
            panic!("expected withdrawal sets")
        };
        assert!(excluded.is_empty());
        let want: BTreeSet<BTreeSet<String>> = want.iter().map(|set| set.iter().map(|s| s.to_string()).collect()).collect();
        assert_eq!(sentences(&program, &sets), want);
        for set in &sets {
            assert!(!held(set), "{set:?}");
            for skip in 0..set.len() {
                let smaller: Vec<Fact> = set.iter().enumerate().filter(|(i, _)| *i != skip).map(|(_, f)| f.clone()).collect();
                assert!(held(&smaller), "{smaller:?}");
            }
        }
    }
}

#[test]
fn spec_7_4_a_never_claim_gets_verified_minimal_addition_sets() {
    let program = dungeon_static();
    let lies = program.relation_id("level/lies-in").unwrap();
    let no_missing_key = Claim::Never(StateClaim::Some {
        variables: vec![Variable::new("d"), Variable::new("k")],
        body: vec![
            atom(&program, "level/locked-by", &[Expr::var("d"), Expr::var("k")]).positive(),
            atom(&program, "level/holds", &[ash(), Expr::var("k")]).negative(),
        ],
    });
    let Intervention::Found { sets, excluded } =
        achieve(&program, &[], &no_missing_key, &[lies], AchieveMode::Possible, Budget::default()).unwrap()
    else {
        panic!("expected placements")
    };
    assert!(excluded.is_empty());
    let six: BTreeSet<String> = ["Entrance", "Hall", "Library", "Armory", "Crypt", "Vault"]
        .iter()
        .map(|room| format!("Silver-Key lies in {room}"))
        .collect();
    assert_eq!(placements(&program, &sets), six);
    let silver = [Value::entity("Hero", "Ash"), Value::entity("Key", "Silver-Key")];
    assert!(!holds_after(&program, &[], &[], "level/holds", &silver));
    for set in &sets {
        assert!(holds_after(&program, set, &[], "level/holds", &silver), "{set:?}");
    }
}

#[test]
fn spec_7_4_a_temporal_claim_on_the_event_dungeon_is_unsupported() {
    let mut program = dungeon_static();
    let moves = RelationId(program.relations.len());
    program.relations.push(Relation::new("move", RelationKind::Event, [Slot::new("hero", Type::entity("Hero"))]));
    program.events.push(Event::new(moves));
    let lies = program.relation_id("level/lies-in").unwrap();
    let reach = program.relation_id("level/reach").unwrap();
    let possibly = Claim::Eventually(StateClaim::Holds(Fact::new(reach, [Value::entity("Hero", "Ash"), Value::entity("Room", "Shrine")]).atom()));
    let unsupported = |result: Intervention| matches!(result, Intervention::Unknown(Reason::Unsupported { .. }));
    assert!(unsupported(achieve(&program, &[], &possibly, &[lies], AchieveMode::Possible, Budget::default()).unwrap()));
    assert!(unsupported(prevent(&program, &[], &possibly, &[lies], Budget::default()).unwrap()));
}
