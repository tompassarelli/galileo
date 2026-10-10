use crate::engine::{derive, Derivation, Meter, Origin, Premise};
use crate::*;
use crate::check::DependencyKind;
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
