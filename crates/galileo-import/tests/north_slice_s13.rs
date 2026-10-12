#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s13_replays_the_conversation_and_attachment_recordings_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s13");
    let mut mismatches = Vec::new();
    let mut coverage: std::collections::BTreeMap<String, north_slice::Coverage> = Default::default();
    for recording in [
        "tests/external/north-slices/clause_owns_image_identity_draft_rollover_and_submission_membership.jsonl",
        "tests/external/north-slices/clause_rejects_unknown_switch_and_preserves_active_conversation_on_failure.jsonl",
        "tests/external/north-slices/unreached_failed_new_conversation.jsonl",
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
    let handlers = [
        "observe-conversation",
        "attach-image",
        "detach-image",
        "copy-submission-attachment",
        "clear-submitted-attachment",
        "request-new-conversation",
        "settle-new-conversation",
        "fail-new-conversation",
        "request-switch-conversation",
        "settle-switch-conversation",
        "fail-switch-conversation",
    ];
    for handler in handlers {
        let counted = coverage.get(handler).unwrap_or_else(|| panic!("s13 declares no {handler}"));
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
