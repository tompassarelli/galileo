#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s18_replays_the_goal_revision_recordings_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s18");
    let mut mismatches = Vec::new();
    let mut coverage: std::collections::BTreeMap<String, north_slice::Coverage> = Default::default();
    let known = [16, 21].map(|transaction| north_slice::OutOfSlice {
        recording: "clause_creates_orders_selects_and_edits_goals_with_revisions.jsonl",
        session: 1,
        transaction,
        slice: "S19",
        rule: 127,
        reason: "command-goal's /goal edit clause",
    });
    for recording in [
        "tests/external/north-slices/clause_creates_orders_selects_and_edits_goals_with_revisions.jsonl",
        "tests/external/north-slices/clause_drives_the_tui_goal_create_inspect_edit_inspect_journey.jsonl",
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
        "select-goal",
        "command-goal",
        "goal-title",
        "goal-objective",
    ];
    for handler in handlers {
        let counted = coverage.get(handler).unwrap_or_else(|| panic!("s18 declares no {handler}"));
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
