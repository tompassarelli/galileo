use std::path::Path;
use std::process::Command;

use galileo_syntax::schema::Schema;

#[test]
fn schema_extract_matches_beagle_extract_schema_on_firn() {
    let home = std::env::var("HOME").unwrap_or_default();
    let firn = std::env::var("FIRN_ROOT")
        .unwrap_or_else(|_| format!("{home}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9"));
    let beagle = std::env::var("BEAGLE_PATH").unwrap_or_else(|_| format!("{home}/code/beagle/main"));
    let extractor = Path::new(&beagle).join("bin/beagle-extract-schema");
    let nix = Command::new("nix").arg("--version").output().is_ok_and(|o| o.status.success());
    if std::env::var_os("GALILEO_FULL").is_none() {
        eprintln!("runs only with GALILEO_FULL=1: evaluates a whole host's options");
        return;
    }
    assert!(nix && Path::new(&firn).is_dir() && extractor.is_file(), "needs nix, FIRN_ROOT and {}", extractor.display());
    let reference = Command::new(&extractor)
        .args(["--target", "nixosConfigurations.whiterabbit.options", "--out", "/dev/stdout", "--flake", &firn])
        .output()
        .unwrap();
    assert!(reference.status.success(), "{}", String::from_utf8_lossy(&reference.stderr));
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("schema.json");
    let galileo = Command::new(env!("CARGO_BIN_EXE_galileo"))
        .args(["schema", "extract", "--host", "whiterabbit", "--flake", &firn, "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(galileo.status.success(), "{}", String::from_utf8_lossy(&galileo.stderr));
    assert!(reference.stdout.len() > 2);
    let ours = Schema::parse("galileo", &std::fs::read_to_string(&out).unwrap()).unwrap();
    let theirs = Schema::parse("beagle", &String::from_utf8(reference.stdout).unwrap()).unwrap();
    let view = |s: &Schema| s.options.values().map(|o| (o.path.clone(), o.ty.clone(), o.values.clone())).collect::<Vec<_>>();
    assert!(view(&ours) == view(&theirs), "galileo schema extract differs from beagle-extract-schema");
    assert!(ours.get("services.kanata.enable").is_some_and(|o| !o.declarations.is_empty()));
}
