use galileo_core::RelationKind;
use galileo_runtime::{Program, Session};
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn north_menu_clause_reference_requires_every_recorded_occurrence() {
    let source = std::fs::read_to_string(
        "/home/tom/code/north/pins/eaee1838d1968b7caba65195866e5d52fd30b2e1/clause/north.clause",
    )
    .unwrap();
    let imported = galileo_import::clause_menu::import(&source).unwrap();
    let lowered = galileo_syntax::lower::lower(&imported.source, "north-menu.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let program = Program::from(lowered.program);
    let available: BTreeSet<_> = program
        .relations
        .iter()
        .filter(|relation| relation.kind == RelationKind::Event)
        .map(|relation| relation.name.as_str())
        .collect();
    let mut missing = Vec::new();
    let mut sessions = 0;
    let mut transactions = 0;
    let mut occurrences = 0;
    let mut skips = 0;
    let mut rejections = 0;
    let mut restores = 0;
    let mut evaluations = 0;
    let mut handlers = BTreeSet::new();
    let mut evaluated = BTreeSet::new();
    let mut selected = BTreeSet::new();
    for (sequence, input) in [
        (
            "history",
            include_str!("../../../tests/external/north-menu/history.jsonl"),
        ),
        (
            "restart",
            include_str!("../../../tests/external/north-menu/restart.jsonl"),
        ),
    ] {
        let session = Session::new(&program);
        assert_eq!(session.revision(), 0);
        let records: Vec<Value> = input
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        for record in &records {
            match record["kind"].as_str().unwrap() {
                "session" => sessions += 1,
                "transaction" if record["boundary"] == "begin" => transactions += 1,
                "occurrence" => {
                    occurrences += 1;
                    let handler = record["handler"].as_str().unwrap();
                    handlers.insert(handler.to_owned());
                    if !available.contains(handler) {
                        let observation = |kind: &str| {
                            records
                                .iter()
                                .find(|other| {
                                    other["kind"] == kind
                                        && other["session"] == record["session"]
                                        && other["transaction"] == record["transaction"]
                                        && other["ordinal"] == record["ordinal"]
                                })
                                .unwrap()
                        };
                        let decision = format!(
                            "DECISION 8.7: {sequence} session={} transaction={} ordinal={} handler={handler}; Clause admission={} rule_applied={} arguments={}; Galileo event=absent; exact atomic replay blocked before dispatch",
                            record["session"], record["transaction"], record["ordinal"],
                            observation("admission")["result"],
                            observation("evaluation")["rule_applied"], record["arguments"],
                        );
                        eprintln!("{decision}");
                        missing.push(decision);
                    }
                }
                "evaluation" => {
                    skips += usize::from(record["rule_applied"] == false);
                    for clause in record["clauses"].as_array().unwrap() {
                        evaluations += 1;
                        evaluated.insert(clause["rule"].as_u64().unwrap());
                        if clause["selected"] == true {
                            selected.insert(clause["rule"].as_u64().unwrap());
                        }
                    }
                }
                "admission" => rejections += usize::from(record["result"] != "admitted"),
                "continuation" if record["operation"] == "restore" => restores += 1,
                _ => (),
            }
        }
    }
    eprintln!(
        "North Clause reference census: sessions={sessions} transactions={transactions} occurrences={occurrences} handlers={} evaluated_clauses={} selected_clauses={} evaluation_instances={evaluations} restores={restores} skips={skips} rejections={rejections}; Galileo imported_events={} missing_occurrences={} replayed_occurrences=0 compared_projections=0 restored_sessions=0",
        handlers.len(), evaluated.len(), selected.len(), available.len(), missing.len(),
    );
    assert!(
        missing.is_empty(),
        "incomplete external-reference replay: {} unsupported occurrences; first: {}",
        missing.len(),
        missing.first().map(String::as_str).unwrap_or("none"),
    );
    panic!("incomplete checkpoint: observation replay has not been implemented");
}
