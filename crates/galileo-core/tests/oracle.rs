use galileo_core::supports::{failure_sets, minimal_supports, Strength, Support};
use galileo_core::why::sentence;
use galileo_core::*;
use std::collections::BTreeSet;

fn relation(p: &mut ProgramBuilder, name: &str, slots: &[(&str, &str)], template: &str) -> RelationId {
    let kind = if name.ends_with("route") || name.ends_with("depends") || name.ends_with("affected") {
        RelationKind::Derived
    } else {
        RelationKind::Static
    };
    let slots = slots.iter().map(|(name, ty)| Slot::new(*name, Type::entity(*ty)));
    p.relation(Relation::new(name, kind, slots).template(template))
}

fn term(text: &str, ty: &str) -> Expr {
    match text.strip_prefix('?') {
        Some(name) => Expr::var(name),
        None => Expr::value(Value::entity(ty, text)),
    }
}

fn literal(id: RelationId, terms: &[(&str, &str)]) -> Literal {
    Atom::new(id, terms.iter().map(|(text, ty)| term(text, ty))).positive()
}

fn fact(id: RelationId, terms: &[(&str, &str)]) -> Fact {
    Fact::new(id, terms.iter().map(|(name, ty)| Value::entity(*ty, *name)))
}

fn hospital() -> Program {
    let mut p = Program::builder();
    for ty in ["Space", "Door", "Inspection"] {
        p.entity_type(ty);
    }
    for space in ["ICU-A", "East-Corridor", "West-Corridor", "North-Exit", "Isolation-Room"] {
        p.entity(Entity::named("Space", space));
    }
    p.entity(Entity::named("Inspection", "Fire-Marshal-Inspection"));
    for n in 101..=106 {
        p.entity(Entity::named("Door", format!("Door {n}")));
    }
    let connects = relation(&mut p, "egress/connects", &[("door", "Door"), ("origin", "Space"), ("destination", "Space")], "{door} connects {origin} to {destination}");
    let passed = relation(&mut p, "egress/passed", &[("door", "Door"), ("inspection", "Inspection")], "{door} passed {inspection}");
    let route = relation(&mut p, "egress/route", &[("origin", "Space"), ("destination", "Space")], "{origin} has a usable egress path to {destination}");
    for n in 101..=104 {
        p.fact(fact(passed, &[(&format!("Door {n}"), "Door"), ("Fire-Marshal-Inspection", "Inspection")]));
    }
    for (n, from, to) in [
        (101, "ICU-A", "East-Corridor"),
        (102, "East-Corridor", "North-Exit"),
        (103, "ICU-A", "West-Corridor"),
        (104, "West-Corridor", "North-Exit"),
        (105, "Isolation-Room", "East-Corridor"),
        (106, "Isolation-Room", "West-Corridor"),
    ] {
        p.fact(fact(connects, &[(&format!("Door {n}"), "Door"), (from, "Space"), (to, "Space")]));
    }
    let inspected = literal(passed, &[("?door", "Door"), ("Fire-Marshal-Inspection", "Inspection")]);
    p.rule(Rule::new("egress/direct-route", Atom::new(route, [Expr::var("origin"), Expr::var("destination")])).when([
        literal(connects, &[("?door", ""), ("?origin", ""), ("?destination", "")]),
        inspected.clone(),
    ]));
    p.rule(Rule::new("egress/recursive-route", Atom::new(route, [Expr::var("origin"), Expr::var("destination")])).when([
        literal(connects, &[("?door", ""), ("?origin", ""), ("?intermediate", "")]),
        inspected,
        literal(route, &[("?intermediate", ""), ("?destination", "")]),
    ]));
    p.build()
}

fn impact() -> Program {
    let mut p = Program::builder();
    for ty in ["Module", "Change"] {
        p.entity_type(ty);
    }
    for module in ["North", "Store", "Relay", "Beagle", "South"] {
        p.entity(Entity::named("Module", module));
    }
    p.entity(Entity::named("Change", "compiler-change"));
    let imports = relation(&mut p, "impact/imports", &[("consumer", "Module"), ("dependency", "Module")], "{consumer} imports {dependency}");
    let depends = relation(&mut p, "impact/depends", &[("consumer", "Module"), ("dependency", "Module")], "{consumer} depends on {dependency}");
    let changes = relation(&mut p, "impact/changes", &[("change", "Change"), ("component", "Module")], "{change} changes {component}");
    let affected = relation(&mut p, "impact/affected", &[("change", "Change"), ("consumer", "Module")], "{change} affects {consumer}");
    for (consumer, dependency) in [("North", "Store"), ("North", "Relay"), ("Store", "Beagle"), ("Relay", "Beagle")] {
        p.fact(fact(imports, &[(consumer, "Module"), (dependency, "Module")]));
    }
    p.fact(fact(changes, &[("compiler-change", "Change"), ("Beagle", "Module")]));
    let head = |id, a: &str, b: &str| Atom::new(id, [Expr::var(a), Expr::var(b)]);
    p.rule(Rule::new("impact/direct-dependency", head(depends, "consumer", "dependency")).when([literal(imports, &[("?consumer", ""), ("?dependency", "")])]));
    p.rule(Rule::new("impact/recursive-dependency", head(depends, "consumer", "dependency")).when([
        literal(imports, &[("?consumer", ""), ("?intermediate", "")]),
        literal(depends, &[("?intermediate", ""), ("?dependency", "")]),
    ]));
    p.rule(Rule::new("impact/impact", head(affected, "change", "consumer")).when([
        literal(changes, &[("?change", ""), ("?component", "")]),
        literal(depends, &[("?consumer", ""), ("?component", "")]),
    ]));
    p.build()
}

fn holds(verdict: Verdict<Vec<Support>, ()>, program: &Program) -> BTreeSet<BTreeSet<String>> {
    let Verdict::Holds(sets) = verdict else { panic!("expected an answer, got {verdict:?}") };
    sets.iter()
        .map(|set| set.iter().map(|f| sentence(program, f.relation, &f.values)).collect())
        .collect()
}

fn smallest(strength: Strength, program: &Program) -> BTreeSet<BTreeSet<String>> {
    let Strength::Smallest { sets, complete: true } = strength else { panic!("expected complete failure sets, got {strength:?}") };
    holds(Verdict::Holds(sets), program)
}

fn expected(groups: &[&[&str]]) -> BTreeSet<BTreeSet<String>> {
    groups.iter().map(|g| g.iter().map(|s| s.to_string()).collect()).collect()
}

fn passed(n: u32) -> String {
    format!("Door {n} passed Fire-Marshal-Inspection")
}

#[test]
fn hospital_matches_the_original_oracle() {
    let program = hospital();
    let route = program.relation_id("egress/route").unwrap();
    let passed_id = program.relation_id("egress/passed").unwrap();
    let north = [Value::entity("Space", "ICU-A"), Value::entity("Space", "North-Exit")];
    let budget = Budget::default();

    assert_eq!(
        holds(minimal_supports(&program, &[], route, &north, budget).unwrap(), &program),
        expected(&[
            &["Door 101 connects ICU-A to East-Corridor", &passed(101), "Door 102 connects East-Corridor to North-Exit", &passed(102)],
            &["Door 103 connects ICU-A to West-Corridor", &passed(103), "Door 104 connects West-Corridor to North-Exit", &passed(104)],
        ]),
    );
    assert_eq!(
        smallest(failure_sets(&program, &[], route, &north, &[passed_id], budget).unwrap(), &program),
        expected(&[
            &[&passed(101), &passed(103)],
            &[&passed(101), &passed(104)],
            &[&passed(102), &passed(103)],
            &[&passed(102), &passed(104)],
        ]),
    );

    let mut withdrawn = program.clone();
    withdrawn.facts.retain(|f| sentence(&program, f.relation, &f.values) != passed(101));
    assert_eq!(withdrawn.facts.len() + 1, program.facts.len());
    assert_eq!(
        smallest(failure_sets(&withdrawn, &[], route, &north, &[passed_id], budget).unwrap(), &withdrawn),
        expected(&[&[&passed(103)], &[&passed(104)]]),
    );
}

#[test]
fn impact_matches_the_original_oracle() {
    let program = impact();
    let affected = program.relation_id("impact/affected").unwrap();
    let imports = program.relation_id("impact/imports").unwrap();
    let target = [Value::entity("Change", "compiler-change"), Value::entity("Module", "North")];
    let budget = Budget::default();

    assert_eq!(
        holds(minimal_supports(&program, &[], affected, &target, budget).unwrap(), &program),
        expected(&[
            &["compiler-change changes Beagle", "North imports Relay", "Relay imports Beagle"],
            &["compiler-change changes Beagle", "North imports Store", "Store imports Beagle"],
        ]),
    );
    assert_eq!(
        smallest(failure_sets(&program, &[], affected, &target, &[imports], budget).unwrap(), &program),
        expected(&[
            &["North imports Relay", "North imports Store"],
            &["North imports Relay", "Store imports Beagle"],
            &["North imports Store", "Relay imports Beagle"],
            &["Relay imports Beagle", "Store imports Beagle"],
        ]),
    );
}

fn achievement(selected: &[&str]) -> Program {
    let mut p = Program::builder();
    for ty in ["Start", "Option", "State"] {
        p.entity_type(ty);
    }
    p.entity(Entity::named("Start", "South"));
    for option in ["Beagle", "North", "Relay", "Store"] {
        p.entity(Entity::named("Option", option));
    }
    p.entity(Entity::named("State", "Ready"));
    let selects = relation(&mut p, "choice/selects", &[("start", "Start"), ("option", "Option")], "{start} selects {option}");
    let reached = p.relation(
        Relation::new("choice/reached", RelationKind::Derived, [Slot::new("start", Type::entity("Start")), Slot::new("state", Type::entity("State"))])
            .template("{start} reached {state}"),
    );
    for option in selected {
        p.fact(fact(selects, &[("South", "Start"), (option, "Option")]));
    }
    p.rule(Rule::new("choice/selection-reaches-ready", Atom::new(reached, [Expr::var("start"), term("Ready", "State")]))
        .when([literal(selects, &[("?start", ""), ("?option", "")])]));
    p.build()
}

#[test]
fn typed_achievement_strength_before_and_after_the_four_additions() {
    let target = [Value::entity("Start", "South"), Value::entity("State", "Ready")];
    let budget = Budget::default();

    let program = achievement(&[]);
    let reached = program.relation_id("choice/reached").unwrap();
    let selects = program.relation_id("choice/selects").unwrap();
    let strength = failure_sets(&program, &[], reached, &target, &[selects], budget).unwrap();
    assert_eq!(strength.size(), Some(0));

    let program = achievement(&["Beagle", "North", "Relay", "Store"]);
    assert_eq!(
        smallest(failure_sets(&program, &[], reached, &target, &[selects], budget).unwrap(), &program),
        expected(&[&["South selects Beagle", "South selects North", "South selects Relay", "South selects Store"]]),
    );
}

fn guarded() -> (Program, RelationId, RelationId) {
    let mut p = Program::builder();
    let a = p.relation(Relation::new("a", RelationKind::Static, []).template("a"));
    let s = p.relation(Relation::new("s", RelationKind::Derived, []).template("s"));
    p.fact(Fact::new(a, []));
    p.rule(Rule::new("s-when-a", Atom::new(s, [])).when([Atom::new(a, []).positive()]));
    p.rule(Rule::new("s-when-not-a", Atom::new(s, [])).when([Atom::new(a, []).negative()]));
    (p.build(), s, a)
}

#[test]
fn s_when_a_or_not_a_is_unbreakable_within_a() {
    let (program, s, a) = guarded();
    let strength = failure_sets(&program, &[], s, &[], &[a], Budget::default()).unwrap();
    assert_eq!(strength, Strength::Unbreakable);
    let starved = failure_sets(&program, &[], s, &[], &[a], Budget::work(3)).unwrap();
    assert!(matches!(starved, Strength::Unknown(_)), "{starved:?}");

    let mut p = Program::builder();
    let pattern: Vec<_> = (0..4)
        .map(|i| p.relation(Relation::new(format!("a{i}"), RelationKind::Static, []).template(format!("a{i}"))))
        .collect();
    let s = p.relation(Relation::new("s", RelationKind::Derived, []).template("s"));
    for &a in &pattern[..3] {
        p.fact(Fact::new(a, []));
    }
    p.rule(Rule::new("s-when-a0-and-not-a3", Atom::new(s, [])).when([
        Atom::new(pattern[0], []).positive(),
        Atom::new(pattern[3], []).negative(),
    ]));
    let program = p.build();
    let partial = failure_sets(&program, &[], s, &[], &pattern, Budget::work(14)).unwrap();
    assert_eq!(partial, Strength::Smallest { sets: vec![vec![Fact::new(pattern[0], [])]], complete: false });
    let complete = failure_sets(&program, &[], s, &[], &pattern, Budget::work(34)).unwrap();
    assert_eq!(complete, Strength::Smallest { sets: vec![vec![Fact::new(pattern[0], [])]], complete: true });
}
