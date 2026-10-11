use galileo_core::state::{step, State, Transition};
use galileo_core::*;
use std::collections::{BTreeMap, BTreeSet};

fn e(ty: &str, name: &str) -> Value {
    Value::entity(ty, name)
}

fn slot(name: &str, ty: Type) -> Slot {
    Slot::new(name, ty)
}

fn run(program: &Program, state: &State, event: RelationId, arguments: &[Value]) -> Transition {
    step(program, state, event, arguments, Budget::default()).expect("step")
}

#[test]
fn a_dungeon_move_withdraws_the_old_room_by_the_mode() {
    let mut p = Program::builder();
    for ty in ["Hero", "Room", "Door"] {
        p.entity_type(ty);
    }
    let leads = p.relation(Relation::new(
        "leads-from",
        RelationKind::Static,
        [slot("door", Type::entity("Door")), slot("here", Type::entity("Room")), slot("there", Type::entity("Room"))],
    ));
    let is_in = p.relation(
        Relation::new("is-in", RelationKind::State, [slot("hero", Type::entity("Hero")), slot("room", Type::entity("Room"))])
            .mode(Mode::ExactlyOne { key: 0, values: vec![1] }),
    );
    let moves = p.relation(Relation::new(
        "move",
        RelationKind::Event,
        [slot("hero", Type::entity("Hero")), slot("door", Type::entity("Door"))],
    ));
    p.fact(Fact::new(leads, [e("Door", "Front-Door"), e("Room", "Entrance"), e("Room", "Hall")]));
    p.initially(Fact::new(is_in, [e("Hero", "Ash"), e("Room", "Entrance")]));
    p.event(Event::new(moves));
    p.handler(
        Handler::new(Atom::new(moves, [Expr::var("hero"), Expr::var("door")])).clause(
            HandlerClause::new()
                .when([
                    Atom::new(leads, [Expr::var("door"), Expr::var("here"), Expr::var("there")]).positive(),
                    Atom::new(is_in, [Expr::var("hero"), Expr::var("here")]).positive(),
                ])
                .action(Action::Set(Atom::new(is_in, [Expr::var("hero"), Expr::var("there")]))),
        ),
    );
    let program = p.build();
    let pre = State::initial(&program);
    let t = run(&program, &pre, moves, &[e("Hero", "Ash"), e("Door", "Front-Door")]);
    assert_eq!(t.state.facts, BTreeSet::from([(is_in, vec![e("Hero", "Ash"), e("Room", "Hall")])]));
    assert!(t.effects.is_empty());
}

#[test]
fn withdrawing_and_including_one_fact_is_a_no_op() {
    let mut p = Program::builder();
    p.entity_type("Key");
    p.entity_type("Room");
    let lies = p.relation(Relation::new(
        "lies-in",
        RelationKind::State,
        [slot("key", Type::entity("Key")), slot("room", Type::entity("Room"))],
    ));
    let touch = p.relation(Relation::new("touch", RelationKind::Event, [slot("key", Type::entity("Key"))]));
    p.initially(Fact::new(lies, [e("Key", "Bronze-Key"), e("Room", "Library")]));
    p.event(Event::new(touch));
    let fact = || Atom::new(lies, [Expr::var("key"), Expr::var("room")]);
    p.handler(
        Handler::new(Atom::new(touch, [Expr::var("key")])).clause(
            HandlerClause::new()
                .when([fact().positive()])
                .action(Action::Withdraw(fact()))
                .action(Action::Include(fact())),
        ),
    );
    let program = p.build();
    let pre = State::initial(&program);
    let t = run(&program, &pre, touch, &[e("Key", "Bronze-Key")]);
    assert_eq!(t.substitutions, 1);
    assert_eq!(t.state, pre);
}

fn menu() -> (Program, RelationId, RelationId, RelationId) {
    let mut p = Program::builder();
    p.state_entity_type("MenuRow", [slot("key", Type::Text), slot("label", Type::Text), slot("order", Type::Int)]);
    let shown = p.relation(Relation::new("shown", RelationKind::State, [slot("row", Type::entity("MenuRow"))]));
    let add = p.relation(Relation::new(
        "add",
        RelationKind::Event,
        [slot("key", Type::Text), slot("label", Type::Text), slot("order", Type::Int)],
    ));
    let drop = p.relation(Relation::new("drop", RelationKind::Event, [slot("row", Type::entity("MenuRow"))]));
    for (name, ty) in [("key", Type::Text), ("label", Type::Text), ("order", Type::Int)] {
        p.relation(
            Relation::new(format!("MenuRow.{name}"), RelationKind::State, [slot("MenuRow", Type::entity("MenuRow")), slot(name, ty)])
                .mode(Mode::ExactlyOne { key: 0, values: vec![1] }),
        );
    }
    p.event(Event::new(add));
    p.event(Event::new(drop));
    p.handler(
        Handler::new(Atom::new(add, [Expr::var("k"), Expr::var("l"), Expr::var("o")])).clause(
            HandlerClause::new()
                .action(Action::Create {
                    variable: Variable::new("r"),
                    entity_type: "MenuRow".into(),
                    fields: BTreeMap::from([
                        ("key".to_string(), Expr::var("k")),
                        ("label".to_string(), Expr::var("l")),
                        ("order".to_string(), Expr::var("o")),
                    ]),
                })
                .action(Action::Include(Atom::new(shown, [Expr::var("r")]))),
        ),
    );
    p.handler(
        Handler::new(Atom::new(drop, [Expr::var("r")]))
            .clause(HandlerClause::new().action(Action::Delete(Expr::var("r")))),
    );
    (p.build(), shown, add, drop)
}

fn row_fields() -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("key".to_string(), Value::text("a")),
        ("label".to_string(), Value::text("Alpha")),
        ("order".to_string(), Value::Int(1)),
    ])
}

#[test]
fn a_create_sets_every_field() {
    let (program, shown, add, _) = menu();
    let pre = State::initial(&program);
    let t = run(&program, &pre, add, &[Value::text("a"), Value::text("Alpha"), Value::Int(1)]);
    let row = Entity::created("MenuRow", "MenuRow#0", 0);
    assert_eq!(t.state.live, BTreeSet::from([row.clone()]));
    let mut facts = BTreeSet::from([(shown, vec![Value::Entity(row.clone())])]);
    for (field, value) in row_fields() {
        facts.insert((program.field_relation("MenuRow", &field).unwrap(), vec![Value::Entity(row.clone()), value]));
    }
    assert_eq!(t.state.facts, facts);
    assert_eq!(t.state.next, 1);
}

#[test]
fn a_delete_removes_every_field_and_the_entity() {
    let (program, _, add, drop) = menu();
    let created = run(&program, &State::initial(&program), add, &[Value::text("a"), Value::text("Alpha"), Value::Int(1)]);
    let row = Value::Entity(Entity::created("MenuRow", "MenuRow#0", 0));
    let t = run(&program, &created.state, drop, &[row]);
    assert!(t.state.live.is_empty());
    assert!(t.state.facts.is_empty());
    assert_eq!(t.state.next, 1);
}

fn notes(swapped: bool) -> (Program, RelationId, RelationId) {
    let mut p = Program::builder();
    p.state_entity_type("Note", [slot("text", Type::Text)]);
    let say = p.relation(Relation::new("say", RelationKind::Effect, [slot("text", Type::Text)]));
    let go = p.relation(Relation::new("go", RelationKind::Event, []));
    p.relation(
        Relation::new("Note.text", RelationKind::State, [slot("Note", Type::entity("Note")), slot("text", Type::Text)])
            .mode(Mode::ExactlyOne { key: 0, values: vec![1] }),
    );
    p.effect(Effect::new(say));
    p.event(Event::new(go));
    let clause = |text: &str| {
        HandlerClause::new()
            .action(Action::Emit(Atom::new(say, [Expr::value(text)])))
            .action(Action::Create {
                variable: Variable::new("n"),
                entity_type: "Note".into(),
                fields: BTreeMap::from([("text".to_string(), Expr::value(text))]),
            })
    };
    let (first, second) = if swapped { ("apple", "zebra") } else { ("zebra", "apple") };
    p.handler(Handler::new(Atom::new(go, [])).clause(clause(first)).clause(clause(second)));
    (p.build(), say, go)
}

#[test]
fn swapping_two_clauses_changes_neither_the_effects_nor_the_ids() {
    let (program, say, go) = notes(false);
    let t = run(&program, &State::initial(&program), go, &[]);
    assert_eq!(t.effects, vec![(say, vec![Value::text("apple")]), (say, vec![Value::text("zebra")])]);
    let ids: Vec<(String, Value)> = t.state.facts.iter().filter_map(|(relation, values)| {
        if *relation != program.field_relation("Note", "text").unwrap() {
            return None;
        }
        let Value::Entity(entity) = &values[0] else { panic!("field owner is an entity") };
        Some((entity.name.clone(), values[1].clone()))
    }).collect();
    assert_eq!(ids, vec![("Note#0".to_string(), Value::text("apple")), ("Note#1".to_string(), Value::text("zebra"))]);
    let (swapped, _, _) = notes(true);
    assert_eq!(run(&swapped, &State::initial(&swapped), go, &[]), t);
}

#[test]
fn spec_5_3_no_match_rejects_unless_the_handler_may_skip() {
    let mut p = Program::builder();
    let ready = p.relation(Relation::new("ready", RelationKind::State, []));
    let go = p.relation(Relation::new("go", RelationKind::Event, []));
    p.event(Event::new(go));
    p.handler(Handler::new(Atom::new(go, [])).clause(HandlerClause::new().when([Atom::new(ready, []).positive()])));
    let mut program = p.build();
    let pre = State { next: 7, ..State::initial(&program) };
    let original = pre.clone();
    assert!(matches!(step(&program, &pre, go, &[], Budget::default()), Err(EvalError::Type { at, .. }) if at == "step"));
    program.handlers[0].may_skip = true;
    let admitted = run(&program, &pre, go, &[]);
    assert_eq!(admitted.state, original);
    assert_eq!(admitted.substitutions, 0);
    assert!(admitted.effects.is_empty());
}

#[test]
fn spec_5_3_conflicting_writes_reject_the_whole_step() {
    let mut p = Program::builder();
    p.entity_type("Hero");
    let position = p.relation(Relation::new("position", RelationKind::State,
        [slot("hero", Type::entity("Hero")), slot("room", Type::Text)])
        .mode(Mode::ExactlyOne { key: 0, values: vec![1] }).template("{hero} is in {room}"));
    let go = p.relation(Relation::new("go", RelationKind::Event, []));
    p.entity(Entity::named("Hero", "Ash"));
    p.initially(Fact::new(position, [e("Hero", "Ash"), Value::text("Entrance")]));
    p.event(Event::new(go));
    for room in ["Hall", "Vault"] {
        p.handler(Handler::new(Atom::new(go, [])).clause(HandlerClause::new()
            .action(Action::Set(Atom::new(position, [Expr::Value(e("Hero", "Ash")), Expr::value(room)])))));
    }
    let program = p.build();
    let pre = State::initial(&program);
    assert!(matches!(step(&program, &pre, go, &[], Budget::default()), Err(EvalError::Type { at, .. }) if at == "step"));
}

#[test]
fn spec_5_3_every_live_exactly_one_slot_must_be_filled() {
    let mut p = Program::builder();
    p.state_entity_type("Unit", []);
    let position = p.relation(Relation::new("position", RelationKind::State,
        [slot("unit", Type::entity("Unit")), slot("room", Type::Text)])
        .mode(Mode::ExactlyOne { key: 0, values: vec![1] }).template("{unit} is in {room}"));
    let go = p.relation(Relation::new("go", RelationKind::Event, []));
    p.event(Event::new(go));
    p.handler(Handler::new(Atom::new(go, [])).clause(HandlerClause::new().action(Action::Create {
        variable: Variable::new("u"), entity_type: "Unit".into(), fields: BTreeMap::new(),
    })));
    let program = p.build();
    let pre = State::initial(&program);
    assert!(matches!(step(&program, &pre, go, &[], Budget::default()), Err(EvalError::Type { at, .. }) if at == "step"));
    assert!(!pre.facts.iter().any(|(relation, _)| *relation == position));
}

#[test]
fn spec_5_3_derived_modes_reject_after_state_modes() {
    let mut p = Program::builder();
    p.entity_type("Hero");
    p.entity(Entity::named("Hero", "Ash"));
    let offers = p.relation(Relation::new("offers", RelationKind::State, [slot("room", Type::Text)]));
    let chosen = p.relation(Relation::new("chosen", RelationKind::Derived,
        [slot("hero", Type::entity("Hero")), slot("room", Type::Text)])
        .mode(Mode::ExactlyOne { key: 0, values: vec![1] }).template("{hero} chooses {room}"));
    let go = p.relation(Relation::new("go", RelationKind::Event, []));
    p.initially(Fact::new(offers, [Value::text("Hall")]));
    p.rule(Rule::new("choose", Atom::new(chosen, [Expr::Value(e("Hero", "Ash")), Expr::var("room")]))
        .when([Atom::new(offers, [Expr::var("room")]).positive()]));
    p.event(Event::new(go));
    p.handler(Handler::new(Atom::new(go, [])).clause(HandlerClause::new()
        .action(Action::Include(Atom::new(offers, [Expr::value("Vault")])))));
    let program = p.build();
    let pre = State::initial(&program);
    assert!(matches!(step(&program, &pre, go, &[], Budget::default()), Err(EvalError::Type { at, .. }) if at == "step"));
}

#[test]
fn spec_6_3_invariants_reject_the_post_state() {
    let mut p = Program::builder();
    let ready = p.relation(Relation::new("ready", RelationKind::State, []));
    let go = p.relation(Relation::new("go", RelationKind::Event, []));
    p.initially(Fact::new(ready, []));
    p.requirement(Requirement::invariant("ready", StateClaim::Holds(Atom::new(ready, []))));
    p.event(Event::new(go));
    p.handler(Handler::new(Atom::new(go, [])).clause(HandlerClause::new().action(Action::Withdraw(Atom::new(ready, [])))));
    let program = p.build();
    let pre = State::initial(&program);
    assert!(matches!(step(&program, &pre, go, &[], Budget::default()), Err(EvalError::Type { at, .. }) if at == "step"));
}

#[test]
fn spec_5_3_ensures_reads_each_matched_clauses_post_state_binding() {
    let mut p = Program::builder();
    let ready = p.relation(Relation::new("ready", RelationKind::State, [slot("name", Type::Text)]).template("{name} is ready"));
    let go = p.relation(Relation::new("go", RelationKind::Event, [slot("name", Type::Text)]));
    p.initially(Fact::new(ready, [Value::text("Ash")]));
    p.event(Event::new(go));
    p.handler(Handler::new(Atom::new(go, [Expr::var("name")])).clause(HandlerClause::new()
        .action(Action::Withdraw(Atom::new(ready, [Expr::var("name")])))
        .ensures(StateClaim::Holds(Atom::new(ready, [Expr::var("name")])))));
    let program = p.build();
    let pre = State::initial(&program);
    assert!(matches!(step(&program, &pre, go, &[Value::text("Ash")], Budget::default()), Err(EvalError::Type { at, .. }) if at == "step"));
}

#[test]
fn spec_5_3_including_an_entity_deleted_in_the_same_step_rejects() {
    let (mut program, shown, add, drop) = menu();
    let created = run(&program, &State::initial(&program), add, &[Value::text("a"), Value::text("Alpha"), Value::Int(1)]);
    program.handlers[1].clauses[0].actions.push(Action::Include(Atom::new(shown, [Expr::var("r")])));
    let pre = created.state;
    let row = Value::Entity(Entity::created("MenuRow", "MenuRow#0", 0));
    assert!(matches!(step(&program, &pre, drop, &[row], Budget::default()), Err(EvalError::Type { at, .. }) if at == "step"));
}

#[test]
fn spec_5_4_competing_effects_reject_but_equal_requests_deduplicate() {
    let (mut program, say, go) = notes(false);
    program.effects[0].at_most_one_per_step = true;
    let pre = State::initial(&program);
    assert!(matches!(step(&program, &pre, go, &[], Budget::default()), Err(EvalError::Type { at, .. }) if at == "step"));
    program.handlers[0].clauses[0].actions[0] = Action::Emit(Atom::new(say, [Expr::value("apple")]));
    let other = RelationId(program.relations.len());
    program.relations.push(Relation::new("write", RelationKind::Effect, [slot("text", Type::Text)]));
    program.effects.push(Effect::new(other).at_most_one_per_step());
    program.handlers[0].clauses[0].actions.push(Action::Emit(Atom::new(other, [Expr::value("entry")])));
    let admitted = run(&program, &pre, go, &[]);
    assert_eq!(admitted.effects, vec![(say, vec![Value::text("apple")]), (other, vec![Value::text("entry")])]);
    assert_eq!(admitted.state.next, 2);
    assert_eq!(admitted.state.live.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Note#0", "Note#1"]);
}
