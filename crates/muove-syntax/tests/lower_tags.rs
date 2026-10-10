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
