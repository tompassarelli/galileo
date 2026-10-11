use std::path::{Path, PathBuf};
use std::process::Command;

const FILES: [&str; 7] = [
    "modules/protonup-qt/default.nix",
    "modules/claude/default.nix",
    "modules/users/users.nix",
    "modules/beagle/default.nix",
    "modules/kanata/default.nix",
    "modules/kanata/kanata.nix",
    "flake.nix",
];

#[test]
fn f3_f4_committed_firn_constructs_rebuild_byte_identically() {
    let root = PathBuf::from(std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!(
            "{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9",
            std::env::var("HOME").unwrap()
        )
    }));
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("firn-constructs");
    std::fs::create_dir_all(&scratch).unwrap();
    let mut compared = 0;
    let mut equal = 0;
    let mut rejected = 0;
    let mut mismatches = 0;
    let mut failures = Vec::new();
    for (index, file) in FILES.iter().enumerate() {
        let original = root.join(file);
        let imported = Command::new(env!("CARGO_BIN_EXE_galileo"))
            .args(["import", "nix"])
            .arg(&original)
            .output()
            .unwrap();
        if !imported.status.success() {
            rejected += 1;
            failures.push(format!(
                "{file}: import: {}",
                String::from_utf8_lossy(&imported.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
            ));
            continue;
        }
        let source = scratch.join(format!("case-{index}.gal"));
        std::fs::write(&source, imported.stdout).unwrap();
        let built = Command::new(env!("CARGO_BIN_EXE_galileo"))
            .args(["build", "--stdout"])
            .arg(&source)
            .output()
            .unwrap();
        if !built.status.success() {
            rejected += 1;
            failures.push(format!(
                "{file}: build: {}",
                String::from_utf8_lossy(&built.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
            ));
            continue;
        }
        let expected = std::fs::read(&original).unwrap();
        compared += 1;
        if expected == built.stdout {
            equal += 1;
            println!("{file}: {} bytes, byte-identical", expected.len());
        } else {
            mismatches += 1;
            let first = expected
                .iter()
                .zip(&built.stdout)
                .position(|(a, b)| a != b)
                .unwrap_or(expected.len().min(built.stdout.len()));
            failures.push(format!(
                "{file}: byte {first}, original {} bytes, rebuilt {} bytes",
                expected.len(),
                built.stdout.len()
            ));
        }
    }
    for failure in &failures {
        println!("{failure}");
    }
    println!("firn construct coverage: 8 required constructs, {} files, {compared} comparisons, {equal} byte-identical, {mismatches} byte mismatches, {rejected} rejected files", FILES.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(compared, FILES.len());
}
