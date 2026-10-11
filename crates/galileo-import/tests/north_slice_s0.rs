#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s0_replays_the_north_menu_recordings_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s0");
    let mut mismatches = Vec::new();
    let mut coverage: std::collections::BTreeMap<String, north_slice::Coverage> = Default::default();
    for recording in ["tests/external/north-menu/history.jsonl", "tests/external/north-menu/restart.jsonl"] {
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
    for (handler, counted) in &coverage {
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
    assert_eq!(coverage.len(), 13, "s0 declares 13 handlers");
    for mismatch in &mismatches {
        eprintln!("{mismatch}");
    }
    assert!(mismatches.is_empty(), "{} mismatches; first: {}", mismatches.len(), mismatches[0]);
}
