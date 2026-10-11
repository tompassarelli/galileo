use super::{execute, Delta, Mark, Store};
use crate::expr::Meter;
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn maintain(
    program: &Program,
    rules: &[(usize, &Rule)],
    schedules: &[Vec<usize>],
    heads: &BTreeSet<RelationId>,
    stores: &mut BTreeMap<RelationId, Store>,
    deltas: &BTreeMap<RelationId, Delta>,
    previous: Option<&Mark>,
    meter: &mut Meter,
) -> Result<(), EvalError> {
    let Some(previous) = previous else { return Ok(()); };
    let mut before = previous.relations.clone();
    let mut removed: BTreeMap<_, _> = deltas.iter().map(|(relation, delta)| (*relation, Delta {
        inserted: delta.deleted.clone(), deleted: delta.inserted.clone(),
    })).collect();
    loop {
        let mut rows = BTreeSet::new();
        for (index, rule) in rules {
            for row in execute::pass(program, rule, &schedules[*index], &mut before, &mut removed, false, meter)? {
                rows.insert((rule.head.relation, row));
            }
        }
        let mut changed = false;
        for (relation, row) in rows {
            if before.get(&relation).is_some_and(|store| store.count(&row) > 0)
                && removed.entry(relation).or_default().inserted.count(&row) == 0 {
                removed.entry(relation).or_default().inserted.insert(row);
                changed = true;
            }
        }
        if !changed { break; }
    }
    for relation in heads {
        if let Some(delta) = removed.get(relation) {
            let store = stores.entry(*relation).or_default();
            for row in delta.inserted.presence() {
                while store.count(row) > 0 { store.remove(row); }
            }
        }
    }
    Ok(())
}

pub(super) fn insert(
    program: &Program,
    rules: &[(usize, &Rule)],
    schedules: &[Vec<usize>],
    heads: &BTreeSet<RelationId>,
    stores: &mut BTreeMap<RelationId, Store>,
    meter: &mut Meter,
) -> Result<(), EvalError> {
    let mut frontier = BTreeMap::new();
    let mut first = true;
    loop {
        let mut rows = BTreeSet::new();
        for (index, rule) in rules {
            let derived = if first {
                execute::rule(program, rule, &schedules[*index], stores, meter)?
            } else {
                execute::pass(program, rule, &schedules[*index], stores, &mut frontier, false, meter)?
            };
            rows.extend(derived.into_iter().map(|row| (rule.head.relation, row)));
        }
        first = false;
        frontier.clear();
        for (relation, row) in rows {
            let store = stores.entry(relation).or_default();
            if store.count(&row) == 0 {
                store.insert(row.clone());
                frontier.entry(relation).or_insert_with(Delta::default).inserted.insert(row);
            }
        }
        if frontier.is_empty() { break; }
    }
    for relation in heads {
        let store = stores.entry(*relation).or_default();
        for row in store.tuples().clone() {
            while store.count(&row) > 1 { store.remove(&row); }
        }
    }
    Ok(())
}
