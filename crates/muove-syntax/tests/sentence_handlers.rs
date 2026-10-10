use muove_syntax::{read_program, read_sentences, Action, Claim, Question};

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

    let asked = format!(
        "{DUNGEON}\nwhy not Ash is in Shrine via held-keys-open\nprevent Ash is in Vault\n  using: lies-in\n"
    );
    let questions = read_sentences(&asked).unwrap().questions;
    assert!(
        matches!(&questions[0].1, Question::WhyNot { via: Some(r), .. } if r == "held-keys-open")
    );
    assert!(matches!(&questions[1].1, Question::Prevent { using, .. } if using.len() == 1));
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

    let shared =
        "use level\nstatic {hero: Hero} is in {room: Room}\nAsh is a Hero\nAsh is in Hall\n";
    assert_eq!(
        read_program(&[("level", level), ("game", shared)]).unwrap_err().1.message,
        "this can be read more than one way:\n  as is-in from level: {hero} is in {room}\n  as is-in: {hero} is in {room}"
    );
}
