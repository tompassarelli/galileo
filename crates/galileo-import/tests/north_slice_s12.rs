#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s12_replays_the_delegation_and_abandoned_turn_recordings_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s12");
    let mut mismatches = Vec::new();
    let mut compared = 0;
    let mut coverage: std::collections::BTreeMap<String, north_slice::Coverage> = Default::default();
    for recording in [
        "tests/external/north-slices/clause_owns_delegation_admission_and_settlement.jsonl",
        "tests/external/north-slices/clause_owns_delegation_failure_before_and_after_spawn.jsonl",
        "tests/external/north-slices/unreached_abandoned_turn.jsonl",
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
        compared += replay.compared;
        mismatches.extend(replay.mismatches);
        for (handler, counted) in replay.coverage {
            let total = coverage.entry(handler).or_default();
            total.occurrences += counted.occurrences;
            total.matched += counted.matched;
            total.skipped.extend(counted.skipped);
        }
    }
    assert!(compared > 0, "no transaction compared");
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
    for handler in [
        "child-spawned",
        "settle-success",
        "abandon-turn",
        "settle-interrupted",
        "settle-failure",
        "settle-delegation-success",
        "fail-delegation-before-child",
        "fail-delegation-after-child",
    ] {
        assert!(coverage.contains_key(handler), "s12 declares {handler}");
    }
    for mismatch in &mismatches {
        eprintln!("{mismatch}");
    }
    assert!(mismatches.is_empty(), "{} mismatches; first: {}", mismatches.len(), mismatches[0]);
}
