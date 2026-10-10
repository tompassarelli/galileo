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
    for program in [examples::dungeon_static(), examples::tags()] {
        let cost = evaluate(&program, &[], Budget::default()).unwrap().work;
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
    assert!(matches!(
        evaluate(&overflow.build(), &[], Budget::work(0)),
        Err(EvalError::Unknown(Reason::OutOfBudget { resource: Resource::Work, .. }))
    ));
}
