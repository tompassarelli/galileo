#[path = "support/north_slice.rs"]
mod north_slice;

use galileo_core::Value;
use galileo_import::clause_checkpoint;
use galileo_runtime::Session;
use std::collections::BTreeSet;

const RECORDING: &str = "tests/external/north-slices/usage_display_distinguishes_missing_values_and_preserves_conversation_scope.jsonl";

fn fixture() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(RECORDING);
    let input = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let record: serde_json::Value = input
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|record| record["kind"] == "projection" && record["transaction"] == 10)
        .expect("transaction 10's projection");
    let hex = record["projection_hex"].as_str().unwrap();
    let mut term: Vec<u8> = (0..hex.len()).step_by(2).map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap()).collect();
    let old = 99.0f64.to_le_bytes();
    let at: Vec<usize> = term.windows(8).enumerate().filter(|(_, window)| *window == old).map(|(at, _)| at).collect();
    assert_eq!(at.len(), 1, "the F64 99.0 occurs once");
    term[at[0]..at[0] + 8].copy_from_slice(&12.5f64.to_le_bytes());
    term
}

#[test]
fn cutover_drops_usage_rows_and_the_next_report_recreates_them() {
    let s3 = north_slice::program("examples/north/s3");
    let term = fixture();

    let error = clause_checkpoint::convert(&s3, &term, &[]).unwrap_err();
    assert!(error.contains("UsageRow.value") && error.contains("12.5"), "{error}");
    let error = clause_checkpoint::convert(&s3, &term, &["UsageRows"]).unwrap_err();
    assert!(error.contains("UsageRows"), "{error}");

    let converted = clause_checkpoint::convert(&s3, &term, &["UsageRow"]).unwrap();
    let names_usage_row = |value: &Value| matches!(value, Value::Entity(entity) if entity.entity_type == "UsageRow");
    assert!(!converted.state.live.iter().any(|entity| entity.entity_type == "UsageRow"));
    assert!(!converted.state.facts.iter().any(|(_, values)| values.iter().any(names_usage_row)));
    let usage: Vec<(&str, usize)> = converted
        .dropped
        .iter()
        .filter(|(table, _)| *table == "known-usage-row" || table.starts_with("usage-row-"))
        .map(|(table, rows)| (table.as_str(), *rows))
        .collect();
    eprintln!("usage tables dropped: {usage:?}");
    assert_eq!(usage.len(), 9, "{usage:?}");
    assert!(usage.iter().all(|(_, rows)| *rows == 11), "{usage:?}");

    let north = Value::Entity(s3.entities.iter().find(|entity| entity.name == "north-main").unwrap().clone());
    let text = |text: &str| Value::Text(text.into());
    let usage_empty = s3.relation_id("North.usage-empty").unwrap();
    assert!(converted.state.facts.contains(&(usage_empty, vec![north.clone(), text("Usage has not been reported yet.")])));
    let mut session = Session::converted(&s3, converted.state);
    let event = |name: &str, arguments: Vec<Value>| {
        let mut values = vec![north.clone()];
        values.extend(arguments);
        (s3.relation_id(name).unwrap_or_else(|| panic!("event {name}")), values)
    };
    let events = [
        event("withdraw-usage-row", vec![text("other"), text("context")]),
        event("withdraw-usage-row", vec![text("other"), text("tokens")]),
        event("observe-context-usage", vec![text("other"), Value::Int(990), Value::Int(1000), Value::Int(9999)]),
    ];
    session.run_all(&events).unwrap();
    let state = session.state();
    let rows: Vec<_> = state.live.iter().filter(|entity| entity.entity_type == "UsageRow").collect();
    assert_eq!(rows.len(), 2);
    let field = |name: &str| s3.relation_id(name).unwrap_or_else(|| panic!("relation {name}"));
    let (owner, value) = (field("UsageRow.owner"), field("UsageRow.value"));
    let mut values = BTreeSet::new();
    for row in rows {
        let row = Value::Entity(row.clone());
        assert!(state.facts.contains(&(owner, vec![row.clone(), text("other")])));
        for (relation, fact) in &state.facts {
            if *relation == value && fact[0] == row {
                values.insert(fact[1].clone());
            }
        }
    }
    assert_eq!(values, BTreeSet::from([Value::Int(99), Value::Int(9999)]));
}
