use galileo_core::{evaluate, Budget, Mode, Model, Program, Value};
use galileo_syntax::lower::{lower, lower_program};
use std::collections::BTreeMap;

const TAGS: &str = include_str!("../../../lib/firn/tags.gal");

const FIREFOX: &str = "\
use firn/tags

firefox is a Module
browsers is a Tag
firefox is tagged browsers
browsers sets myConfig.modules.firefox.default to true for firefox
";

const WHITERABBIT: &str = "\
use firn/tags

chrome is a Module
qutebrowser is a Module
zen-browser is a Module
mako is a Module
river is a Module
anytype is a Module
bun is a Module
cargo is a Module
clang is a Module
terminal is a Tag
cli-tools is a Tag
theming is a Tag
auth is a Tag
javascript is a Tag
python is a Tag
rust is a Tag
desktop is a Tag
linux is a Platform
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

fn by_template(model: &Model, program: &Program) -> BTreeMap<String, Vec<Vec<Value>>> {
    program
        .relations
        .iter()
        .enumerate()
        .map(|(i, r)| (r.template.clone(), model.tuples(galileo_core::RelationId(i)).cloned().collect()))
        .collect()
}

#[test]
fn spec_9_6_modules_link_into_the_single_module_model() {
    let linked = lower_program(&[
        ("modules/firefox/default", FIREFOX),
        ("hosts/whiterabbit/tags", WHITERABBIT),
        ("firn/tags", TAGS),
    ])
    .unwrap();
    assert!(linked.diagnostics.is_empty(), "{:#?}", linked.diagnostics);
    assert!(
        linked.program.relations.iter().all(|r| r.name.starts_with("firn/tags/")),
        "{:?}",
        linked.program.relations.iter().map(|r| &r.name).collect::<Vec<_>>()
    );
    let strip = |text: &str| text.replace("use firn/tags\n", "");
    let single = lower(&format!("{TAGS}{}{}", strip(FIREFOX), strip(WHITERABBIT)), "single.gal").unwrap();
    assert!(single.diagnostics.is_empty(), "{:#?}", single.diagnostics);
    let model = evaluate(&linked.program, &[], Budget::default()).unwrap();
    let expected = evaluate(&single.program, &[], Budget::default()).unwrap();
    assert_eq!(by_template(&model, &linked.program), by_template(&expected, &single.program));
}

#[test]
fn a_sentence_resolves_within_its_module_and_what_it_uses() {
    let linked = lower_program(&[
        ("level", "type Hero, Room\ngiven {hero: Hero} is in {room: Room}\n"),
        ("story", "type Hero, Chapter\ngiven {hero: Hero} is in {chapter: Chapter}\n"),
        ("game", "use level\n\nash is a Hero\nhall is a Room\nash is in hall\n"),
    ])
    .unwrap();
    assert!(linked.diagnostics.is_empty(), "{:#?}", linked.diagnostics);
    let mut names: Vec<&str> = linked.program.relations.iter().map(|r| r.name.as_str()).collect();
    names.sort();
    assert_eq!(
        names,
        ["level/is-a-Hero", "level/is-a-Room", "level/is-in", "story/is-a-Chapter", "story/is-a-Hero", "story/is-in"]
    );
    let level = ["level/is-a-Hero", "level/is-a-Room", "level/is-in"].map(|n| linked.program.relation_id(n).unwrap());
    let facts: Vec<_> = linked.program.facts.iter().map(|f| f.relation).collect();
    assert_eq!(facts, level);
}

#[test]
fn a_shape_keeps_every_mode_line() {
    let linked = lower_program(&[(
        "pairs",
        "type A, B\ngiven {a: A} pairs with {b: B}\n  at most one b per a\n  at most one a per b\n",
    )])
    .unwrap();
    assert!(linked.diagnostics.is_empty(), "{:#?}", linked.diagnostics);
    let modes = &linked.program.relations[0].modes;
    assert_eq!(modes.len(), 2);
    assert!(modes.iter().all(|m| matches!(m, Mode::AtMostOne { .. })));
}
