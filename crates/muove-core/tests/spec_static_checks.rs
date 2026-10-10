use muove_core::*;

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
    assert_eq!(
        messages(p.build()),
        "grow-text: value invention in recursion: head slot value gets ?next in [?text ++ \"x\"], a new value built from ?text of the recursive sentence ?text is text"
    );
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
    let result = messages(p.build());
    assert!(
        result.contains(
            "query-edge: stratification cycle: a reads b through a query, b reads a through not"
        ),
        "{result}"
    );
    assert!(
        result.contains(
            "negative-edge: stratification cycle: b reads a through not, a reads b through a query"
        ),
        "{result}"
    );
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
    assert_eq!(
        messages(p.build()),
        "hidden-not: stratification cycle: chosen reads chosen through a query"
    );
}

#[test]
fn spec_4_3_mutual_bindings_have_no_grounding_schedule() {
    let mut p = Program::builder();
    let r = relation(&mut p, "r");
    p.rule(
        Rule::new("binding-cycle", atom(r, "x"))
            .at(SourceSpan {
                file: "binding-cycle.muove".into(),
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
    assert_eq!(
        messages(p.build()),
        "binding-cycle.muove:3:1: binding-cycle: nothing grounds ?y, which ?x = ?y needs"
    );
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
    assert_eq!(
        messages(p.build()),
        "a-if-not-b: stratification cycle: a reads b through not, b reads a"
    );
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
        let checked = check(&program).unwrap();
        assert_eq!(checked.schedules.len(), program.rules.len());
        for edge in &checked.dependencies {
            if edge.kind != DependencyKind::Positive {
                assert!(checked.strata[edge.body.0] < checked.strata[edge.head.0]);
            }
        }
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
    let checked = check(&p.build()).unwrap();
    let order = &checked.schedules[0];
    let position = |literal| order.iter().position(|index| *index == literal).unwrap();
    assert!(position(3) < position(2));
    assert!(position(2) < position(0));
    assert!(position(2) < position(1));
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
    assert_eq!(check(&p.build()).unwrap().schedules[0].len(), 2);
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
    let program = p.build();
    let checked = check(&program).unwrap();
    assert!(checked.strata[input.0] < checked.strata[r.0]);
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
    assert_eq!(
        messages(p.build()),
        "wrap: value invention in recursion: head slot value gets ?y in [?x], a new value built from ?x of the recursive sentence ?x is r"
    );
    let mut p = Program::builder();
    let r = relation(&mut p, "r");
    p.rule(Rule::new("alias-of-invention", atom(r, "next")).when([
        Literal::bind("next", Expr::var("alias")),
        Literal::bind("alias", Expr::var("grown")),
        Literal::member("grown", Expr::List(vec![Expr::var("seed")])),
        Literal::bind("seed", Expr::var("x")),
        atom(r, "x").positive(),
    ]));
    assert_eq!(
        messages(p.build()),
        "alias-of-invention: value invention in recursion: head slot value gets ?grown in [?seed], a new value built from ?x of the recursive sentence ?x is r through ?seed"
    );
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
    assert_eq!(check(&program(true)).unwrap().schedules[0], vec![1, 0]);
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
    let cycle_error = "definition rows: nothing grounds ?y, which ?x = ?y needs";
    let free_error = "definition rows: nothing grounds ?unbound, which ?x in [?unbound] needs";
    assert_eq!(messages(program(cycle(), true, false)), cycle_error);
    assert_eq!(messages(program(cycle(), false, false)), cycle_error);
    assert_eq!(messages(program(free(), true, false)), free_error);
    assert_eq!(messages(program(free(), true, true)), free_error);
}
