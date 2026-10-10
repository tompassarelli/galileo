use muove_core::state::{step, State, Transition};
use muove_core::*;
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
    assert_eq!(t.state.fields, BTreeMap::from([(row.clone(), row_fields())]));
    assert_eq!(t.state.facts, BTreeSet::from([(shown, vec![Value::Entity(row)])]));
    assert_eq!(t.state.next, 1);
}

#[test]
fn a_delete_removes_every_field_and_the_entity() {
    let (program, _, add, drop) = menu();
    let created = run(&program, &State::initial(&program), add, &[Value::text("a"), Value::text("Alpha"), Value::Int(1)]);
    let row = Value::Entity(Entity::created("MenuRow", "MenuRow#0", 0));
    let t = run(&program, &created.state, drop, &[row]);
    assert!(t.state.live.is_empty());
    assert!(t.state.fields.is_empty());
    assert!(t.state.facts.is_empty());
    assert_eq!(t.state.next, 1);
}

fn notes(swapped: bool) -> (Program, RelationId, RelationId) {
    let mut p = Program::builder();
    p.state_entity_type("Note", [slot("text", Type::Text)]);
    let say = p.relation(Relation::new("say", RelationKind::Effect, [slot("text", Type::Text)]));
    let go = p.relation(Relation::new("go", RelationKind::Event, []));
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
    let ids: Vec<(String, Value)> = t.state.fields.iter().map(|(n, f)| (n.name.clone(), f["text"].clone())).collect();
    assert_eq!(ids, vec![("Note#0".to_string(), Value::text("apple")), ("Note#1".to_string(), Value::text("zebra"))]);
    let (swapped, _, _) = notes(true);
    assert_eq!(run(&swapped, &State::initial(&swapped), go, &[]), t);
}
