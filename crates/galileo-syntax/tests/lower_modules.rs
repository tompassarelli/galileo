use galileo_core::{evaluate, Budget, Model, Program, Value};
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
        .map(|(i, r)| {
            let tuples = model
                .tuples(galileo_core::RelationId(i))
                .map(|tuple| {
                    tuple
                        .iter()
                        .map(|value| match value {
                            Value::Entity(entity) => Value::entity(
                                entity.entity_type.rsplit('/').next().unwrap(),
                                entity.name.clone(),
                            ),
                            value => value.clone(),
                        })
                        .collect()
                })
                .collect();
            (r.template.clone(), tuples)
        })
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
fn a_second_at_most_one_line_is_enforced() {
    let error = lower_program(&[(
        "pairs",
        "type A, B\ngiven {a: A} pairs with {b: B}\n  at most one b per a\n  at most one a per b\n\nx is a A\ny is a A\nq is a B\nx pairs with q\ny pairs with q\n",
    )])
    .unwrap_err();
    assert_eq!((error.line, error.column), (10, 1), "{error:?}");
    assert!(error.message.contains("break `at most one a per b`"), "{error:?}");
}

const MENU: &str = include_str!("menu.gal");

#[test]
fn spec_9_7_the_menu_links_with_north_core() {
    assert!(MENU.starts_with("use north/core\n") && !MENU.contains("type North"));
    let linked = lower_program(&[("menu", MENU), ("north/core", "type North\n")]).unwrap();
    assert!(linked.diagnostics.is_empty(), "{:#?}", linked.diagnostics);
    assert!(linked.program.entity_types.iter().any(|t| t.name == "north/core/North"));
    let stub = linked.readings.iter().find(|r| r.shape == "is-a-North" && r.file == "north/core");
    assert!(stub.is_some(), "{:#?}", linked.readings.iter().map(|r| (&r.file, r.line, &r.shape)).collect::<Vec<_>>());
}

#[test]
fn decisions_d_41_a_call_resolves_to_its_module_or_what_it_uses() {
    let wrap = "define wrap(index: Int, count: Int): Int\n  index\n";
    let call = "define step(index: Int): Int\n  wrap(index, 3)\n";
    let errors = |modules: &[(&str, &str)]| {
        lower_program(modules).unwrap().diagnostics.iter().map(|e| e.message.clone()).collect::<Vec<_>>()
    };
    assert_eq!(errors(&[("core", wrap), ("menu", &format!("use core\n\n{call}"))]), Vec::<String>::new());
    assert_eq!(
        errors(&[("core", wrap), ("menu", call)]),
        ["menu: `wrap` is defined by core, which this module does not use"]
    );
    assert_eq!(
        errors(&[("a", wrap), ("b", wrap), ("menu", &format!("use b\nuse a\n\n{call}"))]),
        ["menu: `wrap` is defined by a/wrap and b/wrap"]
    );
    assert_eq!(
        errors(&[("core", &format!("private {wrap}")), ("menu", &format!("use core\n\n{call}"))]),
        ["menu: `wrap` is private to core"]
    );
}
