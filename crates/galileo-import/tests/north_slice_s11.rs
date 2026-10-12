#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s11_replays_the_turn_observation_and_submit_recordings_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s11");
    let mut mismatches = Vec::new();
    let mut coverage: std::collections::BTreeMap<String, north_slice::Coverage> = Default::default();
    let known: [north_slice::OutOfSlice; 0] = [];
    for recording in [
        "tests/external/north-slices/conversation_turns_queues_and_drafts_are_independent_of_selection.jsonl",
        "tests/external/north-slices/clause_owns_submit_and_settlement.jsonl",
        "tests/external/north-slices/snapshot_replay_preserves_full_history_drafts_and_acceptance.jsonl",
        "tests/external/north-slices/clause_owns_delegation_admission_and_settlement.jsonl",
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
        "observe-turn",
        "observe-stored-turn",
        "clear-turn",
        "finish-observed-turn",
        "expire-steering",
        "submit",
        "delegate",
    ];
    for handler in handlers {
        let counted = coverage.get(handler).unwrap_or_else(|| panic!("s11 declares no {handler}"));
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
