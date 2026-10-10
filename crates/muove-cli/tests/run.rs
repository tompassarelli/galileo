use muove_core::state::{Run, State};
use muove_core::*;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

const DUNGEON: &str = include_str!("../../muove-syntax/tests/dungeon.muove");
const WALK: &str = "Ash moves through Front-Door\nAsh moves through Armory-Gate\nAsh moves through Library-Arch\nAsh picks up Bronze-Key\nAsh moves through Crypt-Stair\nAsh picks up Iron-Key\n";

#[test]
fn the_public_dungeon_log_replays_every_revision_and_continues_after_a_locked_door() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("run-dungeon");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("dungeon.muove"), DUNGEON).unwrap();
    std::fs::write(dir.join("walk.events"), WALK).unwrap();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
    let first = Command::new(env!("CARGO_BIN_EXE_muove")).args(["run", "dungeon.muove", "--events", "walk.events"]).current_dir(&dir).output().unwrap();
    assert_eq!(first.status.code(), Some(1), "{}", String::from_utf8_lossy(&first.stderr));
    assert!(first.stderr.is_empty());
    let repeated = Command::new(env!("CARGO_BIN_EXE_muove")).args(["run", "dungeon.muove", "--events", "walk.events"]).current_dir(&dir).output().unwrap();
    assert_eq!(repeated.status.code(), Some(1));
    assert_eq!(first.stdout, repeated.stdout);
    let output = String::from_utf8(first.stdout).unwrap();
    let lines: Vec<_> = output.lines().collect();
    assert_eq!(lines.len(), 7);
    let program = muove_syntax::lower::lower_program(&[("dungeon", DUNGEON)]).unwrap().program;
    let mut run = Run::new(&program);
    assert_eq!(lines[0], format!("{{\"revision\":0,\"hash\":\"{}\"}}", run.state.hash(&program)));
    let moves = program.relations.iter().position(|r| r.template == "{hero} moves through {door}").map(RelationId).unwrap();
    let picks = program.relations.iter().position(|r| r.template == "{hero} picks up {key}").map(RelationId).unwrap();
    let events = [
        (moves, vec![Value::entity("Hero", "Ash"), Value::entity("Door", "Front-Door")]),
        (moves, vec![Value::entity("Hero", "Ash"), Value::entity("Door", "Armory-Gate")]),
        (moves, vec![Value::entity("Hero", "Ash"), Value::entity("Door", "Library-Arch")]),
        (picks, vec![Value::entity("Hero", "Ash"), Value::entity("Key", "Bronze-Key")]),
        (moves, vec![Value::entity("Hero", "Ash"), Value::entity("Door", "Crypt-Stair")]),
        (picks, vec![Value::entity("Hero", "Ash"), Value::entity("Key", "Iron-Key")]),
    ];
    let mut previous = (0, run.state.hash(&program));
    for (index, (event, arguments)) in events.iter().enumerate() {
        let record = run.run(&program, *event, arguments, Budget::default()).unwrap();
        assert!(lines[index + 1].starts_with(&format!("{{\"line\":{},\"event\":\"{}\",\"admitted\":", index + 1, WALK.lines().nth(index).unwrap())));
        assert!(lines[index + 1].contains(&format!("\"revision\":{},\"hash\":\"{}\"", record.revision, record.hash)));
        assert_eq!(record.revision, [1, 1, 2, 3, 4, 5][index]);
        if index == 1 {
            assert_eq!(record.result.as_ref().unwrap_err().kind.as_str(), "no match");
            assert!(lines[index + 1].contains("\"admitted\":false,\"rejection\":\"no match\",\"explanation\":["));
            assert_eq!((record.revision, record.hash.clone()), previous);
        } else {
            assert!(record.result.as_ref().unwrap().is_empty());
            assert!(lines[index + 1].contains("\"admitted\":true"));
            assert!(lines[index + 1].ends_with("\"effects\":[]}"));
        }
        previous = (record.revision, record.hash.clone());
    }
    let replayed = Run::replay(&program, &run.log, Budget::default()).unwrap();
    assert_eq!(replayed, run);
    let mut corrupted = run.log.clone();
    corrupted[2].hash = "0000000000000000".into();
    assert!(matches!(Run::replay(&program, &corrupted, Budget::default()), Err(EvalError::Type { at, .. }) if at == "event log occurrence 3"));
    let mut atomic = Run::new(&program);
    let before = atomic.state.clone();
    let record = atomic.run_all(&program, &events[..2], Budget::default()).unwrap();
    assert_eq!(record.result.as_ref().unwrap_err().kind.as_str(), "no match");
    assert_eq!(record.revision, 0);
    assert_eq!(record.hash, before.hash(&program));
    assert_eq!(atomic.state, before);

    std::fs::write(dir.join("walk.events"), "Ash moves through Front-Door\nAhs moves through Library-Arch\n").unwrap();
    let invalid = Command::new(env!("CARGO_BIN_EXE_muove")).args(["run", "dungeon.muove", "--events", "walk.events"]).current_dir(&dir).output().unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    let diagnostic = String::from_utf8(invalid.stderr).unwrap();
    assert!(diagnostic.contains("walk.events:2"), "{diagnostic}");
    assert!(diagnostic.contains("the nearest is `Ash`"), "{diagnostic}");
    std::fs::write(dir.join("walk.events"), "# walk\n\nAsh moves through Front-Door\n").unwrap();
    let admitted = Command::new(env!("CARGO_BIN_EXE_muove")).args(["run", "--events", "walk.events"]).current_dir(&dir).output().unwrap();
    assert_eq!(admitted.status.code(), Some(0), "{}", String::from_utf8_lossy(&admitted.stderr));
    let admitted = String::from_utf8(admitted.stdout).unwrap();
    assert_eq!(admitted.lines().count(), 2);
    assert!(admitted.lines().nth(1).unwrap().starts_with("{\"line\":3,"));
    let help = Command::new(env!("CARGO_BIN_EXE_muove")).arg("help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout).unwrap().contains("muove run [PATH...] --events FILE"));
}

#[test]
fn a_create_then_delete_keeps_the_allocator_in_the_replayed_hash() {
    let mut builder = Program::builder();
    builder.state_entity_type("Token", []);
    let create = builder.relation(Relation::new("create", RelationKind::Event, []));
    let delete = builder.relation(Relation::new("delete", RelationKind::Event, [Slot::new("token", Type::entity("Token"))]));
    builder.event(Event::new(create));
    builder.event(Event::new(delete));
    builder.handler(Handler::new(Atom::new(create, [])).clause(HandlerClause::new().action(Action::Create {
        variable: Variable::new("token"), entity_type: "Token".into(), fields: BTreeMap::new(),
    })));
    builder.handler(Handler::new(Atom::new(delete, [Expr::var("token")])).clause(HandlerClause::new().action(Action::Delete(Expr::var("token")))));
    let program = builder.build();
    let empty = State::initial(&program);
    let mut run = Run::new(&program);
    assert!(run.run(&program, create, &[], Budget::default()).unwrap().result.is_ok());
    let token = run.state.live.first().unwrap().clone();
    assert!(run.run(&program, delete, &[Value::Entity(token)], Budget::default()).unwrap().result.is_ok());
    assert_eq!(run.revision, 2);
    assert_eq!(run.state.facts, empty.facts);
    assert_eq!(run.state.live, empty.live);
    assert_eq!(run.state.next, 1);
    assert_ne!(run.state.hash(&program), empty.hash(&program));
    assert_eq!(Run::replay(&program, &run.log, Budget::default()).unwrap(), run);
}

#[test]
fn d51_query_and_check_read_the_last_admitted_state_and_preserve_temporal_claims() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("query-events-dungeon");
    std::fs::create_dir_all(&dir).unwrap();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
    std::fs::write(dir.join("dungeon.muove"), DUNGEON).unwrap();
    std::fs::write(dir.join("walk.events"), WALK).unwrap();
    let invoke = |arguments: &[&str]| Command::new(env!("CARGO_BIN_EXE_muove")).args(arguments).current_dir(&dir).output().unwrap();
    let initial = invoke(&["query", "dungeon.muove", "all ?r where Ash is in ?r", "--json"]);
    assert_eq!(initial.status.code(), Some(0), "{}", String::from_utf8_lossy(&initial.stderr));
    assert_eq!(initial.stdout, b"[\"Entrance\"]\n");
    let final_room = invoke(&["query", "dungeon.muove", "all ?r where Ash is in ?r", "--events", "walk.events", "--json"]);
    assert_eq!(final_room.status.code(), Some(0), "{}", String::from_utf8_lossy(&final_room.stderr));
    assert_eq!(final_room.stdout, b"[\"Crypt\"]\n");
    let flag_after_json = invoke(&["query", "dungeon.muove", "all ?r where Ash is in ?r", "--json", "--events", "walk.events"]);
    assert_eq!(flag_after_json.status.code(), Some(0));
    assert_eq!(flag_after_json.stdout, final_room.stdout);
    let original_temporal = invoke(&["check", "dungeon.muove"]);
    let final_temporal = invoke(&["check", "dungeon.muove", "--events", "walk.events"]);
    assert_eq!(original_temporal.status.code(), Some(2));
    assert_eq!(final_temporal.status.code(), original_temporal.status.code());
    assert_eq!(final_temporal.stdout, original_temporal.stdout);
    assert_eq!(final_temporal.stderr, original_temporal.stderr);
    let state_claim = DUNGEON.split("requirements for the level").next().unwrap().to_owned() + "requirements for the level\n  Ash is in Hall\n";
    std::fs::write(dir.join("dungeon.muove"), state_claim).unwrap();
    std::fs::write(dir.join("walk.events"), "Ash moves through Front-Door\nAsh moves through Armory-Gate\n").unwrap();
    let before = invoke(&["check", "dungeon.muove"]);
    assert_eq!(before.status.code(), Some(1), "{}", String::from_utf8_lossy(&before.stderr));
    let after = invoke(&["check", "dungeon.muove", "--events", "walk.events"]);
    assert_eq!(after.status.code(), Some(0), "{}", String::from_utf8_lossy(&after.stderr));
    assert!(String::from_utf8(after.stdout).unwrap().contains("Ash is in Hall holds."));
    std::fs::write(dir.join("walk.events"), "Ash moves through Front-Door\nAhs moves through Library-Arch\n").unwrap();
    for arguments in [
        vec!["query", "dungeon.muove", "all ?r where Ash is in ?r", "--events", "walk.events", "--json"],
        vec!["check", "dungeon.muove", "--events", "walk.events"],
    ] {
        let unreadable = invoke(&arguments);
        assert_eq!(unreadable.status.code(), Some(2));
        assert!(unreadable.stdout.is_empty());
        let diagnostic = String::from_utf8(unreadable.stderr).unwrap();
        assert!(diagnostic.contains("walk.events:2"), "{diagnostic}");
        assert!(diagnostic.contains("the nearest is `Ash`"), "{diagnostic}");
    }
}
