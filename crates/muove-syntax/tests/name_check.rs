use muove_syntax::check::{check, SourceFile};

const DEFAULT: &str = include_str!("../../../tests/external/kanata/default.muove");
const KANATA: &str = include_str!("../../../tests/external/kanata/kanata.muove");

fn diagnostics(kanata: &str) -> Vec<String> {
    let files = [
        SourceFile::parse("modules/kanata/default.muove", DEFAULT).unwrap(),
        SourceFile::parse("modules/kanata/kanata.muove", kanata).unwrap(),
    ];
    check(&files, &Default::default()).iter().map(ToString::to_string).collect()
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

#[test]
fn spec_3_2_unknown_schema_option_path_gets_did_you_mean() {
    let schema = muove_syntax::schema::Schema::parse(
        "schema.json",
        r#"[{"name":"services.kanata.enable","t":"bool"},{"name":"services.kanata.package","t":"package"},{"name":"services.kanata.keyboards","t":"attrsOf","inner":{"t":"submodule"}},{"name":"hardware.uinput.enable","t":"bool"},{"name":"users.users","t":"attrsOf","inner":{"t":"submodule"}},{"name":"users.groups","t":"attrsOf","inner":{"t":"submodule"}},{"name":"services.udev.extraRules","t":"lines"},{"name":"systemd.services","t":"attrsOf","inner":{"t":"submodule"}}]"#,
    )
    .unwrap();
    let check_with = |kanata: &str| -> Vec<String> {
        let files = [
            SourceFile::parse("modules/kanata/default.muove", DEFAULT).unwrap(),
            SourceFile::parse("modules/kanata/kanata.muove", kanata).unwrap(),
        ];
        check(&files, &schema).iter().map(|d| d.message.clone()).collect()
    };
    assert_eq!(check_with(KANATA), Vec::<String>::new());
    let typo = KANATA.replace("services.kanata\n      enable", "services.kanata\n      enabel");
    assert_eq!(
        check_with(&typo),
        ["`enabel` isn't an option of `services.kanata`. Did you mean `services.kanata.enable`? (schema.json)"]
    );
}
