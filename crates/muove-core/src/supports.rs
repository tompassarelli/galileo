use crate::engine::{derive, Derivation, Meter, Origin, Premise};
use crate::*;
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

pub fn failure_sets(
    program: &Program,
    state: &[Fact],
    relation: RelationId,
    values: &[Value],
    pattern: &[RelationId],
    budget: Budget,
) -> Result<Verdict<Vec<Support>, ()>, EvalError> {
    run(program, state, relation, values, budget, |derivation, cone, meter| {
        if cone.negated {
            return Err(Reason::Unsupported {
                construct: "a failure set under negation".into(),
                at: sentence(program, relation, values),
            }
            .into());
        }
        let tracked = |fact: &Fact| pattern.contains(&fact.relation);
        let family = supports(derivation, cone, &tracked, meter)?;
        Ok(facts(derivation, hitting_sets(&family, meter)?))
    })
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
    negated: bool,
}

fn cone(derivation: &Derivation, target: Key, meter: &mut Meter) -> Result<Cone, EvalError> {
    let mut firings: HashMap<Key, Vec<Vec<Key>>> = HashMap::new();
    let mut keys = Vec::new();
    let mut negated = false;
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
                    Premise::Absent(..) => negated = true,
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
        negated,
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
