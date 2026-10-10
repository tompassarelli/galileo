use galileo_syntax::lower::lower;
use galileo_syntax::{
    read_program, read_question, read_sentences, Action, Arg, Claim, Literal, Question, QuestionKind,
};

const DUNGEON: &str = include_str!("dungeon.gal");

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
        "type Thing\ngiven {t: Thing} matters\nwhy matters\ntrace matters\nprevent matters\nachieve matters\n",
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
        "type Module, Host\ngiven {module: Module} is enabled on {host: Host}\n",
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
    for (before, after) in [
        ("on ?hero moves through ?door", "on Ash moves through ?door"),
        ("on ?trap strikes ?hero", "on ?trap strikes ?trap"),
        ("on ?trap strikes ?hero", "on ?_ strikes ?hero"),
    ] {
        let source = DUNGEON.replace(before, after);
        let error = read_sentences(&source).unwrap_err();
        let line = source.lines().position(|line| line == after).unwrap() + 1;
        assert_eq!(error.line, line, "{error:?}");
    }

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
fn spec_6_2_d41_requires_and_among_take_one_sentence() {
    let source = |claim: &str| format!("\
type Hero
state {{hero: Hero}} is ready
state {{hero: Hero}} is awake
Ash is a Hero
initially
  Ash is ready
requirements
  {claim}
");
    for (claim, examples) in [
        (
            "Ash is ready requires Ash is ready and Ash is awake",
            [
                "Ash is ready requires Ash is ready",
                "Ash is ready requires Ash is awake",
            ],
        ),
        (
            "Ash is ready and Ash is awake requires Ash is ready",
            [
                "Ash is ready requires Ash is ready",
                "Ash is awake requires Ash is ready",
            ],
        ),
        (
            "Ash is ready has no single point of failure among Ash is ready and Ash is awake",
            [
                "Ash is ready has no single point of failure among Ash is ready",
                "Ash is ready has no single point of failure among Ash is awake",
            ],
        ),
    ] {
        let error = read_sentences(&source(claim)).unwrap_err();
        assert_eq!(error.line, 8);
        assert!(error.message.contains("one sentence"), "{error}");
        assert!(error.message.contains("one requirement per condition"), "{error}");
        for example in examples {
            assert!(error.message.contains(example), "{error}");
        }
    }

    let split = source("Ash is ready requires Ash is ready\n  Ash is ready requires Ash is awake");
    let lowered = lower(&split, "requires-conjunction.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    assert_eq!(lowered.program.requirements.len(), 2);

    for claim in [
        "(Missing) is ready requires Ash is awake",
        "Ash is ready requires (Missing) is awake",
        "(Missing) is ready has no single point of failure among Ash is awake",
        "Ash is ready has no single point of failure among (Missing) is awake",
    ] {
        let input = source(&format!("{claim}\n  Ash is ready requires Ash is ready"));
        let lowered = lower(&input, "failed-requirement.gal").unwrap();
        assert!(
            lowered.diagnostics.iter().any(|error|
                error.line == 8 && error.message.contains("Missing")
            ),
            "{:#?}", lowered.diagnostics,
        );
        assert_eq!(lowered.program.requirements.len(), 1);
        assert_eq!(
            lowered.program.requirements[0].name,
            "Ash is ready requires Ash is ready",
        );
    }
}

#[test]
fn r16_an_unrelated_module_with_the_same_template_changes_no_reading() {
    let level = "\
type Hero, Room
given {hero: Hero} is in {room: Room}
private given {hero: Hero} rests in {room: Room}
";
    let story = "\
type Hero, Room
given {hero: Hero} is in {room: Room}
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
        "use level\ngiven {hero: Hero} is in {room: Room}\nAsh is a Hero\nAsh is in Hall\n";
    assert_eq!(
        read_program(&[("level", level), ("game", shared)]).unwrap_err().1.message,
        "this can be read more than one way:\n  as is-in from level: {hero} is in {room}\n  as is-in: {hero} is in {room}"
    );
}

#[test]
fn spec_5_2_clause_scope_and_event_may_skip() {
    use galileo_syntax::lower::lower;

    let declarations = "type Hero, Room\nstate entity MenuRow\nstate {hero: Hero} is in {room: Room}\nstate {hero: Hero} lists {row: MenuRow}\nevent move: {hero: Hero} moves\nHall is a Room\nAsh is a Hero\n";
    for (clause, line, message) in [
        ("on ?h moves\n  when\n    ?hero is in ?here\n  then ?h is in Hall\n", 9, "write `when ?hero is in ?here` on one line, then one `and condition` per line"),
        ("on ?h moves\n  then\n    ?h is in Hall\n", 9, "write `then ?h is in Hall` on one line, then one `and post` per line"),
        ("on ?h moves\n  then ?h is in ?z\n", 9, "`?z` isn't bound: it must appear in the event sentence, in `when`, or in an `is a new` line"),
        ("on ?h moves\n  when ?h is a Hero\n  may skip\n", 8, "a handler states what holds afterwards with `then ?hero is in ?there`"),
        ("on ?h moves\n  then ?x is a new Hero\n", 9, "`is a new` needs a `state entity`, but `Hero` isn't one"),
        ("on ?h moves\n  then not ?x is a MenuRow\n  and ?x is a new MenuRow\n", 9, "`?x` is created by this clause; `not ?x is a T` deletes only an entity bound by `on` or `when`"),
        ("on ?h moves\n  then ?h is a new MenuRow\n", 9, "`?h` is already bound by this clause; `is a new` needs a fresh named variable"),
    ] {
        let error = read_sentences(&format!("{declarations}{clause}")).unwrap_err();
        assert_eq!((error.line, error.message.as_str()), (line, message));
    }
    let mixed = format!("{declarations}on ?h moves\n  then ?h is in Hall\n  may skip\non ?hero moves\n  then ?hero is in Hall\n");
    let lowered = lower(&mixed, "mixed.gal").unwrap();
    assert_eq!(lowered.diagnostics.len(), 1);
    assert_eq!(lowered.diagnostics[0].line, 11);
    assert_eq!(lowered.diagnostics[0].message, "clauses of event `move` must all say `may skip` or none does: clause at mixed.gal:8 says it, but clause at mixed.gal:11 omits it");

    for clause in [
        "on ?h moves\n  when ?r = Hall\n  and ?s in [Hall]\n  then ?h is in ?r\n  and ?h is in ?s\n",
        "on ?hero moves\n  when ?h is a Hero\n  then ?h is in Hall\n",
        "on ?h moves\n  then ?h lists ?row\n  and ?row is a new MenuRow\n",
    ] {
        let lowered = lower(&format!("{declarations}{clause}"), "scope.gal").unwrap();
        assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    }
    for (file, source) in [("dungeon.gal", DUNGEON), ("menu.gal", include_str!("menu.gal"))] {
        let lowered = lower(source, file).unwrap();
        assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    }
}
