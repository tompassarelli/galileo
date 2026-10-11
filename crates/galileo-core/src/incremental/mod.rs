mod aggregate;
mod execute;
mod dred;
pub mod store;

use crate::expr::Meter;
use crate::history::Change;
use crate::state::Tuple;
use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use store::Store;

#[derive(Clone, Debug, Default)]
struct Delta {
    inserted: Store,
    deleted: Store,
}

#[derive(Clone, Copy, Debug, Default)]
struct Stats {
    built: usize,
    insertion: usize,
    both: usize,
    half: usize,
    cancelled: usize,
    dred: usize,
    summaries: [usize; 6],
    summary_both: [usize; 6],
}

#[derive(Clone, Debug)]
pub struct Mark {
    facts: BTreeSet<Tuple>,
    live: BTreeSet<Entity>,
    relations: BTreeMap<RelationId, Store>,
    versions: BTreeMap<RelationId, u64>,
    summaries: aggregate::State,
}

#[derive(Clone, Debug)]
pub struct Maintained {
    program: Program,
    relation_count: usize,
    aggregates: Vec<aggregate::Plan>,
    static_facts: BTreeSet<Tuple>,
    checked: CheckedProgram,
    current: Mark,
    empty: Tuples,
    stats: Stats,
    work: u64,
}

impl Maintained {
    pub fn build(
        program: &Program,
        facts: &BTreeSet<Tuple>,
        live: &BTreeSet<Entity>,
        budget: Budget,
    ) -> Result<Self, EvalError> {
        check(program).map_err(EvalError::Static)?;
        let relation_count = program.relations.len();
        let mut program = program.clone();
        let aggregates = aggregate::expand(&mut program);
        let mut checked = check(&program).map_err(EvalError::Static)?;
        aggregate::order(&program, &aggregates, &mut checked);
        let mut maintained = Self {
            program: program.clone(),
            relation_count,
            aggregates,
            static_facts: program
                .facts
                .iter()
                .map(|fact| (fact.relation, fact.values.clone()))
                .collect(),
            checked,
            current: Mark {
                facts: facts.clone(),
                live: live.clone(),
                relations: BTreeMap::new(),
                versions: BTreeMap::new(),
                summaries: aggregate::State::new(),
            },
            empty: Tuples::new(),
            stats: Stats::default(),
            work: 0,
        };
        let (relations, summaries, work, stats) = maintained.recompute(&maintained.current, None, budget)?;
        maintained.current.relations = relations;
        maintained.current.summaries = summaries;
        maintained.work = work;
        maintained.stats = stats;
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
        let cancelled: BTreeSet<_> = change.withdrawn.intersection(&change.added).cloned().collect();
        let (relations, summaries, work, stats) =
            self.recompute(&next, Some((&self.current, &cancelled)), budget)?;
        next.relations = relations;
        next.summaries = summaries;
        let changed: BTreeSet<_> = (0..self.relation_count)
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
        self.stats = stats;
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
        self.stats.built
    }

    pub fn insertion_pass_strata(&self) -> usize {
        self.stats.insertion
    }

    pub fn both_pass_strata(&self) -> usize {
        self.stats.both
    }

    pub fn half_rule_relations(&self) -> usize {
        self.stats.half
    }

    pub fn untouched_cancelled_facts(&self) -> usize {
        self.stats.cancelled
    }

    pub fn dred_strata(&self) -> usize {
        self.stats.dred
    }

    pub fn summary_reads(&self) -> [usize; 6] {
        self.stats.summaries
    }

    pub fn summary_both_passes(&self) -> [usize; 6] {
        self.stats.summary_both
    }

    pub fn work(&self) -> u64 {
        self.work
    }

    fn recompute(
        &self,
        next: &Mark,
        step: Option<(&Mark, &BTreeSet<Tuple>)>,
        budget: Budget,
    ) -> Result<(BTreeMap<RelationId, Store>, aggregate::State, u64, Stats), EvalError> {
        let previous = step.map(|(previous, _)| previous);
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
        let mut stores: BTreeMap<RelationId, Store> = previous
            .map(|previous| {
                previous
                    .relations
                    .iter()
                    .map(|(relation, store)| (*relation, store.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let mut meter = Meter {
            limit: budget.work,
            spent: 0,
        };
        let mut stats = Stats::default();
        let mut summaries = previous.map(|p| p.summaries.clone()).unwrap_or_default();
        let mut deltas: BTreeMap<RelationId, Delta> = BTreeMap::new();
        if let Some(previous) = previous {
            for fact in previous.facts.difference(&next.facts) {
                let (relation, row) = fact;
                meter.charge("stated facts")?;
                if !self.static_facts.contains(fact)
                    && stores.entry(*relation).or_default().remove(row)
                {
                    deltas
                        .entry(*relation)
                        .or_default()
                        .deleted
                        .insert(row.clone());
                }
            }
            for fact in next.facts.difference(&previous.facts) {
                let (relation, row) = fact;
                meter.charge("stated facts")?;
                let store = stores.entry(*relation).or_default();
                if !self.static_facts.contains(fact)
                    && store.insert(row.clone())
                {
                    deltas
                        .entry(*relation)
                        .or_default()
                        .inserted
                        .insert(row.clone());
                }
            }
        } else {
            for (relation, row) in program
                .facts
                .iter()
                .map(|fact| (fact.relation, &fact.values))
                .chain(next.facts.iter().map(|(relation, row)| (*relation, row)))
            {
                meter.charge("stated facts")?;
                let store = stores.entry(relation).or_default();
                if store.count(row) == 0 {
                    store.insert(row.clone());
                }
            }
        }
        let top = self.checked.strata.iter().copied().max().unwrap_or(0);
        let fact_changed: BTreeSet<_> = previous
            .into_iter()
            .flat_map(|previous| next.facts.symmetric_difference(&previous.facts))
            .map(|(relation, _)| *relation)
            .collect();
        let mut changed: BTreeSet<_> = deltas.keys().copied().collect();
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
                    || self.aggregates.iter().filter(|p| heads.contains(&p.relation)).any(|p| !p.reads.is_disjoint(&changed))
                    || !heads.is_disjoint(&fact_changed)
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
            let eligible = |relation: RelationId| {
                !recursive
                    && rules
                        .iter()
                        .filter(|(_, rule)| rule.head.relation == relation)
                        .all(|(index, rule)| aggregate::eligible(&program, rule, *index, &self.aggregates))
            };
            let unread = |rule: &Rule| {
                rule.head
                    .arguments
                    .iter()
                    .chain(
                        rule.body
                            .iter()
                            .filter(|literal| !matches!(literal, Literal::Aggregate { .. } | Literal::ArgMin { .. }))
                            .flat_map(|literal| execute::expressions(literal).unwrap()),
                    )
                    .all(|expression| {
                        crate::check::expression_reads(&program, expression).is_disjoint(&changed)
                    })
            };
            if recursive && rules.iter().all(|(_, rule)| execute::eligible(&program, rule)) {
                let halved = rules.iter().any(|(_, rule)| sources(rule).any(|(relation, _)| {
                    deltas.get(&relation).is_some_and(|delta| {
                        let change = delta.inserted.tuples().len() + delta.deleted.tuples().len();
                        let size = stores.get(&relation).map_or(0, |store| store.tuples().len())
                            + delta.deleted.tuples().len() - delta.inserted.tuples().len();
                        change > 0 && 2 * change >= size
                    })
                }));
                let incremental = previous.is_some_and(|p| p.live == next.live)
                    && !halved && rules.iter().all(|(_, rule)| unread(rule));
                if !incremental {
                    for relation in &heads { stores.remove(relation); }
                }
                dred::maintain(&program, &rules, &self.checked.schedules, &heads,
                    &mut stores, &deltas, previous.filter(|_| incremental), &mut meter)?;
                self.restate(&program, next, &heads, &mut stores, &mut meter)?;
                dred::insert(&program, &rules, &self.checked.schedules, &heads, &mut stores, &mut meter)?;
                stats.dred += usize::from(incremental);
                stats.built += usize::from(previous.is_none());
                stats.half += usize::from(halved) * heads.len();
            } else {
            let reader = aggregate::Read { plans: &self.aggregates, state: &summaries, counts: std::cell::RefCell::new([0; 6]) };
            let old_reader = aggregate::Read { plans: &self.aggregates, state: previous.map(|p| &p.summaries).unwrap_or(&summaries), counts: std::cell::RefCell::new([0; 6]) };
            let groups = reader.changes(&old_reader);
            let passes: BTreeSet<RelationId> = heads
                .iter()
                .copied()
                .filter(|relation| {
                    previous.is_some()
                        && eligible(*relation)
                        && rules
                            .iter()
                            .filter(|(_, rule)| rule.head.relation == *relation)
                            .all(|(_, rule)| unread(rule))
                })
                .collect();
            let halved: BTreeSet<RelationId> = rules
                .iter()
                .filter(|(index, rule)| {
                    passes.contains(&rule.head.relation)
                        && (self.aggregates.iter().filter(|p| p.rule == *index).any(|p| {
                            let change = groups.get(&p.relation).map_or(0, BTreeSet::len);
                            let size = old_reader.state.get(&p.relation).map_or(0, BTreeMap::len);
                            change > 0 && 2 * change >= size
                        }) || sources(rule).any(|(relation, _)| {
                            deltas.get(&relation).is_some_and(|delta| {
                                let change =
                                    delta.inserted.tuples().len() + delta.deleted.tuples().len();
                                let size = stores
                                    .get(&relation)
                                    .map_or(0, |store| store.tuples().len())
                                    + delta.deleted.tuples().len()
                                    - delta.inserted.tuples().len();
                                change > 0 && 2 * change >= size
                            })
                        }))
                })
                .map(|(_, rule)| rule.head.relation)
                .collect();
            let fresh: BTreeSet<RelationId> = heads
                .iter()
                .copied()
                .filter(|relation| !passes.contains(relation) || halved.contains(relation))
                .collect();
            for relation in &fresh {
                stores.remove(relation);
            }
            self.restate(&program, next, &fresh, &mut stores, &mut meter)?;
            let mut gained = Vec::new();
            let mut lost = Vec::new();
            let (mut retracting, mut inserting, mut executed) = (false, false, false);
            let mut deferred: Vec<&Rule> = Vec::new();
            for (index, rule) in &rules {
                let relation = rule.head.relation;
                if !eligible(relation) {
                    if !recursive && aggregate::eligible(&program, rule, *index, &self.aggregates) {
                        self.recompute_rules(&program, stratum, &heads, &deferred, &mut stores, &mut meter, budget)?;
                        deferred.clear();
                        for row in execute::summarized(&program, rule, *index, &self.checked.schedules[*index], &mut stores, &mut meter, &reader)? {
                            gained.push((relation, row));
                        }
                        continue;
                    }
                    deferred.push(rule);
                    continue;
                }
                self.recompute_rules(&program, stratum, &heads, &deferred, &mut stores, &mut meter, budget)?;
                deferred.clear();
                let schedule = &self.checked.schedules[*index];
                if fresh.contains(&relation) {
                    executed = true;
                    for row in execute::summarized(&program, rule, *index, schedule, &mut stores, &mut meter, &reader)? {
                        gained.push((relation, row));
                    }
                    continue;
                }
                for (source, negative) in sources(rule) {
                    if let Some(delta) = deltas.get(&source) {
                        let (removing, adding) = if negative {
                            (&delta.inserted, &delta.deleted)
                        } else {
                            (&delta.deleted, &delta.inserted)
                        };
                        retracting |= !removing.tuples().is_empty();
                        inserting |= !adding.tuples().is_empty();
                    }
                }
                for plan in self.aggregates.iter().filter(|p| p.rule == *index) {
                    if groups.get(&plan.relation).is_some_and(|g| !g.is_empty()) {
                        retracting = true;
                        inserting = true;
                    }
                }
                let old_counts = *old_reader.counts.borrow();
                let new_counts = *reader.counts.borrow();
                for row in execute::summarized_pass(&program, rule, schedule, &mut stores, &mut deltas, true, &mut meter, Some((*index, &old_reader)), Some(&groups))? {
                    lost.push((relation, row));
                }
                for row in execute::summarized_pass(&program, rule, schedule, &mut stores, &mut deltas, false, &mut meter, Some((*index, &reader)), Some(&groups))? {
                    gained.push((relation, row));
                }
                for kind in 0..6 {
                    stats.summary_both[kind] += usize::from(old_reader.counts.borrow()[kind] > old_counts[kind] && reader.counts.borrow()[kind] > new_counts[kind]);
                }
            }
            self.recompute_rules(&program, stratum, &heads, &deferred, &mut stores, &mut meter, budget)?;
            for (sum, (new, old)) in stats.summaries.iter_mut().zip(reader.counts.into_inner().into_iter().zip(old_reader.counts.into_inner())) { *sum += new + old; }
            for (relation, row) in gained {
                stores.entry(relation).or_default().insert(row);
            }
            for (relation, row) in lost {
                stores.entry(relation).or_default().remove(&row);
            }
            if previous.is_none() && executed {
                stats.built += 1;
            }
            if !passes.is_empty() {
                stats.insertion += 1;
                stats.both += usize::from(retracting && inserting);
                stats.half += halved.len();
            }
            }
            for relation in &heads {
                if let Some(previous) = previous {
                    let before = previous
                        .relations
                        .get(relation)
                        .map(Store::tuples)
                        .unwrap_or(&self.empty);
                    let after = stores
                        .get(relation)
                        .map(Store::tuples)
                        .unwrap_or(&self.empty);
                    let mut delta = Delta::default();
                    for row in after.difference(before) {
                        delta.inserted.insert(row.clone());
                    }
                    for row in before.difference(after) {
                        delta.deleted.insert(row.clone());
                    }
                    if !delta.inserted.tuples().is_empty() || !delta.deleted.tuples().is_empty() {
                        changed.insert(*relation);
                        deltas.insert(*relation, delta);
                    } else {
                        changed.remove(relation);
                        deltas.remove(relation);
                    }
                }
            }
            let mut failures = Vec::new();
            for relation in &heads {
                let declaration = &program.relations[relation.0];
                for mode in &declaration.modes {
                    let (columns, values, total) = match mode {
                        Mode::ExactlyOne { key, values } => (vec![*key], values, true),
                        Mode::AtMostOne { keys, values } => (keys.clone(), values, false),
                    };
                    let project = |row: &Vec<Value>| {
                        columns.iter().map(|&slot| row[slot].clone()).collect::<Vec<_>>()
                    };
                    let mut keys: BTreeSet<_> = if previous.is_some() {
                        deltas.get(relation).into_iter()
                            .flat_map(|delta| delta.inserted.presence().chain(delta.deleted.presence()))
                            .map(project).collect()
                    } else {
                        stores.get(relation).into_iter().flat_map(Store::presence)
                            .map(project).collect()
                    };
                    let domain = if total {
                        match &declaration.slots[columns[0]].ty {
                            Type::Entity(name) => Some(name),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if let Some(name) = domain {
                        if let Some(previous) = previous {
                            keys.extend(next.live.difference(&previous.live)
                                .filter(|entity| &entity.entity_type == name)
                                .map(|entity| vec![Value::Entity(entity.clone())]));
                        } else {
                            keys.extend(program.entities.iter()
                                .filter(|entity| &entity.entity_type == name)
                                .map(|entity| vec![Value::Entity(entity.clone())]));
                        }
                    }
                    let store = stores.entry(*relation).or_default();
                    for key in keys {
                        let required = domain.is_some_and(|name| {
                            matches!(key.as_slice(), [Value::Entity(entity)]
                                if &entity.entity_type == name && if state_types.contains(name) {
                                    next.live.contains(entity)
                                } else {
                                    program.entities.contains(entity)
                                })
                        });
                        if let Some(failure) = crate::modes::check_key(
                            *relation, key.clone(), values, required, store.matching(&columns, &key),
                        ) {
                            failures.push(failure);
                        }
                    }
                }
            }
            aggregate::update(&program, &self.aggregates, &heads, &changed, &mut stores, &mut summaries, &mut meter, previous.is_some_and(|p| p.live != next.live), &deltas, previous.is_none())?;
            if !failures.is_empty() {
                return Err(EvalError::Mode(failures));
            }
        }
        stats.cancelled = step.map_or(0, |(_, cancelled)| {
            cancelled
                .iter()
                .filter(|(relation, row)| {
                    deltas.get(relation).is_none_or(|delta| {
                        delta.inserted.count(row) == 0 && delta.deleted.count(row) == 0
                    })
                })
                .count()
        });
        Ok((stores, summaries, meter.spent, stats))
    }

    #[allow(clippy::too_many_arguments)]
    fn recompute_rules(
        &self,
        program: &Program,
        stratum: usize,
        heads: &BTreeSet<RelationId>,
        rules: &[&Rule],
        stores: &mut BTreeMap<RelationId, Store>,
        meter: &mut Meter,
        budget: Budget,
    ) -> Result<(), EvalError> {
        if rules.is_empty() {
            return Ok(());
        }
        let mut sub = program.clone();
        sub.rules = rules.iter().map(|rule| (*rule).clone()).collect();
        for (index, relation) in sub.relations.iter_mut().enumerate() {
            if self.checked.strata[index] != stratum && relation.kind == RelationKind::Derived {
                relation.kind = RelationKind::Static;
            }
            if heads.contains(&RelationId(index)) {
                relation.modes.clear();
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
        for relation in rules.iter().map(|rule| rule.head.relation).collect::<BTreeSet<_>>() {
            let store = stores.entry(relation).or_default();
            for row in evaluated.tuples(relation) {
                if store.count(row) == 0 {
                    store.insert(row.clone());
                }
            }
        }
        Ok(())
    }

    fn restate(
        &self,
        program: &Program,
        next: &Mark,
        heads: &BTreeSet<RelationId>,
        stores: &mut BTreeMap<RelationId, Store>,
        meter: &mut Meter,
    ) -> Result<(), EvalError> {
        for (relation, row) in program
            .facts
            .iter()
            .map(|fact| (fact.relation, &fact.values))
            .chain(next.facts.iter().map(|(relation, row)| (*relation, row)))
            .filter(|(relation, _)| heads.contains(relation))
        {
            meter.charge("stated facts")?;
            let store = stores.entry(relation).or_default();
            if store.count(row) == 0 {
                store.insert(row.clone());
            }
        }
        Ok(())
    }
}

fn sources(rule: &Rule) -> impl Iterator<Item = (RelationId, bool)> + '_ {
    rule.body.iter().filter_map(|literal| match literal {
        Literal::Positive(atom) => Some((atom.relation, false)),
        Literal::Negative(atom) => Some((atom.relation, true)),
        _ => None,
    })
}
