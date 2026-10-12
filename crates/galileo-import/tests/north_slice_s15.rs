#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s15_replays_the_input_resolution_recordings_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s15");
    let mut mismatches = Vec::new();
    let mut coverage: std::collections::BTreeMap<String, north_slice::Coverage> = Default::default();
    for recording in [
        "tests/external/north-slices/checkpoint_failure_does_not_release_a_host_effect.jsonl",
        "tests/external/north-slices/spec_6_work_commands_route_without_a_coding_turn.jsonl",
        "tests/external/north-slices/running_input_becomes_steering_and_failed_delivery_stays_unsent.jsonl",
        "tests/external/north-slices/steering_keys_send_the_active_turn_and_keep_rejected_images_editable.jsonl",
        "tests/external/north-slices/switching_running_conversations_keeps_drafts_and_interrupts_only_the_selected_turn.jsonl",
    ] {
        let replay = north_slice::replay(&program, recording);
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
    let handlers = ["command-work", "inspect", "resolve-input", "submit-input"];
    for handler in handlers {
        let counted = coverage.get(handler).unwrap_or_else(|| panic!("s15 declares no {handler}"));
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
