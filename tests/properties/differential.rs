use galileo_core::*;
use galileo_core::history::Change;
use galileo_core::incremental::Maintained;
use galileo_core::state::Tuple;
use std::collections::BTreeSet;

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
    if rng.chance(15) {
        Expr::var(format!("_#neg-{}", rng.next()))
    } else if rng.chance(4) {
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
            body.push(if rng.chance(25) {
                Literal::compare(CompareOp::In, left, Expr::List(vec![right, bound_argument(rng, &bound, 40)]))
            } else {
                Literal::compare(OPS[rng.below(OPS.len())], left, right)
            });
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
    if ranked {
        let input = p.relation(Relation::new("ranked-weight-input", RelationKind::State,
            [Slot::new("group", Type::Int), Slot::new("key", Type::Int), Slot::new("weight", Type::Int), Slot::new("witness", Type::Int)]));
        for group in 0_i64..5 {
            for (key, weight, witness) in [(1, -3, 0), (1, -2, 1), (2, 4, 0), (10, i64::MAX, 0), (11, i64::MAX, 0), (12, -i64::MAX, 0), (13, -i64::MAX, 0)] {
                p.fact(Fact::new(input, [Value::Int(group), Value::Int(key), Value::Int(weight), Value::Int(witness)]));
            }
        }
        for (index, op) in OPS[2..].iter().copied().enumerate() {
            let result = p.relation(Relation::new(format!("ranked-weight-result-{index}"), RelationKind::Derived,
                [Slot::new("group", Type::Int), Slot::new("threshold", Type::Int), Slot::new("sum", Type::Int)]));
            let (op, left, right) = if rng.chance(50) {
                (OPS[2 + (index + 2) % 4], Expr::var("weight-threshold"), Expr::var("weight-key"))
            } else { (op, Expr::var("weight-key"), Expr::var("weight-threshold")) };
            p.rule(Rule::new(format!("ranked-weight-aggregate-{index}"), Atom::new(result,
                [Expr::var("weight-group"), Expr::var("weight-threshold"), Expr::var("weight-sum")])).when([
                Literal::member("weight-group", Expr::List((0_i64..5).map(Expr::value).collect())),
                Literal::member("weight-threshold", Expr::List((-1_i64..=5).map(Expr::value).collect())),
                Literal::aggregate("weight-sum", AggregateOp::Sum(Expr::var("weight-value")), [
                    Atom::new(input, [Expr::var("weight-group"), Expr::var("weight-key"), Expr::var("weight-value"), Expr::var("weight-witness")]).positive(),
                    Literal::compare(op, left, right),
                ]),
            ]));
        }
    }
    let input = p.relation(Relation::new(
        "expression-input",
        RelationKind::Static,
        [Slot::new("item", Type::Int), Slot::new("key", Type::Int), Slot::new("witness", Type::Int)],
    ));
    for _ in 0..rng.range(0, 8) {
        p.fact(Fact::new(
            input,
            [
                Value::Int(rng.below(5) as i64),
                Value::Int(rng.below(5) as i64),
                Value::Int(rng.below(5) as i64),
            ],
        ));
    }
    if rng.chance(50) { p.fact(Fact::new(input, [Value::Int(i64::MAX), Value::Int(rng.below(5) as i64), Value::Int(0)])); }
    let inputs = || Atom::new(input, [Expr::var("item"), Expr::var("key"), Expr::var("witness")]).positive();
    if rng.chance(40) {
        let op = match rng.below(4) {
            0 => AggregateOp::Count,
            1 => AggregateOp::Sum(Expr::var("key")),
            2 => AggregateOp::Min(Expr::var("key")),
            _ => AggregateOp::Max(Expr::var("key")),
        };
        let value = if matches!(op, AggregateOp::Min(_) | AggregateOp::Max(_)) { Type::Int.maybe() } else { Type::Int };
        let result = p.relation(Relation::new(
            "aggregate-result",
            RelationKind::Derived,
            [Slot::new("value", value)],
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
    let argmin = rng.chance(40);
    if argmin {
        for witness in [0_i64, 1] { p.fact(Fact::new(input, [Value::Int(-5), Value::Int(-5), Value::Int(witness)])); }
        let result = p.relation(Relation::new(
            "argmin-result",
            RelationKind::Derived,
            [Slot::new("value", Type::Int.maybe())],
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
    if argmin {
        let result = p.relation(Relation::new("argmax-result", RelationKind::Derived, [Slot::new("value", Type::Int.maybe())]));
        p.rule(Rule::new("argmax", Atom::new(result, [Expr::var("result")])).when([Literal::ArgMin {
            direction: ArgMinDirection::Max, variable: Variable::new("result"), candidate: Expr::var("item"), key: Expr::var("key"), body: vec![inputs()],
        }]));
    }
    if rng.chance(40) {
        let rows = p.relation(Relation::new("summary-input", RelationKind::Static,
            [Slot::new("group", Type::Int), Slot::new("item", Type::Int), Slot::new("key", Type::Int)]));
        let guard = p.relation(Relation::new("summary-guard", RelationKind::Static, [Slot::new("group", Type::Int)]));
        p.fact(Fact::new(guard, [Value::Int(0)]));
        for (group, item, key) in [(0, 0, -2), (1, 0, i64::MAX), (2, 0, i64::MAX - 1)] {
            p.fact(Fact::new(rows, [Value::Int(group), Value::Int(item), Value::Int(key)]));
        }
        for group in 3..16 {
            p.fact(Fact::new(guard, [Value::Int(group)]));
            p.fact(Fact::new(rows, [Value::Int(group), Value::Int(0), Value::Int(-2)]));
        }
        for (group, item, key) in [(5, 1, -2), (6, 1, -2)] {
            p.fact(Fact::new(rows, [Value::Int(group), Value::Int(item), Value::Int(key)]));
        }
        for (kind, op) in [AggregateOp::Count, AggregateOp::Sum(Expr::binary(BinaryOp::Add, Expr::var("key"), Expr::value(1))),
            AggregateOp::Min(Expr::binary(BinaryOp::Add, Expr::var("key"), Expr::value(1))),
            AggregateOp::Max(Expr::binary(BinaryOp::Add, Expr::var("key"), Expr::value(1)))].into_iter().enumerate() {
            let value = if kind >= 2 { Type::Int.maybe() } else { Type::Int };
            let result = p.relation(Relation::new(format!("summary-result-{kind}"), RelationKind::Derived,
                [Slot::new("group", Type::Int), Slot::new("value", value)]));
            p.rule(Rule::new(format!("summary-{kind}"), Atom::new(result, [Expr::var("group"), Expr::var("result")])).when([
                Atom::new(guard, [Expr::var("group")]).positive(),
                Literal::aggregate("result", op, [Atom::new(rows, [Expr::var("group"), Expr::var("item"), Expr::var("key")]).positive()]),
            ]));
        }
        for (kind, direction) in [(4, ArgMinDirection::Min), (5, ArgMinDirection::Max)] {
            let result = p.relation(Relation::new(format!("summary-result-{kind}"), RelationKind::Derived,
                [Slot::new("group", Type::Int), Slot::new("value", Type::Int.maybe())]));
            p.rule(Rule::new(format!("summary-{kind}"), Atom::new(result, [Expr::var("group"), Expr::var("result")])).when([
                Atom::new(guard, [Expr::var("group")]).positive(),
                Literal::ArgMin { direction, variable: Variable::new("result"), candidate: Expr::var("item"), key: Expr::var("key"),
                    body: vec![Atom::new(rows, [Expr::var("group"), Expr::var("item"), Expr::var("key")]).positive()] },
            ]));
        }

    }
    if rng.chance(40) {
        p.definition(Definition {
            name: "offset".into(),
            parameters: vec![Slot::new("value", Type::Int)],
            result: Type::Int,
            body: Expr::binary(BinaryOp::Add, Expr::var("value"), Expr::value(rng.range(1, 4) as i64)),
            effect: EffectLevel::Total,
            source: None,
        });
        let result = p.relation(Relation::new(
            "binding-result",
            RelationKind::Derived,
            [Slot::new("item", Type::Int), Slot::new("value", Type::Int)],
        ));
        let guard = p.relation(Relation::new("binding-guard", RelationKind::Static, [Slot::new("key", Type::Int)]));
        for key in 0_i64..5 { p.fact(Fact::new(guard, [Value::Int(key)])); }
        p.rule(
            Rule::new(
                "binding",
                Atom::new(result, [Expr::var("item"), Expr::var("result")]),
            )
            .when([
                inputs(),
                Atom::new(guard, [Expr::var("item")]).positive(),
                Literal::bind("result", Expr::call("offset", [Expr::var("item")])),
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
    if rng.chance(40) {
        let input = p.relation(Relation::new("query-input", RelationKind::Static,
            [Slot::new("group", Type::Int), Slot::new("item", Type::Int), Slot::new("witness", Type::Int)])
            .mode(Mode::AtMostOne { keys: vec![0], values: vec![1] }));
        let guard = p.relation(Relation::new("query-guard", RelationKind::Static, [Slot::new("group", Type::Int)]));
        for group in 0_i64..16 {
            if group != 1 { p.fact(Fact::new(guard, [Value::Int(group)])); }
            p.fact(Fact::new(input, [Value::Int(group), Value::Int(if group == 1 { i64::MAX } else { -2 }), Value::Int(0)]));
        }
        p.fact(Fact::new(input, [Value::Int(0), Value::Int(-2), Value::Int(1)]));
        let result = p.relation(Relation::new("grouped-all-result", RelationKind::Derived,
            [Slot::new("group", Type::Int), Slot::new("rows", Type::Int.list())]));
        p.rule(Rule::new("grouped-all", Atom::new(result, [Expr::var("group"), Expr::var("rows")])).when([
            Atom::new(guard, [Expr::var("group")]).positive(),
            Literal::bind("rows", Query::all([Variable::new("item")], [
                Atom::new(input, [Expr::var("group"), Expr::var("item"), Expr::var("witness")]).positive(),
            ]).sorted_by(Expr::binary(BinaryOp::Add, Expr::var("item"), Expr::value(1))).into()),
        ]));
        let result = p.relation(Relation::new("grouped-the-result", RelationKind::Derived,
            [Slot::new("group", Type::Int), Slot::new("item", Type::Int.maybe())]));
        p.rule(Rule::new("grouped-the", Atom::new(result, [Expr::var("group"), Expr::var("item")])).when([
            Atom::new(guard, [Expr::var("group")]).positive(),
            Literal::bind("item", Query::the("query-item", [
                Atom::new(input, [Expr::var("group"), Expr::var("query-item"), Expr::var("witness")]).positive(),
            ]).into()),
        ]));
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
    if rng.chance(33) {
        p.state_entity_type("Cell", [Slot::new("born", Type::Int)]);
        let born = p.relation(Relation::new("Cell.born", RelationKind::State, [Slot::new("cell", Type::Entity("Cell".into())), Slot::new("value", Type::Int)]));
        let value = p.relation(Relation::new("Cell.value", RelationKind::State, [Slot::new("cell", Type::Entity("Cell".into())), Slot::new("value", Type::Int)]));
        let result = p.relation(Relation::new("cell-result", RelationKind::Derived, [Slot::new("cell", Type::Entity("Cell".into())), Slot::new("born", Type::Int), Slot::new("value", Type::Int)]).mode(Mode::ExactlyOne { key: 0, values: vec![1, 2] }).mode(Mode::AtMostOne { keys: vec![0], values: vec![1] }));
        for id in 0..rng.range(6, 9) {
            let entity = Entity::created("Cell", format!("cell-{id}"), id as u64);
            p.entity(entity.clone());
            for relation in [born, value] { p.fact(Fact::new(relation, [Value::Entity(entity.clone()), Value::Int(rng.below(5) as i64)])); }
        }
        p.rule(Rule::new("cell-fields", Atom::new(result, [Expr::var("cell"), Expr::var("born"), Expr::var("value")])).when([
            Atom::new(born, [Expr::var("cell"), Expr::var("born")]).positive(),
            Literal::bind("value", Expr::Field { value: Box::new(Expr::var("cell")), field: "value".into() }),
        ]));
        let cached = p.relation(Relation::new("Cell.cached", RelationKind::Derived, [Slot::new("cell", Type::Entity("Cell".into())), Slot::new("value", Type::Int)]));
        p.rule(Rule::new("cell-cached", Atom::new(cached, [Expr::var("cell"), Expr::var("value")])).when([
            Atom::new(value, [Expr::var("cell"), Expr::var("value")]).positive(),
        ]));
        let total = p.relation(Relation::new("cell-summary", RelationKind::Derived, [Slot::new("value", Type::Int)]));
        p.rule(Rule::new("cell-summary", Atom::new(total, [Expr::var("total")])).when([
            Literal::aggregate("total", AggregateOp::Sum(Expr::Field { value: Box::new(Expr::var("cell")), field: "cached".into() }),
                [Atom::new(born, [Expr::var("cell"), Expr::var("born")]).positive()]),
        ]));
        let rank = p.relation(Relation::new("cell-rank", RelationKind::Derived, [Slot::new("cell", Type::Entity("Cell".into())), Slot::new("n", Type::Int)]));
        let field = |cell: &str| Expr::Field { value: Box::new(Expr::var(cell)), field: "value".into() };
        let (mut left, mut right) = (field("prior"), Expr::var("threshold"));
        if rng.chance(50) { std::mem::swap(&mut left, &mut right); }
        let op = if rng.chance(50) { AggregateOp::Count } else { AggregateOp::Sum(field("prior")) };
        let group = if rng.chance(50) { "born" } else { "prior-born" };
        p.rule(Rule::new("cell-rank", Atom::new(rank, [Expr::var("cell"), Expr::var("n")])).when([
            Atom::new(born, [Expr::var("cell"), Expr::var("born")]).positive(),
            Literal::bind("threshold", field("cell")),
            Literal::aggregate("n", op, [Atom::new(born, [Expr::var("prior"), Expr::var(group)]).positive(), Literal::compare(OPS[2 + rng.below(4)], left, right)]),
        ]));
    }
    if rng.chance(90) {
        let int = |name: &str| Slot::new(name, Type::Int);
        let edge = p.relation(Relation::new("join-input", RelationKind::Static, [int("from"), int("to")]));
        let block = p.relation(Relation::new("join-block", RelationKind::Static, [int("node")]));
        let result = p.relation(Relation::new("join-result", RelationKind::Derived, [int("from"), int("to")]));
        for _ in 0..rng.range(10, 20) { p.fact(Fact::new(edge, [Value::Int(rng.below(5) as i64), Value::Int(rng.below(5) as i64)])); }
        for node in 0_i64..5 { if rng.chance(60) { p.fact(Fact::new(block, [Value::Int(node)])); } }
        p.rule(Rule::new("join", Atom::new(result, [Expr::var("from"), Expr::var("to")])).when([
            Atom::new(edge, [Expr::var("from"), Expr::var("via")]).positive(),
            Atom::new(edge, [Expr::var("via"), Expr::var("to")]).positive(),
            Atom::new(block, [Expr::var("via")]).negative(),
        ]));
    }
    if rng.0 % 2 == 0 {
        let seed = p.relation(Relation::new("reach-seed", RelationKind::Static, [Slot::new("node", Type::Int)]));
        let edge = p.relation(Relation::new("reach-edge", RelationKind::Static, [Slot::new("from", Type::Int), Slot::new("to", Type::Int)]));
        let reach = p.relation(Relation::new("reach", RelationKind::Derived, [Slot::new("node", Type::Int)]));
        for node in [0, 10, 20, 30, 40] {
            p.fact(Fact::new(seed, [Value::Int(node)]));
            p.fact(Fact::new(edge, [Value::Int(node), Value::Int(node + 1)]));
            p.fact(Fact::new(edge, [Value::Int(node + 1), Value::Int(node)]));
        }
        for (from, to) in [(10, 2), (11, 2), (2, 3), (3, 2)] {
            p.fact(Fact::new(edge, [Value::Int(from), Value::Int(to)]));
        }
        p.rule(Rule::new("reach-seeded", Atom::new(reach, [Expr::var("node")])).when([
            Atom::new(seed, [Expr::var("node")]).positive(),
        ]));
        p.rule(Rule::new("reach-edge", Atom::new(reach, [Expr::var("to")])).when([
            Atom::new(reach, [Expr::var("from")]).positive(),
            Atom::new(edge, [Expr::var("from"), Expr::var("to")]).positive(),
        ]));
    }
    p.relation(Relation::new("moded-presence", RelationKind::Derived, [Slot::new("item", Type::Int), Slot::new("value", Type::Int)]).mode(Mode::AtMostOne { keys: vec![0], values: vec![1] }));
    let mut program = p.build();
    if program.relation_id("reach").is_some() {
        let floor = RelationId(program.relations.len());
        let sources: Vec<_> = program.relations.iter().enumerate()
            .filter(|(_, relation)| relation.kind == RelationKind::Derived && relation.name != "reach")
            .map(|(index, relation)| (RelationId(index), relation.slots.len())).collect();
        program.relations.push(Relation::new("reach-floor", RelationKind::Derived, []));
        for (relation, arity) in sources {
            program.rules.push(Rule::new(format!("reach-floor-{}", relation.0), Atom::new(floor, []))
                .when([Literal::Boolean(Expr::value(false)),
                    Atom::new(relation, (0..arity).map(|slot| Expr::var(format!("_#reach-{slot}")))).positive()]));
        }
        program.rules.iter_mut().find(|rule| rule.name == "reach-seeded").unwrap()
            .body.push(Atom::new(floor, []).negative());
    }
    (program, ranked)
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
        (Err(EvalError::Unknown(_)), _) | (_, Err(EvalError::Unknown(_))) => Err("Unknown is not agreement".into()),
        (Err(EvalError::Overflow { at: reference }), Err(EvalError::Overflow { at: engine }))
        | (Err(EvalError::Type { at: reference, .. }), Err(EvalError::Type { at: engine, .. })) if reference == engine => Ok(()),
        (Err(EvalError::Static(reference)), Err(EvalError::Static(engine))) if reference == engine => Ok(()),
        (Err(EvalError::Mode(reference)), Err(EvalError::Mode(engine))) if reference == engine => Ok(()),
        (reference, engine) => Err(format!("reference {reference:?}, engine {engine:?}")),
    }
}

#[derive(Default)]
struct Changes {
    batches: usize,
    deletions: usize,
    creations: usize,
    entity_deletions: usize,
    replacements: usize,
    rollbacks: usize,
    unknown: usize,
    errors: usize,
    mode_errors: usize,
    conflicting_keys: usize,
    multiple_modes: usize,
    newly_live_missing: usize,
    executor_strata: usize,
    guarded_overflows: usize,
    insertion_batches: usize,
    both_pass_batches: usize,
    half_rule_batches: usize,
    field_replacement_batches: usize,
    field_creation_batches: usize,
    cancelled_batches: usize,
    dred_batches: usize,
    cycle_seed_deletions: usize,
    seed_replacements: usize,
    query_both_batches: [usize; 2],
    query_head_deletions: usize,
    unread_sort_overflows: usize,
    summary_batches: [usize; 6],
    aggregate_both_batches: [usize; 6],
    aggregate_cancellations: usize,
    aggregate_ties: usize,
    aggregate_last_rows: usize,
    witness_deletions: usize,
    unread_overflows: usize,
    sum_range_exits: usize,
    sum_range_reentries: usize,
    ranked_both: [usize; 4],
    ranked_batches: usize,
    ranked_repeated: usize,
    ranked_negative: usize,
    ranked_out_of_range: usize,
    cell_rank_born: usize,
    cell_rank_values: usize,
    cell_rank_ops: [usize; 4],
    outer_reads: usize,
    outer_changes: usize,
    outer_replacements: usize,
}

fn reference(program: &Program, facts: &BTreeSet<Tuple>, live: &BTreeSet<Entity>) -> Result<Model, EvalError> {
    let mut program = program.clone();
    program.entities.retain(|entity| !program.entity_types.iter().any(|ty| ty.state && ty.name == entity.entity_type));
    program.entities.extend(live.iter().cloned());
    evaluate(&program, &facts.iter().map(|(relation, values)| Fact::new(*relation, values.clone())).collect::<Vec<_>>(), Budget::default())
}

fn mutate(rng: &mut Rng, program: &Program, facts: &BTreeSet<Tuple>, live: &BTreeSet<Entity>, batch: usize, counts: &mut Changes) -> Change {
    let mut change = Change::default();
    let mutable: Vec<_> = program.relations.iter().enumerate().filter(|(_, relation)| !["reach-seed", "reach-edge", "reach", "reach-floor"].contains(&relation.name.as_str()) && relation.slots.iter().all(|slot| slot.ty == Type::Int)).map(|(index, _)| RelationId(index)).collect();
    for _ in 0..rng.range(1, 3) {
        if rng.chance(65) {
            let edge = program.relation_id("join-input").filter(|_| rng.chance(70));
            let candidates: Vec<_> = facts.iter().filter(|(relation, _)| mutable.contains(relation) && edge.is_none_or(|edge| *relation == edge)).collect();
            if !candidates.is_empty() { change.withdrawn.insert(candidates[rng.below(candidates.len())].clone()); }
        }
        let relation = match program.relation_id("join-input") { Some(edge) if rng.chance(60) => edge, _ => mutable[rng.below(mutable.len())] };
        let values = program.relations[relation.0].slots.iter().map(|_| Value::Int(rng.below(5) as i64)).collect();
        change.added.insert((relation, values));
    }
    if let (Some(born), Some(value)) = (program.relation_id("Cell.born"), program.relation_id("Cell.value")) {
        if rng.chance(60) {
            let id = 100 + batch as u64;
            let entity = Entity::created("Cell", format!("cell-{id}"), id);
            change.created.insert(entity.clone());
            for relation in [born, value] { if relation == value || rng.chance(70) { change.added.insert((relation, vec![Value::Entity(entity.clone()), Value::Int(rng.below(5) as i64)])); } }
        }
        if rng.chance(25) {
            for entity in live.iter().take(2) {
                let old = facts.iter().find(|(relation, values)| *relation == born && values.first() == Some(&Value::Entity(entity.clone()))).unwrap();
                let Value::Int(previous) = old.1[1] else { unreachable!() };
                change.added.insert((born, vec![Value::Entity(entity.clone()), Value::Int((previous + 1) % 5)]));
            }
        }
        if !live.is_empty() && rng.chance(40) {
            let entity = live.iter().nth(rng.below(live.len())).unwrap();
            let old = facts.iter().find(|(relation, values)| *relation == born && values.first() == Some(&Value::Entity(entity.clone()))).unwrap();
            let Value::Int(previous) = old.1[1] else { unreachable!() };
            change.withdrawn.insert(old.clone());
            change.added.insert((born, vec![Value::Entity(entity.clone()), Value::Int((previous + 1 + rng.below(4) as i64) % 5)]));
        }
        if !live.is_empty() {
            let entity = live.iter().nth(rng.below(live.len())).unwrap();
            if rng.chance(45) {
                change.deleted.insert(entity.clone());
                change.withdrawn.extend(facts.iter().filter(|(_, values)| values.contains(&Value::Entity(entity.clone()))).cloned());
            } else {
                let old = facts.iter().find(|(relation, values)| *relation == value && values.first() == Some(&Value::Entity(entity.clone()))).unwrap();
                let Value::Int(previous) = old.1[1] else { unreachable!() };
                change.withdrawn.insert(old.clone());
                change.added.insert((value, vec![Value::Entity(entity.clone()), Value::Int((previous + 1 + rng.below(4) as i64) % 5)]));
                counts.replacements += 1;
            }
        }
    }
    if let Some(input) = program.relation_id("summary-input") {
        change.added.retain(|(id, _)| *id != input);
        change.withdrawn.retain(|(id, _)| *id != input);
        if batch % 2 == 0 {
            change.added.insert((input, vec![Value::Int(2), Value::Int(1), Value::Int(i64::MAX - 1)]));
        } else {
            change.withdrawn.insert((input, vec![Value::Int(2), Value::Int(1), Value::Int(i64::MAX - 1)]));
        }
        let row = |group, item, key| (input, vec![Value::Int(group), Value::Int(item), Value::Int(key)]);
        let changing = facts.iter().find(|(id, values)| *id == input && values[0] == Value::Int(3)).unwrap();
        change.withdrawn.insert(changing.clone());
        let flip = changing.1[1] == Value::Int(0);
        change.added.insert(row(3, i64::from(flip), if flip { -3 } else { -2 }));
        let last = row(4, 0, -2);
        if facts.contains(&last) { change.withdrawn.insert(last); } else { change.added.insert(last); }
        change.withdrawn.extend(facts.iter().filter(|(id, values)| *id == input && values[0] == Value::Int(5)).cloned());
        let cancel = facts.contains(&row(5, 0, -2));
        for (item, key) in if cancel { [(0, -3), (1, -1)] } else { [(0, -2), (1, -2)] } { change.added.insert(row(5, item, key)); }
        let tied = row(6, 1, -2);
        if facts.contains(&tied) { change.withdrawn.insert(tied); } else { change.added.insert(tied); }
        let guard = program.relation_id("summary-guard").unwrap();
        change.added.retain(|(id, _)| *id != guard);
        change.withdrawn.retain(|(id, _)| *id != guard);
        change.added.insert((guard, vec![Value::Int(0)]));
        change.withdrawn.insert((guard, vec![Value::Int(1)]));
        if batch % 2 == 0 { change.withdrawn.insert((guard, vec![Value::Int(2)])); }
        else { change.added.insert((guard, vec![Value::Int(2)])); }
        change.withdrawn.insert((input, vec![Value::Int(0), Value::Int(0), Value::Int(-2)]));
        if batch % 3 != 0 { change.added.insert((input, vec![Value::Int(0), Value::Int(0), Value::Int(-2)])); }
    }
    if let Some(input) = program.relation_id("ranked-weight-input") {
        change.added.retain(|(id, _)| *id != input);
        change.withdrawn.retain(|(id, _)| *id != input);
        let group = (batch % 5) as i64;
        let row = (input, vec![Value::Int(group), Value::Int(1), Value::Int(-2), Value::Int(1)]);
        if facts.contains(&row) { change.withdrawn.insert(row); } else { change.added.insert(row); }
    }
    if let Some(input) = program.relation_id("query-input") {
        let guard = program.relation_id("query-guard").unwrap();
        change.added.retain(|(id, _)| *id != guard && *id != input);
        change.withdrawn.retain(|(id, _)| *id != guard && *id != input);
        let duplicate = (input, vec![Value::Int(0), Value::Int(-2), Value::Int(0)]);
        if facts.contains(&duplicate) { change.withdrawn.insert(duplicate); } else { change.added.insert(duplicate); }
        let old = facts.iter().find(|(id, row)| *id == input && row[0] == Value::Int(3)).unwrap();
        change.withdrawn.insert(old.clone());
        change.added.insert((input, vec![Value::Int(3), Value::Int(if old.1[1] == Value::Int(-2) { -3 } else { -2 }), Value::Int(0)]));
    }
    if program.relation_id("argmin-result").is_some() {
        let input = program.relation_id("expression-input").unwrap();
        let winning = vec![Value::Int(-5), Value::Int(-5), Value::Int(0)];
        if batch % 2 == 0 { change.withdrawn.insert((input, winning)); }
        else { change.added.insert((input, winning)); }
    }
    if let Some(seed) = program.relation_id("reach-seed") {
        match batch % 4 {
            0 => { change.withdrawn.insert((seed, vec![Value::Int(0)])); }
            1 => {
                change.withdrawn.insert((seed, vec![Value::Int(10)]));
                change.added.insert((seed, vec![Value::Int(11)]));
            }
            2 => {
                let edge = program.relation_id("reach-edge").unwrap();
                change.withdrawn.insert((edge, vec![Value::Int(10), Value::Int(2)]));
            }
            _ => {
                change.added.insert((seed, vec![Value::Int(0)]));
                change.withdrawn.insert((seed, vec![Value::Int(11)]));
                change.added.insert((seed, vec![Value::Int(10)]));
                let edge = program.relation_id("reach-edge").unwrap();
                change.added.insert((edge, vec![Value::Int(10), Value::Int(2)]));
            }
        }
    }
    counts.deletions += change.withdrawn.len();
    counts.creations += change.created.len();
    counts.entity_deletions += change.deleted.len();
    change
}

fn batches(rng: &mut Rng, program: &Program, seed: u64, index: usize, counts: &mut Changes) {
    let mut program = program.clone();
    let mut facts: BTreeSet<_> = std::mem::take(&mut program.facts).into_iter().map(|fact| (fact.relation, fact.values)).collect();
    let mut live: BTreeSet<_> = program.entities.iter().filter(|entity| program.entity_types.iter().any(|ty| ty.state && ty.name == entity.entity_type)).cloned().collect();
    let initial = reference(&program, &facts, &live);
    let built = Maintained::build(&program, &facts, &live, &[], Budget::default());
    let mut maintained = match built {
        Ok(maintained) => maintained,
        Err(error) => { agree(&initial, &Err(error)).unwrap_or_else(|divergence| panic!("seed {seed:#x}, build {index}: {divergence}\n{program:#?}")); return; }
    };
    let built_model = Model { relations: (0..program.relations.len()).map(RelationId).filter(|id| !maintained.presence(*id).is_empty()).map(|id| (id, std::sync::Arc::new(maintained.presence(id).clone()))).collect(), work: maintained.work() };
    agree(&initial, &Ok(built_model)).unwrap_or_else(|divergence| panic!("seed {seed:#x}, build {index}: {divergence}\n{program:#?}"));
    counts.executor_strata += maintained.executor_built_strata();
    if let (Some(input), Some(guard)) = (program.relation_id("expression-input"), program.relation_id("binding-guard")) {
        counts.guarded_overflows += usize::from(facts.iter().any(|(relation, values)| *relation == input && values[0] == Value::Int(i64::MAX)) && !facts.contains(&(guard, vec![Value::Int(i64::MAX)])));
    }
    let snapshot = |maintained: &Maintained| (0..program.relations.len()).map(|id| (maintained.presence(RelationId(id)).clone(), maintained.version(RelationId(id)))).collect::<Vec<_>>();
    for batch in 0..8 {
        let before = snapshot(&maintained);
        let mark = maintained.mark();
        let change = mutate(rng, &program, &facts, &live, batch, counts);
        let mut next_facts: BTreeSet<_> = facts.difference(&change.withdrawn).cloned().collect();
        next_facts.extend(change.added.iter().cloned());
        let mut next_live: BTreeSet<_> = live.difference(&change.deleted).cloned().collect();
        next_live.extend(change.created.iter().cloned());
        let expected = reference(&program, &next_facts, &next_live);
        let applied = maintained.apply(&change, Budget::default());
        if let Err(EvalError::Mode(failures)) = &expected {
            let mut conflicting: std::collections::BTreeMap<_, BTreeSet<_>> = std::collections::BTreeMap::new();
            let mut keys = BTreeSet::new();
            let mut multiple_modes = false;
            for failure in failures {
                if failure.tuples.len() > 1 { conflicting.entry(failure.relation).or_default().insert(&failure.key); }
                if !keys.insert((failure.relation, &failure.key)) { multiple_modes = true; }
            }
            counts.multiple_modes += usize::from(multiple_modes);
            counts.conflicting_keys += usize::from(conflicting.values().any(|keys| keys.len() > 1));
            counts.newly_live_missing += usize::from(failures.iter().any(|failure| failure.tuples.is_empty() && matches!(failure.key.as_slice(), [Value::Entity(entity)] if change.created.contains(entity) && !live.contains(entity))));
        }
        if matches!(expected, Err(EvalError::Unknown(_))) || matches!(applied, Err(EvalError::Unknown(_))) {
            counts.unknown += 1;
            maintained.undo(mark);
            continue;
        }
        counts.batches += 1;
        if applied.is_ok() {
            if maintained.dred_strata() > 0 {
                counts.dred_batches += 1;
                if let Some(seed) = program.relation_id("reach-seed") {
                    counts.cycle_seed_deletions += usize::from(
                        change.withdrawn.contains(&(seed, vec![Value::Int(0)]))
                        && facts.contains(&(seed, vec![Value::Int(0)]))
                        && !next_facts.contains(&(seed, vec![Value::Int(0)]))
                        && !next_facts.contains(&(seed, vec![Value::Int(1)]))
                        && [10, 20, 30, 40].into_iter().all(|node|
                            next_facts.contains(&(seed, vec![Value::Int(node)]))));
                    counts.seed_replacements += usize::from(
                        change.withdrawn.contains(&(seed, vec![Value::Int(10)]))
                        && facts.contains(&(seed, vec![Value::Int(10)]))
                        && !next_facts.contains(&(seed, vec![Value::Int(10)]))
                        && !facts.contains(&(seed, vec![Value::Int(11)]))
                        && next_facts.contains(&(seed, vec![Value::Int(11)])));
                }
            }
            let ranked_both = maintained.ranked_both_passes();
            for (count, read) in counts.ranked_both.iter_mut().zip(ranked_both) { *count += usize::from(read > 0); }
            if ranked_both.iter().any(|n| *n > 0) {
                counts.ranked_batches += 1;
                if let Some(input) = program.relation_id("ranked-weight-input") {
                    let rows: Vec<_> = next_facts.iter().filter(|(id, _)| *id == input).map(|(_, row)| row).collect();
                    counts.ranked_repeated += usize::from(rows.windows(2).any(|pair| pair[0][..2] == pair[1][..2]));
                    counts.ranked_negative += usize::from(rows.iter().any(|row| matches!(row[2], Value::Int(n) if n < 0)));
                    let mut sum = 0_i128;
                    let mut outside = false;
                    let mut group = None;
                    for row in rows {
                        if group != Some(&row[0]) { group = Some(&row[0]); sum = 0; }
                        let Value::Int(weight) = row[2] else { unreachable!() };
                        sum += i128::from(weight);
                        outside |= i64::try_from(sum).is_err();
                    }
                    counts.ranked_out_of_range += usize::from(outside);
                }
            }
            let queries = maintained.query_both_passes();
            for (count, passes) in counts.query_both_batches.iter_mut().zip(queries) { *count += usize::from(passes > 0); }
            if queries[0] > 0 {
                if let Some(input) = program.relation_id("query-input") {
                    counts.query_head_deletions += usize::from(facts.contains(&(input, vec![Value::Int(0), Value::Int(-2), Value::Int(0)]))
                        && !next_facts.contains(&(input, vec![Value::Int(0), Value::Int(-2), Value::Int(0)]))
                        && next_facts.contains(&(input, vec![Value::Int(0), Value::Int(-2), Value::Int(1)])));
                    let guard = program.relation_id("query-guard").unwrap();
                    counts.unread_sort_overflows += usize::from(next_facts.contains(&(input, vec![Value::Int(1), Value::Int(i64::MAX), Value::Int(0)]))
                        && !next_facts.contains(&(guard, vec![Value::Int(1)])));
                }
            }
            let reads = maintained.summary_reads();
            for (count, read) in counts.summary_batches.iter_mut().zip(reads) { *count += usize::from(read > 0); }
            let both = maintained.summary_both_passes();
            for (count, passes) in counts.aggregate_both_batches.iter_mut().zip(both) { *count += usize::from(passes > 0); }
            if both.iter().all(|n| *n > 0) {
                if let Some(input) = program.relation_id("summary-input") {
                    let rows = |set: &BTreeSet<Tuple>, group| set.iter().filter(|(id, row)| *id == input && row[0] == Value::Int(group)).map(|(_, row)| row.clone()).collect::<BTreeSet<_>>();
                    let before = rows(&facts, 5);
                    let after = rows(&next_facts, 5);
                    let sum = |set: &BTreeSet<Vec<Value>>| set.iter().map(|row| match row[2] { Value::Int(n) => i128::from(n), _ => unreachable!() }).sum::<i128>();
                    counts.aggregate_cancellations += usize::from(before != after && before.len() == after.len() && sum(&before) == sum(&after));
                    counts.aggregate_ties += usize::from(rows(&facts, 6).len() == 2 && rows(&next_facts, 6).len() == 1);
                    counts.aggregate_last_rows += usize::from(rows(&facts, 4).len() == 1 && rows(&next_facts, 4).is_empty());
                }
            }
            if reads[4] > 0 {
                let input = program.relation_id("expression-input").unwrap();
                counts.witness_deletions += usize::from(change.withdrawn.contains(&(input, vec![Value::Int(-5), Value::Int(-5), Value::Int(0)]))
                    && facts.contains(&(input, vec![Value::Int(-5), Value::Int(-5), Value::Int(0)]))
                    && next_facts.contains(&(input, vec![Value::Int(-5), Value::Int(-5), Value::Int(1)])));
            }
            if reads[1] > 0 {
                if let Some(input) = program.relation_id("summary-input") {
                    let guard = program.relation_id("summary-guard").unwrap();
                    counts.unread_overflows += usize::from(next_facts.contains(&(input, vec![Value::Int(1), Value::Int(0), Value::Int(i64::MAX)]))
                        && !next_facts.contains(&(guard, vec![Value::Int(1)])));
                    let sum = |rows: &BTreeSet<Tuple>| rows.iter().filter(|(id, row)| *id == input && row[0] == Value::Int(2)).map(|(_, row)| {
                        let Value::Int(key) = row[2] else { unreachable!() }; i128::from(key) + 1
                    }).sum::<i128>();
                    let before = i64::try_from(sum(&facts)).is_ok();
                    let after = i64::try_from(sum(&next_facts)).is_ok();
                    counts.sum_range_exits += usize::from(before && !after);
                    counts.sum_range_reentries += usize::from(!before && after);
                }
            }
        }
        counts.insertion_batches += usize::from(applied.is_ok() && maintained.insertion_pass_strata() > 0);
        counts.both_pass_batches += usize::from(applied.is_ok() && maintained.both_pass_strata() > 0);
        counts.half_rule_batches += usize::from(applied.is_ok() && maintained.half_rule_relations() > 0);
        counts.cancelled_batches += usize::from(applied.is_ok() && maintained.untouched_cancelled_facts() > 0);
        counts.outer_reads += usize::from(applied.is_ok() && maintained.outer_index_reads() > 0);
        counts.outer_changes += usize::from(applied.is_ok() && maintained.outer_index_changes() > 0);
        counts.outer_replacements += usize::from(applied.is_ok() && maintained.outer_index_replacements() > 0);
        if let (Ok(_), Some(value)) = (&applied, program.relation_id("Cell.value")) {
            let replaced = change.withdrawn.iter().any(|(relation, row)| *relation == value && matches!(&row[0], Value::Entity(entity) if !change.deleted.contains(entity) && change.added.iter().any(|(r, added)| *r == value && added[0] == row[0])));
            counts.field_replacement_batches += usize::from(replaced && maintained.field_both_pass_strata() > 0);
            counts.field_creation_batches += usize::from(!change.created.is_empty() && maintained.field_created_insertion_strata() > 0);
            if program.relation_id("cell-rank").is_some() {
                let born = program.relation_id("Cell.born").unwrap();
                counts.cell_rank_born += usize::from(change.added.iter().any(|tuple| tuple.0 == born && !facts.contains(tuple)) || change.withdrawn.iter().any(|tuple| tuple.0 == born && facts.contains(tuple)));
                counts.cell_rank_values += usize::from(replaced);
                if program.relation_id("ranked-weight-input").is_none() {
                    for (count, read) in counts.cell_rank_ops.iter_mut().zip(maintained.ranked_both_passes()) { *count += usize::from(read > 0); }
                }
            }
        }
        match (&expected, &applied) {
            (Ok(model), Ok(changed)) => {
                let actual_changed: BTreeSet<_> = (0..program.relations.len()).map(RelationId).filter(|id| before[id.0].0 != *maintained.presence(*id)).collect();
                assert_eq!(*changed, actual_changed, "seed {seed:#x}, program {index}, batch {batch}: changed relations");
                for id in (0..program.relations.len()).map(RelationId) {
                    assert!(model.tuples(id).eq(maintained.presence(id).iter()), "seed {seed:#x}, program {index}, batch {batch}, relation {id:?}: reference {:?}, maintained {:?}\nchange {change:#?}\n{program:#?}", model.tuples(id).collect::<Vec<_>>(), maintained.presence(id));
                    assert_eq!(maintained.version(id), before[id.0].1 + u64::from(changed.contains(&id)));
                }
            }
            (Err(_), Err(error)) => { counts.errors += 1; counts.mode_errors += usize::from(matches!(error, EvalError::Mode(_))); agree(&expected, &Err(error.clone())).unwrap_or_else(|divergence| panic!("seed {seed:#x}, program {index}, batch {batch}: {divergence}\nchange {change:#?}\n{program:#?}")); }
            _ => panic!("seed {seed:#x}, program {index}, batch {batch}: reference {expected:?}, maintained {applied:?}\nchange {change:#?}\n{program:#?}"),
        }
        if expected.is_err() || rng.chance(50) {
            maintained.undo(mark);
            assert_eq!(snapshot(&maintained), before, "seed {seed:#x}, program {index}, batch {batch}: undo");
            counts.rollbacks += 1;
        } else {
            facts = next_facts;
            live = next_live;
        }
    }
}


const SHARDS: u64 = 4;

#[derive(Default)]
struct Tally {
    accepted: usize,
    filtered: usize,
    unknown: usize,
    negations: usize,
    generated: usize,
    ranked_generated: usize,
    ranked_compared: usize,
    ranked_unknown: usize,
    aggregates: [usize; 4],
    argmins: usize,
    bindings: usize,
    memberships: usize,
    definitions: usize,
    all: usize,
    the: usize,
    models: usize,
    errors: usize,
    changes: Changes,
}

impl Tally {
    fn add(&mut self, other: Tally) {
        self.accepted += other.accepted;
        self.filtered += other.filtered;
        self.unknown += other.unknown;
        self.negations += other.negations;
        self.generated += other.generated;
        self.ranked_generated += other.ranked_generated;
        self.ranked_compared += other.ranked_compared;
        self.ranked_unknown += other.ranked_unknown;
        for (sum, count) in self.aggregates.iter_mut().zip(other.aggregates) { *sum += count; }
        self.argmins += other.argmins;
        self.bindings += other.bindings;
        self.memberships += other.memberships;
        self.definitions += other.definitions;
        self.all += other.all;
        self.the += other.the;
        self.models += other.models;
        self.errors += other.errors;
        let (sum, c) = (&mut self.changes, other.changes);
        sum.batches += c.batches;
        sum.deletions += c.deletions;
        sum.creations += c.creations;
        sum.entity_deletions += c.entity_deletions;
        sum.replacements += c.replacements;
        sum.rollbacks += c.rollbacks;
        sum.unknown += c.unknown;
        sum.errors += c.errors;
        sum.mode_errors += c.mode_errors;
        sum.conflicting_keys += c.conflicting_keys;
        sum.multiple_modes += c.multiple_modes;
        sum.newly_live_missing += c.newly_live_missing;
        sum.executor_strata += c.executor_strata;
        sum.guarded_overflows += c.guarded_overflows;
        sum.insertion_batches += c.insertion_batches;
        sum.both_pass_batches += c.both_pass_batches;
        sum.half_rule_batches += c.half_rule_batches;
        sum.field_replacement_batches += c.field_replacement_batches;
        sum.field_creation_batches += c.field_creation_batches;
        sum.cancelled_batches += c.cancelled_batches;
        sum.dred_batches += c.dred_batches;
        sum.cycle_seed_deletions += c.cycle_seed_deletions;
        sum.seed_replacements += c.seed_replacements;
        for (sum, count) in sum.summary_batches.iter_mut().zip(c.summary_batches) { *sum += count; }
        for (sum, count) in sum.aggregate_both_batches.iter_mut().zip(c.aggregate_both_batches) { *sum += count; }
        for (sum, count) in sum.query_both_batches.iter_mut().zip(c.query_both_batches) { *sum += count; }
        sum.query_head_deletions += c.query_head_deletions;
        sum.unread_sort_overflows += c.unread_sort_overflows;
        sum.aggregate_cancellations += c.aggregate_cancellations;
        sum.aggregate_ties += c.aggregate_ties;
        sum.aggregate_last_rows += c.aggregate_last_rows;
        sum.witness_deletions += c.witness_deletions;
        sum.unread_overflows += c.unread_overflows;
        sum.sum_range_exits += c.sum_range_exits;
        sum.sum_range_reentries += c.sum_range_reentries;
        for (sum, count) in sum.ranked_both.iter_mut().zip(c.ranked_both) { *sum += count; }
        sum.ranked_batches += c.ranked_batches;
        sum.ranked_repeated += c.ranked_repeated;
        sum.ranked_negative += c.ranked_negative;
        sum.ranked_out_of_range += c.ranked_out_of_range;
        sum.cell_rank_born += c.cell_rank_born;
        sum.cell_rank_values += c.cell_rank_values;
        for (sum, count) in sum.cell_rank_ops.iter_mut().zip(c.cell_rank_ops) { *sum += count; }
        sum.outer_reads += c.outer_reads;
        sum.outer_changes += c.outer_changes;
        sum.outer_replacements += c.outer_replacements;
    }
}

fn shard(seed: u64, programs: usize) -> Tally {
    let mut rng = Rng(seed);
    let mut t = Tally::default();
    while t.accepted < programs {
        let (program, ranked) = generate(&mut rng);
        t.generated += 1;
        t.ranked_generated += usize::from(ranked);
        if check(&program).is_err() {
            t.filtered += 1;
            continue;
        }
        let reference = evaluate(&program, &[], Budget::default());
        let engine = engine::evaluate(&program, &[], Budget::default());
        if matches!(reference, Err(EvalError::Unknown(_)))
            || matches!(engine, Err(EvalError::Unknown(_)))
        {
            t.unknown += 1;
            t.ranked_unknown += usize::from(ranked);
            continue;
        }
        if let Err(divergence) = agree(&reference, &engine) {
            panic!("seed {seed:#x}, program {}: {divergence}\n{program:#?}", t.accepted);
        }
        batches(&mut rng, &program, seed, t.accepted, &mut t.changes);
        t.ranked_compared += usize::from(ranked);
        t.models += usize::from(reference.is_ok());
        t.errors += usize::from(reference.is_err());
        t.definitions += program.definitions.len();
        for literal in program
            .rules
            .iter()
            .filter(|rule| rule.name != "ranked-aggregate")
            .flat_map(|rule| &rule.body)
        {
            match literal {
                Literal::Aggregate { op, .. } => {
                    t.aggregates[match op {
                        AggregateOp::Count => 0,
                        AggregateOp::Sum(_) => 1,
                        AggregateOp::Min(_) => 2,
                        AggregateOp::Max(_) => 3,
                    }] += 1
                }
                Literal::ArgMin { .. } => t.argmins += 1,
                Literal::Bind { value, .. } => {
                    t.bindings += 1;
                    if let Expr::Query(query) = value {
                        match &query.kind {
                            QueryKind::All(_) => t.all += 1,
                            QueryKind::The(_) => t.the += 1,
                        }
                    }
                }
                Literal::Member { .. } => t.memberships += 1,
                _ => {}
            }
        }
        t.negations += program
            .rules
            .iter()
            .flat_map(|rule| &rule.body)
            .filter(|literal| matches!(literal, Literal::Negative(_)))
            .count();
        t.accepted += 1;
    }
    t
}

#[test]
fn engine_agrees_with_the_reference_on_stratified_programs_with_negation() {
    let pinned = std::env::var("GALILEO_SEED").ok().and_then(|s| s.parse().ok());
    let total = std::env::var("GALILEO_PROGRAMS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if std::env::var_os("GALILEO_FULL").is_some() { PROGRAMS } else { PROGRAMS / 20 });
    let seeds: Vec<(u64, usize)> = match pinned {
        Some(seed) => vec![(seed, total)],
        None => (0..SHARDS * PARTS as u64)
            .filter(|i| *i as usize % PARTS == PART)
            .map(|i| (SEED.wrapping_add(i.wrapping_mul(0x9e37_79b9_7f4a_7c15)), total / (SHARDS as usize * PARTS) + usize::from((i as usize) < total % (SHARDS as usize * PARTS))))
            .collect(),
    };
    let programs: usize = seeds.iter().map(|&(_, n)| n).sum();
    let mut t = Tally::default();
    std::thread::scope(|scope| {
        let running: Vec<_> = seeds.iter().map(|&(seed, n)| scope.spawn(move || shard(seed, n))).collect();
        for handle in running {
            t.add(handle.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)));
        }
    });
    let shards: Vec<String> = seeds.iter().map(|(seed, n)| format!("{seed:#x}×{n}")).collect();
    let changes = &t.changes;
    eprintln!(
        "seeds {}: {} generated, {} programs agree, {} filtered by the static checks, {} unknown, {} negations, {} ranked-aggregate programs generated, {} ranked-aggregate programs compared, {} ranked-aggregate programs unknown",
        shards.join(" "), t.generated, t.accepted, t.filtered, t.unknown, t.negations, t.ranked_generated, t.ranked_compared, t.ranked_unknown
    );
    eprintln!(
        "compared forms: count {}, sum {}, min {}, max {}, argmin {}, bindings {}, grounded memberships {}, definitions {}, all {}, the {}; {} models, {} same-kind errors",
        t.aggregates[0], t.aggregates[1], t.aggregates[2], t.aggregates[3], t.argmins, t.bindings, t.memberships, t.definitions, t.all, t.the, t.models, t.errors
    );
    assert_eq!(t.accepted, programs);
    assert!(t.ranked_compared >= programs / 10, "too few ranked-aggregate programs compared");
    eprintln!("maintained: {} batches, {} deletions, {} entity creations, {} entity deletions, {} state-field replacements, {} rollbacks, {} errors ({} full mode failures), {} unknown", changes.batches, changes.deletions, changes.creations, changes.entity_deletions, changes.replacements, changes.rollbacks, changes.errors, changes.mode_errors, changes.unknown);
    eprintln!("derived modes: {} failing batches, {} with two conflicting keys, {} with two failing modes, {} with newly live missing keys", changes.mode_errors, changes.conflicting_keys, changes.multiple_modes, changes.newly_live_missing);
    if std::env::var_os("GALILEO_FULL").is_some() {
        assert!(changes.mode_errors >= 300 / PARTS, "derived-mode failing batches: {}", changes.mode_errors);
        assert!(changes.conflicting_keys >= 50 / PARTS, "derived-mode conflicting-key batches: {}", changes.conflicting_keys);
        assert!(changes.multiple_modes >= 50 / PARTS, "derived-mode multiple-mode batches: {}", changes.multiple_modes);
        assert!(changes.newly_live_missing >= 50 / PARTS, "derived-mode newly-live missing-key batches: {}", changes.newly_live_missing);
    }
    eprintln!("executor: {} strata built, {} programs with overflowing inputs blocked by their earlier guard", changes.executor_strata, changes.guarded_overflows);
    eprintln!("insertion pass: {} batches", changes.insertion_batches);
    eprintln!("both passes: {} batches, {} recomputed by the half rule, {} withdrew and included one fact and took no pass for it", changes.both_pass_batches, changes.half_rule_batches, changes.cancelled_batches);
    eprintln!("field reads: {} batches replaced a live entity's state field below the half rule and took both passes, {} created an entity whose fields a rule reads and took the insertion pass", changes.field_replacement_batches, changes.field_creation_batches);
    assert!(changes.field_replacement_batches >= programs / 10, "too few field-replacement both-pass batches");
    assert!(changes.field_creation_batches >= programs / 10, "too few field-creation insertion-pass batches");
    assert!(changes.insertion_batches >= programs, "too few insertion-pass batches");
    assert!(changes.both_pass_batches >= programs, "too few both-pass batches");
    assert!(changes.cancelled_batches >= programs / 10, "too few withdrawn-and-included batches");
    assert!(changes.executor_strata >= programs / 2, "too few executor-built strata");
    assert!(changes.guarded_overflows >= programs / 20, "too few guarded overflowing inputs");
    eprintln!("recursive DRed: {} batches, {} only seeds of cycles deleted with four other seeds retained, {} deleted seeds with another added",
        changes.dred_batches, changes.cycle_seed_deletions, changes.seed_replacements);
    if std::env::var_os("GALILEO_FULL").is_some() {
        assert!(changes.dred_batches >= 1_000 / PARTS, "too few recursive DRed batches");
        assert!(changes.cycle_seed_deletions >= 100 / PARTS, "too few only-cycle-seed deletions");
        assert!(changes.seed_replacements >= 100 / PARTS, "too few seed replacements");
    }
    eprintln!("summary-reading batches: count {}, sum {}, min {}, max {}, argmin {}, argmax {}; winning-pair witness deletions {}, unread value overflows {}, sum range exits {}, reentries {}",
        changes.summary_batches[0], changes.summary_batches[1], changes.summary_batches[2], changes.summary_batches[3], changes.summary_batches[4], changes.summary_batches[5],
        changes.witness_deletions, changes.unread_overflows, changes.sum_range_exits, changes.sum_range_reentries);
    if std::env::var_os("GALILEO_FULL").is_some() {
        for count in changes.summary_batches { assert!(count >= 2_000 / PARTS, "too few summary-reading batches: {count}"); }
        for count in [changes.witness_deletions, changes.unread_overflows, changes.sum_range_exits, changes.sum_range_reentries] { assert!(count >= 200 / PARTS, "too few summary edge batches: {count}"); }
    }
    eprintln!("aggregate both-pass batches: count {}, sum {}, min {}, max {}, argmin {}, argmax {}; cancellations {}, tied min/max deletions {}, last-row deletions {}",
        changes.aggregate_both_batches[0], changes.aggregate_both_batches[1], changes.aggregate_both_batches[2], changes.aggregate_both_batches[3], changes.aggregate_both_batches[4], changes.aggregate_both_batches[5],
        changes.aggregate_cancellations, changes.aggregate_ties, changes.aggregate_last_rows);
    if std::env::var_os("GALILEO_FULL").is_some() {
        for count in changes.aggregate_both_batches { assert!(count >= 2_000 / PARTS, "too few aggregate both-pass batches: {count}"); }
        for count in [changes.aggregate_cancellations, changes.aggregate_ties, changes.aggregate_last_rows] { assert!(count >= 200 / PARTS, "too few aggregate delta edges: {count}"); }
    }
    eprintln!("ranked both-pass batches: {} unique; < {}, <= {}, > {}, >= {}; repeated keys {}, negative weights {}, running totals outside Int {}",
        changes.ranked_batches, changes.ranked_both[0], changes.ranked_both[1], changes.ranked_both[2], changes.ranked_both[3], changes.ranked_repeated, changes.ranked_negative, changes.ranked_out_of_range);
    if std::env::var_os("GALILEO_FULL").is_some() {
        for count in changes.ranked_both { assert!(count >= 250 / PARTS, "too few ranked operator both-pass batches: {count}"); }
        assert!(changes.ranked_batches >= 1_000 / PARTS, "too few ranked both-pass batches");
        for count in [changes.ranked_repeated, changes.ranked_negative, changes.ranked_out_of_range] { assert!(count >= 250 / PARTS, "too few ranked edge batches: {count}"); }
    }
    eprintln!("cell-rank batches: {} added or removed a Cell.born tuple, {} replaced a live cell's Cell.value; ranked both passes < {}, <= {}, > {}, >= {}",
        changes.cell_rank_born, changes.cell_rank_values, changes.cell_rank_ops[0], changes.cell_rank_ops[1], changes.cell_rank_ops[2], changes.cell_rank_ops[3]);
    if std::env::var_os("GALILEO_FULL").is_some() {
        for count in [changes.cell_rank_born, changes.cell_rank_values] { assert!(count >= 1_000 / PARTS, "too few cell-rank field batches: {count}"); }
        for count in changes.cell_rank_ops { assert!(count >= 250 / PARTS, "too few cell-rank operator batches: {count}"); }
    }
    eprintln!("ranked outer index batches: {} took a changed group's kept bindings from it, {} gained or lost a binding below the half rule, {} replaced a binding whose threshold changed",
        changes.outer_reads, changes.outer_changes, changes.outer_replacements);
    if std::env::var_os("GALILEO_FULL").is_some() {
        assert!(changes.outer_reads >= 1_000, "too few outer index reads: {}", changes.outer_reads);
        assert!(changes.outer_changes >= 1_000, "too few outer index changes: {}", changes.outer_changes);
        assert!(changes.outer_replacements >= 500, "too few outer index replacements: {}", changes.outer_replacements);
    }
    eprintln!("query both-pass batches: all {}, the {}; duplicate-head deletions {}, unread sorted-by overflows {}",
        changes.query_both_batches[0], changes.query_both_batches[1], changes.query_head_deletions, changes.unread_sort_overflows);
    if std::env::var_os("GALILEO_FULL").is_some() {
        for count in changes.query_both_batches { assert!(count >= 1_000 / PARTS, "too few query both-pass batches: {count}"); }
        for count in [changes.query_head_deletions, changes.unread_sort_overflows] { assert!(count >= 100 / PARTS, "too few query edges: {count}"); }
    }
    assert_eq!(changes.batches, programs * 8);
    assert!(changes.mode_errors > 0, "no mode failures compared");
    for count in [changes.deletions, changes.creations, changes.entity_deletions, changes.replacements, changes.rollbacks] { assert!(count >= programs / 10, "too few mutation shapes: {count}"); }
}
