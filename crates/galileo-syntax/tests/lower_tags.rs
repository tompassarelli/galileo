use galileo_core::{evaluate, examples, Atom, Budget, Expr, Model, Program, RelationId, Value};
use std::collections::BTreeSet;
use galileo_syntax::lower::{lower, lower_program};
use std::collections::BTreeMap;

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

rule enabled-by-default: ?module is enabled on ?host by default through ?tag
  when ?host enables ?tag
  and ?module is tagged ?tag
  and not ?host drops ?module from ?tag

rule default-membership-enables: ?module is enabled on ?host by ?tag
  when ?module is enabled on ?host by default through ?tag

rule enabled-by-opt-in: ?module is enabled on ?host by ?tag
  when ?host enables ?tag
  and ?host adds ?module to ?tag
  and ?module can opt into ?tag

rule enabled: ?module is enabled on ?host
  when ?module is enabled on ?host by ?tag
  and not ?host disables ?module

rule tag-overrides: ?host gets ?path set to ?value by ?module
  when ?module is enabled on ?host
  and ?module is enabled on ?host by default through ?tag
  and ?tag sets ?path to ?value for ?module

requirements for tags
  every ?host, ?module, ?tag where ?host adds ?module to ?tag: ?module can opt into ?tag
  every ?module, ?host, ?platform where ?module is enabled on ?host and ?host runs ?platform: ?module supports ?platform
";

const WHITERABBIT: &str = "
firefox is a Module
chrome is a Module
qutebrowser is a Module
zen-browser is a Module
mako is a Module
river is a Module
anytype is a Module
bun is a Module
cargo is a Module
clang is a Module
browsers is a Tag
terminal is a Tag
cli-tools is a Tag
theming is a Tag
auth is a Tag
javascript is a Tag
python is a Tag
rust is a Tag
desktop is a Tag
linux is a Platform

firefox is tagged browsers
browsers sets myConfig.modules.firefox.default to true for firefox

whiterabbit is a Host
whiterabbit runs linux
whiterabbit enables [terminal, cli-tools, theming, auth, javascript, python, rust]
whiterabbit enables browsers
whiterabbit adds [chrome, qutebrowser, zen-browser] to browsers
whiterabbit enables desktop
whiterabbit drops mako from desktop
whiterabbit adds river to desktop
whiterabbit disables [anytype, bun, cargo, clang]
";

fn is_sentence_line(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty()
        && !["type ", "exactly ", "at most ", "requirements ", "every "]
            .iter()
            .any(|p| text.starts_with(p))
}

#[test]
fn spec_9_6_tags_lowers_with_a_reading_for_every_sentence() {
    let mut rules = lower(include_str!("../../../lib/firn/tags.gal"), "firn/tags").unwrap().program.rules;
    for rule in &mut rules {
        rule.source = None;
        rule.head.source = None;
        for literal in &mut rule.body {
            match literal {
                galileo_core::Literal::Positive(atom) | galileo_core::Literal::Negative(atom) => atom.source = None,
                other => panic!("unexpected tags literal: {other:?}"),
            }
        }
    }
    assert_eq!(rules, vec![
        galileo_core::Rule::new("enabled-by-default", Atom::new(RelationId(13), [Expr::var("module"), Expr::var("host"), Expr::var("tag")])).when([
            Atom::new(RelationId(2), [Expr::var("host"), Expr::var("tag")]).positive(),
            Atom::new(RelationId(0), [Expr::var("module"), Expr::var("tag")]).positive(),
            Atom::new(RelationId(3), [Expr::var("host"), Expr::var("module"), Expr::var("tag")]).negative(),
        ]),
        galileo_core::Rule::new("default-membership-enables", Atom::new(RelationId(12), [Expr::var("module"), Expr::var("host"), Expr::var("tag")])).when([
            Atom::new(RelationId(13), [Expr::var("module"), Expr::var("host"), Expr::var("tag")]).positive(),
        ]),
        galileo_core::Rule::new("enabled-by-opt-in", Atom::new(RelationId(12), [Expr::var("module"), Expr::var("host"), Expr::var("tag")])).when([
            Atom::new(RelationId(2), [Expr::var("host"), Expr::var("tag")]).positive(),
            Atom::new(RelationId(4), [Expr::var("host"), Expr::var("module"), Expr::var("tag")]).positive(),
            Atom::new(RelationId(1), [Expr::var("module"), Expr::var("tag")]).positive(),
        ]),
        galileo_core::Rule::new("supported-everywhere", Atom::new(RelationId(11), [Expr::var("module"), Expr::var("host")])).when([
            Atom::new(RelationId(18), [Expr::var("module")]).positive(),
            Atom::new(RelationId(6), [Expr::var("host"), Expr::var("platform")]).positive(),
            Atom::new(RelationId(8), [Expr::var("platform")]).positive(),
        ]),
        galileo_core::Rule::new("supported-by-platform", Atom::new(RelationId(11), [Expr::var("module"), Expr::var("host")])).when([
            Atom::new(RelationId(6), [Expr::var("host"), Expr::var("platform")]).positive(),
            Atom::new(RelationId(7), [Expr::var("module"), Expr::var("platform")]).positive(),
        ]),
        galileo_core::Rule::new("enabled", Atom::new(RelationId(14), [Expr::var("module"), Expr::var("host")])).when([
            Atom::new(RelationId(12), [Expr::var("module"), Expr::var("host"), Expr::var("tag")]).positive(),
            Atom::new(RelationId(11), [Expr::var("module"), Expr::var("host")]).positive(),
            Atom::new(RelationId(5), [Expr::var("host"), Expr::var("module")]).negative(),
        ]),
        galileo_core::Rule::new("flake-input-url", Atom::new(RelationId(16), [Expr::var("name"), Expr::var("url")])).when([
            Atom::new(RelationId(10), [Expr::var("module"), Expr::var("name"), Expr::var("url")]).positive(),
        ]),
        galileo_core::Rule::new("tag-overrides", Atom::new(RelationId(15), [Expr::var("host"), Expr::var("path"), Expr::var("value"), Expr::var("module")])).when([
            Atom::new(RelationId(14), [Expr::var("module"), Expr::var("host")]).positive(),
            Atom::new(RelationId(13), [Expr::var("module"), Expr::var("host"), Expr::var("tag")]).positive(),
            Atom::new(RelationId(9), [Expr::var("tag"), Expr::var("path"), Expr::var("value"), Expr::var("module")]).positive(),
        ]),
    ]);

    let lowered = lower(TAGS, "lib/firn/tags.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    assert_eq!(lowered.program.relations.len(), 17);
    assert_eq!(lowered.program.rules.len(), 5);
    let read: Vec<usize> = lowered.readings.iter().map(|r| r.line).collect();
    let requirements = TAGS.lines().position(|l| l.starts_with("requirements")).unwrap();
    for (index, text) in TAGS.lines().enumerate().take(requirements) {
        if is_sentence_line(text) {
            assert!(read.contains(&(index + 1)), "no reading for line {}: {text}", index + 1);
        }
    }
}

fn by_template(model: &Model, program: &Program) -> BTreeMap<String, Vec<Vec<Value>>> {
    program
        .relations
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let id = galileo_core::RelationId(i);
            (r.template.clone(), model.tuples(id).cloned().collect())
        })
        .collect()
}

#[test]
fn lowered_tags_and_whiterabbit_evaluate_to_the_hand_built_reference_model() {
    let source = format!("{TAGS}{WHITERABBIT}");
    let lowered = lower(&source, "whiterabbit.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let model = evaluate(&lowered.program, &[], Budget::default()).unwrap();
    let reference = examples::tags();
    let expected = evaluate(&reference, &[], Budget::default()).unwrap();
    let expected = by_template(&expected, &reference);
    let mut lowered = by_template(&model, &lowered.program);
    lowered.retain(|template, _| expected.contains_key(template));
    assert_eq!(lowered, expected);
}

fn tuples(source: &str, template: &str) -> Vec<Vec<Value>> {
    let lowered = lower(source, "case.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{source}\n{:#?}", lowered.diagnostics);
    let model = evaluate(&lowered.program, &[], Budget::default()).unwrap();
    by_template(&model, &lowered.program).remove(template).unwrap()
}

#[test]
fn sentence_fillers_lower_to_their_source_meaning() {
    let anonymous = "given edge: {from: Int} connects {to: Int}\nderived seen: {value: Int} was seen\n1 connects 2\nrule any-edge: 9 was seen\n  when ?_ connects ?_\n";
    assert_eq!(tuples(anonymous, "{value} was seen"), [[Value::Int(9)]]);
    let bare = "type Host\ngiven link: {from: Host} links to {to: Host}\nderived isolated: {host: Host} is isolated\nrule isolate: ?h is isolated\n  when ?h is a Host\n  and not ?h links to _\n";
    let error = lower(bare, "underscore.gal").unwrap_err();
    assert_eq!((error.line, error.column), (6, 3));
    assert!(error.message.contains("`_`") && error.message.contains("`?_`"), "{error}");
    let escaped = "given words: {value: Text} has text\n\"line\\nnext\" has text\n";
    assert_eq!(tuples(escaped, "{value} has text"), [[Value::Text("line\nnext".into())]]);
    let computed = "given answer: {value: Int} answers\n(1 + 2) answers\n";
    assert_eq!(tuples(computed, "{value} answers"), [[Value::Int(3)]]);

    let clash = "given seed: {value: Int} begins\nderived result: {value: Int} finishes\n3 begins\nrule clash: ?x finishes\n  when ?x begins\n  and ?rows = all ?x where ?x begins\n";
    let diagnostics = lower(clash, "clash.gal").unwrap().diagnostics;
    assert!(
        diagnostics.iter().any(|d| (d.line, d.column) == (6, 3) && d.message.contains("`?x`") && d.message.contains("line 6") && d.message.contains("line 4")),
        "{diagnostics:#?}"
    );
    let quoted = "given words: {value: Text} has text\nderived copied: {value: Text} was copied\n\"a\" has text\nrule copy: \"{all ?x where ?x has text}\" was copied\n  when ?x has text\n";
    let diagnostics = lower(quoted, "quoted.gal").unwrap().diagnostics;
    assert!(
        diagnostics.iter().any(|d| (d.line, d.column) == (4, 1) && d.message.contains("`?x`") && d.message.contains("line 4") && d.message.contains("line 5")),
        "{diagnostics:#?}"
    );

    let entity = lower("type Hero\ngiven ready: {hero: Hero} is ready\nAsh is a Hero\nAsh is ready\n", "e.gal").unwrap();
    let reading = entity.readings.iter().find(|r| r.line == 3).expect("a reading for `Ash is a Hero`");
    assert_eq!((reading.shape.as_str(), reading.slots[0].filler.as_deref()), ("is-a-Hero", Some("Ash")));
}

fn names(source: &str, template: &str) -> BTreeSet<String> {
    tuples(source, template)
        .into_iter()
        .map(|t| match &t[..] {
            [Value::Entity(e)] => e.name.clone(),
            other => panic!("{other:?}"),
        })
        .collect()
}

#[test]
fn an_entity_exists_only_by_its_declaration() {
    let firefox = "use firn/tags\nfirefox is a Module\n";
    let host = "use firn/tags\nwhiterabbit is a Host\nwhiterabbit disables firefx\n";
    let lowered = lower_program(&[("firn/tags", TAGS), ("modules/firefox", firefox), ("hosts/whiterabbit", host)]).unwrap();
    assert_eq!(lowered.diagnostics.len(), 1, "{:#?}", lowered.diagnostics);
    assert_eq!((lowered.diagnostics[0].line, lowered.diagnostics[0].column), (3, 1));
    assert!(
        lowered.diagnostics[0].message.ends_with("`firefx` isn't a declared Module; the nearest is `firefox`, and `firefx is a Module` declares a new one"),
        "{}",
        lowered.diagnostics[0].message
    );

    let dungeon = include_str!("dungeon.gal").replace("  never Ash is in Shrine\n", "  never Ash is in Shrine\n  never Ash is in Shirne\n");
    let lowered = lower(&dungeon, "dungeon.gal").unwrap();
    assert_eq!(lowered.diagnostics.len(), 1, "{:#?}", lowered.diagnostics);
    assert_eq!((lowered.diagnostics[0].line, lowered.diagnostics[0].column), (113, 3));
    assert!(lowered.diagnostics[0].message.contains("`Shirne`") && lowered.diagnostics[0].message.contains("`Shrine`"));

    let both = format!("{TAGS}python is a Module\npython is a Tag\n");
    assert!(names(&both, "{entity} is a Module").contains("python"));
    assert!(names(&both, "{entity} is a Tag").contains("python"));
    let twice = lower(&format!("{both}python is a Module\n"), "twice.gal").unwrap();
    assert_eq!(twice.warnings.len(), 1, "{:#?}", twice.warnings);
}

#[test]
fn the_corpus_names_only_declared_entities() {
    let corpus = [
        ("dungeon", include_str!("dungeon.gal")),
        ("menu", include_str!("menu.gal")),
        ("dispatch", include_str!("../../../lib/firn/dispatch.gal")),
    ];
    for (name, source) in corpus {
        let lowered = lower(source, name).unwrap();
        assert!(lowered.diagnostics.is_empty(), "{name}: {:#?}", lowered.diagnostics);
    }
    let kanata = lower_program(&[
        ("default", include_str!("../../../tests/external/kanata/default.gal")),
        ("kanata", include_str!("../../../tests/external/kanata/kanata.gal")),
    ])
    .unwrap();
    assert!(kanata.diagnostics.is_empty(), "kanata: {:#?}", kanata.diagnostics);
}
