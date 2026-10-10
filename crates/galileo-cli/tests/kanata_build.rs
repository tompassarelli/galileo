use std::path::Path;
use std::process::Command;

#[test]
fn a1_galileo_build_emits_firn_kanata_byte_for_byte() {
    let firn = std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!("{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9", std::env::var("HOME").unwrap())
    });
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/external/kanata");
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("kanata");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    for name in ["default", "kanata"] {
        std::fs::copy(source.join(format!("{name}.gal")), scratch.join(format!("{name}.gal"))).unwrap();
    }
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).arg("build").arg(&scratch).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    for name in ["default", "kanata"] {
        let built = std::fs::read_to_string(scratch.join(format!("{name}.nix"))).unwrap();
        let expected = std::fs::read_to_string(format!("{firn}/modules/kanata/{name}.nix")).unwrap();
        assert_eq!(built, expected, "{name}.nix");
    }
}

#[test]
fn build_check_and_stdout_against_firn_kanata() {
    let firn = std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!("{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9", std::env::var("HOME").unwrap())
    });
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/external/kanata");
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("kanata-check");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    for name in ["default", "kanata"] {
        std::fs::copy(source.join(format!("{name}.gal")), scratch.join(format!("{name}.gal"))).unwrap();
        std::fs::copy(format!("{firn}/modules/kanata/{name}.nix"), scratch.join(format!("{name}.nix"))).unwrap();
    }
    let galileo = || Command::new(env!("CARGO_BIN_EXE_galileo"));
    let clean = galileo().args(["build", "--check"]).arg(&scratch).output().unwrap();
    assert_eq!(clean.status.code(), Some(0), "{}", String::from_utf8_lossy(&clean.stderr));
    assert!(clean.stdout.is_empty());

    let target = scratch.join("kanata.nix");
    let mut bytes = std::fs::read(&target).unwrap();
    let at = bytes.iter().position(|&b| b == b'=').unwrap();
    bytes[at] = b'~';
    std::fs::write(&target, &bytes).unwrap();
    let line = bytes[..at].iter().filter(|&&b| b == b'\n').count() + 1;
    let drift = galileo().args(["build", "--check"]).arg(&scratch).output().unwrap();
    assert_eq!(drift.status.code(), Some(1));
    let diff = String::from_utf8(drift.stdout).unwrap();
    assert!(diff.contains(&format!("{}:{line}\n", target.display())), "{diff}");
    assert!(diff.contains("\n-") && diff.contains("\n+"), "{diff}");
    assert_eq!(std::fs::read(&target).unwrap(), bytes);

    for name in ["default", "kanata"] {
        let printed = galileo().args(["build", "--stdout"]).arg(scratch.join(format!("{name}.gal"))).output().unwrap();
        assert_eq!(printed.status.code(), Some(0), "{}", String::from_utf8_lossy(&printed.stderr));
        assert_eq!(printed.stdout, std::fs::read(format!("{firn}/modules/kanata/{name}.nix")).unwrap(), "{name}");
    }
}
