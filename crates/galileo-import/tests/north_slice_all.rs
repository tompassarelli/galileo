#[path = "support/north_slice.rs"]
mod north_slice;

#[test]
fn whole_north_program_replays_every_recording_straight() {
    let program = north_slice::program_all();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut recordings: Vec<String> = ["tests/external/north-slices", "tests/external/north-menu"]
        .iter()
        .flat_map(|dir| {
            std::fs::read_dir(root.join(dir))
                .unwrap()
                .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                .filter(|name| name.ends_with(".jsonl"))
                .map(move |name| format!("{dir}/{name}"))
                .collect::<Vec<_>>()
        })
        .collect();
    recordings.sort();
    assert_eq!(recordings.len(), 45, "33 + 10 slice recordings and 2 menu recordings");
    let mut mismatches = Vec::new();
    let mut coverage: std::collections::BTreeMap<String, north_slice::Coverage> = Default::default();
    for recording in &recordings {
        let replay = north_slice::replay_straight(&program, recording);
        eprintln!("{}: compared={} mismatches={}", replay.recording, replay.compared, replay.mismatches.len());
        assert!(replay.skipped.is_empty(), "{recording}: a straight replay skips nothing");
        assert!(replay.compared > 0, "{recording}: no transaction compared");
        mismatches.extend(replay.mismatches);
        for (handler, counted) in replay.coverage {
            let total = coverage.entry(handler).or_default();
            total.occurrences += counted.occurrences;
            total.matched += counted.matched;
        }
    }
    let unreachable = ["inspect"];
    let mut unmatched = Vec::new();
    for (handler, counted) in &coverage {
        eprintln!("coverage {handler}: occurrences={} matched={}", counted.occurrences, counted.matched);
        if counted.matched == 0 && !unreachable.contains(&handler.as_str()) {
            unmatched.push(handler.clone());
        }
    }
    let matched = coverage.values().filter(|counted| counted.matched > 0).count();
    eprintln!("handlers matched={matched}/{} unreachable={unreachable:?}", coverage.len());
    for mismatch in &mismatches {
        eprintln!("{mismatch}");
    }
    assert!(mismatches.is_empty(), "{} mismatches; first: {}", mismatches.len(), mismatches[0]);
    assert_eq!(coverage.len(), 135, "the program declares 135 handlers");
    assert!(unmatched.is_empty(), "handlers never matched: {unmatched:?}");
}
