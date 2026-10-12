#[path = "../../galileo-syntax/tests/fixtures/dungeon_walk.rs"]
mod dungeon_walk;

use galileo_core::{RelationId, Value};
use galileo_runtime::{Error, Fields, Projection, RejectionKind, Session, Verdict};
use std::collections::BTreeMap;
use std::sync::Arc;

#[test]
fn menu_scale_kept_projection_agrees_with_reference_after_every_occurrence() {
    #[derive(Debug, PartialEq, Eq)]
    struct MenuProjection {
        kind: String,
        title: String,
        query: String,
        help: String,
        empty: String,
        selection: i64,
        rows: Arc<[Value]>,
    }

    impl Projection for MenuProjection {
        type Arguments = galileo_core::Entity;

        fn view(program: &galileo_runtime::Program) -> &galileo_runtime::View {
            program
                .views
                .iter()
                .find(|view| view.name == "north-menu/Menu")
                .unwrap()
        }

        fn arguments(north: Self::Arguments) -> Vec<Value> {
            vec![Value::Entity(north)]
        }

        fn decode(fields: &mut Fields<'_>) -> Result<Self, galileo_core::EvalError> {
            let text = |value: &Value| match value {
                Value::Text(text) => Ok(text.clone()),
                _ => Err(galileo_core::EvalError::Type {
                    at: "menu text".into(),
                    message: "expected Text".into(),
                }),
            };
            Ok(Self {
                kind: fields.field("kind", text)?,
                title: fields.field("title", text)?,
                query: fields.field("query", text)?,
                help: fields.field("help", text)?,
                empty: fields.field("empty", text)?,
                selection: fields.field("selection", |value| match value {
                    Value::Int(selection) => Ok(*selection),
                    _ => Err(galileo_core::EvalError::Type {
                        at: "selection".into(),
                        message: "expected Int".into(),
                    }),
                })?,
                rows: fields.field("rows", |value| match value {
                    Value::List(rows) => Ok(Arc::from(rows.clone())),
                    _ => Err(galileo_core::EvalError::Type {
                        at: "rows".into(),
                        message: "expected list".into(),
                    }),
                })?,
            })
        }
    }

    let lowered = galileo_syntax::lower::lower_program(&[(
        "north-menu",
        include_str!("../../../examples/north-menu.gal"),
    )])
    .unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let program = galileo_runtime::Program::from(lowered.program);
    let north = program
        .entities
        .iter()
        .find(|entity| entity.name == "north-main")
        .unwrap()
        .clone();
    let view = program
        .views
        .iter()
        .find(|view| view.name == "north-menu/Menu")
        .unwrap();
    let arguments = [Value::Entity(north.clone())];
    let event = |name: &str, values: Vec<Value>| {
        let mut arguments = vec![Value::Entity(north.clone())];
        arguments.extend(values);
        (
            program.relation_id(&format!("north-menu/{name}")).unwrap(),
            arguments,
        )
    };
    let mut events = vec![
        event("clear-menu-rows", vec![]),
        event(
            "open-settings-menu",
            vec![Value::text("config"), Value::text("Context Switchboard")],
        ),
    ];
    let rows: Vec<usize> = if std::env::var_os("GALILEO_FULL").is_some() { (0..30).collect() } else { vec![0, 1, 2, 20] };
    events.extend(rows.iter().map(|i| {
        event(
            "offer-row",
            vec![
                Value::text(format!("row-{i}")),
                Value::text(format!("Row {i}")),
                Value::text("Settings entry"),
                Value::text("on"),
            ],
        )
    }));
    events.extend([
        event("query-menu", vec![Value::text("")]),
        event("move-menu", vec![Value::Int(1)]),
        event("query-menu", vec![Value::text("Row 2")]),
        event("move-menu", vec![Value::Int(-1)]),
        event("query-menu", vec![Value::text("missing")]),
        event("move-menu", vec![Value::Int(1)]),
        event("query-menu", vec![Value::text("")]),
        event("close-menu", vec![]),
    ]);
    let mut session = Session::new(&program);
    let mut reference = galileo_core::state::State::initial(&program);
    let model =
        galileo_core::eval::evaluate(&program, &program.initially, Default::default()).unwrap();
    assert_eq!(
        session.project(view, &arguments).unwrap(),
        galileo_core::eval::project(&program, &model, view, &arguments, Default::default())
            .unwrap(),
        "menu_scale initial projection"
    );
    let initial =
        galileo_core::eval::project(&program, &model, view, &arguments, Default::default())
            .unwrap();
    assert_eq!(
        session.view::<MenuProjection>(north.clone()).unwrap(),
        MenuProjection::decode(&mut Fields::new(&initial)).unwrap()
    );
    let rejected = vec![
        event(
            "offer-row",
            vec![
                Value::text("row-rejected"),
                Value::text("Row rejected"),
                Value::text("Settings entry"),
                Value::text("on"),
            ],
        ),
        event("query-menu", vec![Value::text("missing")]),
        event("move-menu", vec![Value::Int(1)]),
    ];
    let closing = events.len() - 1;
    let mut last = initial.clone();
    for (index, (relation, values)) in events.into_iter().enumerate() {
        if index == closing {
            assert!(
                matches!(session.run_all(&rejected), Err(Error::Rejected(rejection)) if rejection.kind == RejectionKind::NoMatch)
            );
            assert_eq!(session.state(), &reference, "menu_scale rejected run_all state");
            assert_eq!(session.project(view, &arguments).unwrap(), last, "menu_scale view after a rejected run_all");
        }
        let transition =
            galileo_core::state::step(&program, &reference, relation, &values, Default::default());
        let result = session.run((relation, values));
        match transition {
            Ok(transition) => {
                result.unwrap();
                reference = transition.state;
            }
            Err(galileo_core::EvalError::Type { at, message }) if at == "step" => {
                assert!(
                    message.contains("has no matching clause"),
                    "menu_scale reference rejection: {message}"
                );
                assert!(
                    matches!(result, Err(Error::Rejected(rejection)) if rejection.kind == RejectionKind::NoMatch)
                );
                assert_eq!(session.state(), &reference, "menu_scale rejected state");
            }
            Err(error) => panic!("menu_scale reference occurrence {}: {error:?}", index + 1),
        }
        let mut current = (*program).clone();
        current.entities.retain(|entity| {
            !program
                .entity_types
                .iter()
                .any(|ty| ty.state && ty.name == entity.entity_type)
        });
        current.entities.extend(reference.live.iter().cloned());
        let facts = reference
            .facts
            .iter()
            .map(|(relation, values)| galileo_core::Fact::new(*relation, values.clone()))
            .collect::<Vec<_>>();
        let model = galileo_core::eval::evaluate(&current, &facts, Default::default()).unwrap();
        let fresh =
            galileo_core::eval::project(&current, &model, view, &arguments, Default::default())
                .unwrap();
        assert_eq!(
            session.project(view, &arguments).unwrap(),
            fresh,
            "menu_scale occurrence {}",
            index + 1
        );
        let typed = session.view::<MenuProjection>(north.clone()).unwrap();
        last = fresh.clone();
        let decoded = MenuProjection::decode(&mut Fields::new(&fresh)).unwrap();
        assert_eq!(typed, decoded, "menu_scale typed occurrence {}", index + 1);
        assert_eq!(
            session
                .as_of(session.revision())
                .unwrap()
                .view::<MenuProjection>(north.clone())
                .unwrap(),
            decoded,
            "menu_scale snapshot occurrence {}",
            index + 1
        );
    }
    println!("menu_scale projection agreement: rows={} disagreements=0 typed_disagreements=0", rows.len());
}

#[test]
fn public_dungeon_checkpoint_continues_with_history_pending_entities_and_allocator() {
    let source = format!(
        "{}\n{}",
        include_str!("../../galileo-syntax/tests/dungeon.gal"),
        r#"
state entity Footprint
  room: Room

effect arrival: report Hero arriving in Room
  settled by acknowledged
event acknowledged: Hero acknowledges arrival in Room
state acknowledgement: Hero acknowledged Room

on ?hero moves through ?door
  when ?hero is in ?here
  and ?door leads from ?here to ?there
  and ?door opens for ?hero
  then ?footprint is a new Footprint {room: ?there}
  and report ?hero arriving in ?there

on ?hero acknowledges arrival in ?room
  then ?hero acknowledged ?room

view Journey for {hero: Hero}
  room: the ?room where ?hero is in ?room
  footprints: all ?footprint where ?footprint is a Footprint
"#
    );
    let lowered = galileo_syntax::lower::lower(&source, "dungeon.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let program = galileo_runtime::Program::from(lowered.program);
    let relation = |template: &str| {
        program
            .relations
            .iter()
            .position(|r| r.template == template)
            .map(RelationId)
            .unwrap()
    };
    let arrival = relation("report {hero} arriving in {room}");
    let acknowledged = relation("{hero} acknowledges arrival in {room}");
    let location = relation("{hero} is in {room}");
    let ash = Value::entity("Hero", "Ash");
    let events: Vec<_> = dungeon_walk::WALK
        .iter()
        .map(|(verb, ty, name)| (relation(dungeon_walk::template(verb)), vec![ash.clone(), Value::entity(*ty, *name)]))
        .chain([(acknowledged, vec![ash.clone(), Value::entity("Room", "Hall")])])
        .collect();
    let journey = program
        .views
        .iter()
        .find(|view| view.name == "Journey")
        .unwrap();
    let mut straight = Session::new(&program);
    let mut before_checkpoint = Session::new(&program);
    let initial = before_checkpoint.state().clone();
    assert!(
        matches!(before_checkpoint.run_all(&events[..2]), Err(Error::Rejected(rejection)) if rejection.kind == RejectionKind::NoMatch)
    );
    assert_eq!(before_checkpoint.state(), &initial);
    assert_eq!(before_checkpoint.revision(), 0);
    assert!(before_checkpoint.pending().is_empty());
    assert_eq!(before_checkpoint.as_of(0).unwrap().state(), &initial);
    let mut at_two = None;
    let mut view_at_two = None;
    for (index, event) in events.iter().enumerate() {
        let result = straight.run(event.clone());
        if index < 4 {
            assert_eq!(before_checkpoint.run(event.clone()), result);
        }
        if index == 2 {
            at_two = Some(before_checkpoint.state().clone());
            view_at_two = Some(before_checkpoint.project(journey, &[ash.clone()]).unwrap());
        }
    }
    assert_eq!(before_checkpoint.revision(), 3);
    assert_eq!(before_checkpoint.state().live.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Footprint#0", "Footprint#1"]);
    assert_eq!(before_checkpoint.state().next, 2);
    let pending = vec![
        (arrival, vec![ash.clone(), Value::entity("Room", "Hall")]),
        (arrival, vec![ash.clone(), Value::entity("Room", "Library")]),
    ];
    assert_eq!(before_checkpoint.pending(), pending);
    let checkpoint = before_checkpoint.checkpoint();
    let mut wrong_program = (*program).clone();
    wrong_program.handlers.clear();
    let wrong_program = galileo_runtime::Program::from(wrong_program);
    assert!(matches!(
        Session::restore(&wrong_program, checkpoint.clone()),
        Err(Error::WrongProgram)
    ));
    assert!(matches!(
        Session::restore(
            &program,
            galileo_core::state::Run::new(&program).checkpoint()
        ),
        Err(Error::WrongProgram)
    ));
    let saved = before_checkpoint.save().concat();
    assert!(matches!(Session::open(&wrong_program, &saved), Err(Error::WrongProgram)));
    for end in 0..saved.len() {
        assert!(Session::open(&program, &saved[..end]).is_err(), "prefix of {end} bytes opened");
    }
    let mut opened = Session::open(&program, &saved).unwrap();
    assert_eq!(opened.revision(), 3);
    assert_eq!(opened.state(), before_checkpoint.state());
    assert_eq!(opened.pending(), pending);
    assert_eq!(opened.state().live.len(), 2);
    assert_eq!(opened.state().next, 2);
    assert_eq!(opened.save().concat(), saved);
    assert_eq!(opened.as_of(3).unwrap().state(), before_checkpoint.state());
    assert!(
        matches!(opened.as_of(2), Err(galileo_runtime::EvalError::Type { message, .. }) if message.contains("first revision 3"))
    );
    for event in &events[4..] {
        opened.run(event.clone()).unwrap();
    }
    assert_eq!(opened.revision(), 6);
    assert_eq!(opened.state().hash(&program), straight.state().hash(&program));
    let mut large = before_checkpoint.state().clone();
    large.facts.insert((location, vec![Value::text("x".repeat(5000)), Value::entity("Room", "Hall")]));
    let large = Session::converted(&program, large);
    let parts = large.save();
    assert_eq!(parts.iter().filter(|part| part.as_slice() == "x".repeat(5000).as_bytes()).count(), 1);
    assert_eq!(parts, large.save());
    assert_eq!(Session::open(&program, &parts.concat()).unwrap().state(), large.state());
    let mut restored = Session::restore(&program, checkpoint).unwrap();
    assert_eq!(restored.state(), before_checkpoint.state());
    assert_eq!(restored.pending(), pending);
    assert_eq!(restored.as_of(2).unwrap().state(), at_two.as_ref().unwrap());
    assert_eq!(
        restored
            .as_of(2)
            .unwrap()
            .project(journey, &[ash.clone()])
            .unwrap(),
        view_at_two.unwrap()
    );
    let in_library = (
        location,
        vec![ash.clone(), Value::entity("Room", "Library")],
    );
    let Verdict::Holds(history) = restored.as_of(2).unwrap().why(&in_library).unwrap() else {
        panic!("Library must hold at revision 2")
    };
    assert!(history.contains("since revision 2"));
    assert!(history.contains("Ash moves through Library-Arch"));
    for event in &events[4..] {
        restored.run(event.clone()).unwrap();
    }
    assert_eq!(
        restored.state().hash(&program),
        straight.state().hash(&program)
    );
    assert_eq!(restored.state(), straight.state());
    assert_eq!(restored.revision(), 6);
    assert_eq!(restored.state().live.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Footprint#0", "Footprint#1", "Footprint#2"]);
    assert_eq!(restored.state().next, 3);
    assert_eq!(
        restored.pending(),
        &[
            (arrival, vec![ash.clone(), Value::entity("Room", "Library")]),
            (arrival, vec![ash.clone(), Value::entity("Room", "Crypt")]),
        ]
    );
    let final_view = restored.project(journey, &[ash.clone()]).unwrap();
    let Value::Record(fields) = &final_view else {
        panic!("Journey must be a record")
    };
    assert_eq!(fields["room"], Value::entity("Room", "Crypt"));
    let Value::List(footprints) = &fields["footprints"] else { panic!("footprints must be a list") };
    let footprints: Vec<_> = footprints.iter().map(|v| match v { Value::Entity(e) => e.name.as_str(), other => panic!("{other:?}") }).collect();
    assert_eq!(footprints, ["Footprint#0", "Footprint#1", "Footprint#2"]);
    assert_eq!(
        final_view,
        straight.project(journey, &[ash.clone()]).unwrap()
    );
    assert!(
        matches!(restored.as_of(7), Err(galileo_runtime::EvalError::Type { message, .. }) if message.contains("last revision 6"))
    );
    assert_eq!(restored.as_of(2).unwrap().state(), at_two.as_ref().unwrap());
    for revision in 0..=6 {
        assert_eq!(
            restored.as_of(revision).unwrap().state(),
            straight.as_of(revision).unwrap().state()
        );
    }
    assert_eq!(
        restored
            .as_of(0)
            .unwrap()
            .project(journey, &[ash.clone()])
            .unwrap(),
        Value::Record(BTreeMap::from([
            ("room".into(), Value::entity("Room", "Entrance")),
            ("footprints".into(), Value::List(vec![])),
        ]))
    );
    let in_crypt = (location, vec![ash, Value::entity("Room", "Crypt")]);
    assert!(
        matches!(restored.why(&in_crypt).unwrap(), Verdict::Holds(history) if history.contains("since revision 4"))
    );
    println!("dungeon: revisions=6 history_states=7 live=3 allocator=3 pending=2 final_hash={} checkpoint_revision=3 historical_revision=2 wrong_program=rejected atomic_rejection=unchanged", restored.state().hash(&program));
}

#[test]
fn save_reopens_facts_of_two_relations_sharing_a_name_under_their_own_relation() {
    let mut p = galileo_core::Program::builder();
    let slot = || [galileo_core::Slot::new("n", galileo_core::Type::Int)];
    let first = p.relation(galileo_core::Relation::new("shared", galileo_core::RelationKind::State, slot()));
    let second = p.relation(galileo_core::Relation::new("shared", galileo_core::RelationKind::State, slot()));
    let seed = p.relation(galileo_core::Relation::new("seed", galileo_core::RelationKind::Event, slot()));
    let consume = p.relation(galileo_core::Relation::new("consume", galileo_core::RelationKind::Event, slot()));
    p.event(galileo_core::Event::new(seed));
    p.event(galileo_core::Event::new(consume));
    let n = || [galileo_core::Expr::var("n")];
    p.handler(
        galileo_core::Handler::new(galileo_core::Atom::new(seed, n()))
            .clause(galileo_core::HandlerClause::new().action(galileo_core::Action::Include(galileo_core::Atom::new(second, n())))),
    );
    p.handler(
        galileo_core::Handler::new(galileo_core::Atom::new(consume, n())).clause(
            galileo_core::HandlerClause::new()
                .when([galileo_core::Literal::Positive(galileo_core::Atom::new(second, n()))])
                .action(galileo_core::Action::Withdraw(galileo_core::Atom::new(second, n()))),
        ),
    );
    let program = galileo_runtime::Program::from(p.build());
    let mut original = Session::new(&program);
    original.run((seed, vec![Value::Int(42)])).unwrap();
    assert!(original.state().facts.contains(&(second, vec![Value::Int(42)])));
    assert!(!original.state().facts.contains(&(first, vec![Value::Int(42)])));
    let mut reopened = Session::open(&program, &original.save().concat()).unwrap();
    assert_eq!(reopened.state(), original.state());
    assert_eq!(reopened.save(), original.save());
    original.run((consume, vec![Value::Int(42)])).unwrap();
    reopened.run((consume, vec![Value::Int(42)])).unwrap();
    assert_eq!(reopened.state(), original.state());
}
