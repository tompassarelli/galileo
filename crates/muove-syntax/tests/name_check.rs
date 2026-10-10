use muove_syntax::check::{check, SourceFile};

const DEFAULT: &str = include_str!("../../../tests/external/kanata/default.muove");
const KANATA: &str = include_str!("../../../tests/external/kanata/kanata.muove");

fn diagnostics(kanata: &str) -> Vec<String> {
    let files = [
        SourceFile::parse("modules/kanata/default.muove", DEFAULT).unwrap(),
        SourceFile::parse("modules/kanata/kanata.muove", kanata).unwrap(),
    ];
    check(&files).iter().map(ToString::to_string).collect()
}

#[test]
fn spec_9_5_kanata_checks_clean() {
    assert_eq!(diagnostics(KANATA), Vec::<String>::new());
}

#[test]
fn spec_9_5_misspelled_option_field() {
    let source = KANATA.replace("devices: cfg.devices", "devices: cfg.devics");
    assert_eq!(
        diagnostics(&source),
        ["modules/kanata/kanata.muove:21:24: `devics` isn't an option of `myConfig.modules.kanata`. \
Did you mean `devices`? (declared at modules/kanata/default.muove:4)\n              devices: cfg.devics"]
    );
}
