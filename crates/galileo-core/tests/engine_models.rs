use galileo_core::*;

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
