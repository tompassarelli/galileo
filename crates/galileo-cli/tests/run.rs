use galileo_core::state::{RejectionKind, Run, State};
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
        (moves, vec![Value::entity("dungeon/Hero", "Ash"), Value::entity("dungeon/Door", "Front-Door")]),
        (moves, vec![Value::entity("dungeon/Hero", "Ash"), Value::entity("dungeon/Door", "Armory-Gate")]),
        (moves, vec![Value::entity("dungeon/Hero", "Ash"), Value::entity("dungeon/Door", "Library-Arch")]),
        (picks, vec![Value::entity("dungeon/Hero", "Ash"), Value::entity("dungeon/Key", "Bronze-Key")]),
        (moves, vec![Value::entity("dungeon/Hero", "Ash"), Value::entity("dungeon/Door", "Crypt-Stair")]),
        (picks, vec![Value::entity("dungeon/Hero", "Ash"), Value::entity("dungeon/Key", "Iron-Key")]),
    ];
    let mut previous = (0, run.state.hash(&program));
    for (index, (event, arguments)) in events.iter().enumerate() {
        let record = run.run(&program, *event, arguments, Budget::default()).unwrap();
        assert!(lines[index + 1].starts_with(&format!("{{\"line\":{},\"event\":\"{}\",\"admitted\":", index + 1, WALK.lines().nth(index).unwrap())));
        assert!(lines[index + 1].contains(&format!("\"revision\":{},\"hash\":\"{}\"", record.revision, record.hash)));
        assert_eq!(record.revision, [1, 1, 2, 3, 4, 5][index]);
        if index == 1 {
            assert_eq!(record.result.as_ref().unwrap_err().kind, RejectionKind::NoMatch);
            assert!(lines[index + 1].contains("\"admitted\":false,\"rejection\":\"no match\",\"explanation\":["));
            assert_eq!((record.revision, record.hash.clone()), previous);
        } else {
            assert!(record.result.as_ref().unwrap().is_empty());
            assert!(lines[index + 1].contains("\"admitted\":true"));
            assert!(lines[index + 1].ends_with("\"effects\":[]}"));
        }
        previous = (record.revision, record.hash.clone());
    }

    let chain = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["why", "Ash", "holds", "Iron-Key", "dungeon.gal", "--events", "walk.events"]).current_dir(&dir).output().unwrap();
    assert_eq!(chain.status.code(), Some(0), "{}", String::from_utf8_lossy(&chain.stderr));
    assert!(chain.stderr.is_empty());
    let chain = String::from_utf8(chain.stdout).unwrap();
    assert_eq!(chain.lines().next(), Some("Ash holds Iron-Key, since revision 5"));
    assert_eq!(chain.lines().filter(|line| line.starts_with("revision ")).collect::<Vec<_>>(), [
        "revision 5: Ash picks up Iron-Key (walk.events:6), by the clause at dungeon.gal:75, when at revision 4:",
        "revision 4: Ash moves through Crypt-Stair (walk.events:5), by the clause at dungeon.gal:68, when at revision 3:",
        "revision 2: Ash moves through Library-Arch (walk.events:3), by the clause at dungeon.gal:68, when at revision 1:",
        "revision 1: Ash moves through Front-Door (walk.events:1), by the clause at dungeon.gal:68, when at revision 0:",
    ]);
    for leaf in [
        "Ash is in Crypt, since revision 4",
        "Ash is in Library, since revision 2",
        "Ash is in Hall, since revision 1",
        "Iron-Key lies in Crypt, since revision 0 (dungeon.gal:37)",
        "Ash is in Entrance, since revision 0 (dungeon.gal:35)",
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
        (vec!["Ash", "is", "in", "Hall"], "held from revision 1 until revision 2: superseded by Ash is in Library", Some("revision 2: Ash moves through Library-Arch (walk.events:3), by the clause at dungeon.gal:68, when at revision 1:"), Some("Ash is in Hall, since revision 1")),
        (vec!["Iron-Key", "lies", "in", "Crypt"], "held from revision 0 until revision 5: withdrawn", Some("revision 5: Ash picks up Iron-Key (walk.events:6), by the clause at dungeon.gal:75, when at revision 4:"), Some("Iron-Key lies in Crypt, since revision 0 (dungeon.gal:37)")),
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
    assert!(diagnostic.contains("`Ash`"), "{diagnostic}");
    std::fs::write(dir.join("walk.events"), "# walk\n\nAsh moves through Front-Door\n").unwrap();
    let admitted = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["run", "--events", "walk.events"]).current_dir(&dir).output().unwrap();
    assert_eq!(admitted.status.code(), Some(0), "{}", String::from_utf8_lossy(&admitted.stderr));
    let admitted = String::from_utf8(admitted.stdout).unwrap();
    assert_eq!(admitted.lines().count(), 2);
    assert!(admitted.lines().nth(1).unwrap().starts_with("{\"line\":3,"));
    let help = Command::new(env!("CARGO_BIN_EXE_galileo")).arg("help").output().unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.lines().any(|line| line.contains("galileo run") && line.contains("--events FILE")), "{help}");
}

#[test]
fn spec_7_3_d56_groups_each_clauses_when_proof_and_each_substitution() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("history-clause-groups");
    std::fs::create_dir_all(&dir).unwrap();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
    std::fs::write(dir.join("start.events"), "start\n").unwrap();
    let declarations = "\
type Hero
Ash is a Hero
Bo is a Hero
given Hero trained
Ash trained
Bo trained
state Hero is ready
derived all set
event start
rule both: all set
  when Ash is ready
  and Bo is ready
";
    for (clauses, bo_line) in [
        ("on start\n  when Ash trained\n  then Ash is ready\non start\n  when Bo trained\n  then Bo is ready\n", 16),
        ("on start\n  when ?h trained\n  then ?h is ready\n", 13),
    ] {
        std::fs::write(dir.join("team.gal"), declarations.to_owned() + clauses).unwrap();
        let chain = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["why", "all", "set", "team.gal", "--events", "start.events"]).current_dir(&dir).output().unwrap();
        assert_eq!(chain.status.code(), Some(0), "{}", String::from_utf8_lossy(&chain.stderr));
        assert!(chain.stderr.is_empty());
        let chain = String::from_utf8(chain.stdout).unwrap();
        let block = chain.lines().skip_while(|line| !line.starts_with("revision ")).collect::<Vec<_>>();
        assert_eq!(block.len(), 5, "{chain}");
        assert_eq!(block[0], "revision 1: start (start.events:1), when at revision 0:");
        assert_eq!(block[1], "  by the clause at team.gal:13:");
        assert!(block[2].starts_with("    Ash trained"), "{chain}");
        assert_eq!(block[3], format!("  by the clause at team.gal:{bo_line}:"));
        assert!(block[4].starts_with("    Bo trained"), "{chain}");
    }
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
    for (relation, entity_type, name) in [(moves, "dungeon/Door", "Front-Door"), (moves, "dungeon/Door", "Armory-Gate"), (moves, "dungeon/Door", "Library-Arch"), (picks, "dungeon/Key", "Bronze-Key"), (moves, "dungeon/Door", "Crypt-Stair"), (picks, "dungeon/Key", "Iron-Key")] {
        run.run(&program, relation, &[Value::entity("dungeon/Hero", "Ash"), Value::entity(entity_type, name)], Budget::default()).unwrap();
    }
    let opens = program.relations.iter().position(|r| r.template == "{door} opens for {hero}").map(RelationId).unwrap();
    let doors = Query::all([Variable::new("door")], [Atom::new(opens, [Expr::var("door"), Expr::value(Value::entity("dungeon/Hero", "Ash"))]).positive()]);
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
    assert_eq!(original_temporal.status.code(), Some(1));
    assert_eq!(final_temporal.status.code(), original_temporal.status.code());
    assert_eq!(final_temporal.stdout, original_temporal.stdout);
    assert_eq!(final_temporal.stderr, original_temporal.stderr);
    let state_claim = DUNGEON.split("requirements for the level").next().unwrap().to_owned() + "requirements for the level\n  Ash is in Hall\n";
    std::fs::write(dir.join("dungeon.gal"), state_claim).unwrap();
    let hall = invoke(&["check", "dungeon.gal", "--events", "walk.events", "--as-of", "1"]);
    assert_eq!(hall.status.code(), Some(0), "{}", String::from_utf8_lossy(&hall.stderr));
    assert!(String::from_utf8(hall.stdout).unwrap().contains("Ash is in Hall holds in the state after walk.events at revision 1."));
    let library = invoke(&["check", "dungeon.gal", "--events", "walk.events", "--as-of", "2"]);
    assert_eq!(library.status.code(), Some(1), "{}", String::from_utf8_lossy(&library.stderr));
    std::fs::write(dir.join("walk.events"), "Ash moves through Front-Door\nAsh moves through Armory-Gate\n").unwrap();
    let before = invoke(&["check", "dungeon.gal"]);
    assert_eq!(before.status.code(), Some(1), "{}", String::from_utf8_lossy(&before.stderr));
    let after = invoke(&["check", "dungeon.gal", "--events", "walk.events"]);
    assert_eq!(after.status.code(), Some(0), "{}", String::from_utf8_lossy(&after.stderr));
    assert!(String::from_utf8(after.stdout).unwrap().contains("Ash is in Hall holds in the state after walk.events at revision 1."));
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
        assert!(diagnostic.contains("`Ash`"), "{diagnostic}");
    }
}

#[test]
fn spec_5_5_search_replays_the_shortest_dungeon_witness_and_reports_open_domains() {
    use galileo_core::search::{search, Limits, Scope, SearchReason, SearchVerdict};
    let program = galileo_syntax::lower::lower_program(&[("dungeon", DUNGEON)]).unwrap().program;
    let location = program.relations.iter().position(|relation| relation.template == "{hero} is in {room}").map(RelationId).unwrap();
    let at = |room| StateClaim::Holds(Atom::new(location, [Expr::value(Value::entity("dungeon/Hero", "Ash")), Expr::value(Value::entity("dungeon/Room", room))]));
    let goal = Claim::Eventually(at("Vault"));
    let result = search(&program, &goal, Limits::default()).unwrap();
    assert_eq!(result.verdict, SearchVerdict::Holds);
    let witness = result.witness.as_ref().unwrap();
    assert_eq!(witness.len(), 5);
    let mut alphabet: Vec<_> = result.alphabet.iter().map(|event| program.relations[event.relation.0].template.as_str()).collect();
    alphabet.sort();
    assert_eq!(alphabet, ["{hero} moves through {door}", "{hero} picks up {key}", "{trap} strikes {hero}"]);
    assert!(result.alphabet.iter().flat_map(|event| &event.slots).all(|slot| !slot.open));
    assert_eq!(result, search(&program, &goal, Limits::default()).unwrap());
    let mut run = Run::new(&program);
    for (event, arguments) in witness { assert!(run.run(&program, *event, arguments, Budget::default()).unwrap().result.is_ok()); }
    assert!(run.state.facts.contains(&(location, vec![Value::entity("dungeon/Hero", "Ash"), Value::entity("dungeon/Room", "Vault")])));
    assert_eq!(run.revision, 5);
    assert_eq!(Run::replay(&program, &run.log, Budget::default()).unwrap(), run);
    let never = search(&program, &Claim::Never(at("Shrine")), Limits::default()).unwrap();
    assert_eq!(never.verdict, SearchVerdict::Holds);
    assert!(matches!(never.scope, Scope::Complete { .. }));
    assert!(never.rejected > 0);
    let counterexample = search(&program, &Claim::Never(at("Vault")), Limits::default()).unwrap();
    assert_eq!(counterexample.verdict, SearchVerdict::Fails);
    assert_eq!(counterexample.witness.unwrap().len(), 5);
    let always = search(&program, &Claim::Always(at("Entrance")), Limits::default()).unwrap();
    assert_eq!(always.verdict, SearchVerdict::Fails);
    assert_eq!(always.witness.unwrap().len(), 1);
    for limits in [
        Limits { states: 0, ..Limits::default() },
        Limits { states: 1, ..Limits::default() },
        Limits { depth: 0, ..Limits::default() },
        Limits { work: 0, ..Limits::default() },
        Limits { work: 1, ..Limits::default() },
    ] {
        for claim in [&goal, &Claim::Never(at("Vault"))] {
            let bounded = search(&program, claim, limits).unwrap();
            assert!(matches!(bounded.verdict, SearchVerdict::Unknown(_)), "{bounded:?}");
            assert!(matches!(bounded.scope, Scope::Incomplete { .. }));
            assert!(bounded.work <= limits.work);
        }
    }
    let at_boundary = search(&program, &goal, Limits { depth: 5, ..Limits::default() }).unwrap();
    assert_eq!(at_boundary.verdict, SearchVerdict::Holds);
    assert_eq!(at_boundary.witness.unwrap().len(), 5);
    for domain in ["sampled from", "only in", ""] {
        let source = format!("state compromised\nevent enter: enter ?code: Text\n{}on enter ?code\n  when ?code != \"ok\"\n  then compromised\n  may skip\n", if domain.is_empty() { String::new() } else { format!("  code {domain} [\"ok\"]\n") });
        let program = galileo_syntax::lower::lower_program(&[("codes", &source)]).unwrap().program;
        let compromised = program.relations.iter().position(|relation| relation.template == "compromised").map(RelationId).unwrap();
        let event = program.events[0].relation;
        let claim = Claim::Never(StateClaim::Holds(Atom::new(compromised, [])));
        let result = search(&program, &claim, Limits::default()).unwrap();
        if domain == "only in" {
            assert_eq!(result.verdict, SearchVerdict::Holds);
            assert_eq!(result.scope, Scope::Complete { states: 1 });
        } else {
            assert!(matches!(&result.verdict, SearchVerdict::Unknown(reasons) if reasons.iter().any(|reason| matches!(reason, SearchReason::OpenDomain { slot, .. } if slot == "code"))));
            assert_eq!(result.sampled, Some(true));
        }
        assert_eq!(result.alphabet[0].slots[0].values.len(), usize::from(!domain.is_empty()));
        let mut run = Run::new(&program);
        let record = run.run(&program, event, &[Value::text("boom")], Budget::default()).unwrap();
        if domain == "only in" {
            assert_eq!(record.result.as_ref().unwrap_err().kind, RejectionKind::ArgumentDomain);
            assert_eq!(run.revision, 0);
        } else {
            assert!(record.result.is_ok());
            assert!(run.state.facts.contains(&(compromised, vec![])));
            let mut sampled_boom = program.clone();
            sampled_boom.events[0].domains.insert(0, ArgumentDomain::Sampled(vec![Value::text("boom")]));
            let counterexample = search(&sampled_boom, &claim, Limits::default()).unwrap();
            assert_eq!(counterexample.verdict, SearchVerdict::Fails);
            let witness = search(&sampled_boom, &Claim::Eventually(StateClaim::Holds(Atom::new(compromised, []))), Limits::default()).unwrap();
            assert_eq!(witness.verdict, SearchVerdict::Holds);
            assert!(matches!(witness.scope, Scope::Incomplete { .. }));
        }
    }
    let mut invalid = program.clone();
    invalid.rules[0].head.relation = RelationId(usize::MAX);
    assert!(matches!(search(&invalid, &goal, Limits::default()), Err(EvalError::Static(_))));
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("check-dungeon-search");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("dungeon.gal"), DUNGEON).unwrap();
    let check = |extra: &[&str]| Command::new(env!("CARGO_BIN_EXE_galileo")).args(["check", "dungeon.gal"]).args(extra).current_dir(&dir).output().unwrap();
    let full = check(&[]);
    let stdout = String::from_utf8(full.stdout).unwrap();
    let mut lines = stdout.lines();
    assert!(lines.any(|line| line == "possibly Ash is in Vault holds: a reachable state satisfies it, by a 5-step witness."), "{stdout}");
    let steps: Vec<_> = lines.by_ref().take_while(|line| line.starts_with("  ")).collect();
    assert_eq!(steps.len(), 5, "{stdout}");
    for ((step, line), (event, arguments)) in steps.iter().enumerate().zip(witness) {
        let rendered: Vec<_> = arguments.iter().map(|value| match value { Value::Entity(entity) => entity.name.as_str(), _ => unreachable!() }).collect();
        let words = program.relations[event.0].template.replace("{hero}", rendered[0]).replace("{door}", rendered[1]).replace("{key}", rendered[1]);
        assert_eq!(*line, format!("  {}. {words}", step + 1));
    }
    assert!(stdout.contains(&format!("never Ash is in Shrine holds {}.", never.scope)), "{stdout}");
    for name in ["?hero is in Vault only after ?hero is in Library", "?trap strikes ?hero only when ?trap warned ?hero"] {
        assert!(stdout.lines().any(|line| line.starts_with(&format!("{name} holds across all ")) && line.ends_with("reachable states (complete).")), "{stdout}");
    }
    assert!(stdout.contains("possibly Ash is in Vault has no single point of failure among ?key lies in ?room fails: ?key = Iron-Key, ?room = Crypt is a counterexample."), "{stdout}");
    assert_eq!(full.status.code(), Some(1));
    let small = check(&["--budget", "3000"]);
    let stdout = String::from_utf8(small.stdout).unwrap();
    for name in ["possibly Ash is in Vault", "never Ash is in Shrine", "?hero is in Vault only after ?hero is in Library", "?trap strikes ?hero only when ?trap warned ?hero", "possibly Ash is in Vault has no single point of failure"] {
        assert!(stdout.lines().any(|line| line.starts_with(name) && line.contains(" is unknown:") && line.contains("(incomplete)")), "{stdout}");
    }
    assert_eq!(small.status.code(), Some(2));
    std::fs::write(dir.join("broken.gal"), format!("{DUNGEON}  ?hero is in Library only after ?hero holds Iron-Key\n  ?trap strikes ?hero only when ?trap hurt ?hero\n")).unwrap();
    let broken = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["check", "broken.gal"]).current_dir(&dir).output().unwrap();
    let stdout = String::from_utf8(broken.stdout).unwrap();
    assert!(stdout.contains("?hero is in Library only after ?hero holds Iron-Key fails: a 2-step trace reaches a counterexample."), "{stdout}");
    assert!(stdout.lines().any(|line| line.starts_with("?trap strikes ?hero only when ?trap hurt ?hero fails: a ")), "{stdout}");
    let level = DUNGEON.split("requirements for the level").next().unwrap();
    let requirement = |name: &str, rules: &str, claim: &str, extra: &[&str]| {
        std::fs::write(dir.join(name), format!("{level}{rules}requirements for the level\n  {claim}\n")).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["check", name]).args(extra).current_dir(&dir).output().unwrap();
        (output.status.code(), String::from_utf8(output.stdout).unwrap())
    };
    let bronze = "Armory-Gate opens for Ash requires Ash holds Bronze-Key";
    let (code, stdout) = requirement("requires.gal", "", bronze, &[]);
    assert_eq!((code, stdout.as_str()), (Some(0), format!("{bronze} holds {}.\n", never.scope).as_str()));
    let (code, stdout) = requirement("requires.gal", "", bronze, &["--budget", "3000"]);
    assert_eq!(code, Some(2));
    assert!(stdout.starts_with(&format!("{bronze} is unknown:")) && stdout.contains("(incomplete)"), "{stdout}");
    let bypass = "rule crypt-bypass: Shrine-Door opens for ?hero\n  when ?hero is in Crypt\n\n";
    let silver = "Shrine-Door opens for Ash requires Ash holds Silver-Key";
    let (code, stdout) = requirement("bypass.gal", bypass, silver, &[]);
    assert_eq!(code, Some(1));
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines.len(), 8, "{stdout}");
    assert!(lines[4].starts_with("  ") && lines[4].ends_with(":90: Shrine-Door opens for Ash holds without Ash holds Silver-Key."), "{stdout}");
    assert_eq!(lines[5], "  Shrine-Door opens for Ash still holds with every Ash holds Silver-Key blocked:");
    assert!(lines[6].contains("/crypt-bypass (") && lines[6].ends_with("bypass:86)") && lines[7].contains("Ash is in Crypt"), "{stdout}");
    assert_eq!(lines[..4], [
        format!("{silver} fails: a 3-step trace reaches a counterexample.").as_str(),
        "  1. Ash moves through Front-Door", "  2. Ash moves through Library-Arch", "  3. Ash moves through Crypt-Stair",
    ]);
    let robust = "Armory-Gate opens for Ash has no single point of failure among Ash holds ?key";
    let (code, stdout) = requirement("robust.gal", "", robust, &[]);
    assert_eq!(code, Some(1));
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines[5], "  Armory-Gate opens for Ash holds only through Ash holds Bronze-Key:", "{stdout}");
    assert!(lines[6..].iter().any(|line| line.contains("Ash holds Bronze-Key")), "{stdout}");
    assert!(lines[4].starts_with("  ") && lines[4].ends_with(":87: withdrawing Ash holds Bronze-Key breaks Armory-Gate opens for Ash."), "{stdout}");
    assert_eq!(lines[..4], [
        format!("{robust} fails: a 3-step trace reaches a counterexample.").as_str(),
        "  1. Ash moves through Front-Door", "  2. Ash moves through Library-Arch", "  3. Ash picks up Bronze-Key",
    ]);
    println!("SPEC 5.5: shortest={} admitted={} rejected={} work={} scope={}; complete shrine scope={}", witness.len(), result.admitted, result.rejected, result.work, result.scope, never.scope);
}

#[test]
fn spec_7_3_every_traced_witness_runs_and_replays_to_its_sentence() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("trace-replay");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("dungeon.gal"), DUNGEON).unwrap();
    let galileo = |args: &[&str]| Command::new(env!("CARGO_BIN_EXE_galileo")).args(args).current_dir(&dir).output().unwrap();
    for (goal, steps) in [("Ash is in Vault", 5), ("Ash holds Iron-Key", 4), ("Ash is in Hall", 1)] {
        let words: Vec<_> = goal.split(' ').collect();
        let traced = galileo(&[&["trace"], &words[..], &["dungeon.gal"]].concat());
        assert_eq!(traced.status.code(), Some(0), "{goal}: {}", String::from_utf8_lossy(&traced.stderr));
        let trace = String::from_utf8(traced.stdout).unwrap();
        assert_eq!(trace.lines().count(), steps, "{goal}: {trace}");
        std::fs::write(dir.join("witness.events"), &trace).unwrap();
        let run = galileo(&["run", "dungeon.gal", "--events", "witness.events"]);
        assert_eq!(run.status.code(), Some(0), "{goal}: {}", String::from_utf8_lossy(&run.stderr));
        assert_eq!(String::from_utf8(run.stdout).unwrap().matches("\"admitted\":true").count(), steps, "{goal}");
        let replayed = galileo(&[&["replay"], &words[..], &["dungeon.gal", "--events", "witness.events"]].concat());
        assert_eq!(String::from_utf8(replayed.stdout).unwrap(), format!("{goal} holds after {steps} events at revision {steps}\n"));
        assert_eq!(replayed.status.code(), Some(0));
        let short: String = trace.lines().take(steps - 1).map(|line| format!("{line}\n")).collect();
        std::fs::write(dir.join("short.events"), if short.is_empty() { "Ash moves through Armory-Gate\n".into() } else { short }).unwrap();
        let replayed = galileo(&[&["replay"], &words[..], &["dungeon.gal", "--events", "short.events"]].concat());
        assert_eq!(replayed.status.code(), Some(1), "{goal}: {}", String::from_utf8_lossy(&replayed.stdout));
    }
    let unreachable = galileo(&["trace", "Ash", "is", "in", "Shrine", "dungeon.gal"]);
    assert_eq!(unreachable.status.code(), Some(1));
    assert!(unreachable.stdout.is_empty());
}
