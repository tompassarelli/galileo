use galileo_core::*;

const SEED: u64 = 0x3707_5eed;
const PROGRAMS: usize = 10_000;
const VARIABLES: [&str; 4] = ["a", "b", "c", "d"];
const OPS: [CompareOp; 6] = [
    CompareOp::Equal,
    CompareOp::NotEqual,
    CompareOp::Less,
    CompareOp::LessEqual,
    CompareOp::Greater,
    CompareOp::GreaterEqual,
];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn range(&mut self, low: usize, high: usize) -> usize {
        low + self.below(high - low + 1)
    }
    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
}

fn constant(rng: &mut Rng) -> Expr {
    Expr::value(rng.below(5) as i64)
}

fn bound_argument(rng: &mut Rng, bound: &[&'static str], percent_constant: usize) -> Expr {
    if bound.is_empty() || rng.chance(percent_constant) {
        constant(rng)
    } else {
        Expr::var(bound[rng.below(bound.len())])
    }
}

fn negated_argument(rng: &mut Rng, bound: &[&'static str]) -> Expr {
    if rng.chance(4) {
        Expr::var(VARIABLES[rng.below(VARIABLES.len())])
    } else {
        bound_argument(rng, bound, 15)
    }
}

fn generate(rng: &mut Rng) -> (Program, bool) {
    let mut p = Program::builder();
    let relation_count = rng.range(2, 6);
    let stated = rng.range(1, 3.min(relation_count - 1));
    let mut arities = Vec::new();
    let mut ids = Vec::new();
    for index in 0..relation_count {
        let arity = rng.range(0, 2);
        let kind = if index < stated {
            RelationKind::Static
        } else {
            RelationKind::Derived
        };
        let slots = (0..arity).map(|slot| Slot::new(format!("s{slot}"), Type::Int));
        ids.push(p.relation(Relation::new(format!("r{index}"), kind, slots)));
        arities.push(arity);
    }
    for index in 0..relation_count {
        let count = if index < stated {
            rng.range(0, 8)
        } else if rng.chance(15) {
            rng.range(1, 2)
        } else {
            0
        };
        for _ in 0..count {
            let values = (0..arities[index]).map(|_| Value::Int(rng.below(5) as i64));
            p.fact(Fact::new(ids[index], values.collect::<Vec<_>>()));
        }
    }
    for rule in 0..rng.range(1, 6) {
        let head = rng.range(stated, relation_count - 1);
        let mut body = Vec::new();
        let mut bound: Vec<&'static str> = Vec::new();
        for _ in 0..rng.range(1, 3) {
            let relation = rng.below(relation_count);
            let arguments: Vec<Expr> = (0..arities[relation])
                .map(|_| {
                    if rng.chance(20) {
                        constant(rng)
                    } else {
                        let variable = VARIABLES[rng.below(VARIABLES.len())];
                        if !bound.contains(&variable) {
                            bound.push(variable);
                        }
                        Expr::var(variable)
                    }
                })
                .collect();
            body.push(Atom::new(ids[relation], arguments).positive());
        }
        for _ in 0..rng.range(0, 2) {
            let relation = rng.below(relation_count);
            let arguments: Vec<Expr> = (0..arities[relation])
                .map(|_| negated_argument(rng, &bound))
                .collect();
            body.push(Atom::new(ids[relation], arguments).negative());
        }
        if !bound.is_empty() && rng.chance(30) {
            let left = bound_argument(rng, &bound, 0);
            let right = bound_argument(rng, &bound, 40);
            body.push(Literal::compare(OPS[rng.below(OPS.len())], left, right));
        }
        for position in (1..body.len()).rev() {
            body.swap(position, rng.below(position + 1));
        }
        let arguments: Vec<Expr> = (0..arities[head])
            .map(|_| negated_argument(rng, &bound))
            .collect();
        p.rule(Rule::new(format!("rule{rule}"), Atom::new(ids[head], arguments)).when(body));
    }
    let ranked = rng.chance(20);
    if ranked {
        let candidates: Vec<_> = (0..stated).filter(|&index| arities[index] > 0).collect();
        let (relation, arity) = if candidates.is_empty() {
            let relation = p.relation(Relation::new(
                "ranked-input",
                RelationKind::Static,
                [Slot::new("key", Type::Int)],
            ));
            for _ in 0..rng.range(0, 8) {
                p.fact(Fact::new(relation, [Value::Int(rng.below(5) as i64)]));
            }
            (relation, 1)
        } else {
            let index = candidates[rng.below(candidates.len())];
            (ids[index], arities[index])
        };
        let grouped = arity == 2 && rng.chance(50);
        let key = rng.below(arity);
        let arguments: Vec<_> = (0..arity)
            .map(|slot| {
                Expr::var(if slot == key {
                    "ranked-key"
                } else if grouped {
                    "ranked-group"
                } else {
                    "ranked-value"
                })
            })
            .collect();
        let mut left = Expr::var("ranked-key");
        let mut right = Expr::var("ranked-outer");
        if rng.chance(50) {
            std::mem::swap(&mut left, &mut right);
        }
        let op = if rng.chance(50) {
            AggregateOp::Count
        } else {
            AggregateOp::Sum(Expr::var(if arity == 2 && !grouped && rng.chance(50) {
                "ranked-value"
            } else {
                "ranked-key"
            }))
        };
        let result = p.relation(Relation::new(
            "ranked-result",
            RelationKind::Derived,
            [
                Slot::new("outer", Type::Int),
                Slot::new("group", Type::Int),
                Slot::new("n", Type::Int),
            ],
        ));
        let mut body = vec![Literal::member(
            "ranked-outer",
            Expr::List((-1_i64..=5).map(Expr::value).collect()),
        )];
        if grouped {
            body.push(Literal::member(
                "ranked-group",
                Expr::List((0_i64..=4).map(Expr::value).collect()),
            ));
        }
        body.push(Literal::aggregate(
            "ranked-n",
            op,
            [
                Atom::new(relation, arguments).positive(),
                Literal::compare(OPS[2 + rng.below(4)], left, right),
            ],
        ));
        p.rule(
            Rule::new(
                "ranked-aggregate",
                Atom::new(
                    result,
                    [
                        Expr::var("ranked-outer"),
                        if grouped { Expr::var("ranked-group") } else { Expr::value(0) },
                        Expr::var("ranked-n"),
                    ],
                ),
            )
            .when(body),
        );
    }
    let input = p.relation(Relation::new(
        "expression-input",
        RelationKind::Static,
        [Slot::new("item", Type::Int), Slot::new("key", Type::Int)],
    ));
    for _ in 0..rng.range(0, 8) {
        p.fact(Fact::new(
            input,
            [
                Value::Int(rng.below(5) as i64),
                Value::Int(rng.below(5) as i64),
            ],
        ));
    }
    let inputs = || Atom::new(input, [Expr::var("item"), Expr::var("key")]).positive();
    if rng.chance(40) {
        let op = match rng.below(4) {
            0 => AggregateOp::Count,
            1 => AggregateOp::Sum(Expr::var("key")),
            2 => AggregateOp::Min(Expr::var("key")),
            _ => AggregateOp::Max(Expr::var("key")),
        };
        let result = p.relation(Relation::new(
            "aggregate-result",
            RelationKind::Derived,
            [Slot::new("value", Type::Int)],
        ));
        let mut body = vec![inputs()];
        if rng.chance(50) {
            body.push(Literal::compare(
                OPS[rng.below(OPS.len())],
                Expr::var("key"),
                constant(rng),
            ));
        }
        p.rule(
            Rule::new("aggregate", Atom::new(result, [Expr::var("result")]))
                .when([Literal::aggregate("result", op, body)]),
        );
    }
    if rng.chance(40) {
        let result = p.relation(Relation::new(
            "argmin-result",
            RelationKind::Derived,
            [Slot::new("value", Type::Int)],
        ));
        p.rule(
            Rule::new("argmin", Atom::new(result, [Expr::var("result")])).when([Literal::argmin(
                "result",
                Expr::var("item"),
                Expr::var("key"),
                [inputs()],
            )]),
        );
    }
    if rng.chance(40) {
        p.definition(Definition {
            name: "offset".into(),
            parameters: vec![Slot::new("value", Type::Int)],
            result: Type::Int,
            body: Expr::binary(BinaryOp::Add, Expr::var("value"), constant(rng)),
            effect: EffectLevel::Total,
            source: None,
        });
        let result = p.relation(Relation::new(
            "binding-result",
            RelationKind::Derived,
            [Slot::new("item", Type::Int), Slot::new("value", Type::Int)],
        ));
        p.rule(
            Rule::new(
                "binding",
                Atom::new(result, [Expr::var("item"), Expr::var("result")]),
            )
            .when([
                Literal::bind("result", Expr::call("offset", [Expr::var("key")])),
                inputs(),
            ]),
        );
    }
    if rng.chance(40) {
        let result = p.relation(Relation::new(
            "membership-result",
            RelationKind::Derived,
            [Slot::new("item", Type::Int), Slot::new("member", Type::Int)],
        ));
        p.rule(
            Rule::new(
                "membership",
                Atom::new(result, [Expr::var("item"), Expr::var("member")]),
            )
            .when([
                Literal::member("member", Expr::var("list")),
                Literal::bind(
                    "list",
                    Expr::List(vec![Expr::var("key"), Expr::var("key"), constant(rng)]),
                ),
                inputs(),
            ]),
        );
    }
    if rng.chance(30) {
        let multiple = rng.chance(50);
        let variables = if multiple {
            vec![Variable::new("item"), Variable::new("key")]
        } else {
            vec![Variable::new("item")]
        };
        let mut query = Query::all(variables, [inputs()]);
        if rng.chance(70) {
            query = query.sorted_by(Expr::var(if multiple && rng.chance(50) {
                "key"
            } else {
                "item"
            }));
        }
        let row_type = if multiple {
            Type::Record([("item".into(), Type::Int), ("key".into(), Type::Int)].into())
        } else {
            Type::Int
        };
        let result = p.relation(Relation::new(
            "all-result",
            RelationKind::Derived,
            [Slot::new("rows", row_type.list())],
        ));
        p.rule(
            Rule::new("all", Atom::new(result, [Expr::var("rows")]))
                .when([Literal::bind("rows", query.into())]),
        );
    }
    if rng.chance(30) {
        let functional = p.relation(
            Relation::new(
                "functional-input",
                RelationKind::Static,
                [Slot::new("item", Type::Int), Slot::new("value", Type::Int)],
            )
            .mode(Mode::AtMostOne {
                keys: vec![0],
                values: vec![1],
            }),
        );
        for item in 0..5 {
            if rng.chance(70) {
                p.fact(Fact::new(
                    functional,
                    [Value::Int(item), Value::Int(rng.below(5) as i64)],
                ));
            }
        }
        let result = p.relation(Relation::new(
            "the-result",
            RelationKind::Derived,
            [
                Slot::new("item", Type::Int),
                Slot::new("value", Type::Int.maybe()),
            ],
        ));
        p.rule(
            Rule::new(
                "the",
                Atom::new(result, [Expr::var("outer"), Expr::var("result")]),
            )
            .when([
                Literal::bind(
                    "result",
                    Query::the(
                        "value",
                        [
                            Atom::new(functional, [Expr::var("outer"), Expr::var("value")])
                                .positive(),
                        ],
                    )
                    .into(),
                ),
                Literal::member("outer", Expr::List((0_i64..=4).map(Expr::value).collect())),
            ]),
        );
    }
    (p.build(), ranked)
}

fn agree(reference: &Result<Model, EvalError>, engine: &Result<Model, EvalError>) -> Result<(), String> {
    match (reference, engine) {
        (Ok(reference), Ok(engine)) => {
            for (relation, tuples) in &reference.relations {
                if engine.relations.get(relation) != Some(tuples) {
                    return Err(format!(
                        "relation r{}: reference {:?}, engine {:?}",
                        relation.0,
                        tuples,
                        engine.relations.get(relation)
                    ));
                }
            }
            match engine.relations.keys().find(|r| !reference.relations.contains_key(r)) {
                Some(relation) => Err(format!(
                    "relation r{}: reference empty, engine {:?}",
                    relation.0, engine.relations[relation]
                )),
                None => Ok(()),
            }
        }
        (Err(reference), Err(engine))
            if std::mem::discriminant(reference) == std::mem::discriminant(engine) => Ok(()),
        (reference, engine) => Err(format!("reference {reference:?}, engine {engine:?}")),
    }
}

#[test]
fn engine_agrees_with_the_reference_on_stratified_programs_with_negation() {
    let seed = std::env::var("GALILEO_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(SEED);
    let programs = std::env::var("GALILEO_PROGRAMS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if std::env::var_os("GALILEO_FULL").is_some() { PROGRAMS } else { PROGRAMS / 20 });
    let mut rng = Rng(seed);
    let (mut accepted, mut filtered, mut unknown, mut negations) = (0, 0, 0, 0);
    let (mut generated, mut ranked_generated, mut ranked_compared, mut ranked_unknown) = (0, 0, 0, 0);
    let mut aggregates = [0; 4];
    let (mut argmins, mut bindings, mut memberships, mut definitions, mut all, mut the) =
        (0, 0, 0, 0, 0, 0);
    let (mut models, mut errors) = (0, 0);
    while accepted < programs {
        let (program, ranked) = generate(&mut rng);
        generated += 1;
        ranked_generated += usize::from(ranked);
        if check(&program).is_err() {
            filtered += 1;
            continue;
        }
        let reference = evaluate(&program, &[], Budget::default());
        let engine = engine::evaluate(&program, &[], Budget::default());
        if matches!(reference, Err(EvalError::Unknown(_)))
            || matches!(engine, Err(EvalError::Unknown(_)))
        {
            unknown += 1;
            ranked_unknown += usize::from(ranked);
            continue;
        }
        if let Err(divergence) = agree(&reference, &engine) {
            panic!("seed {seed:#x}, program {accepted}: {divergence}\n{program:#?}");
        }
        ranked_compared += usize::from(ranked);
        models += usize::from(reference.is_ok());
        errors += usize::from(reference.is_err());
        definitions += program.definitions.len();
        for literal in program
            .rules
            .iter()
            .filter(|rule| rule.name != "ranked-aggregate")
            .flat_map(|rule| &rule.body)
        {
            match literal {
                Literal::Aggregate { op, .. } => {
                    aggregates[match op {
                        AggregateOp::Count => 0,
                        AggregateOp::Sum(_) => 1,
                        AggregateOp::Min(_) => 2,
                        AggregateOp::Max(_) => 3,
                    }] += 1
                }
                Literal::ArgMin { .. } => argmins += 1,
                Literal::Bind { value, .. } => {
                    bindings += 1;
                    if let Expr::Query(query) = value {
                        match &query.kind {
                            QueryKind::All(_) => all += 1,
                            QueryKind::The(_) => the += 1,
                        }
                    }
                }
                Literal::Member { .. } => memberships += 1,
                _ => {}
            }
        }
        negations += program
            .rules
            .iter()
            .flat_map(|rule| &rule.body)
            .filter(|literal| matches!(literal, Literal::Negative(_)))
            .count();
        accepted += 1;
    }
    eprintln!(
        "seed {seed:#x}: {generated} generated, {accepted} programs agree, {filtered} filtered by the static checks, {unknown} unknown, {negations} negations, {ranked_generated} ranked-aggregate programs generated, {ranked_compared} ranked-aggregate programs compared, {ranked_unknown} ranked-aggregate programs unknown"
    );
    eprintln!(
        "compared forms: count {}, sum {}, min {}, max {}, argmin {argmins}, bindings {bindings}, grounded memberships {memberships}, definitions {definitions}, all {all}, the {the}; {models} models, {errors} same-kind errors",
        aggregates[0], aggregates[1], aggregates[2], aggregates[3]
    );
    assert!(ranked_compared >= programs / 10, "too few ranked-aggregate programs compared");
}
