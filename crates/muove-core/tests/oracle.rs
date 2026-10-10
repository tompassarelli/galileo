use muove_core::supports::{failure_sets, minimal_supports, Support};
use muove_core::why::sentence;
use muove_core::*;
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
        holds(failure_sets(&program, &[], route, &north, &[passed_id], budget).unwrap(), &program),
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
        holds(failure_sets(&withdrawn, &[], route, &north, &[passed_id], budget).unwrap(), &withdrawn),
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
        holds(failure_sets(&program, &[], affected, &target, &[imports], budget).unwrap(), &program),
        expected(&[
            &["North imports Relay", "North imports Store"],
            &["North imports Relay", "Store imports Beagle"],
            &["North imports Store", "Relay imports Beagle"],
            &["Relay imports Beagle", "Store imports Beagle"],
        ]),
    );
}
