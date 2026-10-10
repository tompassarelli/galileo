use muove_core::{evaluate, examples, Budget, Model, Program, Value};
use muove_syntax::lower::lower;
use std::collections::BTreeMap;

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

rule enabled-by-default
  ?module is enabled on ?host by default through ?tag
  when
    ?host enables ?tag
    ?module is tagged ?tag
    not ?host drops ?module from ?tag

rule default-membership-enables
  ?module is enabled on ?host by ?tag
  when
    ?module is enabled on ?host by default through ?tag

rule enabled-by-opt-in
  ?module is enabled on ?host by ?tag
  when
    ?host enables ?tag
    ?host adds ?module to ?tag
    ?module can opt into ?tag

rule enabled
  ?module is enabled on ?host
  when
    ?module is enabled on ?host by ?tag
    not ?host disables ?module

rule tag-overrides
  ?host gets ?path set to ?value by ?module
  when
    ?module is enabled on ?host
    ?module is enabled on ?host by default through ?tag
    ?tag sets ?path to ?value for ?module

requirements for tags
  every ?host, ?module, ?tag where ?host adds ?module to ?tag: ?module can opt into ?tag
  every ?module, ?host, ?platform where ?module is enabled on ?host and ?host runs ?platform: ?module supports ?platform
";

const WHITERABBIT: &str = "
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
        && !["type ", "rule ", "when", "exactly ", "at most ", "requirements ", "every "]
            .iter()
            .any(|p| text.starts_with(p))
}

#[test]
fn spec_9_6_tags_lowers_with_a_reading_for_every_sentence() {
    let lowered = lower(TAGS, "lib/firn/tags.muove").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    assert_eq!(lowered.program.relations.len(), 13);
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
            let id = muove_core::RelationId(i);
            (r.template.clone(), model.tuples(id).cloned().collect())
        })
        .collect()
}

#[test]
fn lowered_tags_and_whiterabbit_evaluate_to_the_hand_built_reference_model() {
    let source = format!("{TAGS}{WHITERABBIT}");
    let lowered = lower(&source, "whiterabbit.muove").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let model = evaluate(&lowered.program, &[], Budget::default()).unwrap();
    let reference = examples::tags();
    let expected = evaluate(&reference, &[], Budget::default()).unwrap();
    assert_eq!(by_template(&model, &lowered.program), by_template(&expected, &reference));
}

fn tuples(source: &str, template: &str) -> Vec<Vec<Value>> {
    let lowered = lower(source, "case.muove").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{source}\n{:#?}", lowered.diagnostics);
    let model = evaluate(&lowered.program, &[], Budget::default()).unwrap();
    by_template(&model, &lowered.program).remove(template).unwrap()
}

#[test]
fn sentence_fillers_lower_to_their_source_meaning() {
    let anonymous = "static edge: {from: Int} connects {to: Int}\nderived seen: {value: Int} was seen\n1 connects 2\nrule any-edge\n  9 was seen\n  when\n    ?_ connects ?_\n";
    assert_eq!(tuples(anonymous, "{value} was seen"), [[Value::Int(9)]]);
    let bare = "type Host\nstatic link: {from: Host} links to {to: Host}\nderived isolated: {host: Host} is isolated\nrule isolate\n  ?h is isolated\n  when\n    ?h is a Host\n    not ?h links to _\n";
    let error = lower(bare, "underscore.muove").unwrap_err();
    assert_eq!(error.line, 8);
    assert_eq!(
        error.message,
        "a bare `_` isn't a filler; write `?_` for an anonymous variable"
    );
    let escaped = "static words: {value: Text} has text\n\"line\\nnext\" has text\n";
    assert_eq!(tuples(escaped, "{value} has text"), [[Value::Text("line\nnext".into())]]);
    let computed = "static answer: {value: Int} answers\n(1 + 2) answers\n";
    assert_eq!(tuples(computed, "{value} answers"), [[Value::Int(3)]]);

    let clash = "static seed: {value: Int} begins\nderived result: {value: Int} finishes\n3 begins\nrule clash\n  ?x finishes\n  when\n    ?x begins\n    ?rows = all ?x where ?x begins\n";
    let messages: Vec<String> = lower(clash, "clash.muove").unwrap().diagnostics.iter().map(|e| e.to_string()).collect();
    assert!(
        messages.iter().any(|m| m.contains("`?x`") && m.contains("line 8") && m.contains("line 5")),
        "{messages:#?}"
    );
    let quoted = "static words: {value: Text} has text\nderived copied: {value: Text} was copied\n\"a\" has text\nrule copy\n  \"{all ?x where ?x has text}\" was copied\n  when\n    ?x has text\n";
    let messages: Vec<String> = lower(quoted, "quoted.muove").unwrap().diagnostics.iter().map(|e| e.to_string()).collect();
    assert!(
        messages.iter().any(|m| m.contains("`?x` is declared by the query on line 5, but also appears outside it on line 7")),
        "{messages:#?}"
    );

    let entity = lower("type Hero\nstatic ready: {hero: Hero} is ready\nAsh is a Hero\nAsh is ready\n", "e.muove").unwrap();
    let reading = entity.readings.iter().find(|r| r.line == 3).expect("a reading for `Ash is a Hero`");
    assert_eq!((reading.shape.as_str(), reading.slots[0].filler.as_deref()), ("Hero", Some("Ash")));
}
