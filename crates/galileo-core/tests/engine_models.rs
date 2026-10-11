use galileo_core::*;
use galileo_core::eval::project;

#[test]
fn engine_models_equal_the_reference_interpreter() {
    for program in [examples::dungeon_static(), examples::tags()] {
        let reference = evaluate(&program, &[], Budget::default()).unwrap();
        let engine = engine::evaluate(&program, &[], Budget::default()).unwrap();
        assert!(!reference.relations.is_empty());
        assert_eq!(engine.relations, reference.relations);
    }
}

#[test]
fn spec_9_7_matching_rows_reads_entity_fields_from_the_evaluated_state() {
    for optional_annotation in [false, true] {
        let mut p = Program::builder();
        p.entity_type("North");
        p.state_entity_type("MenuRow", [
            Slot::new("key", Type::Text),
            Slot::new("label", Type::Text),
            Slot::new("description", Type::Text),
            Slot::new("annotation", if optional_annotation { Type::Text.maybe() } else { Type::Text }),
            Slot::new("order", Type::Int),
        ]);
        let north = Value::entity("North", "north");
        let files = Value::Entity(Entity::created("MenuRow", "Files", 0));
        let logs = Value::Entity(Entity::created("MenuRow", "Logs", 1));
        let lists = p.relation(Relation::new("north/lists", RelationKind::State, [
            Slot::new("north", Type::entity("North")),
            Slot::new("row", Type::entity("MenuRow")),
        ]));
        let query = p.relation(Relation::new("north/menu-query", RelationKind::State, [
            Slot::new("north", Type::entity("North")), Slot::new("query", Type::Text),
        ]).mode(Mode::ExactlyOne { key: 0, values: vec![1] }));
        let matching = p.relation(Relation::new("north/matching-rows", RelationKind::Derived, [
            Slot::new("row", Type::entity("MenuRow")), Slot::new("north", Type::entity("North")),
        ]));
        let mut state = vec![
            Fact::new(lists, [north.clone(), files.clone()]),
            Fact::new(lists, [north.clone(), logs.clone()]),
            Fact::new(query, [north.clone(), Value::text("fi")]),
        ];
        for (name, ty, values) in [
            ("key", Type::Text, [Value::text("files"), Value::text("logs")]),
            ("label", Type::Text, [Value::text("Files"), Value::text("Logs")]),
            ("description", Type::Text, [Value::text(""), Value::text("")]),
            ("annotation", Type::Text, [Value::text(""), Value::text("")]),
            ("order", Type::Int, [Value::Int(0), Value::Int(1)]),
        ] {
            let mode = if name == "annotation" && optional_annotation {
                Mode::AtMostOne { keys: vec![0], values: vec![1] }
            } else {
                Mode::ExactlyOne { key: 0, values: vec![1] }
            };
            let relation = p.relation(Relation::new(format!("north/MenuRow.{name}"), RelationKind::State, [
                Slot::new("row", Type::entity("MenuRow")), Slot::new(name, ty),
            ]).mode(mode));
            if name != "annotation" || !optional_annotation {
                for (row, value) in [files.clone(), logs.clone()].into_iter().zip(values) {
                    state.push(Fact::new(relation, [row, value]));
                }
            }
        }
        let field = |name: &str| Expr::Field { value: Box::new(Expr::var("row")), field: name.into() };
        let text = Expr::binary(BinaryOp::Concat, field("label"),
            Expr::binary(BinaryOp::Concat, Expr::value(" "),
                Expr::binary(BinaryOp::Concat, field("description"),
                    Expr::binary(BinaryOp::Concat, Expr::value(" "), field("key")))));
        p.rule(Rule::new("matching-rows", Atom::new(matching, [Expr::var("row"), Expr::var("north")])).when([
            Atom::new(lists, [Expr::var("north"), Expr::var("row")]).positive(),
            Atom::new(query, [Expr::var("north"), Expr::var("query")]).positive(),
            Literal::Boolean(Expr::call("contains", [
                Expr::call("lowercase", [text]), Expr::call("lowercase", [Expr::var("query")]),
            ])),
        ]));
        let program = p.build();
        let reference = evaluate(&program, &state, Budget::default()).unwrap();
        let engine = engine::evaluate(&program, &state, Budget::default()).unwrap();
        assert_eq!(engine.relations, reference.relations);
        assert_eq!(reference.tuples(matching).cloned().collect::<Vec<_>>(), [vec![files.clone(), north.clone()]]);

        let label = View { name: "label".into(), parameters: vec![Slot::new("row", Type::entity("MenuRow"))],
            fields: vec![(Slot::new("label", Type::Text), field("label"))], source: None };
        let annotation = View { name: "annotation".into(), fields: vec![(Slot::new("annotation", Type::Text.maybe()), field("annotation"))], ..label.clone() };
        for model in [&reference, &engine] {
            assert_eq!(project(&program, model, &label, &[files.clone()], Budget::work(1)).unwrap(),
                Value::Record(std::collections::BTreeMap::from([("label".into(), Value::text("Files"))])));
            assert!(matches!(project(&program, model, &label, &[files.clone()], Budget::work(0)),
                Err(EvalError::Unknown(Reason::OutOfBudget { resource: Resource::Work, .. }))));
            assert_eq!(project(&program, model, &annotation, &[files.clone()], Budget::default()).unwrap(),
                Value::Record(std::collections::BTreeMap::from([("annotation".into(),
                    if optional_annotation { Value::Null } else { Value::text("") })])));
            let values = std::collections::BTreeMap::from([("label".into(), Value::text("Files"))]);
            for value in [Value::Record(values.clone()), Value::Attrs(values.clone())] {
                let view = View { fields: vec![(Slot::new("label", Type::Text), Expr::Field {
                    value: Box::new(Expr::value(value)), field: "label".into(),
                })], ..label.clone() };
                assert_eq!(project(&program, model, &view, &[files.clone()], Budget::work(0)).unwrap(),
                    Value::Record(values.clone()));
            }
        }

        let query_fact = state.iter_mut().find(|fact| fact.relation == query).unwrap();
        query_fact.values[1] = Value::text("lo");
        let reference = evaluate(&program, &state, Budget::default()).unwrap();
        let engine = engine::evaluate(&program, &state, Budget::default()).unwrap();
        assert_eq!(engine.relations, reference.relations);
        assert_eq!(reference.tuples(matching).cloned().collect::<Vec<_>>(), [vec![logs, north]]);
    }
}

#[test]
fn engine_skips_index_keys_the_reference_never_evaluates() {
    let overflow = Expr::binary(BinaryOp::Add, Expr::value(i64::MAX), Expr::value(1_i64));
    for (key, facts) in [
        (overflow.clone(), vec![]),
        (overflow, vec![Fact::new(RelationId(0), [Value::Int(1), Value::Int(0)])]),
        (Expr::call("unimplemented", []), vec![Fact::new(RelationId(0), [Value::Int(1), Value::Int(0)])]),
    ] {
        let mut program = Program::default();
        program.relations = vec![
            Relation::new("input", RelationKind::Static, [Slot::new("a", Type::Int), Slot::new("b", Type::Int)]),
            Relation::new("out", RelationKind::Derived, [Slot::new("value", Type::Int)]),
        ];
        program.facts = facts;
        program.rules.push(
            Rule::new("skipped-key", Atom::new(RelationId(1), [Expr::value(1_i64)]))
                .when([Atom::new(RelationId(0), [Expr::value(0_i64), key]).positive()]),
        );
        let reference = evaluate(&program, &[], Budget::default()).unwrap();
        let engine = engine::evaluate(&program, &[], Budget::default()).unwrap();
        assert_eq!(engine.relations, reference.relations);
    }
}

#[test]
fn engine_cost_on_the_101_room_chain_stays_under_20000_units() {
    let model = engine::evaluate(&examples::chain(101), &[], Budget::default()).unwrap();
    assert!(model.work <= 20_000, "{}", model.work);
}
