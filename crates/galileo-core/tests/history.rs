use galileo_core::history::{Change, Span};
use galileo_core::state::{step, Run};
use galileo_core::*;
use std::collections::BTreeMap;

#[path = "../../galileo-syntax/tests/fixtures/dungeon_walk.rs"]
mod dungeon_walk;

#[test]
fn d52_dungeon_walk_keeps_each_logged_state_and_records_only_net_changes() {
    let mut p = galileo_core::examples::dungeon_static();
    let relation = |name: &str| p.relations.iter().position(|r| r.name == name).map(RelationId).unwrap();
    let leads = relation("level/leads");
    let unlocked = relation("level/unlocked");
    let locked = relation("level/locked-by");
    let lies = relation("level/lies-in");
    let holds = relation("level/holds");
    p.relations[lies.0].kind = RelationKind::State;
    p.relations[lies.0].modes = vec![Mode::AtMostOne { keys: vec![0], values: vec![1] }];
    p.relations[holds.0].kind = RelationKind::State;
    p.rules.retain(|rule| rule.head.relation != holds);
    p.initially = p.facts.iter().filter(|f| f.relation == lies).cloned().collect();
    p.facts.retain(|f| f.relation != lies);
    let is_in = RelationId(p.relations.len());
    p.relations.push(Relation::new("is-in", RelationKind::State, [Slot::new("hero", Type::entity("Hero")), Slot::new("room", Type::entity("Room"))]).mode(Mode::ExactlyOne { key: 0, values: vec![1] }));
    let opens = RelationId(p.relations.len());
    p.relations.push(Relation::new("opens", RelationKind::Derived, [Slot::new("door", Type::entity("Door")), Slot::new("hero", Type::entity("Hero"))]));
    let warned = RelationId(p.relations.len());
    p.relations.push(Relation::new("warned", RelationKind::State, [Slot::new("trap", Type::entity("Trap")), Slot::new("hero", Type::entity("Hero"))]));
    let moves = RelationId(p.relations.len());
    p.relations.push(Relation::new("move", RelationKind::Event, [Slot::new("hero", Type::entity("Hero")), Slot::new("door", Type::entity("Door"))]));
    let picks = RelationId(p.relations.len());
    p.relations.push(Relation::new("pick", RelationKind::Event, [Slot::new("hero", Type::entity("Hero")), Slot::new("key", Type::entity("Key"))]));
    p.events.extend([Event::new(moves), Event::new(picks)]);
    p.initially.push(Fact::new(is_in, [Value::entity("Hero", "Ash"), Value::entity("Room", "Entrance")]));
    p.rules.push(Rule::new("unlocked", Atom::new(opens, [Expr::var("door"), Expr::value(Value::entity("Hero", "Ash"))])).when([Atom::new(unlocked, [Expr::var("door")]).positive()]));
    p.rules.push(Rule::new("held-key", Atom::new(opens, [Expr::var("door"), Expr::var("hero")])).when([
        Atom::new(locked, [Expr::var("door"), Expr::var("key")]).positive(),
        Atom::new(holds, [Expr::var("hero"), Expr::var("key")]).positive(),
    ]));
    let moving = [
        Atom::new(is_in, [Expr::var("hero"), Expr::var("here")]).positive(),
        Atom::new(leads, [Expr::var("door"), Expr::var("here"), Expr::var("there")]).positive(),
        Atom::new(opens, [Expr::var("door"), Expr::var("hero")]).positive(),
    ];
    p.handlers.push(Handler::new(Atom::new(moves, [Expr::var("hero"), Expr::var("door")])).clause(HandlerClause::new().when(moving.clone()).action(Action::Set(Atom::new(is_in, [Expr::var("hero"), Expr::var("there")])))));
    let armed = p.relations.iter().position(|r| r.name == "level/armed-in").map(RelationId).unwrap();
    let telegraphed = p.relations.iter().position(|r| r.name == "level/telegraphed").map(RelationId).unwrap();
    p.handlers.push(Handler::new(Atom::new(moves, [Expr::var("hero"), Expr::var("door")])).clause(HandlerClause::new().when(moving.into_iter().chain([
        Atom::new(armed, [Expr::var("trap"), Expr::var("there")]).positive(),
        Atom::new(telegraphed, [Expr::var("trap")]).positive(),
    ])).action(Action::Include(Atom::new(warned, [Expr::var("trap"), Expr::var("hero")])))));
    p.handlers.push(Handler::new(Atom::new(picks, [Expr::var("hero"), Expr::var("key")])).clause(HandlerClause::new().when([
        Atom::new(is_in, [Expr::var("hero"), Expr::var("room")]).positive(),
        Atom::new(lies, [Expr::var("key"), Expr::var("room")]).positive(),
    ]).action(Action::Withdraw(Atom::new(lies, [Expr::var("key"), Expr::var("room")]))).action(Action::Include(Atom::new(holds, [Expr::var("hero"), Expr::var("key")])))));
    let events: Vec<_> = dungeon_walk::WALK.iter().map(|(verb, ty, name)| (if *verb == "moves through" { moves } else { picks }, vec![Value::entity("Hero", dungeon_walk::HERO), Value::entity(*ty, *name)])).collect();
    let mut run = Run::new(&p);
    let initial = run.state.clone();
    assert_eq!(run.history.as_of(0), Some(initial.clone()));
    for (index, (event, values)) in events.iter().enumerate() {
        let before = run.state.clone();
        let history = run.history.clone();
        let transition = step(&p, &before, *event, values, Budget::default());
        let record = run.run(&p, *event, values, Budget::default()).unwrap().clone();
        assert_eq!(record.revision, dungeon_walk::REVISIONS[index]);
        if index == dungeon_walk::LOCKED {
            assert_eq!(record.result.unwrap_err().kind.as_str(), "no match");
            assert_eq!(run.history, history);
            assert_eq!(run.state, before);
        } else {
            let after = &run.state;
            let expected = Change {
                added: after.facts.difference(&before.facts).cloned().collect(),
                withdrawn: before.facts.difference(&after.facts).cloned().collect(),
                created: after.live.difference(&before.live).cloned().collect(),
                deleted: before.live.difference(&after.live).cloned().collect(),
                ..Change::default()
            };
            assert_eq!(transition.unwrap().change, expected);
            assert_eq!(run.history.changes.last(), Some(&expected));
        }
        assert_eq!(run.history.as_of(record.revision).unwrap().hash(&p), record.hash);
    }
    assert_eq!(run.history.as_of(5), Some(run.state.clone()));
    assert_eq!(run.history.as_of(6), None);
    assert_eq!(run.history.facts[&(is_in, vec![Value::entity("Hero", "Ash"), Value::entity("Room", "Hall")])], vec![Span { from: 1, until: Some(2) }]);
    assert_eq!(run.history.facts[&(lies, vec![Value::entity("Key", "Iron-Key"), Value::entity("Room", "Crypt")])], vec![Span { from: 0, until: Some(5) }]);
    assert_eq!(Run::replay(&p, &run.log, Budget::default()).unwrap(), run);
    let mut corrupted = run.log.clone();
    corrupted[2].hash = "0000000000000000".into();
    assert!(matches!(Run::replay(&p, &corrupted, Budget::default()), Err(EvalError::Type { at, .. }) if at == "event log occurrence 3"));
    let mut atomic = Run::new(&p);
    let initial_history = atomic.history.clone();
    assert!(atomic.run_all(&p, &events[..2], Budget::default()).unwrap().result.is_err());
    assert_eq!(atomic.history, initial_history);
    assert!(atomic.run_all(&p, &[events[0].clone(), events[2].clone()], Budget::default()).unwrap().result.is_ok());
    assert_eq!(atomic.revision, 1);
    assert_eq!(atomic.history.as_of(0), Some(initial.clone()));
    assert_eq!(atomic.history.as_of(1), Some(atomic.state.clone()));
    assert!(!atomic.history.facts.contains_key(&(is_in, vec![Value::entity("Hero", "Ash"), Value::entity("Room", "Hall")])));
    assert_eq!(atomic.history.changes[0].added, atomic.state.facts.difference(&initial.facts).cloned().collect());
    assert_eq!(atomic.history.changes[0].withdrawn, initial.facts.difference(&atomic.state.facts).cloned().collect());
}

#[test]
fn d52_created_token_and_its_field_keep_their_lifetime_and_allocator() {
    let mut p = Program::builder();
    p.state_entity_type("Token", [Slot::new("label", Type::Text)]);
    let label = p.relation(Relation::new("Token.label", RelationKind::State, [Slot::new("token", Type::entity("Token")), Slot::new("label", Type::Text)]).mode(Mode::ExactlyOne { key: 0, values: vec![1] }));
    let create = p.relation(Relation::new("create", RelationKind::Event, []));
    let delete = p.relation(Relation::new("delete", RelationKind::Event, [Slot::new("token", Type::entity("Token"))]));
    p.event(Event::new(create));
    p.event(Event::new(delete));
    p.handler(Handler::new(Atom::new(create, [])).clause(HandlerClause::new().action(Action::Create {
        variable: Variable::new("token"), entity_type: "Token".into(), fields: BTreeMap::from([("label".into(), Expr::value("first"))]),
    })));
    p.handler(Handler::new(Atom::new(delete, [Expr::var("token")])).clause(HandlerClause::new().action(Action::Delete(Expr::var("token")))));
    let p = p.build();
    let mut run = Run::new(&p);
    let initial = run.state.clone();
    assert!(run.run(&p, create, &[], Budget::default()).unwrap().result.is_ok());
    let token = run.state.live.first().unwrap().clone();
    assert_eq!(token.name, "Token#0");
    let field = (label, vec![Value::Entity(token.clone()), Value::Text("first".into())]);
    let at_one = run.history.as_of(1).unwrap();
    assert!(at_one.live.contains(&token));
    assert!(at_one.facts.contains(&field));
    assert_eq!(at_one.next, 1);
    assert_eq!(run.history.changes[0].created, at_one.live.difference(&initial.live).cloned().collect());
    assert_eq!(run.history.changes[0].added, at_one.facts.difference(&initial.facts).cloned().collect());
    assert!(run.run(&p, delete, &[Value::Entity(token.clone())], Budget::default()).unwrap().result.is_ok());
    let at_two = run.history.as_of(2).unwrap();
    assert!(!at_two.live.contains(&token));
    assert!(!at_two.facts.contains(&field));
    assert_eq!(at_two.next, 1);
    assert_eq!(run.history.live[&token], vec![Span { from: 1, until: Some(2) }]);
    assert_eq!(run.history.facts[&field], vec![Span { from: 1, until: Some(2) }]);
    assert_eq!(run.history.changes[1].deleted, at_one.live.difference(&at_two.live).cloned().collect());
    assert_eq!(run.history.changes[1].withdrawn, at_one.facts.difference(&at_two.facts).cloned().collect());
    assert_eq!(Run::replay(&p, &run.log, Budget::default()).unwrap(), run);
    let sources = [1, 2].map(|line| SourceSpan { file: "token.events".into(), start: 0, end: 0, line, column: 1 });
    assert_eq!(why_not::why_not_history(&p, &run, &sources, 1, label, &field.1, None, Budget::default()).unwrap(), Verdict::Holds(()));
    let Verdict::Fails(text) = why_not::why_not_history(&p, &run, &sources, 2, label, &field.1, None, Budget::default()).unwrap() else { panic!("the field stopped holding") };
    assert!(text.contains("held from revision 1 until revision 2: deleted with Token#0"), "{text}");
    assert!(text.contains("(token.events:2)"), "{text}");
}

#[test]
fn d52_withdraw_and_include_keeps_its_span_and_admitted_revision() {
    let mut p = Program::builder();
    p.entity_type("Key");
    p.entity_type("Room");
    let lies = p.relation(Relation::new("lies-in", RelationKind::State, [Slot::new("key", Type::entity("Key")), Slot::new("room", Type::entity("Room"))]));
    let touch = p.relation(Relation::new("touch", RelationKind::Event, [Slot::new("key", Type::entity("Key"))]));
    p.initially(Fact::new(lies, [Value::entity("Key", "Bronze-Key"), Value::entity("Room", "Library")]));
    p.event(Event::new(touch));
    let fact = || Atom::new(lies, [Expr::var("key"), Expr::var("room")]);
    p.handler(Handler::new(Atom::new(touch, [Expr::var("key")])).clause(HandlerClause::new().when([fact().positive()]).action(Action::Withdraw(fact())).action(Action::Include(fact()))));
    let p = p.build();
    let mut run = Run::new(&p);
    let before = run.history.clone();
    assert!(run.run(&p, touch, &[Value::entity("Key", "Bronze-Key")], Budget::default()).unwrap().result.is_ok());
    assert_eq!(run.revision, 1);
    assert_eq!(run.history.facts, before.facts);
    assert_eq!(run.history.live, before.live);
    assert_eq!(run.history.changes, vec![Change::default()]);
    assert_eq!(run.history.as_of(0), run.history.as_of(1));
}
