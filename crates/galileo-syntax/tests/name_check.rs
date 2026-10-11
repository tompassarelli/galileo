use galileo_syntax::check::{check, Diagnostic, SourceFile};

const DEFAULT: &str = include_str!("../../../tests/external/kanata/default.gal");
const KANATA: &str = include_str!("../../../tests/external/kanata/kanata.gal");

fn found(kanata: &str) -> Vec<Diagnostic> {
    let files = [
        SourceFile::parse("modules/kanata/default.gal", DEFAULT).unwrap(),
        SourceFile::parse("modules/kanata/kanata.gal", kanata).unwrap(),
    ];
    check(&files, &Default::default())
}

fn diagnostics(kanata: &str) -> Vec<String> {
    found(kanata).iter().map(ToString::to_string).collect()
}

fn located(found: &[Diagnostic]) -> Vec<(&str, usize, usize)> {
    found.iter().map(|d| (d.file.as_str(), d.line, d.column)).collect()
}

#[test]
fn spec_9_5_misspelled_option_field() {
    let source = KANATA.replace("devices: cfg.devices", "devices: cfg.devics");
    assert_eq!(
        diagnostics(&source),
        ["modules/kanata/kanata.gal:21:24: `devics` isn't an option of `myConfig.modules.kanata`. \
Did you mean `devices`? (declared at modules/kanata/default.gal:4)\n              devices: cfg.devics"]
    );
}

#[test]
fn spec_3_2_unknown_schema_option_path_gets_did_you_mean() {
    let schema = galileo_syntax::schema::Schema::parse(
        "schema.json",
        r#"[{"name":"services.kanata.enable","t":"bool"},{"name":"services.kanata.package","t":"package"},{"name":"services.kanata.keyboards","t":"attrsOf","inner":{"t":"submodule"}},{"name":"hardware.uinput.enable","t":"bool"},{"name":"users.users","t":"attrsOf","inner":{"t":"submodule"}},{"name":"users.groups","t":"attrsOf","inner":{"t":"submodule"}},{"name":"services.udev.extraRules","t":"lines"},{"name":"systemd.services","t":"attrsOf","inner":{"t":"submodule"}}]"#,
    )
    .unwrap();
    let check_with = |kanata: &str| -> Vec<Diagnostic> {
        let files = [
            SourceFile::parse("modules/kanata/default.gal", DEFAULT).unwrap(),
            SourceFile::parse("modules/kanata/kanata.gal", kanata).unwrap(),
        ];
        check(&files, &schema)
    };
    assert!(check_with(KANATA).is_empty());
    let typo = KANATA.replace("services.kanata\n      enable", "services.kanata\n      enabel");
    let typo = check_with(&typo);
    assert_eq!(located(&typo), [("modules/kanata/kanata.gal", 17, 7)]);
    assert!(
        ["`enabel`", "`services.kanata.enable`", "schema.json"].iter().all(|fact| typo[0].message.contains(fact)),
        "{}",
        typo[0].message
    );
    let empty_schema = galileo_syntax::schema::Schema::parse("empty.json", "[]").unwrap();
    let files = [SourceFile::parse(
        "empty-schema.gal",
        "foreign nixos-options from ./empty.json\nnix module\n  arguments: config, lib, ...\n  config\n    services.kanata.enabel: true\n",
    )
    .unwrap()];
    assert_eq!(
        check(&files, &empty_schema).iter().map(|d| d.message.clone()).collect::<Vec<_>>(),
        ["`services` isn't an option of `config`."]
    );
}

#[test]
fn spec_2_nix_bodies_bind_nix_globals_and_a_block_binds_a_name_once() {
    let source = KANATA.replace(
        "  let cfg = config.myConfig.modules.kanata\n",
        "  let cfg = config.myConfig.modules.kanata\n  let port = toString(cfg.port)\n  let cfg = import(./other.nix)\n",
    );
    let found = found(&source);
    assert_eq!(located(&found), [("modules/kanata/kanata.gal", 5, 7)]);
    assert!(found[0].message.contains("`cfg`") && found[0].message.contains("line 3"), "{}", found[0].message);
}

#[test]
fn spec_3_2_without_a_schema_only_nodes_holding_galileo_options_are_closed() {
    let source = KANATA
        .replace("      enable: true\n", "      enable: config.services.kanata.enable\n")
        .replace(
            "    users.groups.kanata: {}\n",
            "    users.groups.kanata: {}\n    myConfig.modules.kanata.enabel: true\n    home-manager.users.tom: nix module(config, ...)\n      home.sessionVariables.U: config.home.username\n",
        );
    let found = found(&source);
    assert_eq!(located(&found), [("modules/kanata/kanata.gal", 16, 29)]);
    assert!(
        ["`enabel`", "`myConfig.modules.kanata.enable`", "modules/kanata/default.gal:3"].iter().all(|fact| found[0].message.contains(fact)),
        "{}",
        found[0].message
    );
}
