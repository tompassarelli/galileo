use galileo_core::{RelationId, Value};
use galileo_runtime::{Error, RejectionKind, Run, Verdict};
use std::collections::BTreeMap;

#[test]
fn public_dungeon_checkpoint_continues_with_history_pending_entities_and_allocator() {
    let source = format!(
        "{}\n{}",
        include_str!("../../galileo-syntax/tests/dungeon.gal"),
        r#"
state entity Footprint
  room: Room

effect arrival: report {hero: Hero} arriving in {room: Room}
  settled by acknowledged
event acknowledged: {hero: Hero} acknowledges arrival in {room: Room}
state acknowledgement: {hero: Hero} acknowledged {room: Room}

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
    let program = lowered.program;
    let relation = |template: &str| {
        program
            .relations
            .iter()
            .position(|r| r.template == template)
            .map(RelationId)
            .unwrap()
    };
    let moves = relation("{hero} moves through {door}");
    let picks = relation("{hero} picks up {key}");
    let arrival = relation("report {hero} arriving in {room}");
    let acknowledged = relation("{hero} acknowledges arrival in {room}");
    let location = relation("{hero} is in {room}");
    let ash = Value::entity("Hero", "Ash");
    let events = [
        (
            moves,
            vec![ash.clone(), Value::entity("Door", "Front-Door")],
        ),
        (
            moves,
            vec![ash.clone(), Value::entity("Door", "Armory-Gate")],
        ),
        (
            moves,
            vec![ash.clone(), Value::entity("Door", "Library-Arch")],
        ),
        (picks, vec![ash.clone(), Value::entity("Key", "Bronze-Key")]),
        (
            moves,
            vec![ash.clone(), Value::entity("Door", "Crypt-Stair")],
        ),
        (picks, vec![ash.clone(), Value::entity("Key", "Iron-Key")]),
        (
            acknowledged,
            vec![ash.clone(), Value::entity("Room", "Hall")],
        ),
    ];
    let journey = program
        .views
        .iter()
        .find(|view| view.name == "Journey")
        .unwrap();
    let mut straight = Run::new(&program);
    let mut before_checkpoint = Run::new(&program);
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
        if index == 1 {
            assert!(
                matches!(&result, Err(Error::Rejected(rejection)) if rejection.kind == RejectionKind::NoMatch)
            );
        } else {
            let step = result.as_ref().unwrap();
            assert_eq!(step.revision, [1, 1, 2, 3, 4, 5, 6][index]);
            let expected = if event.0 == moves {
                vec![(
                    arrival,
                    vec![
                        ash.clone(),
                        Value::entity("Room", ["Hall", "", "Library", "", "Crypt", "", ""][index]),
                    ],
                )]
            } else {
                vec![]
            };
            assert_eq!(step.effects, expected);
        }
        if index < 4 {
            assert_eq!(before_checkpoint.run(event.clone()), result);
        }
        if index == 2 {
            at_two = Some(before_checkpoint.state().clone());
            view_at_two = Some(before_checkpoint.view(journey, &[ash.clone()]).unwrap());
        }
    }
    assert_eq!(before_checkpoint.revision(), 3);
    assert_eq!(before_checkpoint.state().live.len(), 2);
    assert_eq!(before_checkpoint.state().next, 2);
    let pending = vec![
        (arrival, vec![ash.clone(), Value::entity("Room", "Hall")]),
        (arrival, vec![ash.clone(), Value::entity("Room", "Library")]),
    ];
    assert_eq!(before_checkpoint.pending(), pending);
    let checkpoint = before_checkpoint.checkpoint();
    let mut wrong_program = program.clone();
    wrong_program.handlers.clear();
    assert!(matches!(
        Run::restore(&wrong_program, checkpoint.clone()),
        Err(Error::WrongProgram)
    ));
    assert!(matches!(
        Run::restore(
            &program,
            galileo_core::state::Run::new(&program).checkpoint()
        ),
        Err(Error::WrongProgram)
    ));
    let mut restored = Run::restore(&program, checkpoint).unwrap();
    assert_eq!(restored.state(), before_checkpoint.state());
    assert_eq!(restored.pending(), pending);
    assert_eq!(restored.as_of(2).unwrap().state(), at_two.as_ref().unwrap());
    assert_eq!(
        restored
            .as_of(2)
            .unwrap()
            .view(journey, &[ash.clone()])
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
    assert_eq!(restored.state().live.len(), 3);
    assert_eq!(restored.state().next, 3);
    assert_eq!(
        restored.pending(),
        &[
            (arrival, vec![ash.clone(), Value::entity("Room", "Library")]),
            (arrival, vec![ash.clone(), Value::entity("Room", "Crypt")]),
        ]
    );
    let final_view = restored.view(journey, &[ash.clone()]).unwrap();
    let Value::Record(fields) = &final_view else {
        panic!("Journey must be a record")
    };
    assert_eq!(fields["room"], Value::entity("Room", "Crypt"));
    assert!(matches!(&fields["footprints"], Value::List(values) if values.len() == 3));
    assert_eq!(final_view, straight.view(journey, &[ash.clone()]).unwrap());
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
            .view(journey, &[ash.clone()])
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
