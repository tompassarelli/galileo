mod execute;
pub mod store;

use crate::expr::Meter;
use crate::history::Change;
use crate::state::Tuple;
use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use store::Store;

#[derive(Clone, Debug)]
pub struct Mark {
    facts: BTreeSet<Tuple>,
    live: BTreeSet<Entity>,
    relations: BTreeMap<RelationId, Store>,
    versions: BTreeMap<RelationId, u64>,
}

#[derive(Clone, Debug)]
pub struct Maintained {
    program: Program,
    checked: CheckedProgram,
    current: Mark,
    empty: Tuples,
    build_strata: usize,
    work: u64,
}

impl Maintained {
    pub fn build(
        program: &Program,
        facts: &BTreeSet<Tuple>,
        live: &BTreeSet<Entity>,
        budget: Budget,
    ) -> Result<Self, EvalError> {
        let checked = check(program).map_err(EvalError::Static)?;
        let mut maintained = Self {
            program: program.clone(),
            checked,
            current: Mark {
                facts: facts.clone(),
                live: live.clone(),
                relations: BTreeMap::new(),
                versions: BTreeMap::new(),
            },
            empty: Tuples::new(),
            build_strata: 0,
            work: 0,
        };
        let (relations, work, strata) = maintained.recompute(&maintained.current, None, budget)?;
        maintained.current.relations = relations;
        maintained.work = work;
        maintained.build_strata = strata;
        Ok(maintained)
    }

    pub fn apply(
        &mut self,
        change: &Change,
        budget: Budget,
    ) -> Result<BTreeSet<RelationId>, EvalError> {
        let mut next = self.current.clone();
        for fact in &change.withdrawn {
            next.facts.remove(fact);
        }
        next.facts.extend(change.added.iter().cloned());
        for entity in &change.deleted {
            next.live.remove(entity);
        }
        next.live.extend(change.created.iter().cloned());
        let (relations, work, _) = self.recompute(&next, Some(&self.current), budget)?;
        next.relations = relations;
        let changed: BTreeSet<_> = (0..self.program.relations.len())
            .map(RelationId)
            .filter(|relation| {
                self.current
                    .relations
                    .get(relation)
                    .map(Store::tuples)
                    .unwrap_or(&self.empty)
                    != next
                        .relations
                        .get(relation)
                        .map(Store::tuples)
                        .unwrap_or(&self.empty)
            })
            .collect();
        for relation in &changed {
            *next.versions.entry(*relation).or_default() += 1;
        }
        self.current = next;
        self.work = work;
        Ok(changed)
    }

    pub fn mark(&self) -> Mark {
        self.current.clone()
    }

    pub fn undo(&mut self, mark: Mark) {
        self.current = mark;
    }

    pub fn presence(&self, relation: RelationId) -> &Tuples {
        self.current
            .relations
            .get(&relation)
            .map(Store::tuples)
            .unwrap_or(&self.empty)
    }

    pub fn version(&self, relation: RelationId) -> u64 {
        self.current.versions.get(&relation).copied().unwrap_or(0)
    }

    pub fn executor_built_strata(&self) -> usize {
        self.build_strata
    }

    pub fn work(&self) -> u64 {
        self.work
    }

    fn recompute(
        &self,
        next: &Mark,
        previous: Option<&Mark>,
        budget: Budget,
    ) -> Result<(BTreeMap<RelationId, Store>, u64, usize), EvalError> {
        let mut program = self.program.clone();
        let state_types: BTreeSet<_> = program
            .entity_types
            .iter()
            .filter(|ty| ty.state)
            .map(|ty| ty.name.clone())
            .collect();
        program
            .entities
            .retain(|entity| !state_types.contains(&entity.entity_type));
        program.entities.extend(next.live.iter().cloned());
        let mut stores: BTreeMap<RelationId, Store> = BTreeMap::new();
        let mut meter = Meter {
            limit: budget.work,
            spent: 0,
        };
        let mut built_strata = 0;
        for fact in program
            .facts
            .iter()
            .map(|fact| (fact.relation, &fact.values))
            .chain(
                next.facts
                    .iter()
                    .map(|(relation, values)| (*relation, values)),
            )
        {
            meter.charge("stated facts")?;
            let store = stores.entry(fact.0).or_default();
            if store.count(fact.1) == 0 {
                store.insert(fact.1.clone());
            }
        }
        let top = self.checked.strata.iter().copied().max().unwrap_or(0);
        let mut changed: BTreeSet<_> = previous
            .into_iter()
            .flat_map(|previous| next.facts.symmetric_difference(&previous.facts))
            .map(|(relation, _)| *relation)
            .collect();
        for stratum in 0..=top {
            let heads: BTreeSet<_> = program
                .relations
                .iter()
                .enumerate()
                .filter(|(index, relation)| {
                    relation.kind == RelationKind::Derived && self.checked.strata[*index] == stratum
                })
                .map(|(index, _)| RelationId(index))
                .collect();
            let touched = previous.is_none_or(|previous| {
                next.live != previous.live
                    || self
                        .checked
                        .dependencies
                        .iter()
                        .filter(|dependency| heads.contains(&dependency.head))
                        .any(|dependency| changed.contains(&dependency.body))
                    || heads.iter().any(|relation| changed.contains(relation))
            });
            if !touched {
                for relation in heads {
                    if let Some(tuples) =
                        previous.and_then(|previous| previous.relations.get(&relation))
                    {
                        stores.insert(relation, tuples.clone());
                    }
                }
                continue;
            }
            let rules: Vec<_> = program
                .rules
                .iter()
                .enumerate()
                .filter(|(_, rule)| heads.contains(&rule.head.relation))
                .collect();
            let recursive = self.checked.dependencies.iter().any(|dependency| {
                heads.contains(&dependency.head)
                    && self.checked.components[dependency.head.0]
                        == self.checked.components[dependency.body.0]
            });
            if !recursive
                && rules
                    .iter()
                    .all(|(_, rule)| execute::eligible(&program, rule))
            {
                let base: BTreeMap<_, _> = heads
                    .iter()
                    .map(|relation| (*relation, stores.get(relation).cloned().unwrap_or_default()))
                    .collect();
                loop {
                    let mut derived = Vec::new();
                    for (index, rule) in &rules {
                        for row in execute::rule(
                            &program,
                            rule,
                            &self.checked.schedules[*index],
                            &mut stores,
                            &mut meter,
                        )? {
                            derived.push((rule.head.relation, row));
                        }
                    }
                    let before: BTreeMap<_, _> = heads
                        .iter()
                        .map(|relation| {
                            (
                                *relation,
                                stores
                                    .get(relation)
                                    .map(Store::tuples)
                                    .cloned()
                                    .unwrap_or_default(),
                            )
                        })
                        .collect();
                    for (relation, store) in &base {
                        stores.insert(*relation, store.clone());
                    }
                    for (relation, row) in derived {
                        stores.entry(relation).or_default().insert(row);
                    }
                    if heads
                        .iter()
                        .all(|relation| stores[relation].tuples() == &before[relation])
                    {
                        break;
                    }
                }
                let failures = crate::modes::check_stratum(
                    &program,
                    &self.checked.strata,
                    stratum,
                    RelationKind::Derived,
                    program.entities.iter(),
                    &|relation, each| {
                        stores
                            .get(&relation)
                            .into_iter()
                            .flat_map(Store::presence)
                            .for_each(|row| each(row));
                    },
                );
                if !failures.is_empty() {
                    return Err(EvalError::Mode(failures));
                }
                if !heads.is_empty() {
                    built_strata += 1;
                }
            } else {
                let mut sub = program.clone();
                sub.rules.retain(|rule| heads.contains(&rule.head.relation));
                for (index, relation) in sub.relations.iter_mut().enumerate() {
                    if self.checked.strata[index] != stratum
                        && relation.kind == RelationKind::Derived
                    {
                        relation.kind = RelationKind::Static;
                    }
                }
                sub.facts = stores
                    .iter()
                    .flat_map(|(relation, store)| {
                        store
                            .presence()
                            .map(|values| Fact::new(*relation, values.clone()))
                    })
                    .collect();
                let evaluated = crate::engine::evaluate(
                    &sub,
                    &[],
                    Budget::work(budget.work.saturating_sub(meter.spent)),
                )?;
                meter.spent += evaluated.work;
                for relation in &heads {
                    let store = stores.entry(*relation).or_default();
                    for row in evaluated.tuples(*relation) {
                        if store.count(row) == 0 {
                            store.insert(row.clone());
                        }
                    }
                }
            }
            for relation in &heads {
                if previous.is_some_and(|previous| {
                    stores
                        .get(relation)
                        .map(Store::tuples)
                        .unwrap_or(&self.empty)
                        != previous
                            .relations
                            .get(relation)
                            .map(Store::tuples)
                            .unwrap_or(&self.empty)
                }) {
                    changed.insert(*relation);
                }
            }
        }
        Ok((stores, meter.spent, built_strata))
    }
}
