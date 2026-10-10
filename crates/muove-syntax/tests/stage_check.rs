use muove_syntax::check::{check, SourceFile};

const DEFAULT: &str = include_str!("../../../tests/external/kanata/default.muove");
const KANATA: &str = include_str!("../../../tests/external/kanata/kanata.muove");
const NIX_LIB: &str = include_str!("../../../lib/firn/nix-lib.muove");

fn diagnostics(extra: &[(&str, &str)]) -> Vec<String> {
    let mut files = vec![
        SourceFile::parse("lib/firn/nix-lib.muove", NIX_LIB).unwrap(),
        SourceFile::parse("modules/kanata/default.muove", DEFAULT).unwrap(),
    ];
    for (name, text) in extra {
        files.push(SourceFile::parse(name, text).unwrap());
    }
    check(&files).iter().map(ToString::to_string).collect()
}

#[test]
fn spec_9_5_kanata_checks_clean_with_nix_lib() {
    assert_eq!(
        diagnostics(&[("modules/kanata/kanata.muove", KANATA)]),
        Vec::<String>::new()
    );
}

#[test]
fn spec_3_3_maybe_port_compares_with_null_but_not_with_numbers() {
    let source = KANATA.replace("when cfg.port != null", "when cfg.port > 1");
    assert_eq!(
        diagnostics(&[("modules/kanata/kanata.muove", &source)]),
        ["modules/kanata/kanata.muove:22:31: `cfg.port` is `maybe Int`, so it may be null, \
and `>` can't use a null value.\n              port: cfg.port when cfg.port > 1"]
    );
}

#[test]
fn spec_3_2_nix_value_in_a_fact_slot_is_a_stage_error() {
    let facts = "\
type Module
static {module: Module} has motd {text: Text}
static {module: Module} sets {value: nix Any}
kanata is a Module
kanata has motd builtins.readFile(./motd)
kanata sets [pkgs.kanata]
";
    assert_eq!(
        diagnostics(&[("modules/kanata/facts.muove", facts)]),
        ["modules/kanata/facts.muove:5:17: `builtins.readFile(./motd)` depends on Nix evaluation, \
so it can't fill the `text` slot, which is `Text`. Muove can't decide it; only a slot typed \
`nix Text` stores a Nix value.\n    kanata has motd builtins.readFile(./motd)"]
    );
}

#[test]
fn nix_lib_signature_checks_argument_types() {
    let source = KANATA.replace("builtins.readFile(cfg.configFile)", "builtins.readFile(cfg.port)");
    assert_eq!(
        diagnostics(&[("modules/kanata/kanata.muove", &source)]),
        ["modules/kanata/kanata.muove:25:37: `builtins.readFile` takes a `Path` as `file`, \
but `cfg.port` is `maybe Int`.\n              config: builtins.readFile(cfg.port)"]
    );
}
