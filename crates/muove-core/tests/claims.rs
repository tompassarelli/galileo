use muove_core::*;

fn verdict<'a>(results: &'a [ClaimResult], name: &str) -> &'a ClaimVerdict {
    &results.iter().find(|r| r.requirement == name).unwrap().verdict
}

fn node(program: &Program, name: &str) -> Atom {
    let holds = program.relation_id("w/holds").unwrap();
    Atom::new(holds, [Expr::value(Value::entity("Node", name))])
}

#[test]
fn r5_check_never_approves_an_unfinished_negative_claim() {
    let program = examples::gate_never();
    let results = check_claims(&program, Budget::work(0)).unwrap();
    assert!(matches!(
        verdict(&results, "never-goal"),
        Verdict::Unknown(Reason::OutOfBudget { .. })
    ));
    assert_eq!(exit_code(&results), 2);
    let results = check_claims(&program, Budget::default()).unwrap();
    assert!(matches!(verdict(&results, "never-goal"), Verdict::Fails(_)));
    assert!(verdict(&results, "goal-holds").approves());
    assert_eq!(exit_code(&results), 1);
}

#[test]
fn r15_a_bare_variable_is_an_error_and_every_is_universal() {
    let mut program = examples::gate();
    let holds = program.relation_id("w/holds").unwrap();
    let bare = Atom::new(holds, [Expr::var("n")]);
    program.requirements = vec![Requirement::new("bare", Claim::State(StateClaim::Holds(bare.clone())))];
    let error = check_claims(&program, Budget::default()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "bare: ?n is not quantified. Write `some ?n:`, `every ?n:` or `no ?n:`"
    );

    let mut program = examples::gate();
    let node_relation = program.relation_id("w/node").unwrap();
    program.entities.push(Entity::named("Node", "Stray"));
    program.facts.push(Fact::new(node_relation, [Value::entity("Node", "Stray")]));
    program.requirements.push(Requirement::new(
        "some-node-holds",
        Claim::State(StateClaim::Some {
            variables: vec![Variable::new("n")],
            body: vec![bare.positive()],
        }),
    ));
    let results = check_claims(&program, Budget::default()).unwrap();
    assert!(verdict(&results, "some-node-holds").approves());
    let stray = Instance::from([(Variable::new("n"), Value::entity("Node", "Stray"))]);
    assert_eq!(verdict(&results, "every-node-holds"), &Verdict::Fails(stray));
    let sentence = claims::sentence(results.iter().find(|r| r.requirement == "every-node-holds").unwrap());
    assert_eq!(sentence, "every-node-holds fails: ?n = Stray is a counterexample.");
}

#[test]
fn an_every_claim_with_no_instances_holds_vacuously() {
    let mut program = examples::gate();
    let holds = program.relation_id("w/holds").unwrap();
    program.requirements = vec![Requirement::new(
        "goal-nodes-hold",
        Claim::State(StateClaim::Every {
            variables: vec![Variable::new("n")],
            domain: vec![
                Atom::new(holds, [Expr::var("n")]).positive(),
                Literal::Compare {
                    op: CompareOp::Equal,
                    left: Expr::var("n"),
                    right: Expr::value(Value::entity("Node", "Nowhere")),
                },
            ],
            body: vec![node(&program, "Goal").positive()],
        }),
    )];
    let results = check_claims(&program, Budget::default()).unwrap();
    assert_eq!(results[0].verdict, Verdict::Holds(Evidence::Every { instances: 0 }));
    assert_eq!(
        claims::sentence(&results[0]),
        "goal-nodes-hold holds vacuously: nothing satisfies its `where`, so there was no instance to check."
    );
    assert_eq!(exit_code(&results), 0);
}

#[test]
fn p7_a_budget_too_small_to_finish_gives_unknown_never_fails() {
    let results = check_claims(&examples::chain(101), Budget::work(100_000)).unwrap();
    assert!(matches!(
        results[0].verdict,
        Verdict::Unknown(Reason::OutOfBudget { resource: Resource::Work, .. })
    ));
    let program = examples::chain(8);
    let mut work = 0;
    loop {
        let results = check_claims(&program, Budget::work(work)).unwrap();
        match &results[0].verdict {
            Verdict::Unknown(_) => work += 1,
            Verdict::Holds(_) => break,
            Verdict::Fails(_) => panic!("budget {work} gave Fails"),
        }
    }
    assert!(work > 0);
}

fn holding(program: &Program, model: &Model) -> Vec<String> {
    let holds = program.relation_id("w/holds").unwrap();
    let mut names: Vec<String> = model
        .tuples(holds)
        .map(|tuple| match &tuple[0] {
            Value::Entity(entity) => entity.name.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    names.sort();
    names
}

fn reference_case(json: &str, name: &str) -> (Vec<String>, bool) {
    let start = json.find(&format!("\"{name}\"")).unwrap();
    let case = &json[start..start + json[start..].find('}').unwrap()];
    let list = &case[case.find('[').unwrap() + 1..case.find(']').unwrap()];
    let facts = list
        .split(',')
        .map(|item| item.trim().trim_matches('"').to_string())
        .filter(|item| !item.is_empty())
        .collect();
    (facts, case.contains("\"complete\": true"))
}

#[test]
fn r8_counterfactual_budget_agrees_with_the_reference_model() {
    let json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../inputs/chatgpt-muove-feedback/muove_review/counterfactual_budget_model_results.json"
    ))
    .unwrap();
    let program = examples::shortcut();
    let gate = node(&program, "Gate");
    let blocked = |relation: RelationId, values: &[Value]| {
        relation == gate.relation && gate.arguments == values.iter().cloned().map(Expr::value).collect::<Vec<_>>()
    };
    let tight = Budget::work(evaluate(&program, &[], Budget::default()).unwrap().work);
    let ours = [
        ("base_budget187", evaluate(&program, &[], tight)),
        ("blocked_budget187", evaluate_blocking(&program, &[], tight, &blocked)),
        ("blocked_complete", evaluate_blocking(&program, &[], Budget::default(), &blocked)),
    ];
    let mut agree = 0;
    for (name, outcome) in &ours {
        let (facts, complete) = reference_case(&json, name);
        match outcome {
            Ok(model) => {
                assert!(complete, "{name}");
                assert_eq!(holding(&program, model), facts, "{name}");
            }
            Err(EvalError::Unknown(Reason::OutOfBudget { .. })) => {
                assert!(!complete, "{name}");
                assert!(!facts.contains(&"Goal".to_string()), "{name}");
            }
            Err(error) => panic!("{name}: {error:?}"),
        }
        agree += 1;
    }
    assert_eq!(agree, 3);
}

#[test]
fn r8_requires_and_no_single_point_carry_counterfactual_budget() {
    let program = examples::shortcut();
    let results = check_claims(&program, Budget::default()).unwrap();
    assert!(verdict(&results, "goal-holds").approves());
    assert_eq!(verdict(&results, "goal-requires-gate"), &Verdict::Fails(Instance::new()));
    assert_eq!(verdict(&results, "goal-survives-gate"), &Verdict::Holds(Evidence::Every { instances: 1 }));

    let mut work = 0;
    let mut base_complete_at = None;
    loop {
        let results = check_claims(&program, Budget::work(work)).unwrap();
        let requires = verdict(&results, "goal-requires-gate");
        let survives = verdict(&results, "goal-survives-gate");
        assert!(!requires.approves(), "budget {work}: `requires` held on a truncated search");
        assert!(!matches!(survives, Verdict::Fails(_)), "budget {work}: a truncated search found a single point of failure");
        if base_complete_at.is_none() && verdict(&results, "goal-holds").approves() {
            base_complete_at = Some(work);
            for claim in [requires, survives] {
                let Verdict::Unknown(Reason::OutOfBudget { at, .. }) = claim else {
                    panic!("budget {work}: {claim:?}");
                };
                assert!(at.contains("counterfactual of"), "{at}");
            }
        }
        if !matches!(requires, Verdict::Unknown(_)) && !matches!(survives, Verdict::Unknown(_)) {
            break;
        }
        work += 1;
    }
    assert!(work > base_complete_at.unwrap());
}

#[test]
fn r11_a_derived_fact_that_becomes_stated_no_longer_depends_on_its_reasons() {
    let mut program = examples::gate();
    let holds = program.relation_id("w/holds").unwrap();
    let goal = node(&program, "Goal");
    let claims = vec![
        Requirement::new("goal-holds", Claim::State(StateClaim::Holds(goal.clone()))),
        Requirement::new(
            "goal-requires-gate",
            Claim::Requires {
                target: goal.clone(),
                condition: node(&program, "Gate"),
            },
        ),
        Requirement::new(
            "each-requires-seed",
            Claim::Requires {
                target: Atom::new(holds, [Expr::var("n")]),
                condition: node(&program, "Seed"),
            },
        ),
        Requirement::new(
            "goal-survives-any-node",
            Claim::NoSinglePointOfFailure {
                claim: Box::new(Claim::State(StateClaim::Holds(goal.clone()))),
                among: Atom::new(holds, [Expr::var("m")]),
            },
        ),
    ];
    program.requirements = claims;
    let seed = Value::entity("Node", "Seed");
    let results = check_claims(&program, Budget::default()).unwrap();
    assert!(verdict(&results, "goal-holds").approves());
    assert!(verdict(&results, "goal-requires-gate").approves());
    assert_eq!(verdict(&results, "each-requires-seed"), &Verdict::Holds(Evidence::Every { instances: 3 }));
    assert_eq!(
        verdict(&results, "goal-survives-any-node"),
        &Verdict::Fails(Instance::from([(Variable::new("m"), seed)]))
    );

    program.facts.push(Fact::new(holds, [Value::entity("Node", "Goal")]));
    let results = check_claims(&program, Budget::default()).unwrap();
    assert!(verdict(&results, "goal-holds").approves());
    assert_eq!(verdict(&results, "goal-requires-gate"), &Verdict::Fails(Instance::new()));
    assert_eq!(
        verdict(&results, "each-requires-seed"),
        &Verdict::Fails(Instance::from([(Variable::new("n"), Value::entity("Node", "Goal"))]))
    );
    assert!(verdict(&results, "goal-survives-any-node").approves());
}

#[test]
fn a_counterexample_found_before_the_budget_runs_out_fails_at_once() {
    let int = |name: &str, kind| Relation::new(name, kind, [Slot::new("x", Type::Int)]);
    let x = || Expr::var("x");
    let no = |name: &str, body: Vec<Literal>| {
        Requirement::new(name, Claim::State(StateClaim::No { variables: vec![Variable::new("x")], body }))
    };
    let mut builder = Program::builder();
    let p = builder.relation(int("p", RelationKind::Static));
    builder.fact(Fact::new(p, [Value::Int(1)]));
    builder.requirement(no("no-p", vec![Atom::new(p, [x()]).positive()]));
    let results = check_claims(&builder.build(), Budget::work(3)).unwrap();
    let one = Instance::from([(Variable::new("x"), Value::Int(1))]);
    assert_eq!(results[0].verdict, Verdict::Fails(one));
    assert_eq!(exit_code(&results), 1);

    let mut builder = Program::builder();
    let p = builder.relation(int("p", RelationKind::Static));
    let s = builder.relation(int("s", RelationKind::Static));
    let r = builder.relation(int("r", RelationKind::Derived));
    builder.fact(Fact::new(p, [Value::Int(1)]));
    builder.fact(Fact::new(s, [Value::Int(1)]));
    builder.rule(Rule::new("r", Atom::new(r, [x()])).when([Atom::new(s, [x()]).positive()]));
    builder.requirement(no(
        "no-p-without-r",
        vec![Atom::new(p, [x()]).positive(), Atom::new(r, [x()]).negative()],
    ));
    let program = builder.build();
    let mut work = 0;
    loop {
        match check_claims(&program, Budget::work(work)).unwrap()[0].verdict {
            Verdict::Unknown(_) => work += 1,
            Verdict::Holds(_) => break,
            Verdict::Fails(_) => panic!("budget {work} gave Fails"),
        }
    }
    assert!(work > 3);
}
