#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s3_replays_the_north_usage_recording_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s3");
    let replay = north_slice::replay(
        &program,
        "tests/external/north-slices/usage_display_distinguishes_missing_values_and_preserves_conversation_scope.jsonl",
    );
    eprintln!(
        "{}: compared={} skipped={} mismatches={}",
        replay.recording,
        replay.compared,
        replay.skipped.len(),
        replay.mismatches.len()
    );
    for (boundary, handler) in &replay.skipped {
        eprintln!("skipped {boundary}: {handler} applied or was rejected outside the program");
    }
    for mismatch in &replay.mismatches {
        eprintln!("{mismatch}");
    }
    for transaction in [4, 5, 6, 10] {
        let marker = format!("transaction={transaction}");
        assert!(
            !replay.skipped.iter().any(|(boundary, _)| boundary.ends_with(&marker)),
            "transaction {transaction} was skipped, not compared"
        );
    }
    assert!(replay.compared >= 4, "compared={}", replay.compared);
    for (handler, counted) in &replay.coverage {
        if counted.occurrences == 0 {
            eprintln!("coverage {handler}: unrecorded");
        } else {
            eprintln!(
                "coverage {handler}: occurrences={} matched={} skipped={} {:?}",
                counted.occurrences,
                counted.matched,
                counted.skipped.len(),
                counted.skipped
            );
        }
    }
    for handler in [
        "count-usage",
        "present-usage",
        "withdraw-usage-row",
        "observe-context-usage",
        "observe-quota-window",
        "observe-reset-credits",
        "command-usage",
    ] {
        assert!(replay.coverage.contains_key(handler), "s3 declares {handler}");
    }
    assert!(
        replay.mismatches.is_empty(),
        "{} mismatches; first: {}",
        replay.mismatches.len(),
        replay.mismatches[0]
    );
}
