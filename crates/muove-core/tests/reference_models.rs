use muove_core::*;

fn rows(model: &Model, program: &Program, relation: &str) -> Vec<String> {
    let id = program.relation_id(relation).unwrap();
    model
        .tuples(id)
        .map(|tuple| {
            let words: Vec<String> = tuple
                .iter()
                .map(|value| match value {
                    Value::Entity(entity) => entity.name.clone(),
                    Value::OptionPath(path) => path.join("."),
                    Value::Nix { normalized } => normalized.clone(),
                    other => format!("{other:?}"),
                })
                .collect();
            words.join(" ")
        })
        .collect()
}

#[test]
fn dungeon_static_model_matches_the_prototype_readme() {
    let program = examples::dungeon_static();
    let model = evaluate(&program, &[], Budget::default()).unwrap();
    assert_eq!(
        rows(&model, &program, "level/reach"),
        ["Ash Armory", "Ash Crypt", "Ash Entrance", "Ash Hall", "Ash Library", "Ash Vault"]
    );
    assert_eq!(
        rows(&model, &program, "level/holds"),
        ["Ash Bronze-Key", "Ash Iron-Key"]
    );
    assert_eq!(rows(&model, &program, "level/hurts"), ["Spike-Trap Ash"]);
}

#[test]
fn tags_model_matches_spec_9_6() {
    let program = examples::tags();
    let model = evaluate(&program, &[], Budget::default()).unwrap();
    assert_eq!(rows(&model, &program, "firn/enabled"), ["firefox whiterabbit"]);
    assert_eq!(
        rows(&model, &program, "firn/enabled-by-default"),
        ["firefox whiterabbit browsers"]
    );
    assert_eq!(
        rows(&model, &program, "firn/gets-set-by"),
        ["whiterabbit myConfig.modules.firefox.default true firefox"]
    );
}

#[test]
fn every_budget_short_of_the_cost_gives_unknown_work_and_no_model() {
    for (program, expected_cost) in [(examples::dungeon_static(), 687), (examples::tags(), 179)] {
        let cost = evaluate(&program, &[], Budget::default()).unwrap().work;
        assert_eq!(cost, expected_cost);
        assert_eq!(evaluate(&program, &[], Budget::work(cost)).unwrap().work, cost);
        for work in 0..cost {
            let result = evaluate(&program, &[], Budget::work(work));
            assert!(
                matches!(
                    result,
                    Err(EvalError::Unknown(Reason::OutOfBudget {
                        resource: Resource::Work,
                        ..
                    }))
                ),
                "budget {work} of {cost}: {result:?}"
            );
        }
    }
    let mut overflow = Program::builder();
    let out = overflow.relation(Relation::new("membership", RelationKind::Derived, [Slot::new("x", Type::Int)]));
    overflow.rule(Rule::new("membership", Atom::new(out, [Expr::var("x")])).when([Literal::member(
        "x",
        Expr::List(vec![Expr::binary(BinaryOp::Add, Expr::value(i64::MAX), Expr::value(1_i64))]),
    )]));
    let overflow = overflow.build();
    for result in [
        evaluate(&overflow, &[], Budget::work(0)),
        engine::evaluate(&overflow, &[], Budget::work(0)),
    ] {
        assert!(matches!(
            result,
            Err(EvalError::Unknown(Reason::OutOfBudget { resource: Resource::Work, .. }))
        ));
    }
}

#[test]
fn spec_6_1_call_chains_charge_before_building_arguments() {
    for parameterized in [false, true] {
        let mut p = Program::builder();
        let out = p.relation(Relation::new(
            "answer",
            RelationKind::Derived,
            [Slot::new("value", Type::Int)],
        ));
        for index in 0..30 {
            let value = if parameterized {
                Expr::var("x")
            } else {
                Expr::value(1_i64)
            };
            let body = if index == 29 {
                value
            } else {
                let arguments = if parameterized { vec![value] } else { vec![] };
                let next = Expr::call(format!("f{}", index + 1), arguments);
                Expr::binary(BinaryOp::Add, next.clone(), next)
            };
            p.definition(Definition {
                name: format!("f{index}"),
                parameters: if parameterized {
                    vec![Slot::new("x", Type::Int)]
                } else {
                    vec![]
                },
                result: Type::Int,
                body,
                effect: EffectLevel::Total,
                source: None,
            });
        }
        let arguments = if parameterized {
            vec![Expr::value(1_i64)]
        } else {
            vec![]
        };
        p.rule(
            Rule::new("answer", Atom::new(out, [Expr::var("n")]))
                .when([Literal::bind("n", Expr::call("f0", arguments))]),
        );
        let program = p.build();
        for result in [
            evaluate(&program, &[], Budget::work(10_000)),
            engine::evaluate(&program, &[], Budget::work(10_000)),
        ] {
            assert!(
                matches!(
                    result,
                    Err(EvalError::Unknown(Reason::OutOfBudget {
                        resource: Resource::Work,
                        ..
                    }))
                ),
                "parameterized={parameterized}: {result:?}"
            );
        }
    }
}
