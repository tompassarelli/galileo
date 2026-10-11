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
    if rng.chance(50) { p.fact(Fact::new(input, [Value::Int(i64::MAX), Value::Int(rng.below(5) as i64)])); }
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
        let result = p.relation(Relation::new("cell-result", RelationKind::Derived, [Slot::new("cell", Type::Entity("Cell".into())), Slot::new("born", Type::Int), Slot::new("value", Type::Int)]));
        for id in 0..rng.range(1, 4) {
            let entity = Entity::created("Cell", format!("cell-{id}"), id as u64);
            p.entity(entity.clone());
            for relation in [born, value] { p.fact(Fact::new(relation, [Value::Entity(entity.clone()), Value::Int(rng.below(5) as i64)])); }
        }
        p.rule(Rule::new("cell-fields", Atom::new(result, [Expr::var("cell"), Expr::var("born"), Expr::var("value")])).when([
            Atom::new(born, [Expr::var("cell"), Expr::var("born")]).positive(),
            Literal::bind("value", Expr::Field { value: Box::new(Expr::var("cell")), field: "value".into() }),
        ]));
    }
    p.relation(Relation::new("moded-presence", RelationKind::Derived, [Slot::new("item", Type::Int), Slot::new("value", Type::Int)]).mode(Mode::AtMostOne { keys: vec![0], values: vec![1] }));
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
    executor_strata: usize,
    guarded_overflows: usize,
}

fn reference(program: &Program, facts: &BTreeSet<Tuple>, live: &BTreeSet<Entity>) -> Result<Model, EvalError> {
    let mut program = program.clone();
    program.entities.retain(|entity| !program.entity_types.iter().any(|ty| ty.state && ty.name == entity.entity_type));
    program.entities.extend(live.iter().cloned());
    evaluate(&program, &facts.iter().map(|(relation, values)| Fact::new(*relation, values.clone())).collect::<Vec<_>>(), Budget::default())
}

fn mutate(rng: &mut Rng, program: &Program, facts: &BTreeSet<Tuple>, live: &BTreeSet<Entity>, batch: usize, counts: &mut Changes) -> Change {
    let mut change = Change::default();
    let mutable: Vec<_> = program.relations.iter().enumerate().filter(|(_, relation)| relation.slots.iter().all(|slot| slot.ty == Type::Int)).map(|(index, _)| RelationId(index)).collect();
    for _ in 0..rng.range(1, 3) {
        if rng.chance(65) {
            let candidates: Vec<_> = facts.iter().filter(|(relation, _)| mutable.contains(relation)).collect();
            if !candidates.is_empty() { change.withdrawn.insert(candidates[rng.below(candidates.len())].clone()); }
        }
        let relation = mutable[rng.below(mutable.len())];
        let values = program.relations[relation.0].slots.iter().map(|_| Value::Int(rng.below(5) as i64)).collect();
        change.added.insert((relation, values));
    }
    if let (Some(born), Some(value)) = (program.relation_id("Cell.born"), program.relation_id("Cell.value")) {
        if rng.chance(60) {
            let id = 100 + batch as u64;
            let entity = Entity::created("Cell", format!("cell-{id}"), id);
            change.created.insert(entity.clone());
            for relation in [born, value] { change.added.insert((relation, vec![Value::Entity(entity.clone()), Value::Int(rng.below(5) as i64)])); }
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
    let built = Maintained::build(&program, &facts, &live, Budget::default());
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
        if matches!(expected, Err(EvalError::Unknown(_))) || matches!(applied, Err(EvalError::Unknown(_))) {
            counts.unknown += 1;
            maintained.undo(mark);
            continue;
        }
        counts.batches += 1;
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
        sum.executor_strata += c.executor_strata;
        sum.guarded_overflows += c.guarded_overflows;
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
    let programs = std::env::var("GALILEO_PROGRAMS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if std::env::var_os("GALILEO_FULL").is_some() { PROGRAMS } else { PROGRAMS / 20 });
    let seeds: Vec<(u64, usize)> = match pinned {
        Some(seed) => vec![(seed, programs)],
        None => (0..SHARDS)
            .map(|i| (SEED.wrapping_add(i.wrapping_mul(0x9e37_79b9_7f4a_7c15)), programs / SHARDS as usize + usize::from((i as usize) < programs % SHARDS as usize)))
            .collect(),
    };
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
    eprintln!("executor: {} strata built, {} programs with overflowing inputs blocked by their earlier guard", changes.executor_strata, changes.guarded_overflows);
    assert!(changes.executor_strata >= programs / 2, "too few executor-built strata");
    assert!(changes.guarded_overflows >= programs / 20, "too few guarded overflowing inputs");
    assert_eq!(changes.batches, programs * 8);
    assert!(changes.mode_errors > 0, "no mode failures compared");
    for count in [changes.deletions, changes.creations, changes.entity_deletions, changes.replacements, changes.rollbacks] { assert!(count >= programs / 10, "too few mutation shapes: {count}"); }
}
