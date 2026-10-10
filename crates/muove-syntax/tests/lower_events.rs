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

fn actions(source: &str, file: &str) -> Vec<String> {
    use muove_core::ir::{Action, Expr, Program};
    fn expr(e: &Expr) -> String {
        match e {
            Expr::Variable(v) => format!("?{}", v.0),
            Expr::Call { function, arguments } => format!(
                "{}({})",
                function.rsplit('/').next().unwrap(),
                arguments.iter().map(expr).collect::<Vec<_>>().join(", ")
            ),
            Expr::Binary { op, left, right } => format!("{} {op:?} {}", expr(left), expr(right)),
            other => format!("{other:?}"),
        }
    }
    let lowered = lower(source, file).unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let p: &Program = &lowered.program;
    let atom = |kind: &str, a: &muove_core::ir::Atom| {
        let args: Vec<String> = a.arguments.iter().map(expr).collect();
        format!("{kind} {}({})", p.relations[a.relation.0].name, args.join(", "))
    };
    p.handlers
        .iter()
        .flat_map(|h| &h.clauses)
        .flat_map(|c| &c.actions)
        .map(|a| match a {
            Action::Set(x) => atom("Set", x),
            Action::Include(x) => atom("Include", x),
            Action::Withdraw(x) => atom("Withdraw", x),
            Action::Emit(x) => atom("Emit", x),
            Action::Create { variable, entity_type, fields } => {
                let fields: Vec<String> = fields.iter().map(|(k, v)| format!("{k}: {}", expr(v))).collect();
                format!("Create ?{} {entity_type} {{{}}}", variable.0, fields.join(", "))
            }
            Action::Delete(x) => format!("Delete {}", expr(x)),
        })
        .collect()
}

#[test]
fn d42_then_lines_lower_to_the_actions_the_six_verbs_gave() {
    assert_eq!(
        actions(DUNGEON, "dungeon.muove"),
        [
            "Set is-in(?hero, ?there)",
            "Include warned(?trap, ?hero)",
            "Withdraw lies-in(?key, ?room)",
            "Include holds(?hero, ?key)",
            "Include hurt(?trap, ?hero)",
        ]
    );
    assert_eq!(
        actions(MENU, "menu.muove"),
        [
            "Set has-menu-selection(?north, wrap(?selected Add ?delta, ?count))",
            "Delete ?row",
        ]
    );
}

#[test]
fn d43_a_sentence_replaces_only_along_one_mode() {
    let board = "\
type Unit, Tile
state {unit: Unit} is on {tile: Tile}
  exactly one tile per unit
  at most one unit per tile
state {unit: Unit} rests on {tile: Tile}
  exactly one tile per unit
event move: {unit: Unit} moves to {tile: Tile}
on ?u moves to ?there
  when
    ?u is on ?here
  then
    not ?u is on ?here
    ?u is on ?there
    ?u rests on ?there
";
    assert_eq!(
        actions(board, "board.muove"),
        ["Withdraw is-on(?u, ?here)", "Include is-on(?u, ?there)", "Set rests-on(?u, ?there)"]
    );
}

#[test]
fn d42_old_verbs_and_partial_new_are_errors_showing_the_then_form() {
    let old = "\
type Hero, Key
state {hero: Hero} holds {key: Key}
event take: {hero: Hero} takes {key: Key}
on ?x takes ?y
  include ?x holds ?y
";
    let error = lower(old, "old.muove").unwrap_err();
    assert_eq!(error.line, 5);
    assert!(error.message.contains("then\n    ?x holds ?y"), "{}", error.message);

    let partial = MENU.replace(
        "event clear-menu-rows",
        "event add-row: {north: North} adds a row\nevent clear-menu-rows",
    ) + "\non ?north adds a row\n  then\n    ?r is a new MenuRow {key: \"a\"}\n    ?north lists ?r\n";
    let lowered = lower(&partial, "menu.muove").unwrap();
    let messages: Vec<&str> = lowered.diagnostics.iter().map(|d| d.message.as_str()).collect();
    assert!(
        messages.iter().any(|m| m.contains("`label`") && m.contains("`order`") && !m.contains("`key`")),
        "{messages:#?}"
    );
}
