use muove_syntax::lower::lower;

const DUNGEON: &str = include_str!("dungeon.muove");
const MENU: &str = include_str!("menu.muove");

#[test]
fn spec_5_6_dungeon_and_9_7_menu_lower_with_no_diagnostics() {
    let dungeon = lower(DUNGEON, "dungeon.muove").unwrap();
    assert!(dungeon.diagnostics.is_empty(), "{:#?}", dungeon.diagnostics);
    let p = &dungeon.program;
    let counts = (p.relations.len(), p.rules.len(), p.events.len(), p.handlers.len(), p.requirements.len());
    assert_eq!(counts, (19, 2, 3, 3, 5));
    assert_eq!(p.handlers[0].clauses.len(), 2);

    let menu = lower(MENU, "menu.muove").unwrap();
    assert!(menu.diagnostics.is_empty(), "{:#?}", menu.diagnostics);
    let p = &menu.program;
    let counts = (p.relations.len(), p.rules.len(), p.events.len(), p.handlers.len(), p.definitions.len(), p.views.len());
    assert_eq!(counts, (10, 2, 2, 2, 1, 1));
    let row = p.entity_types.iter().find(|t| t.name == "MenuRow").unwrap();
    assert!(row.state);
    assert_eq!(row.fields.len(), 5);
    assert_eq!(p.views[0].fields.len(), 4);
}
