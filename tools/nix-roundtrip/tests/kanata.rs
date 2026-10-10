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
            "{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9",
            std::env::var("HOME").unwrap()
        )
    });
    let files = ["default.nix", "kanata.nix"].map(|name| format!("{root}/modules/kanata/{name}"));
    let ast = Command::new(env!("CARGO_BIN_EXE_nix-roundtrip"))
        .arg("--ast")
        .args(&files)
        .output()
        .unwrap();
    assert!(
        ast.status.success(),
        "{}",
        String::from_utf8_lossy(&ast.stderr)
    );
    let ast_text = String::from_utf8(ast.stdout).unwrap();
    assert_eq!(
        ast_text.lines().filter(|line| *line == "Lambda {").count(),
        2
    );
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
    assert!(muove_import::nix::parse("{ broken =").is_err());
}
