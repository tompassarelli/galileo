#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s0_replays_the_north_menu_recordings_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s0");
    let mut mismatches = Vec::new();
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
    }
    for mismatch in &mismatches {
        eprintln!("{mismatch}");
    }
    assert!(mismatches.is_empty(), "{} mismatches; first: {}", mismatches.len(), mismatches[0]);
}
