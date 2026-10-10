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
    assert_eq!(counts, (15, 2, 2, 2, 1, 1));
    assert_eq!(
        p.relations[..10].iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["has-menu-kind", "has-menu-query", "has-menu-selection", "lists", "matches-the-menu-query-of", "has-menu-count", "move-menu", "clear-menu-rows", "is-a-North", "is-a-MenuRow"]
    );
    let row = p.entity_types.iter().find(|t| t.name == "MenuRow").unwrap();
    assert!(row.state);
    assert_eq!(row.fields.len(), 5);
    assert_eq!(p.views[0].fields.len(), 4);

    use muove_core::{Action, Budget, Mode, RelationId, RelationKind, Slot, Type, Value};
    use muove_core::state::{step, State};
    let field_names = ["key", "label", "description", "annotation", "order"];
    for (index, name) in field_names.iter().enumerate() {
        let id = p.field_relation("MenuRow", name).unwrap();
        assert_eq!(id, RelationId(10 + index));
        let relation = &p.relations[id.0];
        assert_eq!(relation.name, format!("MenuRow.{name}"));
        assert_eq!(relation.kind, RelationKind::State);
        assert_eq!(relation.modes, [Mode::ExactlyOne { key: 0, values: vec![1] }]);
        assert_eq!(relation.slots, [Slot::new("MenuRow", Type::entity("MenuRow")), Slot::new(*name, if *name == "order" { Type::Int } else { Type::Text })]);
    }
    let linked = muove_syntax::lower::lower_program(&[("menu", MENU)]).unwrap();
    assert!(linked.diagnostics.is_empty(), "{:#?}", linked.diagnostics);
    for name in field_names {
        let id = linked.program.field_relation("MenuRow", name).unwrap();
        assert_eq!(linked.program.relations[id.0].name, format!("menu/MenuRow.{name}"));
    }

    let create = format!("{MENU}\nevent add-row: {{north: North}} adds a row\non ?north adds a row\n  then\n    ?r is a new MenuRow {{key: \"a\", label: \"b\", description: \"\", annotation: \"\", order: 1}}\n");
    for (annotation, expected) in [("annotation: \"\", ", 5), ("annotation: null, ", 4), ("", 4)] {
        let source = if expected == 5 {
            create.clone()
        } else {
            create.replace("annotation: Text", "annotation: maybe Text").replace("annotation: \"\", ", annotation)
        };
        let lowered = lower(&source, "menu.muove").unwrap();
        assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
        let program = &lowered.program;
        let id = program.field_relation("MenuRow", "annotation").unwrap();
        if expected == 4 {
            assert_eq!(program.relations[id.0].modes, [Mode::AtMostOne { keys: vec![0], values: vec![1] }]);
            assert_eq!(program.relations[id.0].slots[1].ty, Type::Text);
        }
        let event = program.relation_id("add-row").unwrap();
        let handler = program.handlers.iter().find(|h| h.event.relation == event).unwrap();
        let Action::Create { fields, .. } = &handler.clauses[0].actions[0] else { panic!("row creation") };
        assert_eq!(fields.len(), expected);
        assert_eq!(fields.contains_key("annotation"), expected == 5);
        let transition = step(program, &State::initial(program), event, &[Value::entity("North", "north")], Budget::default()).unwrap();
        let row = transition.state.live.iter().next().unwrap();
        let mut facts = std::collections::BTreeSet::new();
        for (field, value) in [("key", Value::text("a")), ("label", Value::text("b")), ("description", Value::text("")), ("annotation", Value::text("")), ("order", Value::Int(1))] {
            if field != "annotation" || expected == 5 {
                facts.insert((program.field_relation("MenuRow", field).unwrap(), vec![Value::Entity(row.clone()), value]));
            }
        }
        facts.insert((program.relation_id("is-a-MenuRow").unwrap(), vec![Value::Entity(row.clone())]));
        assert_eq!(transition.state.facts, facts);
        assert_eq!(transition.state.facts.len(), expected + 1);
    }
    let declared = format!("{MENU}\nRow-1 is a MenuRow\n");
    let error = lower(&declared, "menu.muove").unwrap_err();
    assert_eq!(error.line, MENU.lines().count() + 2);
    assert!(error.message.contains("state entity") && error.message.contains("MenuRow"), "{}", error.message);
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
            "Set is-in(?@event0, ?there)",
            "Include warned(?@event0, ?@event1)",
            "Withdraw lies-in(?@event1, ?room)",
            "Include holds(?@event0, ?@event1)",
            "Include hurt(?@event0, ?@event1)",
        ]
    );
    assert_eq!(
        actions(MENU, "menu.muove"),
        [
            "Set has-menu-selection(?@event0, wrap(?selected Add ?@event1, ?count))",
            "Delete ?row",
            "Withdraw is-a-MenuRow(?row)",
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
        ["Withdraw is-on(?@event0, ?here)", "Include is-on(?@event0, ?@event1)", "Set rests-on(?@event0, ?@event1)"]
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

#[test]
fn spec_5_2_handler_alpha_maps_each_clauses_argument() {
    let source = "\
type Hero
state {hero: Hero} is ready
state {hero: Hero} is awake
event wake: {hero: Hero} wakes
on ?first wakes
  then
    ?first is ready
on ?second wakes
  then
    ?second is awake
";
    let lowered = lower(source, "handler-alpha.muove").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let [handler] = lowered.program.handlers.as_slice() else {
        panic!("one handler for wake");
    };
    assert_eq!(lowered.program.relations[handler.event.relation.0].name, "wake");
    assert_eq!(handler.clauses.len(), 2);
    for (clause, relation) in handler.clauses.iter().zip(["is-ready", "is-awake"]) {
        let [muove_core::Action::Include(action)] = clause.actions.as_slice() else {
            panic!("one inclusion per clause");
        };
        assert_eq!(lowered.program.relations[action.relation.0].name, relation);
        assert_eq!(action.arguments, handler.event.arguments);
    }
}

#[test]
fn spec_4_5_view_slots_and_definition_parameters_have_distinct_forms() {
    let source = "\
type Hero
static {hero: Hero} has label {label: Text}
view Find for {needle: Text}
  rows: all ?hero where ?hero has label ?needle
";
    let lowered = lower(source, "view-text-parameter.muove").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let view = &lowered.program.views[0];
    assert_eq!(view.parameters[0].ty, muove_core::Type::Text);
    let muove_core::Expr::Query(query) = &view.fields[0].1 else {
        panic!("the view reads a query");
    };
    let [muove_core::Literal::Positive(atom)] = query.body.as_slice() else {
        panic!("the query reads one sentence");
    };
    assert_eq!(atom.arguments[1], muove_core::Expr::var("needle"));

    let bare = source.replace("has label ?needle", "has label needle");
    let lowered = lower(&bare, "view-text-parameter.muove").unwrap();
    assert!(
        lowered.diagnostics.iter().any(|e| e.line == 4 && e.message.contains("?needle")),
        "{:#?}", lowered.diagnostics
    );

    let lowered = lower("define f(count: Int): Int\n  ?count\n", "define-parameter.muove").unwrap();
    assert!(
        lowered.diagnostics.iter().any(|e| e.message.contains("?count") && e.message.contains("parameter `count`")),
        "{:#?}", lowered.diagnostics
    );
}
