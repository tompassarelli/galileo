#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s7_replays_the_prompt_answering_recordings_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s7");
    let mut mismatches = Vec::new();
    let mut coverage: std::collections::BTreeMap<String, north_slice::Coverage> = Default::default();
    for recording in [
        "tests/external/north-slices/multiple_questions_accept_choices_and_free_text_without_crossing_threads.jsonl",
        "tests/external/north-slices/tool_forms_use_checked_choices_validate_fields_and_keep_the_chat_draft.jsonl",
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
        "navigate-prompt",
        "answer-prompt-choice",
        "toggle-prompt-option",
        "answer-prompt-multiple",
        "skip-prompt-question",
        "prompt-answer-problem",
        "answer-prompt-text",
    ];
    for handler in handlers {
        let counted = coverage.get(handler).unwrap_or_else(|| panic!("s7 declares no {handler}"));
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
