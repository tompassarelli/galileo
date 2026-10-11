use galileo_syntax::check::SourceFile;

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
    let chain = format!(
        "{vocabulary}view Boss for {{shop: Shop}}\n  name: the ?n where shop has manager ?p and ?p is named ?n\n"
    );
    assert_eq!(the_types("shop.gal", &chain), (vec![], vec!["maybe Text".to_string()]));
}

#[test]
fn decisions_d_45_narrowing_for_sources_and_ordering_in_a_kanata_copy() {
    let default = include_str!("../../../tests/external/kanata/default.gal");
    let kanata = include_str!("../../../tests/external/kanata/kanata.gal");
    let diagnostics = |line: &str| {
        let edited = kanata.replace("          port: cfg.port when cfg.port != null\n", &format!("          {line}\n"));
        let files = [SourceFile::parse("default.gal", default).unwrap(), SourceFile::parse("kanata.gal", &edited).unwrap()];
        galileo_syntax::check::check(&files, &Default::default()).iter().map(ToString::to_string).collect::<Vec<_>>()
    };
    for clean in [
        "port: cfg.port + 1 when cfg.port != null",
        "port: ((m) => if m != null then m + 1 else 0)(cfg.port)",
        "port: if cfg.port == null then 0 else cfg.port + 1",
        "port: cfg.enable and cfg.port != null and cfg.port > 1",
        "port: if \"a\" < \"b\" then \"x\" else \"y\"",
    ] {
        assert_eq!(diagnostics(clean), Vec::<String>::new(), "{clean}");
    }
    for (line, at) in [
        ("port: cfg.port + 1 when cfg.enable", "cfg.port"),
        ("port: 1 < \"b\"", "1"),
        ("for ?d in cfg.devices\n            port: 1", "cfg.devices"),
    ] {
        let found = diagnostics(line);
        assert_eq!(found.len(), 1, "{line}: {found:?}");
        let column = 11 + line.find(at).unwrap();
        assert!(found[0].starts_with(&format!("kanata.gal:22:{column}:")), "{line}: {found:?}");
    }
}
