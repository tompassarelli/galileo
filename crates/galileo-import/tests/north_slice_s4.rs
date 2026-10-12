#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn s4_replays_the_north_reference_recordings_transaction_by_transaction() {
    let program = north_slice::program("examples/north/s4");
    let mut mismatches = Vec::new();
    let mut coverage: std::collections::BTreeMap<String, north_slice::Coverage> = Default::default();
    for recording in [
        "tests/external/north-slices/reference_catalog_can_be_replaced_after_a_full_listing.jsonl",
        "tests/external/north-slices/escape_closes_references_and_typing_reopens_them.jsonl",
        "tests/external/north-slices/reference_menu_filters_renders_and_inserts_without_submitting.jsonl",
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
    for handler in ["clear-references", "query-references", "offer-reference", "move-reference", "dismiss-references"] {
        assert!(coverage.contains_key(handler), "s4 declares {handler}");
    }
    for mismatch in &mismatches {
        eprintln!("{mismatch}");
    }
    assert!(mismatches.is_empty(), "{} mismatches; first: {}", mismatches.len(), mismatches[0]);
}
