use galileo_core::*;

fn verdict<'a>(results: &'a [ClaimResult], name: &str) -> &'a ClaimVerdict {
    &results.iter().find(|result| result.requirement == name).unwrap().verdict
}

fn first_decided(program: &Program) -> u64 {
    (0..=2_000)
        .find(|work| !matches!(verdict(&check_claims(program, Budget::work(*work)).unwrap(), "goal-holds"), Verdict::Unknown(_)))
        .expect("goal-holds was not decided within 2,000 work units")
}

#[test]
fn public_claim_verdicts_are_independent_and_invalid_forms_are_static() {
    let gate = examples::gate();
    let holds = gate.relation_id("w/holds").unwrap();
    let goal = Atom::new(holds, [Expr::value(Value::entity("Node", "Goal"))]);
    let mut unsupported = gate.clone();
    unsupported.requirements.push(Requirement::new(
        "function-value",
        Claim::State(StateClaim::Some {
            variables: vec![],
            body: vec![Literal::bind("f", Expr::Lambda {
                parameters: vec![],
                body: Box::new(Expr::value(1_i64)),
            })],
        }),
    ));
    let results = check_claims(&unsupported, Budget::default()).unwrap();
    assert!(verdict(&results, "goal-holds").approves());
    assert!(verdict(&results, "every-node-holds").approves());
    assert!(matches!(verdict(&results, "function-value"), Verdict::Unknown(Reason::Unsupported { construct, .. }) if construct == "a function value"));
    assert_eq!(results.iter().filter(|result| matches!(result.verdict, Verdict::Unknown(_))).count(), 1);
    let failures = claims::enumerate_claim_failures(&unsupported, Budget::default()).unwrap();
    assert_eq!(failures.iter().filter(|failure| matches!(failure.verdict, Verdict::Unknown(_))).count(), 1);
    assert!(failures[0].verdict.approves() && failures[1].verdict.approves());

    let mut single = gate.clone();
    single.requirements.retain(|requirement| requirement.name == "goal-holds");
    let single_work = first_decided(&single);
    let wider_work = first_decided(&gate);
    let unsupported_work = first_decided(&unsupported);
    assert_eq!(single_work, wider_work);
    assert_eq!(single_work, unsupported_work);
    println!("3.11c independence: Holds=2 Unknown=1; goal first-decided work: single={single_work} wider={wider_work} unsupported={unsupported_work}");

    let quantified = StateClaim::Some {
        variables: vec![Variable::new("n")],
        body: vec![Atom::new(holds, [Expr::var("n")]).positive()],
    };
    let cases = [
        ("some", Claim::NoSinglePointOfFailure {
            claim: Box::new(Claim::State(quantified.clone())),
            among: goal.clone(),
        }),
        ("no", Claim::NoSinglePointOfFailure {
            claim: Box::new(Claim::State(StateClaim::No {
                variables: vec![Variable::new("n")],
                body: vec![Atom::new(holds, [Expr::var("n")]).positive()],
            })),
            among: goal.clone(),
        }),
        ("every", Claim::NoSinglePointOfFailure {
            claim: Box::new(gate.requirements[1].claim.clone()),
            among: goal.clone(),
        }),
        ("requires", Claim::NoSinglePointOfFailure {
            claim: Box::new(Claim::Requires { target: goal.clone(), condition: goal.clone() }),
            among: goal.clone(),
        }),
        ("before", Claim::State(StateClaim::Before(Box::new(StateClaim::Holds(goal.clone()))))),
        ("after", Claim::State(StateClaim::After(Box::new(quantified)))),
        ("before", Claim::Always(StateClaim::Before(Box::new(StateClaim::Holds(goal.clone()))))),
        ("after", Claim::NoSinglePointOfFailure {
            claim: Box::new(Claim::Eventually(StateClaim::After(Box::new(StateClaim::Holds(goal.clone()))))),
            among: goal.clone(),
        }),
    ];
    let static_count = cases.len();
    for (form, claim) in cases {
        let mut program = unsupported.clone();
        program.requirements.push(Requirement::new("invalid", claim).at(SourceSpan {
            file: "public-claim.gal".into(), start: 0, end: 18, line: 7, column: 1,
        }));
        let error = check_claims(&program, Budget::work(0)).unwrap_err();
        assert!(matches!(error, ClaimError::InvalidForm { .. }));
        let message = error.to_string();
        assert!(message.starts_with("public-claim.gal:7:"), "{message}");
        assert!(message.contains(&format!("`{form}`")), "{message}");
        if form == "some" {
            println!("3.11c public some error: {message}");
        }
    }
    println!("3.11c static forms rejected={static_count} before evaluation at work=0");
}

