use galileo_core::engine::evaluate;
use galileo_core::why::sentence;
use galileo_core::{Budget, EvalError, Program, RelationId, RelationKind, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TruthChange {
    Gained(String),
    Lost(String),
    NowStated(String),
    NowDerived(String),
}

impl fmt::Display for TruthChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (label, sentence) = match self {
            Self::Gained(sentence) => ("gained", sentence),
            Self::Lost(sentence) => ("lost", sentence),
            Self::NowStated(sentence) => ("now stated", sentence),
            Self::NowDerived(sentence) => ("now derived", sentence),
        };
        write!(f, "{sentence}: {label}")
    }
}

pub fn truth(
    before: &Program,
    after: &Program,
    budget: Budget,
) -> Result<Vec<TruthChange>, EvalError> {
    let before = facts(before, budget)?;
    let after = facts(after, budget)?;
    let mut changes = Vec::new();
    for (key, (sentence, stated)) in &before {
        match after.get(key) {
            None => changes.push(TruthChange::Lost(sentence.clone())),
            Some((sentence, true)) if !*stated => {
                changes.push(TruthChange::NowStated(sentence.clone()));
            }
            Some((sentence, false)) if *stated => {
                changes.push(TruthChange::NowDerived(sentence.clone()));
            }
            _ => {}
        }
    }
    for (key, (sentence, _)) in &after {
        if !before.contains_key(key) {
            changes.push(TruthChange::Gained(sentence.clone()));
        }
    }
    Ok(changes)
}

type Facts = BTreeMap<(String, Vec<Value>), (String, bool)>;

fn facts(program: &Program, budget: Budget) -> Result<Facts, EvalError> {
    let model = evaluate(program, &[], budget)?;
    let stated: BTreeSet<_> = program
        .facts
        .iter()
        .map(|fact| (fact.relation, &fact.values))
        .collect();
    let mut facts = BTreeMap::new();
    for (index, relation) in program.relations.iter().enumerate() {
        if relation.kind != RelationKind::Derived {
            continue;
        }
        let id = RelationId(index);
        for values in model.tuples(id) {
            facts.insert(
                (relation.name.clone(), values.clone()),
                (sentence(program, id, values), stated.contains(&(id, values))),
            );
        }
    }
    Ok(facts)
}
