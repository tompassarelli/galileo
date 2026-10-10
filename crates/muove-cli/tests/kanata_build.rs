use std::path::Path;
use std::process::Command;

#[test]
fn a1_muove_build_emits_firn_kanata_byte_for_byte() {
    let firn = std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!("{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9", std::env::var("HOME").unwrap())
    });
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/external/kanata");
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("kanata");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    for name in ["default", "kanata"] {
        std::fs::copy(source.join(format!("{name}.muove")), scratch.join(format!("{name}.muove"))).unwrap();
    }
    let out = Command::new(env!("CARGO_BIN_EXE_muove")).arg("build").arg(&scratch).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    for name in ["default", "kanata"] {
        let built = std::fs::read_to_string(scratch.join(format!("{name}.nix"))).unwrap();
        let expected = std::fs::read_to_string(format!("{firn}/modules/kanata/{name}.nix")).unwrap();
        assert_eq!(built, expected, "{name}.nix");
    }
}
