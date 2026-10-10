use std::path::Path;
use std::process::Command;

#[test]
fn schema_extract_matches_beagle_extract_schema_on_firn() {
    let home = std::env::var("HOME").unwrap_or_default();
    let firn = std::env::var("FIRN_ROOT")
        .unwrap_or_else(|_| format!("{home}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9"));
    let beagle = std::env::var("BEAGLE_PATH").unwrap_or_else(|_| format!("{home}/code/beagle/main"));
    let extractor = Path::new(&beagle).join("bin/beagle-extract-schema");
    let nix = Command::new("nix").arg("--version").output().is_ok_and(|o| o.status.success());
    if !nix || !Path::new(&firn).is_dir() || !extractor.is_file() {
        eprintln!("skipped: needs nix, FIRN_ROOT and beagle-extract-schema");
        return;
    }
    let reference = Command::new(&extractor)
        .args(["--target", "nixosConfigurations.whiterabbit.options", "--out", "/dev/stdout", "--flake", &firn])
        .output()
        .unwrap();
    assert!(reference.status.success(), "{}", String::from_utf8_lossy(&reference.stderr));
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("schema.json");
    let muove = Command::new(env!("CARGO_BIN_EXE_muove"))
        .args(["schema", "extract", "--host", "whiterabbit", "--flake", &firn, "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(muove.status.success(), "{}", String::from_utf8_lossy(&muove.stderr));
    assert!(reference.stdout.len() > 2);
    assert!(std::fs::read(&out).unwrap() == reference.stdout, "muove schema extract differs from beagle-extract-schema");
}
