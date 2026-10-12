#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s5_replays_the_chat_and_transcript_recordings_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s5");
    let mut mismatches = Vec::new();
    let mut coverage: std::collections::BTreeMap<String, north_slice::Coverage> = Default::default();
    let known: [north_slice::OutOfSlice; 0] = [];
    for recording in [
        "tests/external/north-slices/chat_batch_preserves_delta_order_and_final_replacement.jsonl",
        "tests/external/north-slices/transcript_controls_scroll_filter_changes_and_copy_without_changing_the_draft.jsonl",
        "tests/external/north-slices/unreached_cleared_chat.jsonl",
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
        "filter-transcript",
        "search-transcript",
        "scroll-transcript",
        "size-transcript",
        "toggle-changes",
        "append-chat",
        "observe-chat-item",
        "clear-chat",
    ];
    for handler in handlers {
        let counted = coverage.get(handler).unwrap_or_else(|| panic!("s5 declares no {handler}"));
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
