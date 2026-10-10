use galileo_syntax::{read_sentences, units, Arg, Kind};
use std::collections::BTreeMap;

const DUNGEON: &str = "\
type Hero, Room, Door, Key, Trap

static {hero: Hero} starts in {room: Room}
static {door: Door} leads from {here: Room} to {there: Room}
static {door: Door} is unlocked
static {door: Door} is locked by {key: Key}
static {key: Key} lies in {room: Room}
static {hero: Hero} can reach {room: Room}
static {hero: Hero} holds {key: Key}
static {trap: Trap} is armed in {room: Room}
static {trap: Trap} is telegraphed
static {trap: Trap} hurts {hero: Hero}

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

static {module: Module} is tagged {tag: Tag}
static {module: Module} can opt into {tag: Tag}
static {host: Host} enables {tag: Tag}
static {host: Host} drops {module: Module} from {tag: Tag}
static {host: Host} adds {module: Module} to {tag: Tag}
static {host: Host} disables {module: Module}
static {host: Host} runs {platform: Platform}
  exactly one platform per host
static {module: Module} supports {platform: Platform}
static {tag: Tag} sets {path: OptionPath} to {value: nix Any} for {module: Module}
derived {module: Module} is enabled on {host: Host} by {tag: Tag}
derived {module: Module} is enabled on {host: Host} by default through {tag: Tag}
derived checkpoint {module: Module} is enabled on {host: Host}
derived {host: Host} gets {path: OptionPath} set to {value: nix Any} by {module: Module}
  at most one value per host, path

rule enabled: ?module is enabled on ?host
  when ?module is enabled on ?host by ?tag
  and not ?host disables ?module
";

fn message(source: &str) -> String {
    read_sentences(source).unwrap_err().message
}

#[test]
fn prototype_dungeon_messages_reproduce() {
    assert_eq!(read_sentences(DUNGEON).unwrap().facts.len(), 6);

    let closest = DUNGEON.replace("Ash starts in Entrance", "Ash begins at Entrance");
    assert_eq!(
        message(&closest),
        "this doesn't match any declared sentence shape. Closest:\n  {hero} starts in {room}\n  {hero} can reach {room}\n  {door} leads from {here} to {there}"
    );

    let unmatched = DUNGEON.replace("Ash starts in Entrance", "Nobody wanders");
    assert_eq!(
        message(&unmatched),
        "this doesn't match any declared sentence shape"
    );

    let ambiguous = DUNGEON.replace(
        "static {hero: Hero} can reach",
        "static arrives: {hero: Hero} can reach {room: Room}\nstatic {hero: Hero} can reach",
    ) + "Ash can reach Hall\n";
    assert_eq!(
        message(&ambiguous),
        "this can be read more than one way:\n  as arrives: {hero} can reach {room}\n  as can-reach: {hero} can reach {room}"
    );
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
    assert_eq!(parsed.facts.len(), 21);
    assert!(parsed.warnings.is_empty());

    let twice = format!("{TAGS}\n{host}whiterabbit runs darwin\n");
    assert_eq!(
        message(&twice),
        "these facts break `exactly one platform per host`:\n  line 24: whiterabbit runs linux
  line 34: whiterabbit runs darwin"
    );
}

#[test]
fn spec_5_4_and_5_5_effect_and_domain_lines() {
    let source = "\
type Turn, North
type TurnStatus = \"ok\" | \"failed\"
effect send: send {prompt: Text} to Codex for {turn: Turn}
  at most one per step
  settled by finished
event finished: Codex finished {turn: Turn} with {status: TurnStatus}
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
    assert_eq!(shapes[3].domains[0].values.len(), 3);
}

#[test]
fn review_2_8_list_slots_and_shape_lines_lose_nothing() {
    let visits = "type Hero, Room\nstatic {hero: Hero} visits {room: Room}\nAsh is a Hero\nHall is a Room\n";
    let fails = |last: &str| {
        let e = read_sentences(&format!("{visits}{last}\n")).unwrap_err();
        (e.line, e.message)
    };
    assert_eq!(
        fails("Ash visits [?room]"),
        (5, "`[?room]`: this line states a fact, so it can't use variables".into())
    );
    assert_eq!(fails("Ash visits [?_]").0, 5);
    assert_eq!(fails("Ash visits ?room").0, 5);
    assert_eq!(
        fails("Ash visits [visits]"),
        (5, "`visits` is a template word; write the name as `` `visits` ``".into())
    );
    assert_eq!(fails("Ash visits visits").0, 5);
    for ground in ["Ash visits Hall", "Ash visits [Hall]", "Ash visits [`visits`]"] {
        assert_eq!(read_sentences(&format!("{visits}{ground}\n")).unwrap().facts.len(), 1);
    }
    let nested = "type Hero, Room\nstatic {hero: Hero} visits {room: Room}\n  at most one room per hero\n    garbage lost\n";
    let e = read_sentences(nested).unwrap_err();
    assert_eq!(e.line, 4);
    assert!(e.message.starts_with("a shape line takes no indented block"));
}
