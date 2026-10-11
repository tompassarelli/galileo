use std::process::Command;

fn nix_parse(source: &str) -> Vec<u8> {
    let output = Command::new("nix-instantiate")
        .args(["--parse", "-E", source])
        .output()
        .expect("Nix available");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn firn_kanata_roundtrip_matches_beagle_bytes_and_nix_parser() {
    let root = std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!(
            "{}/code/nixos-config/pins/4bcfad4cdb806dd7f66401dad5581037cb996ff0",
            std::env::var("HOME").unwrap()
        )
    });
    let files = ["default.nix", "kanata.nix"].map(|name| format!("{root}/modules/kanata/{name}"));
    let theme_switcher = format!("{root}/modules/theme-switcher/default.nix");
    for file in files.into_iter().chain([theme_switcher]) {
        let source = std::fs::read_to_string(&file).unwrap();
        let printed = Command::new(env!("CARGO_BIN_EXE_nix-roundtrip"))
            .arg(&file)
            .output()
            .unwrap();
        assert!(
            printed.status.success(),
            "{}",
            String::from_utf8_lossy(&printed.stderr)
        );
        assert_eq!(printed.stdout, source.as_bytes(), "Beagle bytes: {file}");
        assert_eq!(
            nix_parse(&source),
            nix_parse(&String::from_utf8(printed.stdout).unwrap()),
            "{file}"
        );
    }
    assert!(galileo_import::nix::parse("{ broken =").is_err());
}
