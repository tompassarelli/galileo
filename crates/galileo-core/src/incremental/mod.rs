pub mod store;

use crate::history::Change;
use crate::state::Tuple;
use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Mark {
    facts: BTreeSet<Tuple>,
    live: BTreeSet<Entity>,
    model: Model,
    versions: BTreeMap<RelationId, u64>,
}

#[derive(Clone, Debug)]
pub struct Maintained {
    program: Program,
    checked: CheckedProgram,
    current: Mark,
    empty: Tuples,
}

impl Maintained {
    pub fn build(program: &Program, facts: &BTreeSet<Tuple>, live: &BTreeSet<Entity>, budget: Budget) -> Result<Self, EvalError> {
        let checked = check(program).map_err(EvalError::Static)?;
        let mut maintained = Self {
            program: program.clone(),
            checked,
            current: Mark { facts: facts.clone(), live: live.clone(), model: Model::default(), versions: BTreeMap::new() },
            empty: Tuples::new(),
        };
        maintained.current.model = maintained.recompute(&maintained.current, None, budget)?;
        Ok(maintained)
    }

    pub fn apply(&mut self, change: &Change, budget: Budget) -> Result<BTreeSet<RelationId>, EvalError> {
        let mut next = self.current.clone();
        for fact in &change.withdrawn { next.facts.remove(fact); }
        next.facts.extend(change.added.iter().cloned());
        for entity in &change.deleted { next.live.remove(entity); }
        next.live.extend(change.created.iter().cloned());
        next.model = self.recompute(&next, Some(&self.current), budget)?;
        let changed: BTreeSet<_> = (0..self.program.relations.len()).map(RelationId).filter(|relation| {
            self.current.model.tuples(*relation).ne(next.model.tuples(*relation))
        }).collect();
        for relation in &changed { *next.versions.entry(*relation).or_default() += 1; }
        self.current = next;
        Ok(changed)
    }

    pub fn mark(&self) -> Mark { self.current.clone() }

    pub fn undo(&mut self, mark: Mark) { self.current = mark; }

    pub fn presence(&self, relation: RelationId) -> &Tuples {
        self.current.model.relations.get(&relation).map(Arc::as_ref).unwrap_or(&self.empty)
    }

    pub fn version(&self, relation: RelationId) -> u64 {
        self.current.versions.get(&relation).copied().unwrap_or(0)
    }

    fn recompute(&self, next: &Mark, previous: Option<&Mark>, budget: Budget) -> Result<Model, EvalError> {
        let mut program = self.program.clone();
        let state_types: BTreeSet<_> = program.entity_types.iter().filter(|ty| ty.state).map(|ty| ty.name.clone()).collect();
        program.entities.retain(|entity| !state_types.contains(&entity.entity_type));
        program.entities.extend(next.live.iter().cloned());
        let mut model = Model::default();
        for fact in program.facts.iter().map(|fact| (fact.relation, &fact.values)).chain(next.facts.iter().map(|(relation, values)| (*relation, values))) {
            if model.work == budget.work { return Err(EvalError::Unknown(Reason::OutOfBudget { resource: Resource::Work, at: "stated facts".into() })); }
            model.work += 1;
            Arc::make_mut(model.relations.entry(fact.0).or_default()).insert(fact.1.clone());
        }
        let top = self.checked.strata.iter().copied().max().unwrap_or(0);
        let mut changed: BTreeSet<_> = previous.into_iter().flat_map(|previous| next.facts.symmetric_difference(&previous.facts)).map(|(relation, _)| *relation).collect();
        for stratum in 0..=top {
            let heads: BTreeSet<_> = program.relations.iter().enumerate().filter(|(index, relation)| relation.kind == RelationKind::Derived && self.checked.strata[*index] == stratum).map(|(index, _)| RelationId(index)).collect();
            let touched = previous.is_none_or(|previous| next.live != previous.live || self.checked.dependencies.iter().filter(|dependency| heads.contains(&dependency.head)).any(|dependency| {
                changed.contains(&dependency.body)
            }) || heads.iter().any(|relation| changed.contains(relation)));
            if !touched {
                for relation in heads {
                    if let Some(tuples) = previous.and_then(|previous| previous.model.relations.get(&relation)) { model.relations.insert(relation, tuples.clone()); }
                }
                continue;
            }
            let mut sub = program.clone();
            sub.rules.retain(|rule| heads.contains(&rule.head.relation));
            for (index, relation) in sub.relations.iter_mut().enumerate() {
                if self.checked.strata[index] != stratum && relation.kind == RelationKind::Derived { relation.kind = RelationKind::Static; }
            }
            sub.facts = model.relations.iter().flat_map(|(relation, tuples)| tuples.iter().map(|values| Fact::new(*relation, values.clone()))).collect();
            let evaluated = crate::engine::evaluate(&sub, &[], Budget::work(budget.work.saturating_sub(model.work)))?;
            model.work += evaluated.work;
            for (index, relation) in program.relations.iter().enumerate() {
                if relation.kind == RelationKind::Derived && self.checked.strata[index] == stratum {
                    let id = RelationId(index);
                    if let Some(tuples) = evaluated.relations.get(&id) { model.relations.insert(id, tuples.clone()); }
                    if previous.is_some_and(|previous| model.tuples(id).ne(previous.model.tuples(id))) { changed.insert(id); }
                }
            }
        }
        Ok(model)
    }
}
