use galileo_core::state::{Run, State};
use galileo_core::*;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

const DUNGEON: &str = include_str!("../../galileo-syntax/tests/dungeon.gal");
const WALK: &str = "Ash moves through Front-Door\nAsh moves through Armory-Gate\nAsh moves through Library-Arch\nAsh picks up Bronze-Key\nAsh moves through Crypt-Stair\nAsh picks up Iron-Key\n";

#[test]
fn the_public_dungeon_log_replays_every_revision_and_continues_after_a_locked_door() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("run-dungeon");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("dungeon.gal"), DUNGEON).unwrap();
    std::fs::write(dir.join("walk.events"), WALK).unwrap();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
    let first = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["run", "dungeon.gal", "--events", "walk.events"]).current_dir(&dir).output().unwrap();
    assert_eq!(first.status.code(), Some(1), "{}", String::from_utf8_lossy(&first.stderr));
    assert!(first.stderr.is_empty());
    let repeated = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["run", "dungeon.gal", "--events", "walk.events"]).current_dir(&dir).output().unwrap();
    assert_eq!(repeated.status.code(), Some(1));
    assert_eq!(first.stdout, repeated.stdout);
    let output = String::from_utf8(first.stdout).unwrap();
    let lines: Vec<_> = output.lines().collect();
    assert_eq!(lines.len(), 7);
    let program = galileo_syntax::lower::lower_program(&[("dungeon", DUNGEON)]).unwrap().program;
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

    let chain = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["why", "Ash", "holds", "Iron-Key", "dungeon.gal", "--events", "walk.events"]).current_dir(&dir).output().unwrap();
    assert_eq!(chain.status.code(), Some(0), "{}", String::from_utf8_lossy(&chain.stderr));
    assert!(chain.stderr.is_empty());
    let chain = String::from_utf8(chain.stdout).unwrap();
    assert_eq!(chain.lines().next(), Some("Ash holds Iron-Key, since revision 5"));
    assert_eq!(chain.lines().filter(|line| line.starts_with("revision ")).collect::<Vec<_>>(), [
        "revision 5: Ash picks up Iron-Key (walk.events:6), by the clause at dungeon.gal:96, when at revision 4:",
        "revision 4: Ash moves through Crypt-Stair (walk.events:5), by the clause at dungeon.gal:82, when at revision 3:",
        "revision 2: Ash moves through Library-Arch (walk.events:3), by the clause at dungeon.gal:82, when at revision 1:",
        "revision 1: Ash moves through Front-Door (walk.events:1), by the clause at dungeon.gal:82, when at revision 0:",
    ]);
    for leaf in [
        "Ash is in Crypt, since revision 4",
        "Ash is in Library, since revision 2",
        "Ash is in Hall, since revision 1",
        "Iron-Key lies in Crypt, since revision 0 (dungeon.gal:51)",
        "Ash is in Entrance, since revision 0 (dungeon.gal:49)",
    ] { assert!(chain.contains(leaf), "{chain}"); }
    assert!(!chain.contains("walk.events:2"));
    assert!(!chain.contains("Bronze-Key"));
    let absent = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["why", "Ash", "holds", "Iron-Key", "dungeon.gal", "--events", "walk.events", "--as-of", "4"]).current_dir(&dir).output().unwrap();
    assert_eq!(absent.status.code(), Some(1));
    assert!(String::from_utf8(absent.stdout).unwrap().contains("why not"));
    let cut = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["why", "Ash", "holds", "Iron-Key", "dungeon.gal", "--events", "walk.events", "--budget", "400"]).current_dir(&dir).output().unwrap();
    assert_eq!(cut.status.code(), Some(0), "{}", String::from_utf8_lossy(&cut.stderr));
    let cut = String::from_utf8(cut.stdout).unwrap();
    assert_eq!(cut.lines().next(), Some("Ash holds Iron-Key, since revision 5"));
    assert!(cut.ends_with("earlier causes not shown: out of budget\n"), "{cut}");

    for (words, annotation, occurrence, leaf) in [
        (vec!["Ash", "is", "in", "Hall"], "held from revision 1 until revision 2: superseded by Ash is in Library", Some("revision 2: Ash moves through Library-Arch (walk.events:3), by the clause at dungeon.gal:82, when at revision 1:"), Some("Ash is in Hall, since revision 1")),
        (vec!["Iron-Key", "lies", "in", "Crypt"], "held from revision 0 until revision 5: withdrawn", Some("revision 5: Ash picks up Iron-Key (walk.events:6), by the clause at dungeon.gal:96, when at revision 4:"), Some("Iron-Key lies in Crypt, since revision 0 (dungeon.gal:51)")),
        (vec!["Ash", "holds", "Silver-Key"], "never held in revisions 0 to 5", None, None),
        (vec!["Shrine-Door", "opens", "for", "Ash"], "Ash holds Silver-Key   (never held in revisions 0 to 5)", None, None),
    ] {
        let missing = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["why", "not"]).args(words).args(["dungeon.gal", "--events", "walk.events"]).current_dir(&dir).output().unwrap();
        assert_eq!(missing.status.code(), Some(0), "{}", String::from_utf8_lossy(&missing.stderr));
        let missing = String::from_utf8(missing.stdout).unwrap();
        assert!(missing.contains(annotation), "{missing}");
        if let Some(occurrence) = occurrence { assert_eq!(missing.matches(occurrence).count(), 1, "{missing}"); }
        if let Some(leaf) = leaf { assert!(missing.contains(leaf), "{missing}"); }
        assert!(!missing.contains("walk.events:2"), "{missing}");
    }
    let cut = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["why", "not", "Ash", "is", "in", "Hall", "dungeon.gal", "--events", "walk.events", "--budget", "400"]).current_dir(&dir).output().unwrap();
    assert_eq!(cut.status.code(), Some(0), "{}", String::from_utf8_lossy(&cut.stderr));
    let cut = String::from_utf8(cut.stdout).unwrap();
    assert!(cut.contains("held from revision 1 until revision 2: superseded by Ash is in Library"), "{cut}");
    assert!(cut.ends_with("earlier causes not shown: out of budget\n"), "{cut}");
    let held = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["why", "not", "Ash", "is", "in", "Hall", "dungeon.gal", "--events", "walk.events", "--as-of", "1"]).current_dir(&dir).output().unwrap();
    assert_eq!(held.status.code(), Some(1));
    assert!(String::from_utf8(held.stdout).unwrap().contains("Ash is in Hall holds"));

    std::fs::write(dir.join("walk.events"), "Ash moves through Front-Door\nAhs moves through Library-Arch\n").unwrap();
    let invalid = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["run", "dungeon.gal", "--events", "walk.events"]).current_dir(&dir).output().unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    let diagnostic = String::from_utf8(invalid.stderr).unwrap();
    assert!(diagnostic.contains("walk.events:2"), "{diagnostic}");
    assert!(diagnostic.contains("the nearest is `Ash`"), "{diagnostic}");
    std::fs::write(dir.join("walk.events"), "# walk\n\nAsh moves through Front-Door\n").unwrap();
    let admitted = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["run", "--events", "walk.events"]).current_dir(&dir).output().unwrap();
    assert_eq!(admitted.status.code(), Some(0), "{}", String::from_utf8_lossy(&admitted.stderr));
    let admitted = String::from_utf8(admitted.stdout).unwrap();
    assert_eq!(admitted.lines().count(), 2);
    assert!(admitted.lines().nth(1).unwrap().starts_with("{\"line\":3,"));
    let help = Command::new(env!("CARGO_BIN_EXE_galileo")).arg("help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout).unwrap().contains("galileo run [PATH...] --events FILE"));
}

#[test]
fn a_create_then_delete_keeps_the_allocator_in_the_replayed_hash() {
    let mut builder = Program::builder();
    builder.state_entity_type("Token", []);
    let tokens = builder.relation(Relation::new("is-a-Token", RelationKind::Static, [Slot::new("token", Type::entity("Token"))]));
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
    let query = Query::all([Variable::new("token")], [Atom::new(tokens, [Expr::var("token")]).positive()]);
    assert_eq!(run.history.query_revisions(&program, &query, 0, 2).into_iter().collect::<Vec<_>>(), [1, 2]);
    assert_eq!(run.history.query_revisions(&program, &query, 1, 2).into_iter().collect::<Vec<_>>(), [2]);
}

#[test]
fn d51_d52_query_and_check_read_admitted_revisions_and_preserve_temporal_claims() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("query-events-dungeon");
    std::fs::create_dir_all(&dir).unwrap();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
    std::fs::write(dir.join("dungeon.gal"), DUNGEON).unwrap();
    std::fs::write(dir.join("walk.events"), WALK).unwrap();
    let invoke = |arguments: &[&str]| Command::new(env!("CARGO_BIN_EXE_galileo")).args(arguments).current_dir(&dir).output().unwrap();
    let initial = invoke(&["query", "dungeon.gal", "all ?r where Ash is in ?r", "--json"]);
    assert_eq!(initial.status.code(), Some(0), "{}", String::from_utf8_lossy(&initial.stderr));
    assert_eq!(initial.stdout, b"[\"Entrance\"]\n");
    let final_room = invoke(&["query", "dungeon.gal", "all ?r where Ash is in ?r", "--events", "walk.events", "--json"]);
    assert_eq!(final_room.status.code(), Some(0), "{}", String::from_utf8_lossy(&final_room.stderr));
    assert_eq!(final_room.stdout, b"[\"Crypt\"]\n");
    let flag_after_json = invoke(&["query", "dungeon.gal", "all ?r where Ash is in ?r", "--json", "--events", "walk.events"]);
    assert_eq!(flag_after_json.status.code(), Some(0));
    assert_eq!(flag_after_json.stdout, final_room.stdout);
    for (revision, room) in [("0", "Entrance"), ("1", "Hall"), ("2", "Library"), ("5", "Crypt")] {
        let answer = invoke(&["query", "dungeon.gal", "all ?r where Ash is in ?r", "--events", "walk.events", "--as-of", revision, "--json"]);
        assert_eq!(answer.status.code(), Some(0), "{}", String::from_utf8_lossy(&answer.stderr));
        assert_eq!(answer.stdout, format!("[\"{room}\"]\n").as_bytes());
    }
    let rooms = invoke(&["query", "dungeon.gal", "all ?r where Ash is in ?r", "--events", "walk.events", "--since", "0", "--json"]);
    assert_eq!(rooms.status.code(), Some(0), "{}", String::from_utf8_lossy(&rooms.stderr));
    assert_eq!(String::from_utf8(rooms.stdout).unwrap(), concat!(
        "{\"revision\":0,\"answer\":[\"Entrance\"]}\n",
        "{\"revision\":1,\"line\":1,\"event\":\"Ash moves through Front-Door\",\"added\":[\"Hall\"],\"removed\":[\"Entrance\"]}\n",
        "{\"revision\":2,\"line\":3,\"event\":\"Ash moves through Library-Arch\",\"added\":[\"Library\"],\"removed\":[\"Hall\"]}\n",
        "{\"revision\":4,\"line\":5,\"event\":\"Ash moves through Crypt-Stair\",\"added\":[\"Crypt\"],\"removed\":[\"Library\"]}\n",
    ));
    let doors = invoke(&["query", "dungeon.gal", "all ?d where ?d opens for Ash", "--events", "walk.events", "--since", "0", "--json"]);
    assert_eq!(doors.status.code(), Some(0), "{}", String::from_utf8_lossy(&doors.stderr));
    assert_eq!(String::from_utf8(doors.stdout).unwrap(), concat!(
        "{\"revision\":0,\"answer\":[\"Crypt-Stair\",\"Front-Door\",\"Library-Arch\"]}\n",
        "{\"revision\":3,\"line\":4,\"event\":\"Ash picks up Bronze-Key\",\"added\":[\"Armory-Gate\",\"Vault-Gate\"],\"removed\":[]}\n",
        "{\"revision\":5,\"line\":6,\"event\":\"Ash picks up Iron-Key\",\"added\":[\"Crypt-Door\"],\"removed\":[]}\n",
    ));
    let program = galileo_syntax::lower::lower_program(&[("dungeon", DUNGEON)]).unwrap().program;
    let mut run = Run::new(&program);
    let moves = program.relations.iter().position(|r| r.template == "{hero} moves through {door}").map(RelationId).unwrap();
    let picks = program.relations.iter().position(|r| r.template == "{hero} picks up {key}").map(RelationId).unwrap();
    for (relation, entity_type, name) in [(moves, "Door", "Front-Door"), (moves, "Door", "Armory-Gate"), (moves, "Door", "Library-Arch"), (picks, "Key", "Bronze-Key"), (moves, "Door", "Crypt-Stair"), (picks, "Key", "Iron-Key")] {
        run.run(&program, relation, &[Value::entity("Hero", "Ash"), Value::entity(entity_type, name)], Budget::default()).unwrap();
    }
    let opens = program.relations.iter().position(|r| r.template == "{door} opens for {hero}").map(RelationId).unwrap();
    let doors = Query::all([Variable::new("door")], [Atom::new(opens, [Expr::var("door"), Expr::value(Value::entity("Hero", "Ash"))]).positive()]);
    assert_eq!(run.history.query_revisions(&program, &doors, 0, 5).into_iter().collect::<Vec<_>>(), [3, 5]);
    let bounded = invoke(&["query", "dungeon.gal", "all ?r where Ash is in ?r", "--events", "walk.events", "--since", "2", "--as-of", "4", "--json"]);
    assert_eq!(bounded.status.code(), Some(0), "{}", String::from_utf8_lossy(&bounded.stderr));
    assert_eq!(String::from_utf8(bounded.stdout).unwrap(), concat!(
        "{\"revision\":2,\"answer\":[\"Library\"]}\n",
        "{\"revision\":4,\"line\":5,\"event\":\"Ash moves through Crypt-Stair\",\"added\":[\"Crypt\"],\"removed\":[\"Library\"]}\n",
    ));
    let scalar = invoke(&["query", "dungeon.gal", "the ?r where Ash is in ?r", "--events", "walk.events", "--since", "1", "--as-of", "2", "--json"]);
    assert_eq!(scalar.status.code(), Some(0), "{}", String::from_utf8_lossy(&scalar.stderr));
    assert_eq!(String::from_utf8(scalar.stdout).unwrap(), concat!(
        "{\"revision\":1,\"answer\":\"Hall\"}\n",
        "{\"revision\":2,\"line\":3,\"event\":\"Ash moves through Library-Arch\",\"added\":[\"Library\"],\"removed\":[\"Hall\"]}\n",
    ));
    let list_value = invoke(&["query", "dungeon.gal", "the ?r where Ash is in ?room and ?r = [?room]", "--events", "walk.events", "--since", "1", "--as-of", "2", "--json"]);
    assert_eq!(list_value.status.code(), Some(0), "{}", String::from_utf8_lossy(&list_value.stderr));
    assert_eq!(String::from_utf8(list_value.stdout).unwrap(), concat!(
        "{\"revision\":1,\"answer\":[\"Hall\"]}\n",
        "{\"revision\":2,\"line\":3,\"event\":\"Ash moves through Library-Arch\",\"added\":[[\"Library\"]],\"removed\":[[\"Hall\"]]}\n",
    ));
    for extra in [vec!["--since", "6"], vec!["--since", "3", "--as-of", "2"], vec!["--since", "0", "--as-of", "6"]] {
        let mut arguments = vec!["query", "dungeon.gal", "all ?r where Ash is in ?r", "--events", "walk.events", "--json"];
        arguments.extend(extra);
        let invalid = invoke(&arguments);
        assert_eq!(invalid.status.code(), Some(2));
        assert!(invalid.stdout.is_empty());
        assert!(String::from_utf8(invalid.stderr).unwrap().contains("last revision 5"));
    }
    let without_events = invoke(&["query", "dungeon.gal", "all ?r where Ash is in ?r", "--since", "0", "--json"]);
    assert_eq!(without_events.status.code(), Some(2));
    for command in ["query", "check"] {
        let mut arguments = vec![command, "dungeon.gal"];
        if command == "query" { arguments.extend(["all ?r where Ash is in ?r", "--json"]); }
        let mut past_last = arguments.clone();
        past_last.extend(["--as-of", "6", "--events", "walk.events"]);
        let past_last = invoke(&past_last);
        assert_eq!(past_last.status.code(), Some(2));
        assert!(past_last.stdout.is_empty());
        assert!(String::from_utf8(past_last.stderr).unwrap().contains("last revision 5"));
        arguments.extend(["--as-of", "1"]);
        let without_events = invoke(&arguments);
        assert_eq!(without_events.status.code(), Some(2));
        assert!(String::from_utf8(without_events.stderr).unwrap().contains("last revision is 0"));
    }
    let invalid_query = invoke(&["query", "dungeon.gal", "all ?r where Ahs is in ?r", "--json"]);
    let invalid_query_with_events = invoke(&["query", "dungeon.gal", "all ?r where Ahs is in ?r", "--events", "walk.events", "--as-of", "1", "--json"]);
    assert_eq!(invalid_query.status.code(), Some(1));
    assert_eq!(invalid_query_with_events.status.code(), Some(1));
    assert_eq!(invalid_query_with_events.stderr, invalid_query.stderr);
    let invalid_since = invoke(&["query", "dungeon.gal", "all ?r where Ahs is in ?r", "--events", "walk.events", "--since", "0", "--json"]);
    assert_eq!(invalid_since.status.code(), Some(1));
    assert_eq!(invalid_since.stderr, invalid_query.stderr);
    let help = invoke(&["help"]);
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("--as-of R"));
    assert!(help.contains("--since R"));
    let original_temporal = invoke(&["check", "dungeon.gal"]);
    let final_temporal = invoke(&["check", "dungeon.gal", "--events", "walk.events"]);
    assert_eq!(original_temporal.status.code(), Some(2));
    assert_eq!(final_temporal.status.code(), original_temporal.status.code());
    assert_eq!(final_temporal.stdout, original_temporal.stdout);
    assert_eq!(final_temporal.stderr, original_temporal.stderr);
    let state_claim = DUNGEON.split("requirements for the level").next().unwrap().to_owned() + "requirements for the level\n  Ash is in Hall\n";
    std::fs::write(dir.join("dungeon.gal"), state_claim).unwrap();
    let hall = invoke(&["check", "dungeon.gal", "--events", "walk.events", "--as-of", "1"]);
    assert_eq!(hall.status.code(), Some(0), "{}", String::from_utf8_lossy(&hall.stderr));
    assert!(String::from_utf8(hall.stdout).unwrap().contains("Ash is in Hall holds."));
    let library = invoke(&["check", "dungeon.gal", "--events", "walk.events", "--as-of", "2"]);
    assert_eq!(library.status.code(), Some(1), "{}", String::from_utf8_lossy(&library.stderr));
    std::fs::write(dir.join("walk.events"), "Ash moves through Front-Door\nAsh moves through Armory-Gate\n").unwrap();
    let before = invoke(&["check", "dungeon.gal"]);
    assert_eq!(before.status.code(), Some(1), "{}", String::from_utf8_lossy(&before.stderr));
    let after = invoke(&["check", "dungeon.gal", "--events", "walk.events"]);
    assert_eq!(after.status.code(), Some(0), "{}", String::from_utf8_lossy(&after.stderr));
    assert!(String::from_utf8(after.stdout).unwrap().contains("Ash is in Hall holds."));
    std::fs::write(dir.join("walk.events"), "Ash moves through Front-Door\nAhs moves through Library-Arch\n").unwrap();
    for arguments in [
        vec!["query", "dungeon.gal", "all ?r where Ash is in ?r", "--events", "walk.events", "--json"],
        vec!["check", "dungeon.gal", "--events", "walk.events"],
        vec!["query", "dungeon.gal", "all ?r where Ash is in ?r", "--events", "walk.events", "--as-of", "0", "--json"],
        vec!["check", "dungeon.gal", "--events", "walk.events", "--as-of", "0"],
    ] {
        let unreadable = invoke(&arguments);
        assert_eq!(unreadable.status.code(), Some(2));
        assert!(unreadable.stdout.is_empty());
        let diagnostic = String::from_utf8(unreadable.stderr).unwrap();
        assert!(diagnostic.contains("walk.events:2"), "{diagnostic}");
        assert!(diagnostic.contains("the nearest is `Ash`"), "{diagnostic}");
    }
}
