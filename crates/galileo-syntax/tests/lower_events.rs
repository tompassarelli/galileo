use galileo_core::ir::{Claim, Literal, StateClaim, Value};
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
