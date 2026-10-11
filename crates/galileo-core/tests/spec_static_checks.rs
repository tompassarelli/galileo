use galileo_core::*;

fn relation(builder: &mut ProgramBuilder, name: &str) -> RelationId {
    builder.relation(
        Relation::new(
            name,
            RelationKind::Derived,
            [Slot::new("value", Type::Text)],
        )
        .template(format!("{{value}} is {name}")),
    )
}
fn atom(relation: RelationId, variable: &str) -> Atom {
    Atom::new(relation, [Expr::var(variable)])
}
#[derive(Debug, PartialEq)]
enum Shape {
    Grounds(Vec<String>),
    Cycle(Vec<(String, DependencyKind, String)>),
    Invents(String),
    Other(StaticErrorKind),
}
use DependencyKind::{Negation, Positive, Query as Through};
fn grounds(variables: &[&str]) -> Shape {
    Shape::Grounds(variables.iter().map(|v| v.to_string()).collect())
}
fn cycle(steps: &[(&str, DependencyKind, &str)]) -> Shape {
    Shape::Cycle(steps.iter().map(|(h, k, b)| (h.to_string(), *k, b.to_string())).collect())
}
fn invents(slot: &str) -> Shape {
    Shape::Invents(slot.to_string())
}
fn shape(error: &StaticError) -> (String, Shape) {
    let shape = match &error.kind {
        StaticErrorKind::Grounding { variables, .. } => Shape::Grounds(variables.iter().map(|v| v.0.clone()).collect()),
        StaticErrorKind::Stratification { cycle } => Shape::Cycle(cycle.clone()),
        StaticErrorKind::ValueInvention { slot, .. } => Shape::Invents(slot.clone()),
        other => Shape::Other(other.clone()),
    };
    (error.owner.clone(), shape)
}
fn shapes(program: Program) -> Vec<(String, Shape)> {
    check(&program).unwrap_err().iter().map(shape).collect()
}
fn expect(owner: &str, shape: Shape) -> (String, Shape) {
    (owner.to_string(), shape)
}
fn messages(program: Program) -> String {
    check(&program)
        .unwrap_err()
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn spec_4_3_membership_cannot_grow_text_in_recursion() {
    let mut p = Program::builder();
    let r = relation(&mut p, "text");
    p.rule(Rule::new("grow-text", atom(r, "next")).when([
        atom(r, "text").positive(),
        Literal::member(
            "next",
            Expr::List(vec![Expr::binary(
                BinaryOp::Concat,
                Expr::var("text"),
                Expr::value("x"),
            )]),
        ),
    ]));
    assert_eq!(shapes(p.build()), [expect("grow-text", invents("value"))]);
}

#[test]
fn spec_4_3_query_through_definition_cannot_hide_negative_cycle() {
    let mut p = Program::builder();
    let a = relation(&mut p, "a");
    let b = relation(&mut p, "b");
    p.definition(Definition {
        name: "rows".into(),
        parameters: vec![],
        result: Type::Text.list(),
        effect: EffectLevel::Total,
        source: None,
        body: Query::all([Variable::new("row")], [atom(b, "row").positive()]).into(),
    });
    p.definition(Definition {
        name: "indirect-rows".into(),
        parameters: vec![],
        result: Type::Text.list(),
        effect: EffectLevel::Total,
        source: None,
        body: Expr::call("rows", []),
    });
    p.rule(
        Rule::new("query-edge", atom(a, "x"))
            .when([Literal::member("x", Expr::call("indirect-rows", []))]),
    );
    p.rule(
        Rule::new("negative-edge", Atom::new(b, [Expr::value("x")])).when([Atom::new(
            a,
            [Expr::value("x")],
        )
        .negative()]),
    );
    let result = shapes(p.build());
    assert!(result.contains(&expect("query-edge", cycle(&[("a", Through, "b"), ("b", Negation, "a")]))), "{result:?}");
    assert!(result.contains(&expect("negative-edge", cycle(&[("b", Negation, "a"), ("a", Through, "b")]))), "{result:?}");
    let mut p = Program::builder();
    let chosen = relation(&mut p, "chosen");
    let input = p.relation(Relation::new(
        "input",
        RelationKind::Static,
        [Slot::new("value", Type::Text)],
    ));
    p.definition(Definition {
        name: "unchosen".into(),
        parameters: vec![],
        result: Type::Text.list(),
        effect: EffectLevel::Total,
        source: None,
        body: Query::all(
            [Variable::new("row")],
            [
                atom(input, "row").positive(),
                atom(chosen, "row").negative(),
            ],
        )
        .into(),
    });
    p.rule(
        Rule::new("hidden-not", atom(chosen, "x"))
            .when([Literal::member("x", Expr::call("unchosen", []))]),
    );
    assert_eq!(shapes(p.build()), [expect("hidden-not", cycle(&[("chosen", Through, "chosen")]))]);
}

#[test]
fn spec_4_3_mutual_bindings_have_no_grounding_schedule() {
    let mut p = Program::builder();
    let r = relation(&mut p, "r");
    p.rule(
        Rule::new("binding-cycle", atom(r, "x"))
            .at(SourceSpan {
                file: "binding-cycle.gal".into(),
                start: 0,
                end: 40,
                line: 3,
                column: 1,
            })
            .when([
                Literal::bind("x", Expr::var("y")),
                Literal::bind("y", Expr::var("x")),
            ]),
    );
    let program = p.build();
    let errors = check(&program).unwrap_err();
    let source = errors[0].source.as_ref().unwrap();
    assert_eq!((source.line, source.column), (3, 1));
    assert_eq!(errors.iter().map(shape).collect::<Vec<_>>(), [expect("binding-cycle", grounds(&["y"]))]);
    let mut located = program.clone();
    let mut literal_source = source.clone();
    literal_source.line = 7;
    literal_source.column = 3;
    located.rules[0].body_sources = vec![literal_source];
    let errors = check(&located).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].source.as_ref().map(|source| (source.line, source.column)), Some((7, 3)));
    assert_eq!(errors.iter().map(shape).collect::<Vec<_>>(), [expect("binding-cycle", grounds(&["y"]))]);
}

#[test]
fn spec_4_3_plain_negation_cycle_prints_the_cycle() {
    let mut p = Program::builder();
    let a = relation(&mut p, "a");
    let b = relation(&mut p, "b");
    p.rule(
        Rule::new("a-if-not-b", Atom::new(a, [Expr::value("x")])).when([Atom::new(
            b,
            [Expr::value("x")],
        )
        .negative()]),
    );
    p.rule(Rule::new("b-if-a", atom(b, "x")).when([atom(a, "x").positive()]));
    assert_eq!(shapes(p.build()), [expect("a-if-not-b", cycle(&[("a", Negation, "b"), ("b", Positive, "a")]))]);
}

#[test]
fn spec_4_3_fixture_programs_are_safe_and_body_order_does_not_matter() {
    for mut program in [
        examples::dungeon_static(),
        examples::tags(),
        examples::r13(),
    ] {
        for rule in &mut program.rules {
            rule.body.reverse();
        }
        check(&program).unwrap();
    }
    let mut p = Program::builder();
    let r = relation(&mut p, "r");
    let input = p.relation(Relation::new(
        "input",
        RelationKind::Static,
        [Slot::new("value", Type::Text)],
    ));
    p.rule(Rule::new("schedule", atom(r, "x")).when([
        atom(input, "x").negative(),
        Literal::Boolean(Expr::call("contains", [Expr::var("x"), Expr::value("a")])),
        Literal::bind("x", Expr::var("y")),
        atom(input, "y").positive(),
    ]));
    let program = p.build();
    check(&program).unwrap();
    for literal in [
        Literal::Compare { op: CompareOp::Equal, left: Expr::var("x"), right: Expr::value("a") },
        Literal::Boolean(Expr::call("contains", [Expr::var("x"), Expr::value("a")])),
        Literal::bind("result", Expr::var("x")),
        atom(input, "x").negative(),
    ] {
        let mut handler_program = program.clone();
        let mut clause = HandlerClause::new().when([literal.clone()]).at(SourceSpan {
            file: "handler.gal".into(), start: 0, end: 10, line: 3, column: 1,
        });
        clause.body_sources.push(SourceSpan {
            file: "handler.gal".into(), start: 10, end: 20, line: 4, column: 3,
        });
        handler_program.handlers.push(Handler::new(atom(input, "event")).clause(clause));
        let errors = check(&handler_program).unwrap_err();
        assert_eq!(errors.iter().map(shape).collect::<Vec<_>>(), [expect("handler input", grounds(&["x"]))]);
        assert_eq!(errors[0].source.as_ref().map(|source| (source.line, source.column)), Some((4, 3)));
        handler_program.handlers[0].event = atom(input, "x");
        check(&handler_program).unwrap();
        handler_program.handlers[0].event = atom(input, "event");
        handler_program.handlers[0].clauses[0].body.push(atom(input, "x").positive());
        check(&handler_program).unwrap();
    }
}

#[test]
fn spec_4_3_computed_heads_calls_and_aggregates_respect_recursive_inputs() {
    for expression in [
        Expr::binary(BinaryOp::Concat, Expr::var("x"), Expr::value("x")),
        Expr::call("uppercase", [Expr::var("x")]),
    ] {
        let mut p = Program::builder();
        let r = relation(&mut p, "r");
        p.rule(Rule::new("invent", Atom::new(r, [expression])).when([atom(r, "x").positive()]));
        assert!(messages(p.build()).contains("value invention in recursion"));
    }
    let mut p = Program::builder();
    let a = relation(&mut p, "a");
    p.rule(
        Rule::new("aggregate-cycle", atom(a, "n")).when([Literal::aggregate(
            "n",
            AggregateOp::Count,
            [atom(a, "x").positive()],
        )]),
    );
    assert!(messages(p.build()).contains("a reads a through an aggregate"));
}

#[test]
fn spec_4_3_nested_queries_aggregates_and_argmin_ground_their_results() {
    let mut p = Program::builder();
    let input = p.relation(Relation::new(
        "input",
        RelationKind::Static,
        [Slot::new("value", Type::Text)],
    ));
    let output = relation(&mut p, "output");
    p.rule(Rule::new("nested", atom(output, "selected")).when([
        Literal::argmin(
            "selected",
            Expr::var("candidate"),
            Expr::var("candidate"),
            [Literal::member(
                "candidate",
                Query::all([Variable::new("local")], [atom(input, "local").positive()]).into(),
            )],
        ),
        Literal::aggregate(
            "count",
            AggregateOp::Count,
            [atom(input, "x").positive()],
        ),
    ]));
    check(&p.build()).unwrap();
    let mut p = Program::builder();
    let output = relation(&mut p, "output");
    p.rule(
        Rule::new("unsafe-query", atom(output, "row")).when([Literal::member(
            "row",
            Query::all(
                [Variable::new("x")],
                [
                    Literal::bind("x", Expr::var("y")),
                    Literal::bind("y", Expr::var("x")),
                ],
            )
            .into(),
        )]),
    );
    assert!(messages(p.build()).contains("nothing grounds"));
}

#[test]
fn spec_4_3_lower_relation_values_allow_finite_recursive_computation() {
    let mut p = Program::builder();
    let input = p.relation(Relation::new(
        "input",
        RelationKind::Static,
        [Slot::new("value", Type::Text)],
    ));
    let r = relation(&mut p, "r");
    p.rule(
        Rule::new(
            "finite",
            Atom::new(r, [Expr::call("uppercase", [Expr::var("x")])]),
        )
        .when([atom(r, "existing").positive(), atom(input, "x").positive()]),
    );
    check(&p.build()).unwrap();
}

#[test]
fn spec_4_3_recursive_aliases_are_safe_but_membership_is_not() {
    let mut p = Program::builder();
    let r = relation(&mut p, "r");
    p.rule(Rule::new("alias", atom(r, "z")).when([
        Literal::bind("z", Expr::var("y")),
        Literal::bind("y", Expr::var("x")),
        atom(r, "x").positive(),
    ]));
    check(&p.build()).unwrap();
    let mut p = Program::builder();
    let r = relation(&mut p, "r");
    p.rule(Rule::new("wrap", atom(r, "y")).when([
        atom(r, "x").positive(),
        Literal::member("y", Expr::List(vec![Expr::var("x")])),
    ]));
    assert_eq!(shapes(p.build()), [expect("wrap", invents("value"))]);
    let mut p = Program::builder();
    let r = relation(&mut p, "r");
    p.rule(Rule::new("alias-of-invention", atom(r, "next")).when([
        Literal::bind("next", Expr::var("alias")),
        Literal::bind("alias", Expr::var("grown")),
        Literal::member("grown", Expr::List(vec![Expr::var("seed")])),
        Literal::bind("seed", Expr::var("x")),
        atom(r, "x").positive(),
    ]));
    assert_eq!(shapes(p.build()), [expect("alias-of-invention", invents("value"))]);
}

#[test]
fn spec_4_3_outer_variable_two_levels_down_is_shared_at_both_levels() {
    let program = |grounded_outside: bool| {
        let mut p = Program::builder();
        let item = relation(&mut p, "item");
        let key = relation(&mut p, "key");
        let pair = p.relation(
            Relation::new(
                "pair",
                RelationKind::Static,
                [Slot::new("item", Type::Text), Slot::new("key", Type::Text)],
            )
            .template("{item} pairs with {key}"),
        );
        let out = relation(&mut p, "out");
        let mut body = vec![Literal::aggregate(
            "n",
            AggregateOp::Count,
            [
                atom(item, "a").positive(),
                Literal::aggregate(
                    "m",
                    AggregateOp::Count,
                    [Atom::new(pair, [Expr::var("b"), Expr::var("k")]).positive()],
                ),
            ],
        )];
        if grounded_outside {
            body.push(atom(key, "k").positive());
        }
        p.rule(Rule::new("nested", atom(out, "k")).when(body));
        p.build()
    };
    assert!(
        messages(program(false))
            .starts_with("nested: nothing grounds ?k, which ?n = count where"),
        "{}",
        messages(program(false))
    );
    check(&program(true)).unwrap();
}

#[test]
fn spec_4_3_definitions_are_their_own_owners_called_or_not() {
    let rows = |body: Vec<Literal>| Definition {
        name: "rows".into(),
        parameters: vec![],
        result: Type::Text.list(),
        effect: EffectLevel::Total,
        source: None,
        body: Query::all([Variable::new("x")], body).into(),
    };
    let program = |body: Vec<Literal>, called: bool, caller_input: bool| {
        let mut p = Program::builder();
        let out = relation(&mut p, "out");
        let input = relation(&mut p, "input");
        p.definition(rows(body));
        if called {
            let mut when = vec![Literal::member("row", Expr::call("rows", []))];
            if caller_input {
                when.push(atom(input, "unbound").positive());
            }
            p.rule(Rule::new("caller", atom(out, "row")).when(when));
        }
        p.build()
    };
    let cycle = || vec![Literal::bind("x", Expr::var("y")), Literal::bind("y", Expr::var("x"))];
    let free = || vec![Literal::member("x", Expr::List(vec![Expr::var("unbound")]))];
    let cycle_error = [expect("definition rows", grounds(&["y"]))];
    let free_error = [expect("definition rows", grounds(&["unbound"]))];
    assert_eq!(shapes(program(cycle(), true, false)), cycle_error);
    assert_eq!(shapes(program(cycle(), false, false)), cycle_error);
    assert_eq!(shapes(program(free(), true, false)), free_error);
    assert_eq!(shapes(program(free(), true, true)), free_error);
}

#[test]
fn d49_a_named_variable_only_a_not_mentions_is_not_grounded() {
    let mut p = Program::builder();
    let person = relation(&mut p, "person");
    let borrowed = p.relation(Relation::new(
        "borrowed",
        RelationKind::Static,
        [Slot::new("person", Type::Text), Slot::new("book", Type::Text)],
    ));
    let idle = relation(&mut p, "idle");
    p.rule(Rule::new("idle", atom(idle, "p")).when([
        atom(person, "p").positive(),
        Atom::new(borrowed, [Expr::var("p"), Expr::var("b")]).negative(),
    ]));
    assert!(messages(p.build()).contains("nothing grounds ?b"));
    for binding in [false, true] {
        let mut p = Program::builder();
        let person = relation(&mut p, "person");
        let borrowed = p.relation(Relation::new(
            "borrowed",
            RelationKind::Static,
            [Slot::new("person", Type::Text), Slot::new("book", Type::Int)],
        ));
        let idle = relation(&mut p, "idle");
        let expression = Expr::binary(BinaryOp::Add, Expr::var("_#1"), Expr::value(1));
        let literal = if binding {
            Literal::bind("n", expression)
        } else {
            Atom::new(borrowed, [Expr::var("p"), expression]).negative()
        };
        p.rule(Rule::new("idle", atom(idle, "p")).when([
            atom(person, "p").positive(),
            literal,
        ]));
        let error = messages(p.build());
        assert!(error.contains("nothing grounds ?_, which"), "{error}");
        assert!(!error.contains("_#"), "{error}");
    }
}
