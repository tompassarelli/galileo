use galileo_syntax::check::{check, SourceFile};

const DEFAULT: &str = include_str!("../../../tests/external/kanata/default.gal");
const KANATA: &str = include_str!("../../../tests/external/kanata/kanata.gal");
const NIX_LIB: &str = include_str!("../../../lib/firn/nix-lib.gal");

fn diagnostics(extra: &[(&str, &str)]) -> Vec<String> {
    let mut files = vec![
        SourceFile::parse("lib/firn/nix-lib.gal", NIX_LIB).unwrap(),
        SourceFile::parse("modules/kanata/default.gal", DEFAULT).unwrap(),
    ];
    for (name, text) in extra {
        files.push(SourceFile::parse(name, text).unwrap());
    }
    check(&files, &Default::default()).iter().map(ToString::to_string).collect()
}

#[test]
fn spec_3_3_maybe_port_compares_with_null_but_not_with_numbers() {
    let source = KANATA.replace("when cfg.port != null", "when cfg.port > 1");
    assert_eq!(
        diagnostics(&[("modules/kanata/kanata.gal", &source)]),
        ["modules/kanata/kanata.gal:22:31: `cfg.port` is `maybe Int`, so it may be null, \
and `>` can't use a null value.\n              port: cfg.port when cfg.port > 1"]
    );
}

#[test]
fn spec_3_2_nix_value_in_a_fact_slot_is_a_stage_error() {
    let facts = "\
type Module
given {module: Module} has motd {text: Text}
given {module: Module} sets {value: nix Any}
kanata is a Module
kanata has motd builtins.readFile(./motd)
kanata sets [pkgs.kanata]
";
    assert_eq!(
        diagnostics(&[("modules/kanata/facts.gal", facts)]),
        ["modules/kanata/facts.gal:5:17: `builtins.readFile(./motd)` depends on Nix evaluation, \
so it can't fill the `text` slot, which is `Text`. Galileo can't decide it; only a slot typed \
`nix Text` stores a Nix value.\n    kanata has motd builtins.readFile(./motd)"]
    );
}

#[test]
fn nix_lib_signature_checks_argument_types() {
    let source = KANATA.replace("builtins.readFile(cfg.configFile)", "builtins.readFile(cfg.port)");
    assert_eq!(
        diagnostics(&[("modules/kanata/kanata.gal", &source)]),
        ["modules/kanata/kanata.gal:25:37: `builtins.readFile` takes a `Path` as `file`, \
but `cfg.port` is `maybe Int`.\n              config: builtins.readFile(cfg.port)"]
    );
}

#[test]
fn spec_3_2_composed_values_keep_their_stage_and_maybe() {
    let facts = "\
type Host
given {host: Host} has {value: Text}
h is a Host
";
    let module = "\
nix module
  arguments: config, lib
  option app.port: maybe Port
  option app.name: maybe Text
  option app.enable: Bool
  let cfg = config.app
  config
    value: ";
    let rows = [
        (facts, "h has \"motd {builtins.readFile(./motd)}\"", "depends on Nix evaluation"),
        (facts, "h has ((x) => builtins.readFile(x))(./motd)", "depends on Nix evaluation"),
        (facts, "h has ({ {builtins.readFile(./motd)}: 1 })", "depends on Nix evaluation"),
        (module, "(if cfg.enable then cfg.name else \"a\") ++ \"b\"", "may be null"),
        (module, "first([]) + 1", "may be null"),
    ];
    for (prefix, row, expected) in rows {
        let found = diagnostics(&[("probe.gal", &format!("{prefix}{row}\n"))]);
        assert!(
            found.len() == 1 && found[0].contains(expected),
            "{row}: {found:?}"
        );
    }
}

fn the_types(name: &str, text: &str) -> (Vec<String>, Vec<String>) {
    let file = SourceFile::parse(name, text).unwrap();
    let (diagnostics, index) = galileo_syntax::check::analyze(&[file], &Default::default());
    let types = text
        .match_indices("the ?")
        .filter_map(|(at, _)| index.at(name, at).map(|o| o.ty.to_string()))
        .collect();
    (diagnostics.iter().map(ToString::to_string).collect(), types)
}

#[test]
fn spec_4_5_the_menu_view_fields_are_typed_by_their_exactly_one_modes() {
    let (diagnostics, types) = the_types("menu.gal", include_str!("menu.gal"));
    assert!(!diagnostics.iter().any(|d| d.contains("the ?")), "{diagnostics:?}");
    assert_eq!(types, ["Text", "Text", "Int"]);
}

#[test]
fn spec_4_5_the_needs_a_chain_of_modes_and_at_most_one_makes_it_maybe() {
    let vocabulary = "\
type Shop
type Person
given {shop: Shop} sells {item: Text}
given {shop: Shop} has manager {person: Person}
  at most one person per shop
given {person: Person} is named {name: Text}
  exactly one name per person
";
    let unfixed = format!("{vocabulary}view Stock for {{shop: Shop}}\n  item: the ?x where ?x sells ?y\n");
    let (diagnostics, _) = the_types("shop.gal", &unfixed);
    assert_eq!(
        diagnostics,
        ["shop.gal:9:9: `the ?x where ?x sells ?y` may have several `?x`: no chain of `exactly one` \
or `at most one` modes fixes `?x` from constants and outside variables.\n      item: the ?x where ?x sells ?y"]
    );
    let chain = format!(
        "{vocabulary}view Boss for {{shop: Shop}}\n  name: the ?n where shop has manager ?p and ?p is named ?n\n"
    );
    assert_eq!(the_types("shop.gal", &chain), (vec![], vec!["maybe Text".to_string()]));
}

#[test]
fn spec_4_3_aggregates_take_int_or_text_and_never_entities_or_nix_values() {
    let vocabulary = "\
type Shop, Item
given {shop: Shop} sells {item: Item} for {cost: Int}
given {item: Item} is named {name: Text}
given {shop: Shop} stocks {items: nix Int}
derived {shop: Shop} totals {n: Int}
";
    let rows = [
        ("sum ?t where ?shop sells ?i for ?c and ?i is named ?t", "`sum` takes `Int`, but `?t` is `Text`."),
        ("min ?i where ?shop sells ?i for ?c", "`min` takes `Int` or `Text`, but `?i` is `Item`."),
        ("max ?s where ?shop stocks ?s", "`max` takes `Int` or `Text`, but `?s` is `nix Int`."),
        ("min ?c by ?i where ?shop sells ?i for ?c", "`min … by` takes `Int` or `Text`, but `?i` is `Item`."),
    ];
    for (aggregate, expected) in rows {
        let text = format!("{vocabulary}\nrule probe: ?shop totals ?n\n  when ?shop sells ?_ for ?_\n  and ?n = {aggregate}\n");
        let (diagnostics, _) = the_types("shop.gal", &text);
        assert!(
            diagnostics.len() == 1 && diagnostics[0].starts_with("shop.gal:9:") && diagnostics[0].contains(expected),
            "{aggregate}: {diagnostics:?}"
        );
    }
    let fine = format!(
        "{vocabulary}\nrule probe: ?shop totals ?n\n  when ?shop sells ?_ for ?_\n  and ?n = sum ?c where ?shop sells ?i for ?c\n  and ?low = min ?t where ?shop sells ?i for ?c and ?i is named ?t\n  and ?x = min ?i by ?c where ?shop sells ?i for ?c\n"
    );
    assert_eq!(the_types("shop.gal", &fine).0, Vec::<String>::new());
    let (menu, _) = the_types("menu.gal", include_str!("menu.gal"));
    assert!(!menu.iter().any(|d| d.contains("` takes `")), "{menu:?}");
}
