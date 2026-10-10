use muove_core::*;

fn run(costs: &[(&str, i64)], result: Type, body: Literal) -> Vec<Value> {
    let mut p = Program::builder();
    let cost = p.relation(Relation::new(
        "cost",
        RelationKind::Static,
        [Slot::new("item", Type::Text), Slot::new("amount", Type::Int)],
    ));
    let answer = p.relation(Relation::new(
        "answer",
        RelationKind::Derived,
        [Slot::new("value", result)],
    ));
    for (item, amount) in costs {
        p.fact(Fact::new(cost, [Value::text(*item), Value::Int(*amount)]));
    }
    p.rule(Rule::new("answer", Atom::new(answer, [Expr::var("r")])).when([body]));
    let program = p.build();
    let model = evaluate(&program, &[], Budget::default()).unwrap();
    let engine = engine::evaluate(&program, &[], Budget::default()).unwrap();
    assert_eq!(engine.relations, model.relations);
    model.tuples(answer).map(|tuple| tuple[0].clone()).collect()
}

fn costs(id: usize) -> Literal {
    Atom::new(RelationId(id), [Expr::var("x"), Expr::var("a")]).positive()
}

fn aggregate(op: AggregateOp, costs_: &[(&str, i64)]) -> Vec<Value> {
    run(
        costs_,
        Type::Int,
        Literal::aggregate("r", op, [costs(0)]),
    )
}

fn argmin(costs_: &[(&str, i64)]) -> Vec<Value> {
    run(
        costs_,
        Type::Text,
        Literal::argmin("r", Expr::var("x"), Expr::var("a"), [costs(0)]),
    )
}

#[test]
fn spec_4_3_empty_sum_is_zero() {
    assert_eq!(aggregate(AggregateOp::Sum(Expr::var("a")), &[]), [Value::Int(0)]);
}

#[test]
fn spec_4_3_empty_count_is_zero_and_equal_costs_both_count() {
    assert_eq!(aggregate(AggregateOp::Count, &[]), [Value::Int(0)]);
    assert_eq!(
        aggregate(AggregateOp::Count, &[("a", 1), ("b", 1)]),
        [Value::Int(2)]
    );
}

#[test]
fn spec_4_3_empty_min_binds_nothing() {
    assert_eq!(aggregate(AggregateOp::Min(Expr::var("a")), &[]), []);
}

#[test]
fn spec_4_3_argmin_tie_goes_to_the_smaller_candidate() {
    assert_eq!(
        argmin(&[("c", 1), ("a", 3), ("b", 1)]),
        [Value::text("b")]
    );
}

#[test]
fn spec_4_3_argmin_with_no_candidate_binds_nothing() {
    assert_eq!(argmin(&[]), []);
}

#[test]
fn spec_4_3_sum_of_max_plus_one_minus_one_is_exact_in_both_orders() {
    for order in [
        [("a", i64::MAX), ("b", 1), ("c", -1)],
        [("a", -1), ("b", 1), ("c", i64::MAX)],
    ] {
        assert_eq!(aggregate(AggregateOp::Sum(Expr::var("a")), &order), [Value::Int(i64::MAX)]);
    }
}

#[test]
fn spec_4_5_query_rows_are_records_ranked_by_sort_key() {
    let query = Query::all([Variable::new("x"), Variable::new("a")], [costs(0)])
        .sorted_by(Expr::var("a"));
    let row = |a: i64, x: &str| {
        Value::Record([("a".into(), Value::Int(a)), ("x".into(), Value::text(x))].into())
    };
    assert_eq!(
        run(
            &[("c", 1), ("a", 3), ("b", 2)],
            Type::Record([("a".into(), Type::Int), ("x".into(), Type::Text)].into()).list(),
            Literal::bind("r", query.into()),
        ),
        [Value::List(vec![row(1, "c"), row(2, "b"), row(3, "a")])]
    );
}

#[test]
fn spec_4_5_sort_key_reading_a_condition_only_variable_is_a_static_error() {
    let query = Query::all([Variable::new("x")], [costs(0)]).sorted_by(Expr::var("a"));
    let mut p = Program::builder();
    p.relation(Relation::new(
        "cost",
        RelationKind::Static,
        [Slot::new("item", Type::Text), Slot::new("amount", Type::Int)],
    ));
    let answer = p.relation(Relation::new(
        "answer",
        RelationKind::Derived,
        [Slot::new("value", Type::Text.list())],
    ));
    p.rule(Rule::new("answer", Atom::new(answer, [Expr::var("r")])).when([Literal::bind("r", query.into())]));
    let errors = check(&p.build()).unwrap_err();
    let message = errors[0].to_string();
    assert!(message.contains("sort key ?a reads ?a"), "{message}");
    assert!(!message.contains("nothing grounds"), "{message}");
}
