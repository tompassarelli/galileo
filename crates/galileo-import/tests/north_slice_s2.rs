#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s2_replays_the_connection_and_reconnect_recordings_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s2");
    let mut mismatches = Vec::new();
    let mut coverage: std::collections::BTreeMap<String, north_slice::Coverage> = Default::default();
    let known: [north_slice::OutOfSlice; 0] = [];
    for recording in [
        "tests/external/north-slices/saved_workspace_handles_navigation_while_reconnection_is_pending.jsonl",
        "tests/external/north-slices/restart_storage_fresh_process.jsonl",
        "tests/external/north-slices/unreached_text_commands.jsonl",
        "tests/external/north-slices/unreached_connection_loss_and_reconnect.jsonl",
    ] {
        let replay = north_slice::replay_known(&program, recording, &known);
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
        assert!(replay.compared > 0, "{recording}: no transaction compared");
        mismatches.extend(replay.mismatches);
        for (handler, counted) in replay.coverage {
            let total = coverage.entry(handler).or_default();
            total.occurrences += counted.occurrences;
            total.matched += counted.matched;
            total.skipped.extend(counted.skipped);
        }
    }
    let handlers = [
        "connection-lost",
        "pause-disconnected-inputs",
        "disconnect-prompts",
        "begin-reconnect",
        "finish-reconnect",
        "command-reconnect",
        "begin-conversation-reconciliation",
    ];
    for handler in handlers {
        let counted = coverage.get(handler).unwrap_or_else(|| panic!("s2 declares no {handler}"));
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
    for mismatch in &mismatches {
        eprintln!("{mismatch}");
    }
    assert!(mismatches.is_empty(), "{} mismatches; first: {}", mismatches.len(), mismatches[0]);
}
