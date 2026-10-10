use crate::check::render_value;
use crate::why::{render, sentence, why};
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeFailure {
    pub relation: RelationId,
    pub key: Vec<Value>,
    pub tuples: Vec<Vec<Value>>,
}

pub fn check_stratum(
    program: &Program,
    strata: &[usize],
    stratum: usize,
    rows: &dyn Fn(RelationId, &mut dyn FnMut(&[Value])),
) -> Vec<ModeFailure> {
    let mut failures = Vec::new();
    for (index, relation) in program.relations.iter().enumerate() {
        if relation.kind != RelationKind::Derived || strata.get(index) != Some(&stratum) {
            continue;
        }
        let (keys, values, total) = match &relation.mode {
            Mode::Many => continue,
            Mode::ExactlyOne { key, values } => (vec![*key], values, true),
            Mode::AtMostOne { keys, values } => (keys.clone(), values, false),
        };
        let id = RelationId(index);
        let mut groups: BTreeMap<Vec<Value>, BTreeSet<Vec<Value>>> = BTreeMap::new();
        rows(id, &mut |tuple| {
            let key = keys.iter().map(|&slot| tuple[slot].clone()).collect();
            groups.entry(key).or_default().insert(tuple.to_vec());
        });
        let mut found: BTreeMap<Vec<Value>, Vec<Vec<Value>>> = BTreeMap::new();
        if total {
            if let Type::Entity(name) = &relation.slots[keys[0]].ty {
                for entity in program.entities.iter().filter(|e| &e.entity_type == name) {
                    let key = vec![Value::Entity(entity.clone())];
                    if !groups.contains_key(&key) {
                        found.insert(key, Vec::new());
                    }
                }
            }
        }
        let project = |tuple: &Vec<Value>| -> Vec<Value> {
            values.iter().map(|&slot| tuple[slot].clone()).collect()
        };
        for (key, tuples) in groups {
            let first = tuples.iter().next().expect("a group holds a tuple").clone();
            if let Some(second) = tuples.iter().find(|t| project(t) != project(&first)) {
                found.insert(key, vec![first, second.clone()]);
            }
        }
        failures.extend(found.into_iter().map(|(key, tuples)| ModeFailure {
            relation: id,
            key,
            tuples,
        }));
    }
    failures
}

pub fn describe(program: &Program, failure: &ModeFailure) -> String {
    let relation = &program.relations[failure.relation.0];
    let key: Vec<String> = failure.key.iter().map(render_value).collect();
    match failure.tuples.as_slice() {
        [first, second, ..] => format!(
            "`{}` has two values for {}: {} and {}",
            relation.template,
            key.join(", "),
            sentence(program, failure.relation, first),
            sentence(program, failure.relation, second)
        ),
        _ => format!(
            "`{}` has no value for {}, which its `exactly one` mode requires",
            relation.template,
            key.join(", ")
        ),
    }
}

pub fn report(program: &Program, state: &[Fact], failure: &ModeFailure, budget: Budget) -> String {
    let mut out = vec![format!("Fails: {}.", describe(program, failure))];
    for tuple in &failure.tuples {
        let verdict = match why(program, state, failure.relation, tuple, budget) {
            Ok(verdict) => verdict,
            Err(error) => {
                out.push(format!("{}: no proof: {error:?}", sentence(program, failure.relation, tuple)));
                continue;
            }
        };
        out.push(render(program, failure.relation, tuple, &verdict, budget));
    }
    out.join("\n")
}
