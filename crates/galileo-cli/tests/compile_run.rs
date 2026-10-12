use galileo_core::{Budget, EvalError, Reason, RelationId, Value};
use galileo_runtime::{Error, Program, RejectionKind, Session};
use std::path::Path;
use std::process::Command;

#[path = "../../galileo-syntax/tests/fixtures/dungeon_walk.rs"]
mod dungeon_walk;

#[test]
fn d54_image_boundary_replays_dungeon_sources_with_session_observations() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("compile-run-dungeon");
    std::fs::create_dir_all(&dir).unwrap();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
    let source = format!("use marker\n{}\n{}", include_str!("../../galileo-syntax/tests/dungeon.gal"), r#"
effect arrival: report Hero arriving in Room

on ?hero moves through ?door
  when ?hero is in ?here
  and ?door leads from ?here to ?there
  and ?door opens for ?hero
  then report ?hero arriving in ?there

view Journey for {hero: Hero}
  room: the ?room where ?hero is in ?room
# café"#);
    let marker = "type Marker\n# λ";
    let walk = dungeon_walk::events_text();
    std::fs::write(dir.join("dungeon.gal"), &source).unwrap();
    std::fs::write(dir.join("marker.gal"), marker).unwrap();
    std::fs::write(dir.join("walk.events"), walk).unwrap();
    let invoke = |arguments: &[&str]| Command::new(env!("CARGO_BIN_EXE_galileo")).args(arguments).current_dir(&dir).output().unwrap();
    let compiled = invoke(&["compile", "dungeon.gal", "-o", "dungeon.image"]);
    assert_eq!(compiled.status.code(), Some(0), "{}", String::from_utf8_lossy(&compiled.stderr));
    let bytes = std::fs::read(dir.join("dungeon.image")).unwrap();
    let lowered = galileo_syntax::lower::lower_program(&[("marker", marker), ("dungeon", &source)]).unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let expected = format!("galileo-image 1 {}\nmodule marker {}\n{marker}module dungeon {}\n{source}", galileo_review::identity(&lowered.program), marker.len(), source.len());
    assert_eq!(bytes, expected.as_bytes());
    let program = Program::load(&bytes).unwrap();
    assert_eq!(program.sources(), [("marker".into(), marker.into()), ("dungeon".into(), source.clone())]);
    let from_source = invoke(&["run", "dungeon.gal", "--events", "walk.events"]);
    let from_image = invoke(&["run", "dungeon.image", "--events", "walk.events"]);
    assert_eq!(from_source.status.code(), Some(1), "{}", String::from_utf8_lossy(&from_source.stderr));
    assert_eq!(from_image.status.code(), Some(1), "{}", String::from_utf8_lossy(&from_image.stderr));
    assert_eq!(from_image.stdout, from_source.stdout);
    let output = String::from_utf8(from_image.stdout).unwrap();
    let lines: Vec<_> = output.lines().collect();
    assert_eq!(lines.len(), 7);
    let relation = |template| program.relations.iter().position(|relation| relation.template == template).map(RelationId).unwrap();
    let moves = relation("{hero} moves through {door}");
    let arrival = relation("report {hero} arriving in {room}");
    let ash = Value::entity("dungeon/Hero", "Ash");
    let mut session = Session::new(&program);
    assert_eq!(lines[0], format!("{{\"revision\":0,\"hash\":\"{}\"}}", session.state().hash(&program)));
    let rooms = [Some("Hall"), None, Some("Library"), None, Some("Crypt"), None];
    for (index, ((verb, kind, name), room)) in dungeon_walk::WALK.into_iter().zip(rooms).enumerate() {
        let relation = relation(dungeon_walk::template(verb));
        let kind = format!("dungeon/{kind}");
        let result = session.run((relation, vec![ash.clone(), Value::entity(&kind, name)]));
        assert!(lines[index + 1].contains(&format!("\"revision\":{},\"hash\":\"{}\"", session.revision(), session.state().hash(&program))));
        if index == dungeon_walk::LOCKED {
            assert!(matches!(result, Err(Error::Rejected(rejection)) if rejection.kind == RejectionKind::NoMatch));
            assert!(lines[index + 1].contains("\"admitted\":false,\"rejection\":\"no match\""));
        } else {
            let step = result.unwrap();
            assert_eq!(step.revision, dungeon_walk::REVISIONS[index]);
            let effects = room.map(|room| vec![(arrival, vec![ash.clone(), Value::entity("dungeon/Room", room)])]).unwrap_or_default();
            assert_eq!(step.effects, effects);
            assert!(lines[index + 1].contains("\"admitted\":true"));
            let printed = room.map(|room| format!("[\"report Ash arriving in {room}\"]")).unwrap_or("[]".into());
            assert!(lines[index + 1].ends_with(&format!("\"effects\":{printed}}}")));
        }
    }
    let journey = program.views.iter().find(|view| view.name == "dungeon/Journey").unwrap();
    let Value::Record(view) = session.project(journey, &[ash.clone()]).unwrap() else { panic!("Journey is a record"); };
    assert_eq!(view["room"], Value::entity("dungeon/Room", "Crypt"));
    let projected = invoke(&["query", "dungeon.gal", "the ?r where Ash is in ?r", "--events", "walk.events", "--json"]);
    assert_eq!(projected.status.code(), Some(0), "{}", String::from_utf8_lossy(&projected.stderr));
    assert_eq!(projected.stdout, b"\"Crypt\"\n");
    for budget in ["0", "1000000"] {
        let source_budget = invoke(&["run", "dungeon.gal", "--events", "walk.events", "--budget", budget]);
        let image_budget = invoke(&["run", "dungeon.image", "--events", "walk.events", "--budget", budget]);
        assert_eq!(source_budget.status.code(), Some(if budget == "0" { 2 } else { 1 }));
        assert_eq!(image_budget.status.code(), source_budget.status.code());
        assert_eq!(image_budget.stdout, source_budget.stdout);
        if budget == "0" { assert!(String::from_utf8(image_budget.stdout).unwrap().ends_with("{\"line\":1,\"unknown\":\"budget\"}\n")); }
    }
    assert!(matches!(Session::new(&program).run_with_budget((moves, vec![ash, Value::entity("dungeon/Door", "Front-Door")]), Budget::work(0)), Err(Error::Evaluation(EvalError::Unknown(Reason::OutOfBudget { .. })))));
    let tampered = expected.replace("type Marker", "type Marked");
    assert!(matches!(Program::load(tampered.as_bytes()), Err(Error::WrongProgram)));
    std::fs::write(dir.join("tampered.image"), tampered).unwrap();
    let wrong = invoke(&["run", "tampered.image", "--events", "walk.events"]);
    assert_eq!(wrong.status.code(), Some(2));
    assert!(wrong.stdout.is_empty());
    assert!(String::from_utf8(wrong.stderr).unwrap().contains("WrongProgram"));
    assert!(matches!(Program::load(&bytes[..bytes.len() - 1]), Err(Error::Image(_))));
    std::fs::write(dir.join("walk.events"), "Ash moves through Front-Door\nAhs moves through Library-Arch\n").unwrap();
    let invalid_source = invoke(&["run", "dungeon.gal", "--events", "walk.events"]);
    let invalid_image = invoke(&["run", "dungeon.image", "--events", "walk.events"]);
    assert_eq!(invalid_source.status.code(), Some(2));
    assert_eq!(invalid_image.status.code(), Some(2));
    assert!(invalid_image.stdout.is_empty());
    assert_eq!(invalid_image.stderr, invalid_source.stderr);
    let help = invoke(&["help"]);
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.lines().any(|line| line.contains("galileo compile") && line.contains("-o")), "{help}");
    println!("D54: modules=2 events=6 revisions=5 effects=3 rejected=1 final_room=Crypt final_hash={} source_image_disagreements=0 tamper=WrongProgram budgets=2", session.state().hash(&program));
}
