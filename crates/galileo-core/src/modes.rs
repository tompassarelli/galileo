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

pub fn check_stratum<'a>(
    program: &Program,
    strata: &[usize],
    stratum: usize,
    kind: RelationKind,
    entities: impl Iterator<Item = &'a Entity> + Clone,
    rows: &dyn Fn(RelationId, &mut dyn FnMut(&[Value])),
) -> Vec<ModeFailure> {
    let mut failures = Vec::new();
    for (index, relation) in program.relations.iter().enumerate() {
        if relation.kind != kind || strata.get(index) != Some(&stratum) {
            continue;
        }
        for mode in &relation.modes {
            let (keys, values, total) = match mode {
                Mode::ExactlyOne { key, values } => (vec![*key], values, true),
                Mode::AtMostOne { keys, values } => (keys.clone(), values, false),
            };
            let id = RelationId(index);
            let mut groups: BTreeMap<Vec<Value>, BTreeSet<Vec<Value>>> = BTreeMap::new();
            rows(id, &mut |tuple| {
                let key = keys.iter().map(|&slot| tuple[slot].clone()).collect();
                groups.entry(key).or_default().insert(tuple.to_vec());
            });
            if total {
                if let Type::Entity(name) = &relation.slots[keys[0]].ty {
                    for entity in entities.clone().filter(|e| &e.entity_type == name) {
                        groups.entry(vec![Value::Entity(entity.clone())]).or_default();
                    }
                }
            }
            for (key, tuples) in groups {
                if let Some(failure) = check_key(id, key, values, total, tuples.iter()) {
                    failures.push(failure);
                }
            }
        }
    }
    failures
}

pub(crate) fn check_key<'a>(
    relation: RelationId,
    key: Vec<Value>,
    values: &[usize],
    required: bool,
    tuples: impl Iterator<Item = &'a Vec<Value>>,
) -> Option<ModeFailure> {
    let mut tuples = tuples;
    let Some(first) = tuples.next() else {
        return required.then_some(ModeFailure {
            relation,
            key,
            tuples: Vec::new(),
        });
    };
    let second = tuples.find(|tuple| values.iter().any(|&slot| tuple[slot] != first[slot]))?;
    Some(ModeFailure {
        relation,
        key,
        tuples: vec![first.clone(), second.clone()],
    })
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
                out.push(format!(
                    "{}: no proof: {error:?}",
                    sentence(program, failure.relation, tuple)
                ));
                continue;
            }
        };
        out.push(render(program, failure.relation, tuple, &verdict, budget));
    }
    out.join("\n")
}
