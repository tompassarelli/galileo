use galileo_syntax::{read_sentences, units, Arg, Kind};
use std::collections::BTreeMap;

const DUNGEON: &str = "\
type Hero, Room, Door, Key, Trap

given {hero: Hero} starts in {room: Room}
given {door: Door} leads from {here: Room} to {there: Room}
given {door: Door} is unlocked
given {door: Door} is locked by {key: Key}
given {key: Key} lies in {room: Room}
given {hero: Hero} can reach {room: Room}
given {hero: Hero} holds {key: Key}
given {trap: Trap} is armed in {room: Room}
given {trap: Trap} is telegraphed
given {trap: Trap} hurts {hero: Hero}

Ash is a Hero
Entrance is a Room
Hall is a Room
Library is a Room
Crypt is a Room
Front-Door is a Door
Iron-Key is a Key
Spike-Trap is a Trap

Ash starts in Entrance
Front-Door
  leads from Entrance to Hall
  is unlocked
Iron-Key lies in Crypt
Spike-Trap
  is armed in Hall
  is telegraphed
";

const TAGS: &str = "\
type Module, Tag, Host, Platform

given {module: Module} is tagged {tag: Tag}
given {module: Module} can opt into {tag: Tag}
given {host: Host} enables {tag: Tag}
given {host: Host} drops {module: Module} from {tag: Tag}
given {host: Host} adds {module: Module} to {tag: Tag}
given {host: Host} disables {module: Module}
given {host: Host} runs {platform: Platform}
  exactly one platform per host
given {module: Module} supports {platform: Platform}
given {tag: Tag} sets {path: OptionPath} to {value: nix Any} for {module: Module}
derived {module: Module} is enabled on {host: Host} by {tag: Tag}
derived {module: Module} is enabled on {host: Host} by default through {tag: Tag}
derived milestone {module: Module} is enabled on {host: Host}
derived {host: Host} gets {path: OptionPath} set to {value: nix Any} by {module: Module}
  at most one value per host, path

rule enabled: ?module is enabled on ?host
  when ?module is enabled on ?host by ?tag
  and not ?host disables ?module
";

fn error(source: &str) -> galileo_syntax::Error {
    read_sentences(source).unwrap_err()
}

fn line_of(source: &str, text: &str) -> usize {
    source.lines().position(|line| line == text).unwrap() + 1
}

#[test]
fn prototype_dungeon_messages_reproduce() {
    let closest = DUNGEON.replace("Ash starts in Entrance", "Ash begins at Entrance");
    let e = error(&closest);
    assert_eq!((e.line, e.column), (line_of(&closest, "Ash begins at Entrance"), 1), "{e:?}");
    let offered: Vec<&str> = e.message.lines().skip(1).map(str::trim).collect();
    assert_eq!(offered, ["{hero} starts in {room}", "{hero} can reach {room}", "{door} leads from {here} to {there}"]);

    let unmatched = DUNGEON.replace("Ash starts in Entrance", "Nobody wanders");
    let e = error(&unmatched);
    assert_eq!((e.line, e.column, e.message.lines().count()), (line_of(&unmatched, "Nobody wanders"), 1, 1), "{e:?}");

    let ambiguous = DUNGEON.replace(
        "given {hero: Hero} can reach",
        "given arrives: {hero: Hero} can reach {room: Room}\ngiven {hero: Hero} can reach",
    ) + "Ash can reach Hall\n";
    let e = error(&ambiguous);
    assert_eq!((e.line, e.column), (line_of(&ambiguous, "Ash can reach Hall"), 1), "{e:?}");
    let readings: Vec<&str> = e.message.lines().skip(1).collect();
    assert_eq!(readings.len(), 2, "{e:?}");
    for (reading, shape) in readings.iter().zip(["arrives", "can-reach"]) {
        assert!(reading.contains(shape) && reading.contains("{hero} can reach {room}"), "{e:?}");
    }
}

#[test]
fn spec_9_2_a_slot_takes_one_token() {
    let read = |text: &str| {
        let parsed = read_sentences(TAGS).unwrap();
        let line = galileo_syntax::lex(text).unwrap().remove(0);
        let mut vars = BTreeMap::new();
        let reading = parsed
            .vocabulary
            .read(&line, &units(&line, &line.tokens).unwrap(), Some(&mut vars))
            .unwrap();
        (
            parsed.vocabulary.shapes[reading.shape].name.clone(),
            reading.args,
        )
    };
    let (longer, args) = read("?m is enabled on ?h by default through ?t");
    assert_eq!(longer, "is-enabled-on-by-default-through");
    assert_eq!(args, ["m", "h", "t"].map(|v| Arg::Var(v.into())));
    assert_eq!(read("?m is enabled on ?h by ?t").0, "is-enabled-on-by");

    let boot = "type Host, Responsibility, Domain\ngiven {host: Host} has boot responsibility {r: Responsibility}\ngiven {r: Responsibility} is realized in {d: Domain}\nboot is a Domain\nbackup is a Responsibility\nbackup is realized in boot\n";
    assert!(galileo_syntax::lower::lower(boot, "boot.gal").unwrap().diagnostics.is_empty());
    let e = error(&boot.replace("boot is a Domain", "boot is a Host"));
    assert_eq!((e.line, e.column), (6, 1));
    assert!(["`boot`", "Host", "Domain", "`d`"].iter().all(|fact| e.message.contains(fact)), "{e}");

    let rooms = "type Room\ngiven {a: Room} is in {b: Room}\ngiven {a: Room} is in boot\nx is a Room\nboot is a Room\n";
    let e = error(&format!("{rooms}x is in boot\n"));
    assert_eq!((e.line, e.column), (6, 1));
    assert!(["{a} is in {b}", "is-in-boot", "{a} is in boot"].iter().all(|fact| e.message.contains(fact)), "{e}");
    let quoted = read_sentences(&format!("{rooms}x is in `boot`\n")).unwrap();
    assert_eq!(quoted.facts.len(), 1);
    assert_eq!(quoted.vocabulary.shapes[quoted.facts[0].shape].name, "is-in");
    assert_eq!(quoted.facts[0].args, ["x", "boot"].map(|name| Arg::Name(name.into())));

    for token in ["(if true then 1)", "(1 +)", "(f(,))", "1.5"] {
        let source = format!("given has {{value: Int}}\nhas {token}\n");
        assert_eq!(read_sentences(&source).unwrap_err().line, 2, "{token}");
    }
    let visits = "type Hero, Room\ngiven {hero: Hero} visits {room: Room}\nAsh is a Hero\nHall is a Room\n";
    for token in ["+", "Hall+Cellar", "[Hall+Cellar]"] {
        let source = format!("{visits}Ash visits {token}\n");
        assert_eq!(read_sentences(&source).unwrap_err().line, 5, "{token}");
    }
    assert_eq!(
        read_sentences(&format!("{visits}Ash visits room(1)\n")).unwrap().facts[0].args,
        [Arg::Name("Ash".into()), Arg::Expr("room(1)".into())]
    );
}

#[test]
fn spec_9_6_host_tags_state_one_fact_per_list_element() {
    let host = "\
whiterabbit is a Host
whiterabbit runs linux
whiterabbit enables [terminal, cli-tools, theming, auth, javascript, python, rust]
whiterabbit enables browsers
whiterabbit adds [chrome, qutebrowser, zen-browser] to browsers
whiterabbit enables desktop
whiterabbit drops mako from desktop
whiterabbit adds river to desktop
whiterabbit disables [anytype, bun, cargo, clang]
firefox is tagged browsers
browsers sets myConfig.modules.firefox.default to true for firefox
";
    let parsed = read_sentences(&format!("{TAGS}\n{host}")).unwrap();
    let stated: Vec<String> = parsed.facts.iter().map(|fact| parsed.vocabulary.sentence(fact.shape, &fact.args)).collect();
    assert_eq!(
        stated,
        [
            "whiterabbit runs linux",
            "whiterabbit enables terminal",
            "whiterabbit enables cli-tools",
            "whiterabbit enables theming",
            "whiterabbit enables auth",
            "whiterabbit enables javascript",
            "whiterabbit enables python",
            "whiterabbit enables rust",
            "whiterabbit enables browsers",
            "whiterabbit adds chrome to browsers",
            "whiterabbit adds qutebrowser to browsers",
            "whiterabbit adds zen-browser to browsers",
            "whiterabbit enables desktop",
            "whiterabbit drops mako from desktop",
            "whiterabbit adds river to desktop",
            "whiterabbit disables anytype",
            "whiterabbit disables bun",
            "whiterabbit disables cargo",
            "whiterabbit disables clang",
            "firefox is tagged browsers",
            "browsers sets myConfig.modules.firefox.default to true for firefox",
        ]
    );
    assert!(parsed.warnings.is_empty());

    let twice = format!("{TAGS}\n{host}whiterabbit runs darwin\n");
    let e = error(&twice);
    assert_eq!((e.line, e.column), (34, 1));
    assert!(
        ["exactly one platform per host", "whiterabbit runs linux", "whiterabbit runs darwin"].iter().all(|fact| e.message.contains(fact)),
        "{e}"
    );
}

#[test]
fn spec_5_4_and_5_5_effect_and_domain_lines() {
    let source = "\
type Turn, North
type TurnStatus = \"ok\" | \"failed\"
effect send: send {prompt: Text} to `Codex` for {turn: Turn}
  at most one per step
  settled by finished
event finished: `Codex` finished {turn: Turn} with {status: TurnStatus}
event switch-mode: {north: North} switches to {mode: Text}
  mode only in [\"normal\", \"insert\"]
event type-query: {north: North} types {query: Text}
  query sampled from [\"\", \"fi\", \"zz\"]
";
    let shapes = read_sentences(source).unwrap().vocabulary.shapes;
    assert_eq!(shapes[0].kind, Kind::Effect);
    assert!(shapes[0].one_per_step);
    assert_eq!(shapes[0].settled_by.as_deref(), Some("finished"));
    assert!(shapes[2].domains[0].closed && !shapes[3].domains[0].closed);
    assert_eq!(shapes[3].domains[0].values, ["", "fi", "zz"].map(|v| Arg::Text(v.into())));
}

#[test]
fn review_2_8_list_slots_and_shape_lines_lose_nothing() {
    let visits = "type Hero, Room\ngiven {hero: Hero} visits {room: Room}\nAsh is a Hero\nHall is a Room\n";
    for (last, offending) in [("Ash visits [?room]", "`[?room]`"), ("Ash visits [?_]", "`[?_]`"), ("Ash visits ?room", "`?room`")] {
        let e = read_sentences(&format!("{visits}{last}\n")).unwrap_err();
        assert_eq!((e.line, e.column), (5, 1), "{e}");
        assert!(e.message.contains(offending), "{e}");
    }
    for last in ["Ash visits [visits]", "Ash visits visits"] {
        let lowered = galileo_syntax::lower::lower(&format!("{visits}{last}\n"), "visits.gal").unwrap();
        assert_eq!(lowered.diagnostics.len(), 1);
        let d = &lowered.diagnostics[0];
        assert_eq!((d.line, d.column), (5, 1));
        assert!(["`visits`", "Room", "`visits is a Room`"].iter().all(|fact| d.message.contains(fact)), "{}", d.message);
    }
    for ground in ["Ash visits Hall", "Ash visits [Hall]", "Ash visits [`visits`]"] {
        assert_eq!(read_sentences(&format!("{visits}{ground}\n")).unwrap().facts.len(), 1);
    }
    let nested = "type Hero, Room\ngiven {hero: Hero} visits {room: Room}\n  at most one room per hero\n    garbage lost\n";
    let e = read_sentences(nested).unwrap_err();
    assert_eq!((e.line, e.column), (4, 5));
}

#[test]
fn spec_4_1_slots_named_by_types() {
    let source = "type North, MenuRow\ngiven North lists MenuRow\ngiven North uses `Codex`\nview Menu for North\n  owner: ?north\n";
    let lowered = galileo_syntax::lower::lower(source, "slots.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let relation = &lowered.program.relations[0];
    assert_eq!(relation.name, "lists");
    assert_eq!(relation.template, "{north} lists {menu-row}");
    assert_eq!(relation.slots.iter().map(|slot| slot.name.as_str()).collect::<Vec<_>>(), ["north", "menu-row"]);
    assert_eq!(lowered.program.relations[1].template, "{north} uses Codex");
    assert_eq!(lowered.program.views[0].parameters[0].name, "north");

    for (source, line, offers) in [
        ("type Room\ngiven Room connects to Room\n", 2, vec!["{from: Room}", "{to: Room}"]),
        ("type North\ngiven North calls Codex\n", 2, vec!["North", "`Codex`"]),
        ("type North\nstate North menu kind Text\n", 2, vec!["{name: Text}"]),
        ("type hero\n", 1, vec!["type Hero"]),
    ] {
        let error = galileo_syntax::lower::lower(source, "offers.gal").unwrap_err();
        assert_eq!((error.line, error.column), (line, 1));
        for offer in offers {
            assert!(error.message.contains(offer), "{}", error.message);
        }
    }
}
