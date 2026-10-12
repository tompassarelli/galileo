mod aggregate;
mod ranked;
mod execute;
mod dred;
mod query;
pub mod store;

use crate::expr::Meter;
use crate::history::Change;
use crate::state::Tuple;
use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use store::Store;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    Delta,
    DRed,
    Recompute,
}

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
    summaries: [usize; 8],
    touched: [usize; 3],
    summary_both: [usize; 8],
    ranked_both: [usize; 4],
    field_both: usize,
    field_created: usize,
    outer_reads: usize,
    outer_changes: usize,
    outer_replacements: usize,
    keyed_refreshes: [usize; 2],
    live_skipped: usize,
    sweeps: [usize; 4],
    unswept: usize,
    untracked: usize,
}

#[derive(Clone, Debug)]
struct Current {
    facts: BTreeSet<Tuple>,
    live: BTreeSet<Entity>,
    relations: BTreeMap<RelationId, Store>,
    versions: BTreeMap<RelationId, u64>,
    summaries: aggregate::State,
}

#[derive(Clone, Debug)]
pub struct Mark {
    log: usize,
    stores: BTreeMap<RelationId, store::Mark>,
}

#[derive(Clone, Debug)]
enum Entry {
    Fact(Tuple, bool),
    Live(Entity, bool),
    Version(RelationId),
    Summary(RelationId, Vec<Value>, aggregate::Record),
    Replaced(RelationId, Store),
}

#[derive(Default)]
struct Step<'b> {
    added: Vec<Tuple>,
    withdrawn: Vec<Tuple>,
    created: Vec<Entity>,
    live_types: BTreeSet<String>,
    cancelled: BTreeSet<Tuple>,
    marks: BTreeMap<RelationId, store::Mark>,
    replaced: BTreeMap<RelationId, Store>,
    blocked: Option<(RelationId, &'b dyn Fn(&[Value]) -> bool)>,
    untracked: std::cell::Cell<usize>,
}

impl Step<'_> {
    fn start(&self, stores: &BTreeMap<RelationId, Store>, relation: RelationId) -> Store {
        let (Some(mark), Some(store)) = (self.marks.get(&relation), self.replaced.get(&relation).or(stores.get(&relation))) else {
            return Store::default();
        };
        self.untracked.set(self.untracked.get() + store.counts().len());
        let mut store = store.clone();
        store.undo(*mark);
        store
    }

    fn before<'a>(&'a self, stores: &'a BTreeMap<RelationId, Store>, relation: RelationId) -> std::borrow::Cow<'a, Store> {
        match (self.marks.get(&relation), self.replaced.get(&relation).or(stores.get(&relation))) {
            (Some(mark), Some(store)) if store.changed_since(*mark).is_empty() => std::borrow::Cow::Borrowed(store),
            _ => std::borrow::Cow::Owned(self.start(stores, relation)),
        }
    }

    fn delta(&self, stores: &BTreeMap<RelationId, Store>, relation: RelationId) -> Delta {
        let mut delta = Delta::default();
        let after = stores.get(&relation);
        match (self.replaced.contains_key(&relation), self.marks.get(&relation), after) {
            (false, Some(mark), Some(store)) => {
                for (row, was) in store.changed_since(*mark) {
                    let now = store.count(row) > 0;
                    if now && !was { delta.inserted.add(row.clone()); }
                    if was && !now { delta.deleted.add(row.clone()); }
                }
            }
            _ => {
                let before = self.before(stores, relation);
                let empty = Tuples::new();
                let after = after.map(Store::tuples).unwrap_or(&empty);
                for row in after.difference(before.tuples()) { delta.inserted.add(row.clone()); }
                for row in before.tuples().difference(after) { delta.deleted.add(row.clone()); }
            }
        }
        delta
    }
}

fn replace(step: &mut Option<&mut Step>, stores: &mut BTreeMap<RelationId, Store>, relation: RelationId) {
    if let Some(old) = stores.remove(&relation) {
        if let Some(step) = step.as_deref_mut() { step.replaced.entry(relation).or_insert(old); }
        stores.insert(relation, Store::unlogged());
    }
}

#[derive(Clone, Debug)]
pub struct Maintained {
    program: Program,
    relation_count: usize,
    aggregates: Vec<aggregate::Plan>,
    indexes: Vec<ranked::Index>,
    static_facts: BTreeSet<Tuple>,
    checked: CheckedProgram,
    current: Current,
    log: Vec<Entry>,
    empty: Tuples,
    stats: Stats,
    work: u64,
    pub(crate) invariants: Option<Vec<(String, RelationId, bool)>>,
}

impl Maintained {
    pub fn build(
        program: &Program,
        facts: &BTreeSet<Tuple>,
        live: &BTreeSet<Entity>,
        layers: &[std::ops::Range<usize>],
        budget: Budget,
    ) -> Result<Self, EvalError> {
        let mut original = check(program).map_err(EvalError::Static)?;
        let relation_count = layers.first().map_or(program.relations.len(), |layer| layer.start);
        let mut next = original.strata[..relation_count].iter().max().map_or(0, |top| top + 1);
        for layer in layers {
            let levels: BTreeSet<usize> = original.strata[layer.clone()].iter().copied().collect();
            for stratum in &mut original.strata[layer.clone()] {
                *stratum = next + levels.range(..*stratum).count();
            }
            next += levels.len();
        }
        let mut program = program.clone();
        let aggregates = aggregate::expand(&mut program);
        let mut checked = check(&program).map_err(EvalError::Static)?;
        aggregate::order(&program, &aggregates, &original.strata, &mut checked);
        let state_types = Self::state_types(&program);
        program.entities.retain(|entity| !state_types.contains(&entity.entity_type));
        let static_facts = program.facts.iter().map(|fact| (fact.relation, fact.values.clone())).collect();
        let mut maintained = Self {
            program,
            relation_count,
            aggregates,
            indexes: Vec::new(),
            static_facts,
            checked,
            current: Current {
                facts: facts.clone(),
                live: live.clone(),
                relations: BTreeMap::new(),
                versions: BTreeMap::new(),
                summaries: aggregate::State::new(),
            },
            log: Vec::new(),
            empty: Tuples::new(),
            stats: Stats::default(),
            work: 0,
            invariants: None,
        };
        let indexes = ranked::indexes(&maintained.program, &maintained.aggregates, &maintained.checked.schedules, |rule| {
            let relation = maintained.program.rules[rule].head.relation;
            maintained.stratum_strategies(&maintained.heads(maintained.checked.strata[relation.0]))[&relation] == Strategy::Delta
        });
        maintained.indexes = indexes;
        let mut stores = BTreeMap::new();
        let mut summaries = aggregate::State::new();
        let result = maintained.recompute(&mut stores, &mut summaries, &mut aggregate::Overlay::new(), None, budget);
        stores.values_mut().for_each(Store::log);
        let (work, stats, _) = result?;
        maintained.current.relations = stores;
        maintained.current.summaries = summaries;
        maintained.work = work;
        maintained.stats = stats;
        maintained.commit();
        Ok(maintained)
    }

    pub fn apply(
        &mut self,
        change: &Change,
        budget: Budget,
    ) -> Result<BTreeSet<RelationId>, EvalError> {
        self.advance(change, budget).map(|(changed, _)| changed)
    }

    pub(crate) fn reader<'a, 'm>(&'a mut self, program: &'a Program, meter: &'m mut Meter, at: &'a str) -> query::Reader<'a, 'm> {
        query::Reader::new(program, &mut self.current.relations, meter, at)
    }

    pub fn project_field(&mut self, program: &Program, view: &View, arguments: &[Value], expression: &Expr, budget: &mut Budget) -> Result<Value, EvalError> {
        let binding = view.parameters.iter().zip(arguments).map(|(slot, value)| (Variable::new(&slot.name), value.clone())).collect();
        let mut meter = Meter { limit: budget.work, spent: 0 };
        let value = self.reader(program, &mut meter, &view.name).value(expression, &binding);
        budget.work = budget.work.saturating_sub(meter.spent);
        value
    }

    pub(crate) fn advance(
        &mut self,
        change: &Change,
        budget: Budget,
    ) -> Result<(BTreeSet<RelationId>, Mark), EvalError> {
        self.step(change, None, budget)
    }

    pub fn blocking<T>(
        &mut self,
        relation: RelationId,
        blocked: &dyn Fn(&[Value]) -> bool,
        budget: Budget,
        read: impl FnOnce(&Self) -> T,
    ) -> Result<(T, u64), EvalError> {
        let (work, stats) = (self.work, self.stats);
        let (_, mark) = self.step(&Change::default(), Some((relation, blocked)), budget)?;
        let value = read(self);
        let spent = self.work;
        self.undo(mark);
        self.work = work;
        self.stats = stats;
        Ok((value, spent))
    }

    fn step(
        &mut self,
        change: &Change,
        blocked: Option<(RelationId, &dyn Fn(&[Value]) -> bool)>,
        budget: Budget,
    ) -> Result<(BTreeSet<RelationId>, Mark), EvalError> {
        let mark = self.mark();
        let mut step = Step {
            marks: mark.stores.clone(),
            cancelled: change.withdrawn.intersection(&change.added).cloned().collect(),
            blocked,
            ..Step::default()
        };
        for fact in &change.withdrawn {
            if !change.added.contains(fact) && self.current.facts.remove(fact) {
                self.log.push(Entry::Fact(fact.clone(), true));
                step.withdrawn.push(fact.clone());
            }
        }
        for fact in &change.added {
            if self.current.facts.insert(fact.clone()) {
                self.log.push(Entry::Fact(fact.clone(), false));
                step.added.push(fact.clone());
            }
        }
        for entity in &change.deleted {
            if !change.created.contains(entity) && self.current.live.remove(entity) {
                self.log.push(Entry::Live(entity.clone(), true));
                step.live_types.insert(entity.entity_type.clone());
            }
        }
        for entity in &change.created {
            if self.current.live.insert(entity.clone()) {
                self.log.push(Entry::Live(entity.clone(), false));
                step.live_types.insert(entity.entity_type.clone());
                step.created.push(entity.clone());
            }
        }
        let mut stores = std::mem::take(&mut self.current.relations);
        let mut summaries = std::mem::take(&mut self.current.summaries);
        let mut touched = aggregate::Overlay::new();
        let result = self.recompute(&mut stores, &mut summaries, &mut touched, Some(&mut step), budget);
        self.current.relations = stores;
        self.current.relations.values_mut().for_each(Store::log);
        self.current.summaries = summaries;
        let relations = self.relation_count;
        let (result, changed) = match result {
            Ok((work, stats, changed)) => (Ok((work, stats)), changed.into_iter().filter(|relation| relation.0 < relations).collect::<BTreeSet<_>>()),
            Err(error) => (Err(error), BTreeSet::new()),
        };
        for (relation, store) in std::mem::take(&mut step.replaced) {
            self.log.push(Entry::Replaced(relation, store));
        }
        for (relation, groups) in touched {
            for (key, record) in groups {
                self.log.push(Entry::Summary(relation, key, record));
            }
        }
        let (work, mut stats) = match result {
            Ok(done) => done,
            Err(error) => {
                self.undo(mark);
                return Err(error);
            }
        };
        for relation in &changed {
            *self.current.versions.entry(*relation).or_default() += 1;
            self.log.push(Entry::Version(*relation));
        }
        stats.untracked += step.untracked.get();
        self.work = work;
        self.stats = stats;
        Ok((changed, mark))
    }

    pub fn mark(&self) -> Mark {
        Mark {
            log: self.log.len(),
            stores: self.current.relations.iter().map(|(relation, store)| (*relation, store.mark())).collect(),
        }
    }

    pub fn undo(&mut self, mark: Mark) {
        assert!(mark.log <= self.log.len(), "mark is beyond the undo log");
        while self.log.len() > mark.log {
            match self.log.pop().expect("the log is longer than the mark") {
                Entry::Fact(fact, true) => { self.current.facts.insert(fact); }
                Entry::Fact(fact, false) => { self.current.facts.remove(&fact); }
                Entry::Live(entity, true) => { self.current.live.insert(entity); }
                Entry::Live(entity, false) => { self.current.live.remove(&entity); }
                Entry::Version(relation) => { *self.current.versions.entry(relation).or_default() -= 1; }
                Entry::Summary(relation, key, record) => record.undo(self.current.summaries.entry(relation).or_default(), key),
                Entry::Replaced(relation, store) => { self.current.relations.insert(relation, store); }
            }
        }
        self.current.relations.retain(|relation, store| match mark.stores.get(relation) {
            Some(mark) => { store.undo(*mark); true }
            None => false,
        });
    }

    pub fn commit(&mut self) {
        self.log.clear();
        for store in self.current.relations.values_mut() {
            store.forget();
        }
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

    pub fn ranked_both_passes(&self) -> [usize; 4] { self.stats.ranked_both }

    pub fn summary_reads(&self) -> [usize; 6] {
        self.stats.summaries[..6].try_into().unwrap()
    }

    pub fn summary_both_passes(&self) -> [usize; 6] {
        self.stats.summary_both[..6].try_into().unwrap()
    }

    pub fn query_both_passes(&self) -> [usize; 2] {
        self.stats.summary_both[6..].try_into().unwrap()
    }

    pub fn field_both_pass_strata(&self) -> usize {
        self.stats.field_both
    }

    pub fn field_created_insertion_strata(&self) -> usize {
        self.stats.field_created
    }

    pub fn outer_index_reads(&self) -> usize { self.stats.outer_reads }

    pub fn outer_index_changes(&self) -> usize { self.stats.outer_changes }

    pub fn outer_index_replacements(&self) -> usize { self.stats.outer_replacements }

    pub fn keyed_refreshes(&self) -> [usize; 2] {
        self.stats.keyed_refreshes
    }

    pub fn deleting_sweeps(&self) -> [usize; 4] { self.stats.sweeps }

    pub fn unswept_bindings(&self) -> usize { self.stats.unswept }

    pub fn live_skipped_strata(&self) -> usize {
        self.stats.live_skipped
    }

    pub fn touched_strata(&self) -> [usize; 3] { self.stats.touched }

    pub fn untracked_rows(&self) -> usize { self.stats.untracked }

    pub fn in_recursive_stratum(&self, relation: RelationId) -> bool {
        self.program.relations[relation.0].kind == RelationKind::Derived
            && self.recursive(&self.heads(self.checked.strata[relation.0]))
    }

    pub fn work(&self) -> u64 {
        self.work
    }

    pub fn strategies(&self) -> BTreeMap<RelationId, Strategy> {
        let top = self.checked.strata.iter().copied().max().unwrap_or(0);
        (0..=top)
            .flat_map(|stratum| self.stratum_strategies(&self.heads(stratum)))
            .filter(|(relation, _)| relation.0 < self.relation_count)
            .collect()
    }

    fn heads(&self, stratum: usize) -> BTreeSet<RelationId> {
        self.program
            .relations
            .iter()
            .enumerate()
            .filter(|(index, relation)| relation.kind == RelationKind::Derived && self.checked.strata[*index] == stratum)
            .map(|(index, _)| RelationId(index))
            .collect()
    }

    fn recursive(&self, heads: &BTreeSet<RelationId>) -> bool {
        self.checked.dependencies.iter().any(|dependency| {
            heads.contains(&dependency.head)
                && self.checked.components[dependency.head.0] == self.checked.components[dependency.body.0]
        })
    }

    fn lively(&self, heads: &BTreeSet<RelationId>, step: &Step) -> bool {
        heads.iter().any(|relation| {
            let declaration = &self.program.relations[relation.0];
            declaration.modes.iter().any(|mode| matches!(mode, Mode::ExactlyOne { key, .. }
                if matches!(&declaration.slots[*key].ty, Type::Entity(name) if step.live_types.contains(name))))
        })
    }

    fn stratum_strategies(&self, heads: &BTreeSet<RelationId>) -> BTreeMap<RelationId, Strategy> {
        let program = &self.program;
        let rules: Vec<_> = program.rules.iter().enumerate().filter(|(_, rule)| heads.contains(&rule.head.relation)).collect();
        let recursive = self.recursive(heads);
        let dred = recursive && rules.iter().all(|(_, rule)| execute::eligible(program, rule));
        heads
            .iter()
            .map(|relation| {
                let strategy = if dred {
                    Strategy::DRed
                } else if !recursive
                    && rules
                        .iter()
                        .filter(|(_, rule)| rule.head.relation == *relation)
                        .all(|(index, rule)| aggregate::eligible(program, rule, *index, &self.aggregates))
                {
                    Strategy::Delta
                } else {
                    Strategy::Recompute
                };
                (*relation, strategy)
            })
            .collect()
    }

    fn state_types(program: &Program) -> BTreeSet<String> {
        program.entity_types.iter().filter(|ty| ty.state).map(|ty| ty.name.clone()).collect()
    }

    fn recompute(
        &self,
        stores: &mut BTreeMap<RelationId, Store>,
        summaries: &mut aggregate::State,
        overlay: &mut aggregate::Overlay,
        mut step: Option<&mut Step>,
        budget: Budget,
    ) -> Result<(u64, Stats, BTreeSet<RelationId>), EvalError> {
        let next = &self.current;
        let program = &self.program;
        let state_types = Self::state_types(program);
        let mut meter = Meter {
            limit: budget.work,
            spent: 0,
        };
        let mut stats = Stats::default();
        let mut deltas: BTreeMap<RelationId, Delta> = BTreeMap::new();
        let block = step.as_deref().and_then(|step| step.blocked);
        let blocked = move |relation: RelationId, row: &[Value]| block.is_some_and(|(id, matches)| id == relation && matches(row));
        if let Some(step) = step.as_deref() {
            for fact in &step.withdrawn {
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
            for fact in &step.added {
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
            if let Some((relation, matches)) = block {
                let store = stores.entry(relation).or_default();
                let rows: Vec<_> = store.presence().filter(|row| matches(row)).cloned().collect();
                for row in rows {
                    meter.charge("blocked rows")?;
                    while store.count(&row) > 0 { store.remove(&row); }
                    deltas.entry(relation).or_default().deleted.insert(row);
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
        let fact_changed: BTreeSet<_> = step
            .as_deref()
            .into_iter()
            .flat_map(|step| step.withdrawn.iter().chain(&step.added))
            .map(|(relation, _)| *relation)
            .collect();
        let mut changed: BTreeSet<_> = deltas.keys().copied().collect();
        let mut weights = ranked::Changes::new();
        for stratum in 0..=top {
            let heads = self.heads(stratum);
            let lively = step.as_deref().is_some_and(|step| self.lively(&heads, step));
            let touched = step.as_deref().is_none_or(|_| {
                lively
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
                stats.live_skipped += usize::from(step.as_deref().is_some_and(|step| !step.live_types.is_empty()));
                if block.is_some_and(|(relation, _)| heads.contains(&relation)) {
                    let failures = self.mode_failures(&program, &heads, stores, &deltas, step.as_deref(), &state_types, next);
                    if !failures.is_empty() { return Err(EvalError::Mode(failures)); }
                }
                continue;
            }
            let rules: Vec<_> = program
                .rules
                .iter()
                .enumerate()
                .filter(|(_, rule)| heads.contains(&rule.head.relation))
                .collect();
            let recursive = self.recursive(&heads);
            let strategies = self.stratum_strategies(&heads);
            for (kind, strategy) in [Strategy::Delta, Strategy::DRed, Strategy::Recompute].into_iter().enumerate() {
                stats.touched[kind] += usize::from(strategies.values().any(|s| *s == strategy));
            }
            let eligible = |relation: RelationId| strategies[&relation] == Strategy::Delta;
            let unread = |rule: &Rule| {
                rule.head
                    .arguments
                    .iter()
                    .chain(
                        rule.body
                            .iter()
                            .filter(|literal| !matches!(literal, Literal::Aggregate { .. } | Literal::ArgMin { .. } | Literal::Bind { value: Expr::Query(_), .. }))
                            .flat_map(|literal| execute::expressions(literal).unwrap()),
                    )
                    .all(|expression| {
                        crate::check::expression_reads(&program, expression).is_disjoint(&changed)
                    })
            };
            let unkeyed = |rule: &Rule| {
                rule.head
                    .arguments
                    .iter()
                    .chain(
                        rule.body
                            .iter()
                            .filter(|literal| !matches!(literal, Literal::Aggregate { .. } | Literal::ArgMin { .. } | Literal::Bind { value: Expr::Query(_), .. }))
                            .flat_map(|literal| execute::expressions(literal).unwrap()),
                    )
                    .all(|expression| {
                        crate::check::expression_reads(&program, &execute::unkeyed(expression)).is_disjoint(&changed)
                    })
            };
            if heads.iter().any(|relation| strategies[relation] == Strategy::DRed) {
                let halved = rules.iter().any(|(_, rule)| sources(rule).any(|(relation, _)| {
                    deltas.get(&relation).is_some_and(|delta| {
                        let change = delta.inserted.tuples().len() + delta.deleted.tuples().len();
                        let size = stores.get(&relation).map_or(0, |store| store.tuples().len())
                            + delta.deleted.tuples().len() - delta.inserted.tuples().len();
                        change > 0 && 2 * change >= size
                    })
                }));
                let incremental = step.is_some() && !lively
                    && !halved && rules.iter().all(|(_, rule)| unread(rule));
                if !incremental {
                    for relation in &heads { replace(&mut step, stores, *relation); }
                }
                let before = step.as_deref().filter(|_| incremental).map(|step| {
                    self.checked.dependencies.iter()
                        .filter(|dependency| heads.contains(&dependency.head))
                        .map(|dependency| dependency.body)
                        .chain(heads.iter().copied())
                        .map(|relation| (relation, step.start(stores, relation)))
                        .collect()
                });
                dred::maintain(&program, &rules, &self.checked.schedules, &heads,
                    stores, &deltas, before, &mut meter)?;
                self.restate(&program, next, &heads, stores, &mut meter, &blocked, &mut stats.untracked)?;
                dred::insert(&program, &rules, &self.checked.schedules, &heads, stores, &mut meter, &blocked)?;
                stats.dred += usize::from(incremental);
                stats.built += usize::from(step.is_none());
                stats.half += usize::from(halved) * heads.len();
            } else {
            let reader = aggregate::Read { plans: &self.aggregates, state: &*summaries, overlay: None, counts: std::cell::RefCell::new([0; 8]), ranked_counts: std::cell::RefCell::new([0; 4]) };
            let old_reader = aggregate::Read { plans: &self.aggregates, state: &*summaries, overlay: step.is_some().then_some(&*overlay), counts: std::cell::RefCell::new([0; 8]), ranked_counts: std::cell::RefCell::new([0; 4]) };
            let groups = reader.changes(&old_reader);
            let passes: BTreeSet<RelationId> = heads
                .iter()
                .copied()
                .filter(|relation| {
                    step.is_some()
                        && eligible(*relation)
                        && rules
                            .iter()
                            .filter(|(_, rule)| rule.head.relation == *relation)
                            .all(|(_, rule)| unkeyed(rule))
                })
                .collect();
            let halved: BTreeSet<RelationId> = rules
                .iter()
                .filter(|(index, rule)| {
                    passes.contains(&rule.head.relation)
                        && (self.aggregates.iter().filter(|p| p.rule == *index).any(|p| {
                            if p.ranked.is_some() {
                                return deltas.get(&p.relation).is_some_and(|delta| {
                                    let change = delta.inserted.tuples().len() + delta.deleted.tuples().len();
                                    let size = stores.get(&p.relation).map_or(0, |store| store.tuples().len())
                                        + delta.deleted.tuples().len() - delta.inserted.tuples().len();
                                    change > 0 && 2 * change >= size
                                });
                            }
                            let change = groups.get(&p.relation).map_or(0, BTreeSet::len);
                            let size = old_reader.len(p.relation);
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
                        }) || execute::occurrences(rule).iter().any(|occurrence| {
                            crate::check::field_relations(&program, occurrence.field()).any(|relation| {
                                deltas.get(&relation).is_some_and(|delta| {
                                    let entities: BTreeSet<_> = delta.inserted.presence().chain(delta.deleted.presence())
                                        .filter_map(|row| row.first()).collect();
                                    let size = stores
                                        .get(&relation)
                                        .map_or(0, |store| store.tuples().len())
                                        + delta.deleted.tuples().len()
                                        - delta.inserted.tuples().len();
                                    !entities.is_empty() && 2 * entities.len() >= size
                                })
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
                replace(&mut step, stores, *relation);
            }
            self.restate(&program, next, &fresh, stores, &mut meter, &blocked, &mut stats.untracked)?;
            let mut gained = Vec::new();
            let mut lost = Vec::new();
            let (mut retracting, mut inserting, mut executed) = (false, false, false);
            let mut deferred: Vec<&Rule> = Vec::new();
            for (index, rule) in &rules {
                let relation = rule.head.relation;
                if !eligible(relation) {
                    if !recursive && aggregate::eligible(&program, rule, *index, &self.aggregates) {
                        self.recompute_rules(&program, stratum, &heads, &deferred, stores, &mut meter, budget, &blocked, &mut stats.untracked)?;
                        deferred.clear();
                        for row in execute::summarized(&program, rule, *index, &self.checked.schedules[*index], stores, &mut meter, &reader)? {
                            gained.push((relation, row));
                        }
                        continue;
                    }
                    deferred.push(rule);
                    continue;
                }
                self.recompute_rules(&program, stratum, &heads, &deferred, stores, &mut meter, budget, &blocked, &mut stats.untracked)?;
                deferred.clear();
                let mut outer = Vec::new();
                for index in self.indexes.iter().filter(|outer| outer.rule == *index) {
                    let fill = step.is_none() || fresh.contains(&relation);
                    if fill { replace(&mut step, stores, index.relation()); }
                    let maintained = ranked::maintain(&program, index, stores, &mut deltas, fill, &mut meter)?;
                    stats.outer_changes += usize::from(maintained.changed);
                    stats.outer_replacements += usize::from(maintained.replaced);
                    let mut kept = maintained.kept;
                    let plan = self.aggregates.iter().find(|p| p.rule == index.rule && p.position == kept.position()).unwrap();
                    let op = plan.ranked.as_ref().unwrap().op;
                    for group in groups.get(&plan.relation).into_iter().flatten() {
                        let typed = reader.sweepable(plan.relation, group);
                        if step.is_none() || fresh.contains(&relation) || typed.is_none() || typed != old_reader.sweepable(plan.relation, group) { continue; }
                        let changes = weights.get(&plan.relation).and_then(|w| w.get(group));
                        let keys: BTreeMap<Value, i128> = changes.into_iter().flatten().map(|(key, (weight, _))| (key.clone(), *weight)).collect();
                        stats.unswept += kept.sweep(stores, group, op, &keys);
                        stats.sweeps[ranked::operator(op)] += usize::from(changes.into_iter().flatten().any(|(_, (_, count))| *count < 0));
                    }
                    outer.push(kept);
                }
                let schedule = &self.checked.schedules[*index];
                if fresh.contains(&relation) {
                    executed = true;
                    for row in execute::summarized(&program, rule, *index, schedule, stores, &mut meter, &reader)? {
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
                let seeds = execute::keyed_seeds(&program, rule, &deltas);
                if !seeds.is_empty() {
                    retracting = true;
                    inserting = true;
                    stats.field_both += 1;
                    stats.field_created += usize::from(step.as_deref().is_some_and(|step| {
                        seeds.iter().flat_map(|(_, keys)| keys).any(|entity| step.created.contains(entity))
                    }));
                }
                for plan in self.aggregates.iter().filter(|p| p.rule == *index) {
                    if groups.get(&plan.relation).is_some_and(|g| !g.is_empty()) {
                        retracting = true;
                        inserting = true;
                    }
                }
                let old_ranked = *old_reader.ranked_counts.borrow();
                let new_ranked = *reader.ranked_counts.borrow();
                let old_counts = *old_reader.counts.borrow();
                let new_counts = *reader.counts.borrow();
                for row in execute::summarized_pass(&program, rule, schedule, stores, &mut deltas, true, &mut meter, Some((*index, &old_reader)), Some(&groups), &outer)? {
                    lost.push((relation, row));
                }
                for row in execute::summarized_pass(&program, rule, schedule, stores, &mut deltas, false, &mut meter, Some((*index, &reader)), Some(&groups), &outer)? {
                    gained.push((relation, row));
                }
                stats.outer_reads += outer.iter().map(|kept| kept.visits.get()).sum::<usize>();
                for kind in 0..4 {
                    stats.ranked_both[kind] += usize::from(old_reader.ranked_counts.borrow()[kind] > old_ranked[kind] && reader.ranked_counts.borrow()[kind] > new_ranked[kind]);
                }
                for kind in 0..8 {
                    stats.summary_both[kind] += usize::from(old_reader.counts.borrow()[kind] > old_counts[kind] && reader.counts.borrow()[kind] > new_counts[kind]);
                }
            }
            self.recompute_rules(&program, stratum, &heads, &deferred, stores, &mut meter, budget, &blocked, &mut stats.untracked)?;
            for (sum, (new, old)) in stats.summaries.iter_mut().zip(reader.counts.into_inner().into_iter().zip(old_reader.counts.into_inner())) { *sum += new + old; }
            for (relation, row) in gained {
                if blocked(relation, &row) { continue; }
                stores.entry(relation).or_default().insert(row);
            }
            for (relation, row) in lost {
                stores.entry(relation).or_default().remove(&row);
            }
            if step.is_none() && executed {
                stats.built += 1;
            }
            if !passes.is_empty() {
                stats.insertion += 1;
                stats.both += usize::from(retracting && inserting);
                stats.half += halved.len();
            }
            }
            for relation in &heads {
                if let Some(step) = step.as_deref() {
                    let delta = step.delta(stores, *relation);
                    if !delta.inserted.tuples().is_empty() || !delta.deleted.tuples().is_empty() {
                        changed.insert(*relation);
                        deltas.insert(*relation, delta);
                    } else {
                        changed.remove(relation);
                        deltas.remove(relation);
                    }
                }
            }
            let failures = self.mode_failures(&program, &heads, stores, &deltas, step.as_deref(), &state_types, next);
            let built = step.is_none();
            aggregate::update(&program, &self.aggregates, &heads, &changed, stores, summaries, &mut meter, &deltas, built, overlay, &mut stats.keyed_refreshes, &mut weights, &mut stats.untracked)?;
            if !failures.is_empty() {
                return Err(EvalError::Mode(failures));
            }
        }
        stats.cancelled = step.as_deref().map_or(0, |step| {
            step.cancelled
                .iter()
                .filter(|(relation, row)| {
                    deltas.get(relation).is_none_or(|delta| {
                        delta.inserted.count(row) == 0 && delta.deleted.count(row) == 0
                    })
                })
                .count()
        });
        Ok((meter.spent, stats, changed))
    }

    #[allow(clippy::too_many_arguments)]
    fn mode_failures(
        &self,
        program: &Program,
        heads: &BTreeSet<RelationId>,
        stores: &mut BTreeMap<RelationId, Store>,
        deltas: &BTreeMap<RelationId, Delta>,
        step: Option<&Step>,
        state_types: &BTreeSet<String>,
        next: &Current,
    ) -> Vec<crate::modes::ModeFailure> {
        let mut failures = Vec::new();
        for relation in heads {
            let declaration = &program.relations[relation.0];
            for mode in &declaration.modes {
                let (columns, values, total) = match mode {
                    Mode::ExactlyOne { key, values } => (vec![*key], values, true),
                    Mode::AtMostOne { keys, values } => (keys.clone(), values, false),
                };
                let project = |row: &Vec<Value>| {
                    columns.iter().map(|&slot| row[slot].clone()).collect::<Vec<_>>()
                };
                let mut keys: BTreeSet<_> = if step.is_some() {
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
                    if let Some(step) = step {
                        keys.extend(step.created.iter()
                            .filter(|entity| &entity.entity_type == name)
                            .map(|entity| vec![Value::Entity(entity.clone())]));
                    } else if state_types.contains(name) {
                        keys.extend(next.live.iter()
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
        failures
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
        blocked: &dyn Fn(RelationId, &[Value]) -> bool,
        untracked: &mut usize,
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
            .filter(|(relation, _)| relation.0 < sub.relations.len())
            .flat_map(|(relation, store)| {
                store
                    .presence()
                    .map(|values| Fact::new(*relation, values.clone()))
            })
            .collect();
        *untracked += sub.facts.len();
        let evaluated = crate::engine::evaluate_blocking(
            &sub,
            &[],
            Budget::work(budget.work.saturating_sub(meter.spent)),
            blocked,
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
        next: &Current,
        heads: &BTreeSet<RelationId>,
        stores: &mut BTreeMap<RelationId, Store>,
        meter: &mut Meter,
        blocked: &dyn Fn(RelationId, &[Value]) -> bool,
        untracked: &mut usize,
    ) -> Result<(), EvalError> {
        if heads.is_empty() { return Ok(()); }
        *untracked += program.facts.len();
        let stated = heads.iter().flat_map(|head| {
            next.facts.range((*head, Vec::new())..).take_while(move |(relation, _)| relation == head)
        });
        for (relation, row) in program
            .facts
            .iter()
            .map(|fact| (fact.relation, &fact.values))
            .filter(|(relation, _)| heads.contains(relation))
            .chain(stated.inspect(|_| *untracked += 1).map(|(relation, row)| (*relation, row)))
        {
            meter.charge("stated facts")?;
            if blocked(relation, row) { continue; }
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
