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
