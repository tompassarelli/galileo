use galileo_core::examples::dungeon_static;
use galileo_core::supports::{achieve, AchieveMode, Intervention};
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
        achieve(&program, &[], reach, &shrine, &[lies], AchieveMode::Possible, budget).unwrap()
    else {
        panic!("expected placements")
    };
    assert_eq!(placements(&program, &sets), six);
    assert!(excluded.is_empty());

    let Intervention::Found { sets, excluded } =
        achieve(&program, &[], reach, &shrine, &[lies], AchieveMode::Acceptable, budget).unwrap()
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
        achieve(&permissive, &[], reach, &shrine, &[lies], AchieveMode::Acceptable, budget).unwrap()
    else {
        panic!("expected placements")
    };
    assert_eq!(placements(&permissive, &sets), six);
    assert!(excluded.is_empty());

    let starved = achieve(&program, &[], reach, &shrine, &[lies], AchieveMode::Acceptable, Budget::work(500)).unwrap();
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
        let result = achieve(&program, &[], target, &[], &[candidates], AchieveMode::Possible, Budget::work(work)).unwrap();
        assert!(
            matches!(result, Intervention::Unknown(Reason::OutOfBudget { resource: Resource::Work, ref at }) if at == "intervention candidates"),
            "{result:?}",
        );
    }
    assert_eq!(
        achieve(&program, &[], target, &[], &[candidates], AchieveMode::Possible, Budget::work(2_000)).unwrap(),
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
        achieve(&program, &[], goal, &[], &[a, b], AchieveMode::Acceptable, Budget::default()).unwrap(),
        Intervention::Found {
            sets: vec![vec![a_fact.clone(), b_fact.clone()]],
            excluded: vec![(vec![a_fact.clone()], vec!["never alarm".to_string()])],
        }
    );
    assert_eq!(
        achieve(&program, &[], goal, &[], &[a, b], AchieveMode::Possible, Budget::default()).unwrap(),
        Intervention::Found { sets: vec![vec![a_fact.clone()]], excluded: Vec::new() }
    );
    program.facts.push(a_fact);
    assert_eq!(
        achieve(&program, &[], goal, &[], &[a, b], AchieveMode::Acceptable, Budget::default()).unwrap(),
        Intervention::Found {
            sets: vec![vec![b_fact]],
            excluded: vec![(Vec::new(), vec!["never alarm".to_string()])],
        }
    );
    assert_eq!(
        achieve(&program, &[], goal, &[], &[a, b], AchieveMode::Possible, Budget::default()).unwrap(),
        Intervention::Already
    );
}
