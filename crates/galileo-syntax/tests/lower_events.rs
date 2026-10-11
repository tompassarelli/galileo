use galileo_core::ir::{Claim, Expr, Literal, StateClaim, Value};
use galileo_syntax::lower::lower;

const MENU: &str = include_str!("menu.gal");

#[test]
fn spec_4_1_d61_field_blocks_lower_and_type_without_changing_creation_fields() {
    use galileo_core::{Mode, RelationKind, Slot, Type};
    use galileo_syntax::check::{analyze, SourceFile, Ty};
    use galileo_syntax::schema::Schema;

    let source = "type North\nstate entity MenuRow\n  label: Text\nstate for North\n  menu-kind: Text\n  selected: maybe MenuRow\nderived for MenuRow\n  position: Int\nstate for MenuRow\n  expanded: Bool\nview Menu for North\n  kind: ?north.menu-kind\n  selected: ?north.selected\nview Row for {row: MenuRow}\n  label: ?row.label\n  position: ?row.position\n  expanded: ?row.expanded\n";
    let lowered = lower(source, "fields.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    for (entity, field, kind, ty, optional) in [
        ("North", "menu-kind", RelationKind::State, Type::Text, false),
        ("North", "selected", RelationKind::State, Type::entity("MenuRow"), true),
        ("MenuRow", "position", RelationKind::Derived, Type::Int, false),
        ("MenuRow", "expanded", RelationKind::State, Type::Bool, false),
    ] {
        let id = lowered.program.field_relation(entity, field).unwrap();
        let relation = &lowered.program.relations[id.0];
        assert_eq!(relation.name, format!("{entity}.{field}"));
        assert_eq!(relation.kind, kind);
        assert_eq!(relation.slots, [Slot::new(entity, Type::entity(entity)), Slot::new(field, ty)]);
        assert_eq!(relation.modes, [if optional {
            Mode::AtMostOne { keys: vec![0], values: vec![1] }
        } else {
            Mode::ExactlyOne { key: 0, values: vec![1] }
        }]);
    }
    let row = lowered.program.entity_types.iter().find(|t| t.name == "MenuRow").unwrap();
    assert_eq!(row.fields, [Slot::new("label", Type::Text)]);
    let file = SourceFile::parse("fields.gal", source).unwrap();
    let (diagnostics, index) = analyze(&[file], &Schema::default());
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    for (expr, ty) in [
        ("?north.menu-kind", Ty::Text),
        ("?north.selected", Ty::Maybe(Box::new(Ty::Declared("MenuRow".into())))),
        ("?row.label", Ty::Text), ("?row.position", Ty::Int), ("?row.expanded", Ty::Bool),
    ] {
        assert_eq!(index.at("fields.gal", source.find(expr).unwrap()).unwrap().ty, ty);
    }
    assert!(galileo_syntax::read_sentences(include_str!("../../../lib/firn/tags.gal")).is_ok());
    let linked = galileo_syntax::lower::lower_program(&[("fields", source), ("consumer", "use fields\nview Menu for North\n  kind: ?north.menu-kind\n")]).unwrap();
    assert!(linked.diagnostics.is_empty(), "{:#?}", linked.diagnostics);
    assert!(linked.program.relations.iter().any(|r| r.name == "fields/North.menu-kind"));

    let declarations = "type North\nnorth-main is a North\nstate entity MenuRow\n  key: Text\n  order: Int\nstate for North\n  menu-kind: Text\n  menu-title: Text\n  menu-help: Text\n  menu-empty: Text\n  menu-row-count: Int\n  selected: maybe Text\nstate for MenuRow\n  expanded: Bool\nderived for MenuRow\n  position: Int\nstate North lists MenuRow\nevent open-settings-menu North {kind: Text} {title: Text}\nevent offer-row North {key: Text}\nevent clear-selection North\n";
    let posts = "rule menu-row-position: ?row.position = ?rank\n  when ?north lists ?row\n  and ?rank = count where ?north lists ?prior and ?prior.order < ?row.order\ndefine menu-help(kind: Text): Text\n  if kind == \"document\" then \"↑/↓ scroll · Esc close · /work claim, release, need, unneed or close to edit\"\n  else if kind == \"work\" then \"Type to filter · ↑/↓ choose · Enter open · Esc close · /work search TEXT includes closed issues\"\n  else \"Type to search · ↑/↓ choose · Enter open · Esc close\"\ninitially\n  north-main.menu-kind = \"\"\n  north-main.menu-title = \"\"\n  north-main.menu-help = menu-help(\"\")\n  north-main.menu-empty = \"No matching conversations\"\n  north-main.menu-row-count = 0\n  north-main.selected = \"seed\"\non open-settings-menu ?north ?kind ?title\n  then ?north.menu-kind = ?kind\n  and ?north.menu-title = ?title\n  and ?north.menu-row-count = 0\n  and ?north.menu-help = menu-help(?kind)\n  and ?north.menu-empty = if ?kind == \"work\" then \"No matching work items\" else \"No matching entries\"\non offer-row ?north ?key\n  then ?north.menu-row-count = ?north.menu-row-count + 1\n  and ?north lists ?row\n  and ?row is a new MenuRow {key: ?key, order: ?north.menu-row-count, expanded: true}\non clear-selection ?north\n  then ?north.selected = null\n";
    let fields = lower(&format!("{declarations}{posts}"), "posts.gal").unwrap();
    assert!(fields.diagnostics.is_empty(), "{:#?}", fields.diagnostics);
    let reference_declarations = declarations.replace("state for North\n  menu-kind: Text\n  menu-title: Text\n  menu-help: Text\n  menu-empty: Text\n  menu-row-count: Int\n  selected: maybe Text\n", "state North menu kind {menu-kind: Text}\n  exactly one menu-kind per north\nstate North menu title {menu-title: Text}\n  exactly one menu-title per north\nstate North menu help {menu-help: Text}\n  exactly one menu-help per north\nstate North menu empty {menu-empty: Text}\n  exactly one menu-empty per north\nstate North menu row count {menu-row-count: Int}\n  exactly one menu-row-count per north\nstate North selected {selected: Text}\n  at most one selected per north\n").replace("derived for MenuRow\n  position: Int\n", "derived MenuRow position {position: Int}\n  exactly one position per menu-row\n");
    let reference_posts = posts.replace("north-main.menu-help = menu-help(\"\")", "north-main menu help \"Type to search · ↑/↓ choose · Enter open · Esc close\"")
        .replace("?north.menu-empty = if ?kind == \"work\" then \"No matching work items\" else \"No matching entries\"", "?north menu empty (if ?kind == \"work\" then \"No matching work items\" else \"No matching entries\")")
        .replace("?north.menu-row-count = ?north.menu-row-count + 1", "?north menu row count ((the ?count where ?north menu row count ?count) + 1)")
        .replace("order: ?north.menu-row-count", "order: the ?count where ?north menu row count ?count")
        .replace("north-main.menu-kind =", "north-main menu kind").replace("north-main.menu-title =", "north-main menu title")
        .replace("north-main.menu-empty =", "north-main menu empty").replace("north-main.menu-row-count =", "north-main menu row count")
        .replace("north-main.selected =", "north-main selected").replace("?north.menu-kind =", "?north menu kind")
        .replace("?north.menu-title =", "?north menu title").replace("?north.menu-row-count =", "?north menu row count")
        .replace("?north.menu-help =", "?north menu help").replace("?north.selected = null", "not ?north selected (the ?value where ?north selected ?value)")
        .replace("?row.position =", "?row position");
    let reference = lower(&format!("{reference_declarations}{reference_posts}"), "reference.gal").unwrap();
    assert!(reference.diagnostics.is_empty(), "{:#?}", reference.diagnostics);
    assert_eq!(fields.program.relations.len(), reference.program.relations.len());
    for (actual, expected) in fields.program.relations.iter().zip(&reference.program.relations) {
        assert_eq!(actual.kind, expected.kind);
        assert_eq!(actual.modes, expected.modes);
        assert_eq!(actual.slots.iter().map(|slot| &slot.ty).collect::<Vec<_>>(), expected.slots.iter().map(|slot| &slot.ty).collect::<Vec<_>>());
    }
    assert_eq!(fields.program.initially.len(), reference.program.initially.len());
    for (actual, expected) in fields.program.initially.iter().zip(&reference.program.initially) {
        assert_eq!(actual.relation, expected.relation);
        assert_eq!(actual.values, expected.values);
    }
    assert_eq!(fields.program.rules[0].head.arguments, reference.program.rules[0].head.arguments);
    assert_eq!(fields.program.rules[0].head.relation, reference.program.rules[0].head.relation);
    let normalized = |body: &[Literal]| body.iter().cloned().map(|mut literal| {
        if let Literal::Positive(atom) | Literal::Negative(atom) = &mut literal { atom.source = None; }
        if let Literal::Aggregate { body, .. } = &mut literal {
            for literal in body {
                if let Literal::Positive(atom) | Literal::Negative(atom) = literal { atom.source = None; }
            }
        }
        literal
    }).collect::<Vec<_>>();
    assert_eq!(normalized(&fields.program.rules[0].body), normalized(&reference.program.rules[0].body));
    let actual_actions = &fields.program.handlers[0].clauses[0].actions;
    let expected_actions = &reference.program.handlers[0].clauses[0].actions;
    assert_eq!(actual_actions.len(), expected_actions.len());
    for (actual, expected) in actual_actions.iter().zip(expected_actions) {
        let galileo_core::Action::Set(actual) = actual else { panic!("field replacing post"); };
        let galileo_core::Action::Set(expected) = expected else { panic!("sentence replacing post"); };
        assert_eq!(actual.relation, expected.relation);
        assert_eq!(actual.arguments, expected.arguments);
    }
    let program = &fields.program;
    let kind = program.field_relation("North", "menu-kind").unwrap();
    let selected = program.field_relation("North", "selected").unwrap();
    assert_eq!(program.initially[0].relation, kind);
    assert_eq!(program.initially[0].values, [Value::entity("North", "north-main"), Value::text("")]);
    let clause = &program.handlers[0].clauses[0];
    assert!(matches!(&clause.actions[0], galileo_core::Action::Set(atom) if atom.relation == kind && atom.arguments == [Expr::var("@event0"), Expr::var("@event1")]));
    assert!(matches!(&program.handlers[2].clauses[0].actions[0], galileo_core::Action::Withdraw(atom) if atom.relation == selected));
    assert!(matches!(&program.handlers[1].clauses[0].actions[2], galileo_core::Action::Create { fields, .. } if fields.iter().any(|(name, value)| name == "expanded" && *value == Expr::value(Value::Bool(true)))));
    assert_eq!(program.rules[0].head.relation, program.field_relation("MenuRow", "position").unwrap());
    assert_eq!(program.rules[0].head.arguments, [Expr::var("row"), Expr::var("rank")]);
    for (source, line, key) in [
        (format!("{declarations}on offer-row ?north ?key\n  when ?north lists ?row\n  then ?row.key = \"x\"\n"), declarations.lines().count() + 3, "MenuRow.key"),
        (format!("{declarations}on open-settings-menu ?north ?kind ?title\n  when ?north.menu-kind = \"x\"\n  then ?north.menu-kind = ?kind\n"), declarations.lines().count() + 2, "?north.menu-kind"),
        (format!("{declarations}on offer-row ?north ?key\n  then ?north.menu-kind = null\n"), declarations.lines().count() + 2, "?north.menu-kind"),
    ] {
        let error = lower(&source, "errors.gal").unwrap_err();
        assert_eq!((error.line, error.column), (line, 3));
        assert!(error.message.contains(key), "{error:?}");
    }
}

#[test]
fn spec_6_2_d60_claim_binding_preserves_universal_subjects_and_temporal_scope() {
    use galileo_core::{claims::check_claims, Budget, Verdict};

    let declarations = "type Node, Bit\nA is a Node\nB is a Node\nSeed is a Node\nA1 is a Bit\nA2 is a Bit\nB1 is a Bit\nS1 is a Bit\ngiven {node: Node} lit\ngiven {node: Node} has {bit: Bit}\nderived {node: Node} holds\nrule supported: ?n holds\n  when ?n has ?b\nA has A1\nA has A2\nB has B1\nSeed has S1\nSeed lit\n";
    for (claim, expected) in [
        ("?n holds has no single point of failure among ?n has ?b", "B"),
        ("never ?x lit and ?x holds", "Seed"),
    ] {
        let lowered = lower(&format!("{declarations}requirements\n  {claim}\n"), "binding.gal").unwrap();
        assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
        let results = check_claims(&lowered.program, Budget::default()).unwrap();
        let Verdict::Fails(counterexample) = &results[0].verdict else {
            panic!("expected the stated counterexample: {results:?}");
        };
        assert!(counterexample.values().any(|value| matches!(value, Value::Entity(entity) if entity.name == expected)), "{counterexample:?}");
    }

    let lowered = lower(&format!("{declarations}requirements\n  ?k > 0 and ?k = count where ?m holds\n"), "binding.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let results = check_claims(&lowered.program, Budget::default()).unwrap();
    let [result] = results.as_slice() else {
        panic!("one aggregate claim");
    };
    assert!(matches!(result.verdict, Verdict::Holds(_)));

    let source = format!("{declarations}state {{node: Node}} awake\nevent poke: {{node: Node}} is poked\non ?n is poked\n  when ?m lit\n  and ?k = 1\n  then ?n awake\n  ensures\n    ?n awake and ?m lit and ?k > 0\nrequirements\n  ?n is poked only when ?n lit\n");
    let lowered = lower(&source, "binding.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let Claim::EveryEventWhen { event, condition: StateClaim::Some { variables, body } } = &lowered.program.requirements[0].claim else {
        panic!("one event condition");
    };
    assert!(variables.is_empty());
    let [Literal::Positive(condition)] = body.as_slice() else {
        panic!("the lit condition");
    };
    assert_eq!(condition.arguments, event.arguments);
    let [StateClaim::Some { variables, .. }] = lowered.program.handlers[0].clauses[0].ensures.as_slice() else {
        panic!("one clause contract");
    };
    assert!(variables.is_empty());
}

#[test]
fn spec_4_1_optional_creation_fields_may_be_omitted_or_null() {
    use galileo_core::{Action, Budget, Mode, Type, Value};
    use galileo_core::state::{step, State};
    let create = format!("{MENU}\nevent add-row: {{north: North}} adds a row\non ?north adds a row\n  then ?r is a new MenuRow {{key: \"a\", label: \"b\", description: \"\", annotation: \"\", order: 1}}\n");
    for (annotation, expected) in [("annotation: \"\", ", 5), ("annotation: null, ", 4), ("", 4)] {
        let source = if expected == 5 {
            create.clone()
        } else {
            create.replace("annotation: Text", "annotation: maybe Text").replace("annotation: \"\", ", annotation)
        };
        let lowered = lower(&source, "menu.gal").unwrap();
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
}

fn actions(source: &str, file: &str) -> Vec<String> {
    use galileo_core::ir::{Action, Expr, Program};
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
    let atom = |kind: &str, a: &galileo_core::ir::Atom| {
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
  when ?u is on ?here
  then not ?u is on ?here
  and ?u is on ?there
  and ?u rests on ?there
";
    assert_eq!(
        actions(board, "board.gal"),
        ["Withdraw is-on(?@event0, ?here)", "Include is-on(?@event0, ?@event1)", "Set rests-on(?@event0, ?@event1)"]
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
  then ?first is ready
on ?second wakes
  then ?second is awake
";
    let lowered = lower(source, "handler-alpha.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let [handler] = lowered.program.handlers.as_slice() else {
        panic!("one handler for wake");
    };
    assert_eq!(lowered.program.relations[handler.event.relation.0].name, "wake");
    assert_eq!(handler.clauses.len(), 2);
    for (clause, relation) in handler.clauses.iter().zip(["is-ready", "is-awake"]) {
        let [galileo_core::Action::Include(action)] = clause.actions.as_slice() else {
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
given {hero: Hero} has label {label: Text}
view Find for {needle: Text}
  rows: all ?hero where ?hero has label ?needle
";
    let lowered = lower(source, "view-text-parameter.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let view = &lowered.program.views[0];
    assert_eq!(view.parameters[0].ty, galileo_core::Type::Text);
    let galileo_core::Expr::Query(query) = &view.fields[0].1 else {
        panic!("the view reads a query");
    };
    let [galileo_core::Literal::Positive(atom)] = query.body.as_slice() else {
        panic!("the query reads one sentence");
    };
    assert_eq!(atom.arguments[1], galileo_core::Expr::var("needle"));
}

#[test]
fn d61_addendum_3_an_aggregate_in_a_head_lowers_to_the_bound_rule_of_examples_north_menu() {
    let original = include_str!("../../../examples/north-menu.gal");
    let in_place = original
        .replace("rule menu-count: ?north menu count ?matched\n  when ?north is a North\n  and ?matched = sum 1 where ?north known menu row ?row and ?row menu row visible true", "rule menu-count: ?north menu count (sum 1 where ?north known menu row ?row and ?row menu row visible true)\n  when ?north is a North")
        .replace("rule menu-row-position: ?row menu row position ?rank\n", "rule menu-row-position: ?row menu row position (sum 1 where ?north known menu row ?prior and ?prior.order < ?order and ?prior menu row visible true)\n")
        .replace("\n  and ?rank = sum 1 where ?north known menu row ?prior and ?prior.order < ?order and ?prior menu row visible true", "");
    assert_ne!(in_place, original);
    let unspanned = |text: String| {
        let mut out = String::new();
        let mut rest = text.as_str();
        while let Some(at) = rest.find("source: Some(") {
            out.push_str(&rest[..at]);
            let mut depth = 0;
            let mut end = at;
            for (i, c) in rest[at..].char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => { depth -= 1; if depth == 0 { end = at + i + 1; break; } }
                    _ => {}
                }
            }
            rest = &rest[end..];
        }
        out.push_str(rest);
        out
    };
    let rules = |source: &str, bound: &[&str]| {
        let lowered = lower(source, "north-menu.gal").unwrap();
        assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
        ["north-menu/menu-count", "menu-count", "north-menu/menu-row-position", "menu-row-position"]
            .iter()
            .filter_map(|name| lowered.program.rules.iter().find(|rule| rule.name == *name))
            .map(|rule| {
                let mut text = unspanned(format!("{:?}", (&rule.head, &rule.body)));
                for name in bound {
                    text = text.replace(&format!("Variable(\"{name}\")"), "Variable(\"@\")");
                }
                for n in 1..=9 {
                    text = text.replace(&format!("Variable(\"#aggregate{n}\")"), "Variable(\"@\")");
                }
                text
            })
            .collect::<Vec<_>>()
    };
    let before = rules(original, &["matched", "rank"]);
    assert_eq!(before.len(), 2);
    assert_eq!(rules(&in_place, &[]), before);
}

#[test]
fn spec_4_5_and_9_7_view_fields_carry_the_checkers_types() {
    use galileo_core::Type;
    let lowered = lower(MENU, "menu.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let menu = lowered.program.views.iter().find(|v| v.name == "Menu").unwrap();
    let typed: Vec<_> = menu.fields.iter().map(|(slot, _)| (slot.name.as_str(), slot.ty.clone())).collect();
    assert_eq!(typed, [
        ("kind", Type::Text),
        ("query", Type::Text),
        ("selection", Type::Int),
        ("rows", Type::entity("MenuRow").list()),
    ]);
    let untypable = MENU.replace("  kind: ?north", "  x: the ?r where ?north lists ?r\n  kind: ?north");
    let errors: Vec<_> = lower(&untypable, "menu.gal").unwrap().diagnostics.iter().map(ToString::to_string).collect();
    assert!(errors.iter().any(|e| e.contains("view field `x`")), "{errors:?}");
    let misspelt = MENU.replace("  rows: all ?row where", "  first: all ?row.ordr where");
    let errors: Vec<_> = lower(&misspelt, "menu.gal").unwrap().diagnostics.iter().map(ToString::to_string).collect();
    assert!(errors.iter().any(|e| e.contains("did you mean `order`?")), "{errors:?}");
}

#[test]
fn spec_6_2_d70_before_marks_one_sentence_and_ensures_reads_one_state() {
    let program = "type Node\nA is a Node\nstate {node: Node} lit\nstate {node: Node} awake\nevent poke: {node: Node} is poked\non ?n is poked\n  when ?m lit\n  and ?k = 1\n  then ?n awake\n  ensures\n    ";
    let at = program.lines().count();
    let ensures = |claim: &str| lower(&format!("{program}{claim}\n"), "marks.gal");
    let marked = ensures("before ?n awake and before ?m lit").unwrap();
    let unmarked = ensures("?n awake and ?m lit").unwrap();
    assert!(marked.diagnostics.is_empty() && unmarked.diagnostics.is_empty(), "{:#?}", marked.diagnostics);
    let [StateClaim::Before(inner)] = marked.program.handlers[0].clauses[0].ensures.as_slice() else {
        panic!("one pre-state claim");
    };
    let unsourced = |claim: &StateClaim| {
        let StateClaim::Some { variables, body } = claim.clone() else { panic!("a conjunction") };
        let body = body.into_iter().map(|literal| match literal {
            Literal::Positive(atom) => Literal::Positive(galileo_core::ir::Atom { source: None, ..atom }),
            other => other,
        }).collect::<Vec<_>>();
        (variables, body)
    };
    assert_eq!(unsourced(inner), unsourced(&unmarked.program.handlers[0].clauses[0].ensures[0]));
    for (claim, says) in [
        ("before ?n awake and ?m lit", "one state"),
        ("after ?n awake", "`?n awake`"),
        ("not before ?n awake", "`before not ?n awake`"),
        ("before ?k > 0", "only a sentence"),
    ] {
        let error = ensures(claim).unwrap_err();
        assert_eq!(error.line, at, "{claim}: {error:?}");
        assert!(error.message.contains(says), "{claim}: {error:?}");
    }
    let requirement = lower("type Node\nA is a Node\nstate {node: Node} awake\nrequirements\n  every ?n where before ?n awake: ?n awake\n", "marks.gal").unwrap_err();
    assert_eq!(requirement.line, 5);
    assert!(requirement.message.contains("pre-state"), "{requirement:?}");
}
