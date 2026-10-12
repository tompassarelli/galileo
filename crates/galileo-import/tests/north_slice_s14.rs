#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s14_replays_the_slash_popup_recording_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s14");
    let replay = north_slice::replay(&program, "tests/external/north-slices/slash_popup_exposes_the_basic_command_surface.jsonl");
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
    assert!(replay.compared > 0, "no transaction compared");
    for handler in ["initialize"] {
        let counted = &replay.coverage[handler];
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
    for mismatch in &replay.mismatches {
        eprintln!("{mismatch}");
    }
    assert!(replay.mismatches.is_empty(), "{} mismatches; first: {}", replay.mismatches.len(), replay.mismatches[0]);
}
