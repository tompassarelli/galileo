use crate::engine::{derive, Derivation, Meter, Origin, Premise};
use crate::*;
use crate::check::DependencyKind;
use crate::claims::check_claims_spending;
use std::collections::{HashMap, HashSet};

pub type Support = Vec<Fact>;

type Key = (RelationId, Vec<Value>);
type Set = Vec<usize>;

const AT: &str = "minimal supports";

pub fn minimal_supports(
    program: &Program,
    state: &[Fact],
    relation: RelationId,
    values: &[Value],
    budget: Budget,
) -> Result<Verdict<Vec<Support>, ()>, EvalError> {
    run(program, state, relation, values, budget, |derivation, cone, meter| {
        Ok(facts(derivation, supports(derivation, cone, &|_| true, meter)?))
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Strength {
    AlreadyFails,
    Unbreakable,
    Smallest(Vec<Support>),
    Unknown(Reason),
}

impl Strength {
    pub fn size(&self) -> Option<usize> {
        match self {
            Self::AlreadyFails => Some(0),
            Self::Smallest(sets) => sets.iter().map(Vec::len).min(),
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
    let verdict = if monotone(&checked, relation) {
        run(program, state, relation, values, budget, |derivation, cone, meter| {
            let tracked = |fact: &Fact| pattern.contains(&fact.relation);
            let family = supports(derivation, cone, &tracked, meter)?;
            Ok(facts(derivation, hitting_sets(&family, meter)?))
        })?
    } else {
        verified(program, state, relation, values, pattern, budget)?
    };
    Ok(match verdict {
        Verdict::Fails(()) => Strength::AlreadyFails,
        Verdict::Unknown(reason) => Strength::Unknown(reason),
        Verdict::Holds(sets) if sets.is_empty() => Strength::Unbreakable,
        Verdict::Holds(sets) => Strength::Smallest(sets),
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
        Strength::Smallest(sets) => {
            let size = strength.size().unwrap_or(0);
            let noun = if size == 1 { "fact" } else { "facts" };
            let first = sets[0]
                .iter()
                .map(|f| sentence(program, f.relation, &f.values))
                .collect::<Vec<_>>()
                .join(" and ");
            format!("The smallest failure set of {claim} has {size} {noun}, for example removing {first}.")
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
        sets: Vec<Support>,
        excluded: Vec<(Support, Vec<String>)>,
    },
    Unknown(Reason),
}

pub fn prevent(
    program: &Program,
    state: &[Fact],
    relation: RelationId,
    values: &[Value],
    using: &[RelationId],
    budget: Budget,
) -> Result<Intervention, EvalError> {
    let checked = check(program).map_err(EvalError::Static)?;
    let verdict = if monotone(&checked, relation) {
        run(program, state, relation, values, budget, |derivation, cone, meter| {
            let tracked = |fact: &Fact| using.contains(&fact.relation);
            let family = supports(derivation, cone, &tracked, meter)?;
            Ok(facts(derivation, hitting_sets(&family, meter)?))
        })?
    } else {
        let candidates: Vec<Fact> = program
            .facts
            .iter()
            .chain(state)
            .filter(|fact| using.contains(&fact.relation))
            .cloned()
            .collect();
        let mut meter = Meter { limit: budget.work, spent: 0 };
        let holds = |removed: &[Fact], meter: &mut Meter| {
            recompute(program, state, &[], removed, relation, values, meter)
        };
        match holds(&[], &mut meter) {
            Err(EvalError::Unknown(reason)) => Verdict::Unknown(reason),
            Err(error) => return Err(error),
            Ok(false) => Verdict::Fails(()),
            Ok(true) => match minimal(&candidates, &mut meter, |set, meter| Ok(!holds(set, meter)?)) {
                Err(EvalError::Unknown(reason)) => Verdict::Unknown(reason),
                other => Verdict::Holds(other?),
            },
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
    relation: RelationId,
    values: &[Value],
    using: &[RelationId],
    mode: AchieveMode,
    budget: Budget,
) -> Result<Intervention, EvalError> {
    let checked = check(program).map_err(EvalError::Static)?;
    let mut meter = Meter { limit: budget.work, spent: 0 };
    let candidates = match additions(program, state, using, &mut meter) {
        Ok(candidates) => candidates,
        Err(EvalError::Unknown(reason)) => return Ok(Intervention::Unknown(reason)),
        Err(error) => return Err(error),
    };
    let found = (|| {
        if recompute(program, state, &[], &[], relation, values, &mut meter)? {
            return Ok(None);
        }
        if monotone(&checked, relation) {
            let augmented: Vec<Fact> = state.iter().chain(&candidates).cloned().collect();
            let left = Budget::work(meter.limit - meter.spent);
            let derivation = derive(program, &augmented, left)?;
            meter.spent += derivation.work;
            if derivation.origin(relation, values).is_none() {
                return Ok(Some(Vec::new()));
            }
            let target = cone(&derivation, (relation, values.to_vec()), &mut meter)?;
            let tracked = |fact: &Fact| candidates.contains(fact);
            let sets = supports(&derivation, &target, &tracked, &mut meter)?;
            Ok(Some(facts(&derivation, sets)))
        } else {
            minimal(&candidates, &mut meter, |added, meter| {
                recompute(program, state, added, &[], relation, values, meter)
            })
            .map(Some)
        }
    })();
    let sets = match found {
        Err(EvalError::Unknown(reason)) => return Ok(Intervention::Unknown(reason)),
        Err(error) => return Err(error),
        Ok(None) => return Ok(Intervention::Already),
        Ok(Some(sets)) if sets.is_empty() => return Ok(Intervention::Impossible),
        Ok(Some(sets)) => sets,
    };
    if mode == AchieveMode::Possible {
        return Ok(Intervention::Found { sets, excluded: Vec::new() });
    }
    let mut kept = Vec::new();
    let mut excluded = Vec::new();
    for set in sets {
        if let Err(EvalError::Unknown(reason)) = meter.charge("acceptable interventions") {
            return Ok(Intervention::Unknown(reason));
        }
        let mut changed = program.clone();
        changed.facts.extend(set.iter().cloned());
        changed.initially = state.to_vec();
        let results = match check_claims_spending(&changed, Budget::work(meter.limit - meter.spent)) {
            Ok((results, work)) => {
                meter.spent += work;
                if meter.spent > meter.limit {
                    return Ok(Intervention::Unknown(Reason::OutOfBudget {
                        resource: Resource::Work,
                        at: "acceptable interventions".into(),
                    }));
                }
                results
            }
            Err(ClaimError::Eval(EvalError::Unknown(reason))) => return Ok(Intervention::Unknown(reason)),
            Err(ClaimError::Eval(error)) => return Err(error),
            Err(error) => {
                return Ok(Intervention::Unknown(Reason::Unsupported {
                    construct: error.to_string(),
                    at: "acceptable interventions".into(),
                }))
            }
        };
        let mut broken = Vec::new();
        for result in results {
            match result.verdict {
                Verdict::Holds(_) => {}
                Verdict::Fails(_) => broken.push(result.requirement),
                Verdict::Unknown(reason) => return Ok(Intervention::Unknown(reason)),
            }
        }
        if broken.is_empty() {
            kept.push(set);
        } else {
            excluded.push((set, broken));
        }
    }
    Ok(Intervention::Found { sets: kept, excluded })
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
                    meter.charge("intervention candidates")?;
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

fn minimal(
    candidates: &[Fact],
    meter: &mut Meter,
    mut works: impl FnMut(&[Fact], &mut Meter) -> Result<bool, EvalError>,
) -> Result<Vec<Support>, EvalError> {
    let mut found: Vec<Set> = Vec::new();
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
) -> Result<Verdict<Vec<Support>, ()>, EvalError> {
    let mut meter = Meter {
        limit: budget.work,
        spent: 0,
    };
    match search(program, state, relation, values, pattern, &mut meter) {
        Err(EvalError::Unknown(reason)) => Ok(Verdict::Unknown(reason)),
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
) -> Result<Verdict<Vec<Support>, ()>, EvalError> {
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
        return Ok(Verdict::Fails(()));
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
            if !holds(&removed)? {
                found.push(removed.into_iter().cloned().collect());
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
            return Ok(Verdict::Holds(found));
        }
    }
    Ok(Verdict::Holds(Vec::new()))
}

fn sentence(program: &Program, relation: RelationId, values: &[Value]) -> String {
    crate::why::sentence(program, relation, values)
}

fn run<'p>(
    program: &'p Program,
    state: &'p [Fact],
    relation: RelationId,
    values: &[Value],
    budget: Budget,
    answer: impl FnOnce(&Derivation<'p>, &Cone, &mut Meter) -> Result<Vec<Support>, EvalError>,
) -> Result<Verdict<Vec<Support>, ()>, EvalError> {
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

fn facts(derivation: &Derivation, sets: Vec<Set>) -> Vec<Support> {
    sets.into_iter()
        .map(|set| {
            set.into_iter()
                .map(|index| derivation.stated(index).expect("stated").clone())
                .collect()
        })
        .collect()
}

struct Cone {
    target: Key,
    keys: Vec<Key>,
    firings: HashMap<Key, Vec<Vec<Key>>>,
}

fn cone(derivation: &Derivation, target: Key, meter: &mut Meter) -> Result<Cone, EvalError> {
    let mut firings: HashMap<Key, Vec<Vec<Key>>> = HashMap::new();
    let mut keys = Vec::new();
    let mut queue = vec![target.clone()];
    let mut seen: HashSet<Key> = HashSet::from([target.clone()]);
    while let Some(key) = queue.pop() {
        meter.charge(AT)?;
        let mut bodies = Vec::new();
        for firing in derivation.firings(key.0, &key.1, None, meter)? {
            let mut body = Vec::new();
            for premise in firing.premises {
                match premise {
                    Premise::Present(relation, row) => body.push((relation, row)),
                    _ => {}
                }
            }
            for premise in &body {
                if seen.insert(premise.clone()) {
                    queue.push(premise.clone());
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
    })
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
            let set = if tracked(fact) { vec![index] } else { Vec::new() };
            found.insert(key, vec![set]);
        }
    }
    loop {
        let mut changed = false;
        for key in &cone.keys {
            for body in &cone.firings[key] {
                let mut combined: Vec<Set> = vec![Vec::new()];
                for premise in body {
                    let Some(pool) = found.get(premise) else {
                        combined.clear();
                        break;
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
            for &item in set {
                meter.charge("minimal hitting sets")?;
                insert_minimal(&mut next, union(partial, &[item]), meter)?;
            }
        }
        hitting = next;
    }
    Ok(sorted(hitting))
}

fn union(left: &[usize], right: &[usize]) -> Set {
    let mut set: Set = left.iter().chain(right).copied().collect();
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

fn subset(small: &[usize], large: &[usize]) -> bool {
    small.len() <= large.len() && small.iter().all(|item| large.binary_search(item).is_ok())
}
