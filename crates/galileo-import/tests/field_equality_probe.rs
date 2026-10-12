use galileo_core::state::{self, Run, State};
use galileo_core::*;

fn probe(label: &str, field_type: &str, initial: Option<&str>, rhs: &str, z: i64) {
    let member = if initial.is_some() { "box-a is a Box\n" } else { "" };
    let field_init = initial.map(|value| format!("  box-a\n    f = {value}\n")).unwrap_or_default();
    let targets = if field_type == "Target" { "type Target\ntarget-a is a Target\ntarget-b is a Target\n" } else { "" };
    let source = format!("type Host\nhost is a Host\ntype Box\n{member}{targets}\nstate for Host\n  done: Bool\nstate for Box\n  f: {field_type}\n\ninitially\n  host\n    done = false\n{field_init}\nevent go Host ?z: Int\non go ?h ?z\n  when ?x is a Box\n  and ?x.f == {rhs}\n  then ?h.done = true\n");
    std::fs::write(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../probes/{label}.gal")), &source).unwrap();
    let lowered = match galileo_syntax::lower::lower(&source, label) {
        Ok(lowered) => lowered,
        Err(error) => { eprintln!("CASE {label} LOWER ERROR {error:?}"); return; },
    };
    if !lowered.diagnostics.is_empty() { eprintln!("CASE {label} DIAGNOSTICS {:?}", lowered.diagnostics); return; }
    let program = lowered.program;
    check(&program).unwrap();
    let relation = program.field_relation("Box", "f").unwrap();
    eprintln!("CASE {label} FIELD {:?}", program.relations[relation.0]);
    let event = program.relation_id("go").unwrap();
    let args = [Value::entity("Host", "host"), Value::Int(z)];
    let reference = state::step(&program, &State::initial(&program), event, &args, Budget::default());
    let mut engine = Run::new(&program);
    let actual = engine.run(&program, event, &args, Budget::default()).map(|record| record.result.clone());
    match &reference {
        Ok(transition) => eprintln!("CASE {label} REFERENCE admitted effects={:?} facts={:?}", transition.effects, transition.state.facts),
        Err(error) => eprintln!("CASE {label} REFERENCE {error:?}"),
    }
    eprintln!("CASE {label} ENGINE {actual:?} facts={:?}", engine.state.facts);
    eprintln!("CASE {label} AGREES {}", match (&reference, &actual) { (Ok(t), Ok(Ok(_))) => t.state == engine.state, (Err(EvalError::Type { at, message }), Ok(Err(rejection))) if at == "step" => rejection.explanation == message.split('\n').map(str::to_string).collect::<Vec<_>>(), (Err(a), Err(b)) => a == b, _ => false });
}

#[test]
fn field_equality_probe() {
    for (label, ty, initial, rhs) in [
        ("int_equal", "Int", Some("1"), "1"),
        ("int_unequal", "Int", Some("1"), "2"),
        ("int_decimal", "Int", Some("1"), "1.0"),
        ("int_text", "Int", Some("1"), "\"1\""),
        ("int_null", "Int", Some("1"), "null"),
        ("null_equal", "maybe Int", Some("null"), "null"),
        ("null_int", "maybe Int", Some("null"), "1"),
        ("text_equal", "Text", Some("\"a\""), "\"a\""),
        ("text_unequal", "Text", Some("\"a\""), "\"b\""),
        ("entity_equal", "Target", Some("target-a"), "target-a"),
        ("entity_unequal", "Target", Some("target-a"), "target-b"),
        ("empty_divide", "Int", None, "1 / ?z"),
        ("full_divide", "Int", Some("1"), "1 / ?z"),
    ] { probe(label, ty, initial, rhs, 0); }
    probe("float_decl", "Float", Some("1"), "1", 0);
    probe("full_overflow", "Int", Some("1"), "?z + 1", i64::MAX);
    probe("empty_overflow", "Int", None, "?z + 1", i64::MAX);
}
