#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s8_replays_the_cancellation_recording_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s8");
    let mut mismatches = Vec::new();
    let mut coverage: std::collections::BTreeMap<String, north_slice::Coverage> = Default::default();
    let known: [north_slice::OutOfSlice; 0] = [];
    for recording in [
        "tests/external/north-slices/cancellation_is_explicit_and_unknown_delivery_never_resends.jsonl",
        "tests/external/north-slices/approval_uses_only_offered_decisions_and_retains_exact_request_identity.jsonl",
        "tests/external/north-slices/multiple_questions_accept_choices_and_free_text_without_crossing_threads.jsonl",
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
        "prepare-prompt-response",
        "cancel-prompt",
        "prompt-response-written",
        "resolve-prompt",
        "close-turn-prompts",
        "clear-resolved-answers",
    ];
    for handler in handlers {
        let counted = coverage.get(handler).unwrap_or_else(|| panic!("s8 declares no {handler}"));
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
