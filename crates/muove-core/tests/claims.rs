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
