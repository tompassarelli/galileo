use galileo_core::{evaluate, examples, Budget, Model, Program, Value};
use galileo_syntax::lower::lower;
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
    let escaped = "given words: {value: Text} has text\n\"line\\nnext\" has text\n";
    assert_eq!(tuples(escaped, "{value} has text"), [[Value::Text("line\nnext".into())]]);
    let computed = "given answer: {value: Int} answers\n(1 + 2) answers\n";
    assert_eq!(tuples(computed, "{value} answers"), [[Value::Int(3)]]);

    let entity = lower("type Hero\ngiven ready: {hero: Hero} is ready\nAsh is a Hero\nAsh is ready\n", "e.gal").unwrap();
    let reading = entity.readings.iter().find(|r| r.line == 3).expect("a reading for `Ash is a Hero`");
    assert_eq!((reading.shape.as_str(), reading.slots[0].filler.as_deref()), ("is-a-Hero", Some("Ash")));
}
