use galileo_core::why::{sentence, why, Proof};
use galileo_core::{check_claims, engine, evaluate, Budget, EvalError, Program, Reason, Value, Verdict};
use galileo_syntax::lower::lower;
use std::path::{Path, PathBuf};
use std::process::Command;

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
linux allows every module
whiterabbit enables [terminal, cli-tools, theming, auth, javascript, python, rust]
whiterabbit enables browsers
whiterabbit adds [chrome, qutebrowser, zen-browser] to browsers
whiterabbit enables desktop
whiterabbit drops mako from desktop
whiterabbit adds river to desktop
whiterabbit disables [anytype, bun, cargo, clang]
";

const SECOND_TAG: &str = "
firefox is tagged desktop
desktop sets myConfig.modules.firefox.default to false for firefox
";

const NORTH: &str = "
type North
given {north: North} has item {item: Int}
derived {north: North} has menu count {count: Int}
  exactly one count per north

rule menu-count: ?north has menu count 1
  when ?north has item ?_

alpha is a North
beta is a North
alpha has item 7
";

const THE: &str = "
type Key
given {key: Key} has raw {value: Int}
derived {key: Key} has value {value: Int}
  at most one value per key
derived {key: Key} reads {value: Int}

rule copy: ?key has value ?v
  when ?key has raw ?v

rule pick: ?key reads ?v
  when ?key has raw ?_
  and ?v = the ?x where ?key has value ?x

k is a Key
k has raw 1
k has raw 2
";

fn tags() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib/firn/tags.gal");
    std::fs::read_to_string(path).unwrap()
}

fn write(name: &str, source: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("mode-checks-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("case.gal"), source).unwrap();
    dir
}

fn check(name: &str, source: &str) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).arg("check").arg(write(name, source)).output().unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    (out.status.code().unwrap(), text)
}

fn same_failure(source: &str) -> Vec<galileo_core::modes::ModeFailure> {
    let lowered = lower(source, "case.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let reference = evaluate(&lowered.program, &[], Budget::default());
    let fast = engine::evaluate(&lowered.program, &[], Budget::default());
    assert_eq!(reference, fast);
    match reference {
        Err(EvalError::Mode(failures)) => failures,
        other => panic!("expected a mode failure, got {other:?}"),
    }
}

fn program(source: &str) -> Program {
    let lowered = lower(source, "case.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    lowered.program
}

fn stated_leaves(program: &Program, relation: galileo_core::RelationId, tuple: &[Value]) -> Vec<String> {
    fn walk(program: &Program, proof: &Proof, out: &mut Vec<String>) {
        match proof {
            Proof::Stated { fact } => out.push(sentence(program, fact.relation, &fact.values)),
            Proof::Derived { premises, .. } => premises.iter().for_each(|p| walk(program, p, out)),
            _ => {}
        }
    }
    let Ok(Verdict::Holds(proof)) = why(program, &[], relation, tuple, Budget::default()) else { panic!("{tuple:?} holds") };
    let mut out = Vec::new();
    walk(program, &proof, &mut out);
    out
}

#[test]
fn a_second_tag_setting_the_same_path_fails_the_mode_with_both_proofs() {
    let base = format!("{}{WHITERABBIT}", tags());
    let (code, text) = check("tags-base", &base);
    assert_eq!(code, 1, "{text}");
    let base_program = program(&base);
    assert!(evaluate(&base_program, &[], Budget::default()).is_ok());
    let results = check_claims(&base_program, Budget::default()).unwrap();
    assert_eq!(results.iter().filter(|r| matches!(r.verdict, Verdict::Fails(_))).count(), 1, "{results:?}");

    let conflicting = format!("{base}{SECOND_TAG}");
    let (code, text) = check("tags-conflict", &conflicting);
    assert_eq!(code, 1, "{text}");
    let true_line = "whiterabbit gets myConfig.modules.firefox.default set to true by firefox";
    let false_line = "whiterabbit gets myConfig.modules.firefox.default set to false by firefox";
    assert!(text.contains(true_line) && text.contains(false_line), "{text}");
    let conflicted = program(&conflicting);
    let [failure] = same_failure(&conflicting).try_into().unwrap();
    assert_eq!(conflicted.relations[failure.relation.0].template, "{host} gets {path} set to {value} by {module}");
    assert!(matches!(failure.key.first(), Some(Value::Entity(host)) if host.name == "whiterabbit"), "{failure:?}");
    let tuples: Vec<_> = failure.tuples.iter().map(|t| sentence(&conflicted, failure.relation, t)).collect();
    assert_eq!(tuples, [false_line, true_line]);
    for (tuple, setting) in failure.tuples.iter().zip(["desktop sets myConfig.modules.firefox.default to false for firefox", "browsers sets myConfig.modules.firefox.default to true for firefox"]) {
        let leaves = stated_leaves(&conflicted, failure.relation, tuple);
        assert!(leaves.iter().any(|leaf| leaf == setting), "{setting}: {leaves:?}");
    }
    let results = check_claims(&conflicted, Budget::default()).unwrap();
    let unknown = results.iter().filter(|r| matches!(&r.verdict, Verdict::Unknown(Reason::ModeFails { conflict }) if conflict.contains("whiterabbit"))).count();
    assert_eq!(unknown, 2, "{results:?}");
}

#[test]
fn an_exactly_one_north_without_a_count_fails_naming_it() {
    let (code, text) = check("north", NORTH);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("beta") && !text.contains("alpha"), "{text}");
    let [failure] = same_failure(NORTH).try_into().unwrap();
    assert!(matches!(failure.key.as_slice(), [Value::Entity(north)] if north.name == "beta"), "{failure:?}");
    assert!(failure.tuples.is_empty(), "{failure:?}");
}

#[test]
fn a_the_over_a_conflicting_relation_yields_the_mode_failure() {
    let failures = same_failure(THE);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].tuples.len(), 2);
}
