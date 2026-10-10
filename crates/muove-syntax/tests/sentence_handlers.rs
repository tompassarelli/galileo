use muove_syntax::{
    read_program, read_question, read_sentences, Action, Arg, Claim, Literal, Question, QuestionKind,
};

const DUNGEON: &str = include_str!("dungeon.muove");

#[test]
fn spec_5_6_dungeon_reads() {
    let parsed = read_sentences(DUNGEON).unwrap();
    let v = &parsed.vocabulary;
    assert_eq!(parsed.facts.len(), 3 + 7 * 2 + 2);
    assert_eq!(parsed.rules.len(), 2);

    let events: Vec<&str> = parsed
        .handlers
        .iter()
        .map(|h| v.shapes[h.event.shape].name.as_str())
        .collect();
    assert_eq!(events, ["move", "move", "pick-up", "strike"]);
    let actions: Vec<String> = parsed
        .handlers
        .iter()
        .flat_map(|h| &h.actions)
        .map(|(_, a)| match a {
            Action::Set(r) => format!("set {}", v.sentence(r.shape, &r.args)),
            Action::Include(r) => format!("include {}", v.sentence(r.shape, &r.args)),
            Action::Withdraw(r) => format!("withdraw {}", v.sentence(r.shape, &r.args)),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        actions,
        [
            "set ?hero is in ?there",
            "include ?trap warned ?hero",
            "withdraw ?key lies in ?room",
            "include ?hero holds ?key",
            "include ?trap hurt ?hero",
        ]
    );

    let [level] = parsed.requirements.as_slice() else {
        panic!("one requirements block");
    };
    assert_eq!(level.name.as_deref(), Some("the level"));
    let kinds: Vec<&str> = level
        .requirements
        .iter()
        .map(|r| match &r.claim {
            Claim::Eventually(_) => "eventually",
            Claim::TracePasses(..) => "trace passes",
            Claim::NoSinglePointOfFailure(inner, _) if matches!(**inner, Claim::Eventually(_)) => {
                "eventually, no single point of failure"
            }
            Claim::HappensWhen(..) => "happens when",
            Claim::Never(_) => "never",
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "eventually",
            "trace passes",
            "eventually, no single point of failure",
            "happens when",
            "never"
        ]
    );

    let modules = read_program(&[("dungeon", DUNGEON)]).unwrap();
    let (scope, question) =
        read_question("eventually Ash is in Shrine", QuestionKind::Claim, &modules).unwrap();
    let Question::Claim(Claim::Eventually(goal)) = question else {
        panic!("eventually is the claim's goal");
    };
    let [Literal::Sentence(reading)] = goal.as_slice() else {
        panic!("one sentence in the goal");
    };
    assert_eq!(scope.sentence(reading.shape, &reading.args), "Ash is in Shrine");

    let (scope, question) =
        read_question("Ash holds Iron-Key", QuestionKind::Sentence, &modules).unwrap();
    let Question::Sentence(reading) = question else {
        panic!("a ground sentence");
    };
    assert_eq!(
        scope.sentence(reading.shape, &reading.args),
        "Ash holds Iron-Key"
    );
    assert!(
        read_question("?x holds Iron-Key", QuestionKind::Sentence, &modules)
            .unwrap_err()
            .message
            .contains("?x")
    );
    assert!(matches!(
        read_question("all ?x where ?x holds Iron-Key", QuestionKind::Query, &modules)
            .unwrap()
            .1,
        Question::Query(_)
    ));

    let facts = read_sentences(
        "type Thing\nstatic {t: Thing} matters\nwhy matters\ntrace matters\nprevent matters\nachieve matters\n",
    )
    .unwrap();
    let names: Vec<String> = facts
        .facts
        .iter()
        .map(|fact| fact.args[0].to_string())
        .collect();
    assert_eq!(names, ["why", "trace", "prevent", "achieve"]);

    let modules = read_program(&[(
        "tags",
        "type Module, Host\nstatic {module: Module} is enabled on {host: Host}\n",
    )])
    .unwrap();
    let (scope, question) = read_question(
        "via is enabled on whiterabbit",
        QuestionKind::Sentence,
        &modules,
    )
    .unwrap();
    let Question::Sentence(reading) = question else {
        panic!("via is the module name");
    };
    assert_eq!(reading.args[0], Arg::Name("via".into()));
    assert_eq!(
        scope.sentence(reading.shape, &reading.args),
        "via is enabled on whiterabbit"
    );
}

#[test]
fn spec_6_3_a_temporal_invariant_points_to_when() {
    let source = DUNGEON.replace("  never Ash", "  invariant eventually Ash");
    assert_eq!(
        read_sentences(&source).unwrap_err().message,
        "an `invariant` takes a state claim: a sentence, or a `some`, `no` or `every` claim, checked on one state. A temporal claim depends on which steps are admitted; put an event's precondition in its handler's `when`"
    );
}

#[test]
fn r16_an_unrelated_module_with_the_same_template_changes_no_reading() {
    let level = "\
type Hero, Room
static {hero: Hero} is in {room: Room}
private static {hero: Hero} rests in {room: Room}
";
    let story = "\
type Hero, Room
static {hero: Hero} is in {room: Room}
";
    let game = "\
use level
Ash is a Hero
Ash is in Hall
";
    let reading = |modules: &[(&str, &str)]| {
        let read = read_program(modules).unwrap();
        let game = &read["game"];
        let shape = &game.vocabulary.shapes[game.facts[0].shape];
        (
            shape.module.clone(),
            shape.name.clone(),
            game.facts[0].args.clone(),
        )
    };
    let before = reading(&[("level", level), ("story", story), ("game", game)]);
    let imported = game.replace("use level\n", "use level\nuse story\n");
    let after = reading(&[("level", level), ("story", story), ("game", &imported)]);
    assert_eq!(before.0, "level");
    assert_eq!(before, after);

    let unlisted = read_program(&[
        ("level", level),
        ("game", "use level\nAsh is a Hero\nAsh rests in Hall\n"),
    ]);
    assert!(unlisted
        .unwrap_err()
        .1
        .message
        .starts_with("this doesn't match"));

    let modules = read_program(&[("level", level), ("game", "")]).unwrap();
    let (scope, question) =
        read_question("Ash is in Hall", QuestionKind::Sentence, &modules).unwrap();
    let Question::Sentence(reading) = question else {
        panic!("the question uses level without an explicit use");
    };
    assert_eq!(scope.shapes[reading.shape].module, "level");
    assert!(
        read_question("Ash rests in Hall", QuestionKind::Sentence, &modules)
            .unwrap_err()
            .message
            .starts_with("this doesn't match")
    );

    let shared =
        "use level\nstatic {hero: Hero} is in {room: Room}\nAsh is a Hero\nAsh is in Hall\n";
    assert_eq!(
        read_program(&[("level", level), ("game", shared)]).unwrap_err().1.message,
        "this can be read more than one way:\n  as is-in from level: {hero} is in {room}\n  as is-in: {hero} is in {room}"
    );
}
