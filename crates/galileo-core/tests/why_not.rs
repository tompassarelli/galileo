use galileo_core::why_not::{render, why_not, Attempt, State, Via};
use galileo_core::*;

fn marks<'a>(program: &Program, attempt: &'a Attempt) -> (String, Vec<(&'a str, &'a State)>) {
    let conditions = attempt.conditions.iter().map(|c| (c.text.as_str(), &c.state)).collect();
    (program.rules[attempt.rule].name.clone(), conditions)
}

fn attempts(verdict: &Verdict<(), Vec<Attempt>>) -> &[Attempt] {
    let Verdict::Fails(attempts) = verdict else { panic!("expected Fails, got {verdict:?}") };
    attempts
}

fn assert_pick_up_needs_silver_key(program: &Program, attempt: &Attempt) {
    let (rule, conditions) = marks(program, attempt);
    assert_eq!(rule, "level/pick-up");
    let missing = State::Missing { stated: true, scope: vec![] };
    assert_eq!(conditions, [("Silver-Key lies in ?room", &missing), ("Ash can reach ?room", &State::Waiting)]);
}

fn ask(program: &Program, relation: &str, values: &[Value], via: Option<&Via>) -> (Verdict<(), Vec<why_not::Attempt>>, String) {
    let relation = program.relation_id(relation).unwrap();
    let verdict = why_not(program, &[], relation, values, via, Budget::default()).unwrap();
    let text = render(program, relation, values, via, &verdict, Budget::default());
    println!("{text}\n");
    (verdict, text)
}

#[test]
fn why_not_matches_the_prototype_readme_example() {
    let program = examples::dungeon_static();
    let ash = Value::entity("Hero", "Ash");
    let (key, _) = ask(&program, "level/holds", &[ash.clone(), Value::entity("Key", "Silver-Key")], None);
    let pick_up = attempts(&key).iter().find(|a| program.rules[a.rule].name == "level/pick-up").expect("a pick-up near miss");
    assert_pick_up_needs_silver_key(&program, pick_up);
    let (shrine, _) = ask(&program, "level/reach", &[ash, Value::entity("Room", "Shrine")], None);
    let unlock = attempts(&shrine).iter().find(|a| program.rules[a.rule].name == "level/unlock").expect("an unlock near miss");
    let (_, conditions) = marks(&program, unlock);
    let met = [
        ("Ash can reach Crypt", &State::Met),
        ("Shrine-Door leads from Crypt to Shrine", &State::Met),
        ("Shrine-Door is locked by Silver-Key", &State::Met),
    ];
    assert_eq!(conditions[..3], met);
    assert_eq!(conditions[3].0, "Ash holds Silver-Key");
    assert!(matches!(conditions[3].1, State::Missing { stated: false, .. }), "{:?}", conditions[3].1);
    assert_eq!(conditions.len(), 4);
    let [deeper] = unlock.conditions[3].deeper.as_slice() else { panic!("one deeper near miss: {:?}", unlock.conditions[3].deeper) };
    assert_pick_up_needs_silver_key(&program, deeper);
}

#[test]
fn a_failing_not_names_its_blocker_under_via() {
    let mut program = examples::tags();
    for (line, fact) in program.facts.iter_mut().enumerate() {
        *fact = fact.clone().at(SourceSpan { file: "tags.gal".into(), start: 0, end: 0, line: line + 1, column: 1 });
    }
    let via = Via { rule: "enabled".into(), bindings: vec![] };
    let asked = [Value::entity("Module", "anytype"), Value::entity("Host", "whiterabbit")];
    let (verdict, _) = ask(&program, "firn/enabled", &asked, Some(&via));
    let [attempt] = attempts(&verdict) else { panic!("one near miss under via: {verdict:?}") };
    assert_eq!(program.rules[attempt.rule].name, "enabled");
    let blocked: Vec<_> = attempt.conditions.iter().filter_map(|c| match &c.state {
        State::Blocked { by } => Some((c.text.as_str(), by.as_str())),
        _ => None,
    }).collect();
    let [(text, by)] = blocked.as_slice() else { panic!("one blocked condition: {blocked:?}") };
    assert_eq!(*text, "not whiterabbit disables anytype");
    let disables = program.relation_id("firn/disables").unwrap();
    let blocker = galileo_core::why::sentence(&program, disables, &[Value::entity("Host", "whiterabbit"), Value::entity("Module", "anytype")]);
    assert!(by.starts_with(&blocker) && by.contains("tags.gal:"), "{by}");
}

#[test]
fn a_late_grounded_comparison_and_a_computed_argument_are_judged_by_value() {
    for (name, body, expected) in [
        ("body-order", vec![Literal::compare(CompareOp::Equal, Expr::var("x"), Expr::value(2_i64)), Atom::new(RelationId(0), [Expr::var("x")]).positive()], ("1 == 2", false)),
        ("expression-match", vec![Atom::new(RelationId(0), [Expr::binary(BinaryOp::Add, Expr::value(1_i64), Expr::value(1_i64))]).positive()], ("input 1 + 1", true)),
    ] {
        let mut p = Program::builder();
        let input = p.relation(Relation::new("input", RelationKind::Static, [Slot::new("n", Type::Int)]).template("input {n}"));
        let output = p.relation(Relation::new("output", RelationKind::Derived, []).template("output"));
        assert_eq!(input, RelationId(0));
        p.fact(Fact::new(input, [Value::Int(1)]));
        p.rule(Rule::new(name, Atom::new(output, [])).when(body));
        let program = p.build();
        assert!(!evaluate(&program, &[], Budget::default()).unwrap().contains(output, &[]));
        let (verdict, _) = ask(&program, "output", &[], None);
        assert!(matches!(&verdict, Verdict::Fails(attempts) if attempts.len() == 1 && attempts[0].missing == 1));
        let (text, missing) = expected;
        let failing = attempts(&verdict)[0].conditions.iter().find(|c| c.state != State::Met).unwrap();
        assert_eq!(failing.text, text, "{name}");
        let state_ok = if missing { matches!(failing.state, State::Missing { stated: true, .. }) } else { failing.state == State::False };
        assert!(state_ok, "{name}: {:?}", failing.state);
    }
}
