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

fn generate(rng: &mut Rng) -> Program {
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
    p.build()
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
        (Err(EvalError::Unknown(_)), Err(EvalError::Unknown(_))) => Ok(()),
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
        .unwrap_or(PROGRAMS);
    let mut rng = Rng(seed);
    let (mut accepted, mut filtered, mut unknown, mut negations) = (0, 0, 0, 0);
    while accepted < programs {
        let program = generate(&mut rng);
        if check(&program).is_err() {
            filtered += 1;
            continue;
        }
        let reference = evaluate(&program, &[], Budget::default());
        let engine = engine::evaluate(&program, &[], Budget::default());
        if let Err(divergence) = agree(&reference, &engine) {
            panic!("seed {seed:#x}, program {accepted}: {divergence}\n{program:#?}");
        }
        unknown += usize::from(reference.is_err());
        negations += program
            .rules
            .iter()
            .flat_map(|rule| &rule.body)
            .filter(|literal| matches!(literal, Literal::Negative(_)))
            .count();
        accepted += 1;
    }
    eprintln!(
        "seed {seed:#x}: {accepted} programs agree, {filtered} filtered by the static checks, {unknown} unknown, {negations} negations"
    );
}
