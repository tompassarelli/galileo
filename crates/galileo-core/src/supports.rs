use crate::engine::{derive, Derivation, Origin, Premise};
use crate::expr::Meter;
use crate::*;
use crate::check::DependencyKind;
use crate::claims::{check_claims_spending, claim_on_changed};
use crate::why::Proof;
use std::collections::{HashMap, HashSet};

pub type Support = Vec<Proof>;
pub type FactSet = Vec<Fact>;

type Key = (RelationId, Vec<Value>);
type Absence = (RelationId, Vec<Option<Value>>);
type Set = Vec<Leaf>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Leaf {
    Stated(usize),
    Absent(usize),
}

enum Item {
    Present(Key),
    Absent(usize),
}

const AT: &str = "minimal supports";

pub fn minimal_supports(
    program: &Program,
    state: &[Fact],
    relation: RelationId,
    values: &[Value],
    budget: Budget,
) -> Result<Verdict<Vec<Support>, ()>, EvalError> {
    run(program, state, relation, values, budget, |derivation, cone, meter| {
        let sets = supports(derivation, cone, &|_| true, meter)?;
        sets.into_iter()
            .map(|set| {
                set.into_iter()
                    .map(|leaf| match leaf {
                        Leaf::Stated(index) => Ok(Proof::Stated { fact: derivation.stated(index).expect("stated").clone() }),
                        Leaf::Absent(k) => {
                            let (relation, values) = cone.absent[k].clone();
                            let scope = crate::why::scope(derivation, relation, values.first().and_then(Option::as_ref), meter)?;
                            Ok(Proof::Absent { relation, values, scope })
                        }
                    })
                    .collect()
            })
            .collect()
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Strength {
    AlreadyFails,
    Unbreakable,
    Smallest { sets: Vec<FactSet>, complete: bool },
    Unknown(Reason),
}

impl Strength {
    pub fn size(&self) -> Option<usize> {
        match self {
            Self::AlreadyFails => Some(0),
            Self::Smallest { sets, .. } => sets.iter().map(Vec::len).min(),
            Self::Unbreakable | Self::Unknown(_) => None,
        }
    }
}

pub fn failure_sets(
    program: &Program,
    state: &[Fact],
    relation: RelationId,
    values: &[Value],
    pattern: &[RelationId],
    budget: Budget,
) -> Result<Strength, EvalError> {
    let checked = check(program).map_err(EvalError::Static)?;
    if !monotone(&checked, relation) {
        return verified(program, state, relation, values, pattern, budget);
    }
    let verdict = run(program, state, relation, values, budget, |derivation, cone, meter| {
        let tracked = |fact: &Fact| pattern.contains(&fact.relation);
        let family = supports(derivation, cone, &tracked, meter)?;
        Ok(facts(derivation, hitting_sets(&family, meter)?))
    })?;
    Ok(match verdict {
        Verdict::Fails(()) => Strength::AlreadyFails,
        Verdict::Unknown(reason) => Strength::Unknown(reason),
        Verdict::Holds(sets) if sets.is_empty() => Strength::Unbreakable,
        Verdict::Holds(mut sets) => {
            let size = sets.iter().map(Vec::len).min().expect("nonempty failure sets");
            sets.retain(|set| set.len() == size);
            Strength::Smallest { sets, complete: true }
        }
    })
}

pub fn strength_sentence(
    program: &Program,
    relation: RelationId,
    values: &[Value],
    pattern: &[RelationId],
    strength: &Strength,
) -> String {
    let claim = sentence(program, relation, values);
    let within = pattern
        .iter()
        .filter_map(|id| program.relation(*id))
        .map(|r| format!("`{}`", r.name))
        .collect::<Vec<_>>()
        .join(", ");
    match strength {
        Strength::AlreadyFails => format!("{claim} already fails: its smallest failure set has 0 facts."),
        Strength::Unbreakable => format!("No removal breaks {claim}: it is unbreakable within pattern {within}."),
        Strength::Unknown(reason) => format!("The smallest failure set of {claim} is unknown: {reason}."),
        Strength::Smallest { sets, complete } => {
            let size = strength.size().unwrap_or(0);
            let noun = if size == 1 { "fact" } else { "facts" };
            let first = sets[0]
                .iter()
                .map(|f| sentence(program, f.relation, &f.values))
                .collect::<Vec<_>>()
                .join(" and ");
            let mut sentence = format!("The smallest failure set of {claim} has {size} {noun}, for example removing {first}.");
            if !complete {
                sentence.push_str(" The reported sets are incomplete.");
            }
            sentence
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AchieveMode {
    Possible,
    Acceptable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intervention {
    Already,
    Impossible,
    Found {
        sets: Vec<FactSet>,
        excluded: Vec<(FactSet, Vec<String>)>,
    },
    Unknown(Reason),
}

pub fn prevent(
    program: &Program,
    state: &[Fact],
    claim: &Claim,
    using: &[RelationId],
    budget: Budget,
) -> Result<Intervention, ClaimError> {
    let checked = check(program).map_err(|errors| ClaimError::Eval(EvalError::Static(errors)))?;
    if let Some(reason) = temporal(program, claim, "prevent") {
        return Ok(Intervention::Unknown(reason));
    }
    let ground = ground(claim);
    let verdict = match &ground {
        Some((relation, values)) if monotone(&checked, *relation) => {
            run(program, state, *relation, values, budget, |derivation, cone, meter| {
                let tracked = |fact: &Fact| using.contains(&fact.relation);
                let family = supports(derivation, cone, &tracked, meter)?;
                Ok(facts(derivation, hitting_sets(&family, meter)?))
            })
            .map_err(ClaimError::Eval)?
        }
        _ => {
            let candidates: Vec<Fact> = program
                .facts
                .iter()
                .chain(state)
                .filter(|fact| using.contains(&fact.relation))
                .cloned()
                .collect();
            let mut meter = Meter { limit: budget.work, spent: 0 };
            let found = holds(program, state, claim, &ground, &[], &[], &mut meter).and_then(|already| {
                if !already {
                    return Ok(None);
                }
                minimal(&candidates, &mut meter, |removed, meter| {
                    Ok(!holds(program, state, claim, &ground, &[], removed, meter)?)
                })
                .map(Some)
            });
            match found {
                Err(ClaimError::Eval(EvalError::Unknown(reason))) => Verdict::Unknown(reason),
                Err(error) => return Err(error),
                Ok(None) => Verdict::Fails(()),
                Ok(Some(sets)) => Verdict::Holds(sets),
            }
        }
    };
    Ok(match verdict {
        Verdict::Fails(()) => Intervention::Already,
        Verdict::Unknown(reason) => Intervention::Unknown(reason),
        Verdict::Holds(sets) if sets.is_empty() => Intervention::Impossible,
        Verdict::Holds(sets) => Intervention::Found { sets, excluded: Vec::new() },
    })
}

pub fn achieve(
    program: &Program,
    state: &[Fact],
    claim: &Claim,
    using: &[RelationId],
    mode: AchieveMode,
    budget: Budget,
) -> Result<Intervention, ClaimError> {
    let checked = check(program).map_err(|errors| ClaimError::Eval(EvalError::Static(errors)))?;
    if let Some(reason) = temporal(program, claim, "achieve") {
        return Ok(Intervention::Unknown(reason));
    }
    let ground = ground(claim);
    let mut meter = Meter { limit: budget.work, spent: 0 };
    let candidates = match additions(program, state, using, &mut meter) {
        Ok(candidates) => candidates,
        Err(EvalError::Unknown(reason)) => return Ok(Intervention::Unknown(reason)),
        Err(error) => return Err(ClaimError::Eval(error)),
    };
    let found = (|| -> Result<Option<Vec<FactSet>>, ClaimError> {
        if holds(program, state, claim, &ground, &[], &[], &mut meter)? {
            return Ok(None);
        }
        match &ground {
            Some((relation, values)) if monotone(&checked, *relation) => (|| {
                let augmented: Vec<Fact> = state.iter().chain(&candidates).cloned().collect();
                let left = Budget::work(meter.limit - meter.spent);
                let derivation = derive(program, &augmented, left)?;
                meter.spent += derivation.work;
                if derivation.origin(*relation, values).is_none() {
                    return Ok(Some(Vec::new()));
                }
                let target = cone(&derivation, (*relation, values.clone()), &mut meter)?;
                let tracked = |fact: &Fact| candidates.contains(fact);
                let sets = supports(&derivation, &target, &tracked, &mut meter)?;
                Ok(Some(facts(&derivation, sets)))
            })()
            .map_err(ClaimError::Eval),
            _ => minimal(&candidates, &mut meter, |added, meter| {
                holds(program, state, claim, &ground, added, &[], meter)
            })
            .map(Some),
        }
    })();
    let sets = match found {
        Err(ClaimError::Eval(EvalError::Unknown(reason))) => return Ok(Intervention::Unknown(reason)),
        Err(error) => return Err(error),
        Ok(None) if mode == AchieveMode::Possible => return Ok(Intervention::Already),
        Ok(None) => vec![Vec::new()],
        Ok(Some(sets)) if sets.is_empty() => return Ok(Intervention::Impossible),
        Ok(Some(sets)) => sets,
    };
    if mode == AchieveMode::Possible {
        return Ok(Intervention::Found { sets, excluded: Vec::new() });
    }
    let requirements = |set: &[Fact], meter: &mut Meter| -> Result<Vec<String>, EvalError> {
        meter.charge("acceptable interventions")?;
        let mut changed = program.clone();
        changed.facts.extend(set.iter().cloned());
        changed.initially = state.to_vec();
        let results = match check_claims_spending(&changed, Budget::work(meter.limit - meter.spent)) {
            Ok((results, work)) => {
                meter.spent += work;
                if meter.spent > meter.limit {
                    return Err(Reason::OutOfBudget {
                        resource: Resource::Work,
                        at: "acceptable interventions".into(),
                    }
                    .into());
                }
                results
            }
            Err(ClaimError::Eval(error)) => return Err(error),
            Err(error) => {
                return Err(Reason::Unsupported {
                    construct: error.to_string(),
                    at: "acceptable interventions".into(),
                }
                .into())
            }
        };
        let mut broken = Vec::new();
        for result in results {
            match result.verdict {
                Verdict::Holds(_) => {}
                Verdict::Fails(_) => broken.push(result.requirement),
                Verdict::Unknown(reason) => return Err(reason.into()),
            }
        }
        Ok(broken)
    };
    let mut kept = Vec::new();
    let mut excluded = Vec::new();
    for set in sets {
        let broken = match requirements(&set, &mut meter) {
            Err(EvalError::Unknown(reason)) => return Ok(Intervention::Unknown(reason)),
            Err(error) => return Err(ClaimError::Eval(error)),
            Ok(broken) => broken,
        };
        if broken.is_empty() {
            if set.is_empty() {
                return Ok(Intervention::Already);
            }
            kept.push(set);
        } else {
            excluded.push((set, broken));
        }
    }
    let forbidden = ground.as_ref().is_some_and(|(relation, values)| {
        let target = Fact::new(*relation, values.iter().cloned()).atom();
        program.events.is_empty()
            && program.handlers.is_empty()
            && program.requirements.iter().any(|requirement| {
                matches!(&requirement.claim, Claim::Never(StateClaim::Holds(atom)) if atom == &target)
            })
    });
    if excluded.is_empty() || forbidden {
        return Ok(Intervention::Found { sets: kept, excluded });
    }
    let sets = match minimal(&candidates, &mut meter, |added, meter| {
        if !holds(program, state, claim, &ground, added, &[], meter)? {
            return Ok(false);
        }
        Ok(requirements(added, meter).map_err(ClaimError::Eval)?.is_empty())
    }) {
        Err(ClaimError::Eval(EvalError::Unknown(reason))) => return Ok(Intervention::Unknown(reason)),
        Err(error) => return Err(error),
        Ok(sets) => sets,
    };
    Ok(Intervention::Found { sets, excluded })
}

fn ground(claim: &Claim) -> Option<(RelationId, Vec<Value>)> {
    let Claim::State(StateClaim::Holds(atom)) = claim else {
        return None;
    };
    let values = atom
        .arguments
        .iter()
        .map(|argument| match argument {
            Expr::Value(value) => Some(value.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some((atom.relation, values))
}

fn temporal(program: &Program, claim: &Claim, at: &str) -> Option<Reason> {
    if program.events.is_empty() && program.handlers.is_empty() {
        return None;
    }
    let over_time = |claim: &Claim| {
        matches!(
            claim,
            Claim::Always(_) | Claim::Eventually(_) | Claim::Never(_) | Claim::EveryTracePasses { .. } | Claim::EveryEventWhen { .. }
        )
    };
    let temporal = match claim {
        Claim::NoSinglePointOfFailure { claim, .. } => over_time(claim),
        claim => over_time(claim),
    };
    temporal.then(|| Reason::Unsupported {
        construct: "a temporal claim over a program with events".into(),
        at: at.into(),
    })
}

fn holds(
    program: &Program,
    state: &[Fact],
    claim: &Claim,
    ground: &Option<(RelationId, Vec<Value>)>,
    added: &[Fact],
    removed: &[Fact],
    meter: &mut Meter,
) -> Result<bool, ClaimError> {
    if let Some((relation, values)) = ground {
        return recompute(program, state, added, removed, *relation, values, meter).map_err(ClaimError::Eval);
    }
    const AT: &str = "verified intervention search";
    meter.charge(AT).map_err(ClaimError::Eval)?;
    let left = Budget::work(meter.limit - meter.spent);
    let (verdict, work) = claim_on_changed(program, state, added, removed, claim, left)?;
    meter.spent += work;
    if meter.spent > meter.limit {
        return Err(ClaimError::Eval(Reason::OutOfBudget { resource: Resource::Work, at: AT.into() }.into()));
    }
    match verdict {
        Verdict::Holds(_) => Ok(true),
        Verdict::Fails(_) => Ok(false),
        Verdict::Unknown(reason) => Err(ClaimError::Eval(reason.into())),
    }
}

fn additions(
    program: &Program,
    state: &[Fact],
    using: &[RelationId],
    meter: &mut Meter,
) -> Result<Vec<Fact>, EvalError> {
    let mut out = Vec::new();
    for &id in using {
        let relation = program.relation(id).ok_or_else(|| Reason::Unsupported {
            construct: format!("relation #{}", id.0),
            at: "achieve".into(),
        })?;
        let mut rows: Vec<Vec<Value>> = vec![Vec::new()];
        for slot in &relation.slots {
            let Type::Entity(ty) = &slot.ty else {
                return Err(Reason::Unsupported {
                    construct: format!("adding `{}` facts over a non-entity slot", relation.name),
                    at: "achieve".into(),
                }
                .into());
            };
            let pool: Vec<&Entity> = program.entities.iter().filter(|entity| &entity.entity_type == ty).collect();
            let mut next = Vec::new();
            for row in &rows {
                for entity in &pool {
                    for _ in 0..=row.len() {
                        meter.charge("intervention candidates")?;
                    }
                    let mut grown = row.clone();
                    grown.push(Value::Entity((*entity).clone()));
                    next.push(grown);
                }
            }
            rows = next;
        }
        for values in rows {
            let fact = Fact::new(id, values);
            if !program.facts.iter().chain(state).any(|f| f.relation == fact.relation && f.values == fact.values) {
                out.push(fact);
            }
        }
    }
    Ok(out)
}

fn recompute(
    program: &Program,
    state: &[Fact],
    added: &[Fact],
    removed: &[Fact],
    relation: RelationId,
    values: &[Value],
    meter: &mut Meter,
) -> Result<bool, EvalError> {
    meter.charge("verified intervention search")?;
    let augmented: Vec<Fact> = state.iter().chain(added).cloned().collect();
    let model = evaluate_blocking(program, &augmented, Budget::work(meter.limit - meter.spent), &|r, v| {
        removed.iter().any(|f| f.relation == r && f.values == v)
    })?;
    meter.spent += model.work;
    Ok(model.contains(relation, values))
}

fn minimal<E>(
    candidates: &[Fact],
    meter: &mut Meter,
    mut works: impl FnMut(&[Fact], &mut Meter) -> Result<bool, E>,
) -> Result<Vec<FactSet>, E> {
    let mut found: Vec<Vec<usize>> = Vec::new();
    for size in 1..=candidates.len() {
        let mut chosen: Vec<usize> = (0..size).collect();
        loop {
            if !found.iter().any(|set| subset(set, &chosen)) {
                let set: Vec<Fact> = chosen.iter().map(|&i| candidates[i].clone()).collect();
                if works(&set, meter)? {
                    found.push(chosen.clone());
                }
            }
            let Some(slot) = (0..size).rev().find(|&k| chosen[k] < candidates.len() - size + k) else {
                break;
            };
            chosen[slot] += 1;
            for k in slot + 1..size {
                chosen[k] = chosen[k - 1] + 1;
            }
        }
    }
    Ok(found
        .into_iter()
        .map(|set| set.into_iter().map(|i| candidates[i].clone()).collect())
        .collect())
}

fn monotone(checked: &CheckedProgram, relation: RelationId) -> bool {
    let mut reached = HashSet::from([relation]);
    let mut queue = vec![relation];
    while let Some(head) = queue.pop() {
        for edge in checked.dependencies.iter().filter(|edge| edge.head == head) {
            if edge.kind != DependencyKind::Positive {
                return false;
            }
            if reached.insert(edge.body) {
                queue.push(edge.body);
            }
        }
    }
    true
}

fn verified(
    program: &Program,
    state: &[Fact],
    relation: RelationId,
    values: &[Value],
    pattern: &[RelationId],
    budget: Budget,
) -> Result<Strength, EvalError> {
    let mut meter = Meter {
        limit: budget.work,
        spent: 0,
    };
    match search(program, state, relation, values, pattern, &mut meter) {
        Err(EvalError::Unknown(reason)) => Ok(Strength::Unknown(reason)),
        other => other,
    }
}

fn search(
    program: &Program,
    state: &[Fact],
    relation: RelationId,
    values: &[Value],
    pattern: &[RelationId],
    meter: &mut Meter,
) -> Result<Strength, EvalError> {
    let mut holds = |removed: &[&Fact]| -> Result<bool, EvalError> {
        meter.charge("verified failure-set search")?;
        let left = Budget::work(meter.limit - meter.spent);
        let model = evaluate_blocking(program, state, left, &|r, v| {
            removed.iter().any(|f| f.relation == r && f.values == v)
        })?;
        meter.spent += model.work;
        Ok(model.contains(relation, values))
    };
    if !holds(&[])? {
        return Ok(Strength::AlreadyFails);
    }
    let candidates: Vec<&Fact> = program
        .facts
        .iter()
        .chain(state)
        .filter(|fact| pattern.contains(&fact.relation))
        .collect();
    for size in 1..=candidates.len() {
        let mut found = Vec::new();
        let mut chosen: Vec<usize> = (0..size).collect();
        loop {
            let removed: Vec<&Fact> = chosen.iter().map(|&i| candidates[i]).collect();
            match holds(&removed) {
                Ok(false) => found.push(removed.into_iter().cloned().collect()),
                Ok(true) => {}
                Err(EvalError::Unknown(Reason::OutOfBudget { .. })) if !found.is_empty() => {
                    return Ok(Strength::Smallest { sets: found, complete: false });
                }
                Err(error) => return Err(error),
            }
            let Some(slot) = (0..size).rev().find(|&k| chosen[k] < candidates.len() - size + k) else {
                break;
            };
            chosen[slot] += 1;
            for k in slot + 1..size {
                chosen[k] = chosen[k - 1] + 1;
            }
        }
        if !found.is_empty() {
            return Ok(Strength::Smallest { sets: found, complete: true });
        }
    }
    Ok(Strength::Unbreakable)
}

fn sentence(program: &Program, relation: RelationId, values: &[Value]) -> String {
    crate::why::sentence(program, relation, values)
}

fn run<'p, T>(
    program: &'p Program,
    state: &'p [Fact],
    relation: RelationId,
    values: &[Value],
    budget: Budget,
    answer: impl FnOnce(&Derivation<'p>, &Cone, &mut Meter) -> Result<Vec<T>, EvalError>,
) -> Result<Verdict<Vec<T>, ()>, EvalError> {
    let derivation = match derive(program, state, budget) {
        Err(EvalError::Unknown(reason)) => return Ok(Verdict::Unknown(reason)),
        other => other?,
    };
    let mut meter = Meter {
        limit: budget.work,
        spent: derivation.work,
    };
    if derivation.origin(relation, values).is_none() {
        return Ok(Verdict::Fails(()));
    }
    let result = cone(&derivation, (relation, values.to_vec()), &mut meter)
        .and_then(|cone| answer(&derivation, &cone, &mut meter));
    match result {
        Err(EvalError::Unknown(reason)) => Ok(Verdict::Unknown(reason)),
        Err(error) => Err(error),
        Ok(sets) => Ok(Verdict::Holds(sets)),
    }
}

fn facts(derivation: &Derivation, sets: Vec<Set>) -> Vec<FactSet> {
    sets.into_iter()
        .map(|set| {
            set.into_iter()
                .filter_map(|leaf| match leaf {
                    Leaf::Stated(index) => Some(derivation.stated(index).expect("stated").clone()),
                    Leaf::Absent(_) => None,
                })
                .collect()
        })
        .collect()
}

struct Cone {
    target: Key,
    keys: Vec<Key>,
    firings: HashMap<Key, Vec<Vec<Item>>>,
    absent: Vec<Absence>,
}

fn cone(derivation: &Derivation, target: Key, meter: &mut Meter) -> Result<Cone, EvalError> {
    let mut firings: HashMap<Key, Vec<Vec<Item>>> = HashMap::new();
    let mut absent: Vec<Absence> = Vec::new();
    let mut keys = Vec::new();
    let mut queue = vec![target.clone()];
    let mut seen: HashSet<Key> = HashSet::from([target.clone()]);
    while let Some(key) = queue.pop() {
        meter.charge(AT)?;
        let mut bodies = Vec::new();
        for firing in derivation.firings(key.0, &key.1, None, meter)? {
            let rule = &derivation.program().rules[firing.rule];
            if rule.body.iter().any(unrecorded) {
                return Err(Reason::Unsupported {
                    construct: "a support through an aggregate or query".into(),
                    at: rule.name.clone(),
                }
                .into());
            }
            let mut body = Vec::new();
            for premise in firing.premises {
                match premise {
                    Premise::Present(relation, row) => {
                        let premise = (relation, row);
                        if seen.insert(premise.clone()) {
                            queue.push(premise.clone());
                        }
                        body.push(Item::Present(premise));
                    }
                    Premise::Absent(relation, row) => {
                        let absence = (relation, row);
                        let k = absent.iter().position(|a| *a == absence).unwrap_or_else(|| {
                            absent.push(absence);
                            absent.len() - 1
                        });
                        body.push(Item::Absent(k));
                    }
                    _ => {}
                }
            }
            bodies.push(body);
        }
        firings.insert(key.clone(), bodies);
        keys.push(key);
    }
    Ok(Cone {
        target,
        keys,
        firings,
        absent,
    })
}

fn unrecorded(literal: &Literal) -> bool {
    match literal {
        Literal::Aggregate { .. } | Literal::ArgMin { .. } => true,
        Literal::Positive(atom) | Literal::Negative(atom) => atom.arguments.iter().any(queries),
        Literal::Compare { left, right, .. } => queries(left) || queries(right),
        Literal::Boolean(value) | Literal::Bind { value, .. } | Literal::Member { list: value, .. } => queries(value),
    }
}

fn queries(expression: &Expr) -> bool {
    match expression {
        Expr::Query(_) => true,
        Expr::Value(_) | Expr::Variable(_) => false,
        Expr::List(items) | Expr::Call { arguments: items, .. } => items.iter().any(queries),
        Expr::Record(fields) | Expr::Attrs(fields) => fields.values().any(queries),
        Expr::Field { value, .. } | Expr::Unary { value, .. } => queries(value),
        Expr::Binary { left, right, .. } => queries(left) || queries(right),
        Expr::If { condition, then_value, else_value } => queries(condition) || queries(then_value) || queries(else_value),
        Expr::Lambda { body, .. } => queries(body),
        Expr::Apply { function, arguments } => queries(function) || arguments.iter().any(queries),
    }
}

fn supports(
    derivation: &Derivation,
    cone: &Cone,
    tracked: &dyn Fn(&Fact) -> bool,
    meter: &mut Meter,
) -> Result<Vec<Set>, EvalError> {
    let mut found: HashMap<&Key, Vec<Set>> = HashMap::new();
    for key in &cone.keys {
        if let Some(Origin::Stated(index)) = derivation.origin(key.0, &key.1) {
            let fact = derivation.stated(index).expect("stated");
            let set = if tracked(fact) { vec![Leaf::Stated(index)] } else { Vec::new() };
            found.insert(key, vec![set]);
        }
    }
    loop {
        let mut changed = false;
        for key in &cone.keys {
            for body in &cone.firings[key] {
                let mut combined: Vec<Set> = vec![Vec::new()];
                for item in body {
                    let single;
                    let pool = match item {
                        Item::Absent(k) => {
                            single = vec![vec![Leaf::Absent(*k)]];
                            &single
                        }
                        Item::Present(premise) => match found.get(premise) {
                            Some(pool) => pool,
                            None => {
                                combined.clear();
                                break;
                            }
                        },
                    };
                    let mut next = Vec::new();
                    for partial in &combined {
                        for set in pool {
                            meter.charge(AT)?;
                            insert_minimal(&mut next, union(partial, set), meter)?;
                        }
                    }
                    combined = next;
                }
                let list = found.entry(key).or_default();
                for set in combined {
                    changed |= insert_minimal(list, set, meter)?;
                }
            }
        }
        if !changed {
            break;
        }
    }
    Ok(sorted(found.remove(&cone.target).unwrap_or_default()))
}

fn hitting_sets(family: &[Set], meter: &mut Meter) -> Result<Vec<Set>, EvalError> {
    let mut family: Vec<&Set> = family.iter().collect();
    family.sort_by_key(|set| set.len());
    let mut hitting: Vec<Set> = vec![Vec::new()];
    for set in family {
        let mut next = Vec::new();
        for partial in &hitting {
            meter.charge("minimal hitting sets")?;
            if partial.iter().any(|item| set.binary_search(item).is_ok()) {
                insert_minimal(&mut next, partial.clone(), meter)?;
                continue;
            }
            for item in set {
                meter.charge("minimal hitting sets")?;
                insert_minimal(&mut next, union(partial, std::slice::from_ref(item)), meter)?;
            }
        }
        hitting = next;
    }
    Ok(sorted(hitting))
}

fn union(left: &[Leaf], right: &[Leaf]) -> Set {
    let mut set: Set = left.iter().chain(right).cloned().collect();
    set.sort_unstable();
    set.dedup();
    set
}

fn sorted(mut sets: Vec<Set>) -> Vec<Set> {
    sets.sort();
    sets.sort_by_key(Vec::len);
    sets
}

fn insert_minimal(list: &mut Vec<Set>, set: Set, meter: &mut Meter) -> Result<bool, EvalError> {
    for existing in list.iter() {
        meter.charge(AT)?;
        if subset(existing, &set) {
            return Ok(false);
        }
    }
    list.retain(|existing| !subset(&set, existing));
    list.push(set);
    Ok(true)
}

fn subset<T: Ord>(small: &[T], large: &[T]) -> bool {
    small.len() <= large.len() && small.iter().all(|item| large.binary_search(item).is_ok())
}
