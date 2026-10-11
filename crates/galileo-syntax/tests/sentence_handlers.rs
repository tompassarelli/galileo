use galileo_syntax::lower::lower;
use galileo_syntax::{
    read_program, read_question, read_sentences, Arg, Claim, Literal, Question, QuestionKind,
};

const DUNGEON: &str = include_str!("dungeon.gal");

#[test]
fn spec_5_6_dungeon_reads() {
    let modules = read_program(&[("dungeon", DUNGEON)]).unwrap();
    let (scope, question) =
        read_question("possibly Ash is in Shrine", QuestionKind::Claim, &modules).unwrap();
    let Question::Claim(Claim::Eventually(goal)) = question else {
        panic!("possibly is the claim's goal");
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
    let error = read_question("?x holds Iron-Key", QuestionKind::Sentence, &modules).unwrap_err();
    assert_eq!((error.line, error.column), (1, 1));
    assert!(error.message.contains("`?x`"), "{error}");
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
    for (claim, keyword, examples) in [
        (
            "Ash is ready requires Ash is ready and Ash is awake",
            "`requires`",
            [
                "Ash is ready requires Ash is ready",
                "Ash is ready requires Ash is awake",
            ],
        ),
        (
            "Ash is ready and Ash is awake requires Ash is ready",
            "`requires`",
            [
                "Ash is ready requires Ash is ready",
                "Ash is awake requires Ash is ready",
            ],
        ),
        (
            "Ash is ready has no single point of failure among Ash is ready and Ash is awake",
            "`among`",
            [
                "Ash is ready has no single point of failure among Ash is ready",
                "Ash is ready has no single point of failure among Ash is awake",
            ],
        ),
    ] {
        let error = read_sentences(&source(claim)).unwrap_err();
        assert_eq!((error.line, error.column), (8, 3));
        assert!(error.message.contains(keyword), "{error}");
        for example in examples {
            assert!(error.message.contains(example), "{error}");
        }
    }

    let split = source("Ash is ready requires Ash is ready\n  Ash is ready requires Ash is awake");
    let lowered = lower(&split, "requires-conjunction.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    assert_eq!(
        lowered.program.requirements.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["Ash is ready requires Ash is ready", "Ash is ready requires Ash is awake"],
    );

    for claim in [
        "?h is ready has no single point of failure among ?h is awake",
        "?k > 0 and ?k = count where ?m is ready",
        "some ?h: ?h is ready and ?k > 0 and ?k = count where ?m is awake",
    ] {
        let parsed = read_sentences(&source(claim)).unwrap();
        assert_eq!(parsed.requirements[0].requirements.len(), 1);
    }

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
                (error.line, error.column) == (8, 3) && error.message.contains("`Missing`")
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
    let before = reading(&[("level", level), ("game", game)]);
    let after = reading(&[("level", level), ("story", story), ("game", game)]);
    assert_eq!(before.0, "level");
    assert_eq!(before, after);

    let unlisted = read_program(&[
        ("level", level),
        ("game", "use level\nAsh is a Hero\nAsh rests in Hall\n"),
    ]);
    let (_, error) = unlisted.unwrap_err();
    assert_eq!((error.line, error.column), (3, 1));
    assert!(error.message.contains("{hero} is in {room}") && !error.message.contains("rests in"), "{error}");

    let modules = read_program(&[("level", level), ("game", "")]).unwrap();
    let (scope, question) =
        read_question("Ash is in Hall", QuestionKind::Sentence, &modules).unwrap();
    let Question::Sentence(sentence) = question else {
        panic!("the question uses level without an explicit use");
    };
    assert_eq!(scope.shapes[sentence.shape].module, "level");
    let error = read_question("Ash rests in Hall", QuestionKind::Sentence, &modules).unwrap_err();
    assert_eq!((error.line, error.column), (1, 1));
    assert!(error.message.contains("{hero} is in {room}") && !error.message.contains("rests in"), "{error}");

    let shared =
        "use level\ngiven {hero: Hero} is in {room: Room}\nAsh is a Hero\nAsh is in Hall\n";
    assert_eq!(reading(&[("level", level), ("game", shared)]).0, "game");
}

#[test]
fn spec_5_2_clause_scope_and_event_may_skip() {
    use galileo_syntax::lower::lower;

    let declarations = "type Hero, Room\nstate entity MenuRow\nstate {hero: Hero} is in {room: Room}\nstate {hero: Hero} lists {row: MenuRow}\nevent move: {hero: Hero} moves\nHall is a Room\nAsh is a Hero\n";
    let mixed = format!("{declarations}on ?h moves\n  then ?h is in Hall\n  may skip\non ?hero moves\n  then ?hero is in Hall\n");
    let lowered = lower(&mixed, "mixed.gal").unwrap();
    assert_eq!(lowered.diagnostics.len(), 1);
    let d = &lowered.diagnostics[0];
    assert_eq!((d.line, d.column), (11, 1));
    assert!(["`move`", "`may skip`", "mixed.gal:8", "mixed.gal:11"].iter().all(|fact| d.message.contains(fact)), "{}", d.message);

    for clause in [
        "on ?h moves\n  when ?r = Hall\n  and ?s in [Hall]\n  then ?h is in ?r\n  and ?h is in ?s\n",
        "on ?hero moves\n  when ?h is a Hero\n  then ?h is in Hall\n",
        "on ?h moves\n  then ?h lists ?row\n  and ?row is a new MenuRow\n",
    ] {
        let lowered = lower(&format!("{declarations}{clause}"), "scope.gal").unwrap();
        assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    }
}

#[test]
fn spec_2_k4_scope_names_exports_without_importing_entities_or_transitive_names() {
    let level = "type Hero, Room\ngiven {hero: Hero} is in {room: Room}\n";
    let story = level;
    for uses in ["use level\nuse story\n", "use story\nuse level\n"] {
        let game = format!("{uses}Ash is a Hero\nAsh is in Hall\n");
        let (module, error) =
            read_program(&[("level", level), ("story", story), ("game", &game)]).unwrap_err();
        assert_eq!(module, "game");
        assert_eq!((error.line, error.column), (3, 1));
        assert!(["`Hero`", "level", "story"].iter().all(|fact| error.message.contains(fact)), "{error}");

        let story = "type Person, Place\ngiven {hero: Person} is in {room: Place}\n";
        let parsed = read_program(&[("level", level), ("story", story), ("game", &game)]).unwrap();
        let game = &parsed["game"];
        assert_eq!(game.vocabulary.shapes[game.facts[0].shape].module, "level");
    }

    let a = "type Hero\ngiven {h: Hero} runs\n";
    let b = "type Robot\ngiven {r: Robot} runs\nX is a Robot\n";
    for uses in ["use a\nuse b\n", "use b\nuse a\n"] {
        let app = format!("{uses}X runs\n");
        let (module, error) = read_program(&[("a", a), ("b", b), ("app", &app)]).unwrap_err();
        assert_eq!((module.as_str(), error.line, error.column), ("app", 3, 1));
        assert!(error.message.contains("{h} runs") && error.message.contains("{r} runs"), "{error}");
    }

    let core = "type North\n";
    let menu = "use core\ngiven {north: North} is ready\n";
    let app = "use menu\nmain is a North\nmain is ready\n";
    let (module, error) =
        read_program(&[("core", core), ("menu", menu), ("app", app)]).unwrap_err();
    assert_eq!((module.as_str(), error.line, error.column), ("app", 2, 1));
    assert!(error.message.contains("`North`") && error.message.contains("core"), "{error}");
    let direct = format!("use core\n{app}");
    let parsed = read_program(&[("core", core), ("menu", menu), ("app", &direct)]).unwrap();
    assert_eq!(parsed["app"].facts.iter().map(|f| f.args.clone()).collect::<Vec<_>>(), [vec![Arg::Name("main".into())]]);
    let unnamed = "use menu\nmain is ready\n";
    assert!(read_program(&[("core", core), ("menu", menu), ("app", unnamed)]).is_ok());

    let tags = "type Host, Module\ngiven {host: Host} enables {module: Module}\ngiven {host: Host} disables {module: Module}\nracket is a Module\n";
    let host = "use tags\nwhiterabbit enables racket\nwhiterabbit disables racket\n";
    let parsed = read_program(&[("tags", tags), ("host", host)]).unwrap();
    let host_facts: Vec<_> = parsed["host"].facts.iter().map(|f| parsed["host"].vocabulary.shape_text(f.shape)).collect();
    assert_eq!(host_facts, ["{host} enables {module}", "{host} disables {module}"]);
    assert!(parsed["tags"].exports.entities.is_empty());
}

#[test]
fn spec_4_2_firn_two_type_entity_keeps_both_memberships_and_written_diagnostics() {
    let tags = "type Module, Tag\ngiven {module: Module} is tagged {tag: Tag}\n";
    let catalog = "use firn/tags\ndoom-emacs is a Tag\n";
    let module = "use firn/tags\ndoom-emacs is a Module\ndoom-emacs is tagged doom-emacs\n";
    let sources = [("firn/tags", tags), ("catalog", catalog), ("module", module)];
    let parsed = read_program(&sources).unwrap();
    assert_eq!(parsed["module"].facts.len(), 1);
    assert_eq!(parsed["module"].facts[0].args, [Arg::Name("doom-emacs".into()), Arg::Name("doom-emacs".into())]);

    for (declaration, facts) in [
        ("doom-emacs is a Module", ["`doom-emacs`", "Module", "Tag", "`tag`"]),
        ("doom-emacs is a Tag", ["`doom-emacs`", "Tag", "Module", "`module`"]),
    ] {
        let module = format!("use firn/tags\n{declaration}\ndoom-emacs is tagged doom-emacs\n");
        let (name, error) = read_program(&[("firn/tags", tags), ("module", &module)]).unwrap_err();
        assert_eq!(name, "module");
        assert_eq!((error.line, error.column), (3, 1));
        assert!(facts.iter().all(|fact| error.message.contains(fact)), "{error}");
    }
}

#[test]
fn spec_9_4_keywords_never_shadow_templates() {
    let modules = read_program(&[
        ("firn/tags", include_str!("../../../lib/firn/tags.gal")),
        (
            "firn/responsibilities",
            include_str!("../../../lib/firn/responsibilities.gal"),
        ),
        (
            "backup",
            "use firn/responsibilities\nrequirements\n  backup requires capability gpu\n",
        ),
    ])
    .unwrap();
    let backup = &modules["backup"];
    let Claim::State(body) = &backup.requirements[0].requirements[0].claim else {
        panic!("the keyword belongs to the responsibility template");
    };
    let [Literal::Sentence(reading)] = body.as_slice() else {
        panic!("one responsibility sentence");
    };
    assert_eq!(
        backup.vocabulary.shapes[reading.shape].module,
        "firn/responsibilities"
    );
    assert_eq!(
        backup.vocabulary.sentence(reading.shape, &reading.args),
        "backup requires capability gpu"
    );

    let parsed = read_sentences("given {x: Int} is between {a: Int} and {b: Int}\nrequirements\n  some ?x: ?x is between 1 and 5\n").unwrap();
    let Claim::Quantified { body, .. } = &parsed.requirements[0].requirements[0].claim else {
        panic!("a quantified condition");
    };
    let [Literal::Sentence(reading)] = body.as_slice() else {
        panic!("and belongs to the between template");
    };
    assert_eq!(
        parsed.vocabulary.sentence(reading.shape, &reading.args),
        "?x is between 1 and 5"
    );

    let parsed = read_sentences("given {x: Int} is between {a: Int} and {b: Int}\nrequirements\n  some ?x: ?x > 0 and not ?x is between 1 and 5\n").unwrap();
    let Claim::Quantified { body, .. } = &parsed.requirements[0].requirements[0].claim else {
        panic!("a quantified condition");
    };
    let [Literal::Compare(..), Literal::Not(reading)] = body.as_slice() else {
        panic!("the last and belongs to the negated between template");
    };
    assert_eq!(
        parsed.vocabulary.sentence(reading.shape, &reading.args),
        "?x is between 1 and 5"
    );
}

#[test]
fn spec_9_4_d56_requires_claim_and_template_are_both_readings() {
    let declarations = "type Task\ngiven build\ngiven test\ngiven {a: Task} requires {b: Task}\nbuild is a Task\ntest is a Task\n";
    let parsed = read_sentences(&format!("{declarations}build requires test\n")).unwrap();
    assert_eq!(parsed.facts.len(), 1);
    let fact = &parsed.facts[0];
    assert_eq!(parsed.vocabulary.shape_text(fact.shape), "{a} requires {b}");
    assert_eq!(fact.args, [Arg::Name("build".into()), Arg::Name("test".into())]);
}
