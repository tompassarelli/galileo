#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s10_replays_the_queue_and_steering_recordings_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s10");
    let mut mismatches = Vec::new();
    let mut coverage: std::collections::BTreeMap<String, north_slice::Coverage> = Default::default();
    let known: [north_slice::OutOfSlice; 0] = [];
    for recording in [
        "tests/external/north-slices/queued_inputs_wait_for_settlement_and_keep_their_own_attachments.jsonl",
        "tests/external/north-slices/steering_waits_for_receipts_and_expiry_never_retargets_a_new_turn.jsonl",
        "tests/external/north-slices/steering_keys_send_the_active_turn_and_keep_rejected_images_editable.jsonl",
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
        "prepare-steering",
        "retain-queued-attachment",
        "prepare-queued-input",
        "submit-queued",
        "copy-queued-to-submitted",
        "prepare-input-edit",
        "restore-input-attachment",
        "forget-input-attachment",
        "forget-input",
    ];
    for handler in handlers {
        let counted = coverage.get(handler).unwrap_or_else(|| panic!("s10 declares no {handler}"));
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
