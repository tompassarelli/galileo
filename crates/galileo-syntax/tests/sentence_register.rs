use galileo_syntax::{read_sentences, units, Arg, Kind};
use std::collections::BTreeMap;

const DUNGEON: &str = "\
type Hero, Room, Door, Key, Trap

given Hero starts in Room
given Door leads from ?here: Room to ?there: Room
given Door is unlocked
given Door is locked by Key
given Key lies in Room
given Hero can reach Room
given Hero holds Key
given Trap is armed in Room
given Trap is telegraphed
given Trap hurts Hero

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

given Module is tagged Tag
given Module can opt into Tag
given Host enables Tag
given Host drops Module from Tag
given Host adds Module to Tag
given Host disables Module
given Host runs Platform
  exactly one platform per host
given Module supports Platform
given Tag sets ?path: OptionPath to ?value: nix Any for Module
derived Module is enabled on Host by Tag
derived Module is enabled on Host by default through Tag
derived milestone Module is enabled on Host
derived Host gets ?path: OptionPath set to ?value: nix Any by Module
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
        "given Hero can reach",
        "given arrives: Hero can reach Room\ngiven Hero can reach",
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

    let boot = "type Host, Responsibility, Domain\ngiven Host has boot responsibility ?r: Responsibility\ngiven ?r: Responsibility is realized in ?d: Domain\nboot is a Domain\nbackup is a Responsibility\nbackup is realized in boot\n";
    assert!(galileo_syntax::lower::lower(boot, "boot.gal").unwrap().diagnostics.is_empty());
    let e = error(&boot.replace("boot is a Domain", "boot is a Host"));
    assert_eq!((e.line, e.column), (6, 1));
    assert!(["`boot`", "Host", "Domain", "`d`"].iter().all(|fact| e.message.contains(fact)), "{e}");

    let rooms = "type Room\ngiven ?a: Room is in ?b: Room\ngiven ?a: Room is in boot\nx is a Room\nboot is a Room\n";
    let e = error(&format!("{rooms}x is in boot\n"));
    assert_eq!((e.line, e.column), (6, 1));
    assert!(["{a} is in {b}", "is-in-boot", "{a} is in boot"].iter().all(|fact| e.message.contains(fact)), "{e}");
    let quoted = read_sentences(&format!("{rooms}x is in `boot`\n")).unwrap();
    assert_eq!(quoted.facts.len(), 1);
    assert_eq!(quoted.vocabulary.shapes[quoted.facts[0].shape].name, "is-in");
    assert_eq!(quoted.facts[0].args, ["x", "boot"].map(|name| Arg::Name(name.into())));

    for (token, column) in [("(if true then 1)", 20), ("(1 +)", 9), ("(f(,))", 8), ("1.5", 7)] {
        let source = format!("given has ?value: Int\nhas {token}\n");
        let error = read_sentences(&source).unwrap_err();
        assert_eq!((error.line, error.column), (2, column), "{token}");
        assert!(error.text.as_deref().is_some_and(|text| text.contains(token)), "{error:?}");
    }
    let visits = "type Hero, Room\ngiven Hero visits Room\nAsh is a Hero\nHall is a Room\n";
    for (token, column) in [("+", 12), ("Hall+Cellar", 1), ("[Hall+Cellar]", 1)] {
        let source = format!("{visits}Ash visits {token}\n");
        let error = read_sentences(&source).unwrap_err();
        assert_eq!((error.line, error.column), (5, column), "{token}");
        assert!(error.text.as_deref().is_some_and(|text| text.contains(token)), "{error:?}");
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
effect send: send ?prompt: Text to `Codex` for Turn
  at most one per step
  settled by finished
event finished: `Codex` finished Turn with ?status: TurnStatus
event switch-mode: North switches to ?mode: Text
  mode only in [\"normal\", \"insert\"]
event type-query: North types ?query: Text
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
    let visits = "type Hero, Room\ngiven Hero visits Room\nAsh is a Hero\nHall is a Room\n";
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
    let nested = "type Hero, Room\ngiven Hero visits Room\n  at most one room per hero\n    garbage lost\n";
    let e = read_sentences(nested).unwrap_err();
    assert_eq!((e.line, e.column), (4, 5));

    let derived = "type Node\nderived Node holds\nGoal is a Node\n";
    for (fact, line, column) in [("Goal holds\n", 4, 1), ("initially\n  Goal holds\n", 5, 3)] {
        let error = read_sentences(&format!("{derived}{fact}")).unwrap_err();
        assert_eq!((error.line, error.column), (line, column));
        assert!(error.message.contains("`holds`") && error.message.contains("derived"));
    }
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

    let declarations = "type Door, Room, North, Tag, Module\n";
    let named = "given Door leads from ?here: Room to ?there: Room\nevent North types ?query: Text\ngiven Tag sets ?path: OptionPath to ?value: nix Any for Module\nstate ?rows: List Text are shown\n";
    let named = galileo_syntax::lower::lower(&format!("{declarations}{named}"), "blanks.gal").unwrap();
    assert!(named.diagnostics.is_empty(), "{:?}", named.diagnostics);
    assert_eq!(
        named.program.relations.iter().filter(|relation| !relation.name.starts_with("is-a-")).map(|relation| (relation.name.as_str(), relation.slots.iter().map(|slot| slot.name.as_str()).collect::<Vec<_>>(), relation.template.as_str())).collect::<Vec<_>>(),
        [
            ("leads-from-to", vec!["door", "here", "there"], "{door} leads from {here} to {there}"),
            ("types", vec!["north", "query"], "{north} types {query}"),
            ("sets-to-for", vec!["tag", "path", "value", "module"], "{tag} sets {path} to {value} for {module}"),
            ("are-shown", vec!["rows"], "{rows} are shown"),
        ],
    );

    for (source, offer) in [
        ("type Door\ngiven ?door leads nowhere\n", "?door: Type"),
        ("type Room\ngiven ?room: Room is dark\n", "`Room`"),
    ] {
        let error = galileo_syntax::lower::lower(source, "blanks.gal").unwrap_err();
        assert_eq!((error.line, error.column), (2, 1));
        assert_eq!(error.message.lines().count(), 1);
        assert!(error.message.contains(offer), "{}", error.message);
    }

    for (source, line, offers) in [
        ("type Room\ngiven Room connects to Room\n", 2, vec!["?from: Room", "?to: Room"]),
        ("type North\ngiven North calls Codex\n", 2, vec!["North", "`Codex`"]),
        ("type North\nstate North menu kind Text\n", 2, vec!["?name: Text"]),
        ("type hero\n", 1, vec!["type Hero"]),
        ("type Hero\ngiven {hero: Hero} is wounded\n", 2, vec!["`Hero is wounded`"]),
        ("given {name: Text} is used\n", 1, vec!["`?name: Text is used`"]),
        ("type Room\ngiven {from: Room} reaches {to: Room}\n", 2, vec!["`?from: Room reaches ?to: Room`"]),
        ("type North\nstate North menu kind ?value: Text\n  exactly one value per north\n", 2, vec!["`menu-kind: Text`", "state for North"]),
        ("type North\nderived North badge ?value: Text\n  at most one value per north\n", 2, vec!["`badge: maybe Text`", "derived for North"]),
    ] {
        let error = galileo_syntax::lower::lower(source, "offers.gal").unwrap_err();
        assert_eq!((error.line, error.column), (line, 1));
        for offer in offers {
            assert!(error.message.contains(offer), "{}", error.message);
        }
    }
}

#[test]
fn spec_4_2_a_slot_type_decides_what_a_list_means() {
    let hosts = "type Host, Command\ngiven Host receives ?pkgs: nix Any\ngiven ?c: Command plans ?arguments: List Text\nHost-A is a Host\ndeploy is a Command\n";
    let lowered = galileo_syntax::lower::lower(
        &format!("{hosts}Host-A receives [pkgs.firefox, pkgs.chrome]\ndeploy plans [\"a\", \"b\"]\n"),
        "hosts.gal",
    )
    .unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let p = &lowered.program;
    let values: Vec<_> = p.facts.iter().filter(|f| p.relations[f.relation.0].name != "is-a-Host" && p.relations[f.relation.0].name != "is-a-Command").map(|f| f.values[1].clone()).collect();
    assert_eq!(values, [
        galileo_core::ir::Value::Nix { normalized: "[pkgs.firefox, pkgs.chrome]".into() },
        galileo_core::ir::Value::List(vec!["a".into(), "b".into()]),
    ]);
    assert_eq!(p.relations[1].slots[1].ty, galileo_core::ir::Type::Text.list());
}

const ACCEPTING: &str = "\
given accepting ?kind: Text menu requests ?effect: Text
  at most one effect per kind

accepting ?kind menu requests ?effect
  \"work\"              \"work-show\"
  \"history\"           \"switch-conversation\"
  \"archived-history\"  \"restore-conversation\"
";

#[test]
fn spec_4_2_a_fact_table_states_one_fact_per_row() {
    let lines = "\
given accepting ?kind: Text menu requests ?effect: Text
  at most one effect per kind

accepting \"work\" menu requests \"work-show\"
accepting \"history\" menu requests \"switch-conversation\"
accepting \"archived-history\" menu requests \"restore-conversation\"
";
    let facts = |source: &str| {
        let read = read_sentences(source).unwrap();
        read.facts.iter().map(|f| (f.shape, f.args.clone(), f.line)).collect::<Vec<_>>()
    };
    let table = facts(ACCEPTING);
    assert_eq!(table.len(), 3);
    assert_eq!(table, facts(lines).into_iter().map(|(s, a, l)| (s, a, l + 1)).collect::<Vec<_>>());
    assert_eq!(table[2].2, line_of(ACCEPTING, "  \"archived-history\"  \"restore-conversation\""));
    let narrow = ACCEPTING.replace("\"  ", "\" ").replace("\"            ", "\" ").replace("\"              ", "\" ");
    assert_eq!(facts(&narrow), table);
    assert_eq!(galileo_syntax::fmt::format(&narrow).unwrap(), ACCEPTING);

    let wide = ACCEPTING.replace("  \"history\"           \"switch-conversation\"", "  \"history\" \"switch-conversation\" \"extra\"");
    let e = error(&wide);
    assert_eq!(e.line, line_of(&wide, "  \"history\" \"switch-conversation\" \"extra\""));
    assert!(e.message.contains("?kind ?effect"), "{e}");
    let constant = ACCEPTING.replace("accepting ?kind menu", "accepting \"work\" menu");
    assert_eq!(error(&constant).line, 4);
    for header in ["accepting ?kind menu requests ?kind", "accepting ?_ menu requests ?effect", "accepting (?kind) menu requests ?effect"] {
        assert_eq!(error(&ACCEPTING.replace("accepting ?kind menu requests ?effect", header)).line, 4, "{header}");
    }
    let rowless = "given accepting ?kind: Text menu requests ?effect: Text\naccepting ?kind menu requests ?effect\n";
    assert_eq!(error(rowless).line, 2);
}

#[test]
fn spec_4_2_a_list_declares_one_entity_per_element() {
    let rooms = "Entrance is a Room\nHall is a Room\nLibrary is a Room\nCrypt is a Room\n";
    let list = DUNGEON.replace(rooms, "[Entrance, Hall, Library, Crypt] is a Room\n");
    let entities = |source: &str| {
        let read = read_sentences(source).unwrap();
        assert!(read.warnings.is_empty(), "{:?}", read.warnings);
        let facts = read.facts.iter().map(|f| read.vocabulary.sentence(f.shape, &f.args)).collect::<Vec<_>>();
        let declared = read.entity_facts.iter().map(|f| (read.vocabulary.sentence(f.shape, &f.args), f.line)).collect::<Vec<_>>();
        (facts, declared)
    };
    let (facts, declared) = entities(&list);
    assert_eq!(facts, entities(DUNGEON).0);
    let at = line_of(&list, "[Entrance, Hall, Library, Crypt] is a Room");
    let listed: Vec<_> = declared.iter().filter(|(_, line)| *line == at).map(|(s, _)| s.as_str()).collect();
    assert_eq!(listed, ["Entrance is a Room", "Hall is a Room", "Library is a Room", "Crypt is a Room"]);

    let empty = DUNGEON.replace(rooms, "[] is a Room\n");
    let e = error(&empty);
    assert_eq!(e.line, line_of(&empty, "[] is a Room"), "{e}");

    let twice = list.replace("Front-Door is a Door\n", "Front-Door is a Door\n[Hall, Vault] is a Room\n");
    let read = read_sentences(&twice).unwrap();
    let warned: Vec<_> = read.warnings.iter().map(|w| (w.line, w.message.as_str())).collect();
    assert_eq!(warned, [(line_of(&twice, "[Hall, Vault] is a Room"), format!("`Hall` is already declared a Room on line {at}").as_str())]);
}

#[test]
fn spec_4_2_a_focus_block_writes_its_subject_once() {
    let head = "\
type North, Prewarm
given Prewarm chooses lease ?action: Text for identity ?identity: Bool
given Prewarm writes stamp ?success: Bool
state for North
  menu-kind: Text
  menu-selection: Int
";
    let focus = format!("{head}\
initially
  north-main
    menu-kind = \"\"
    menu-selection = 0
north-main is a North
prewarm
  is a Prewarm
  chooses lease ?action for identity ?identity
    \"contended\"  false
    \"supersede\"  true
  writes stamp true
");
    let flat = format!("{head}\
initially
  north-main.menu-kind = \"\"
  north-main.menu-selection = 0
north-main is a North
prewarm is a Prewarm
prewarm chooses lease \"contended\" for identity false
prewarm chooses lease \"supersede\" for identity true
prewarm writes stamp true
");
    let facts = |source: &str| {
        let read = read_sentences(source).unwrap();
        assert!(read.warnings.is_empty(), "{:?}", read.warnings);
        let mut all: Vec<_> = read.facts.iter().chain(&read.entity_facts).map(|f| (f.line, read.vocabulary.sentence(f.shape, &f.args))).collect();
        all.sort();
        all.into_iter().map(|(line, s)| (s, line)).collect::<Vec<_>>()
    };
    let (focused, flat) = (facts(&focus), facts(&flat));
    assert_eq!(focused.iter().map(|(s, _)| s).collect::<Vec<_>>(), flat.iter().map(|(s, _)| s).collect::<Vec<_>>());
    let cited: Vec<_> = focused.iter().map(|(_, line)| focus.lines().nth(line - 1).unwrap().trim()).collect();
    assert_eq!(cited, ["menu-kind = \"\"", "menu-selection = 0", "north-main is a North", "is a Prewarm", "\"contended\"  false", "\"supersede\"  true", "writes stamp true"]);

    let rowless = focus.replace("    \"contended\"  false\n    \"supersede\"  true\n", "");
    assert_eq!(error(&rowless).line, line_of(&rowless, "  chooses lease ?action for identity ?identity"));
    let stray = focus.replace("    menu-selection = 0", "    menu-sel = 0");
    assert_eq!(error(&stray).line, line_of(&stray, "    menu-sel = 0"));
}
