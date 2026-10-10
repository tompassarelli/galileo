use crate::state::{State, Tuple};
use crate::Entity;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Change {
    pub added: BTreeSet<Tuple>,
    pub withdrawn: BTreeSet<Tuple>,
    pub created: BTreeSet<Entity>,
    pub deleted: BTreeSet<Entity>,
}

impl Change {
    pub(crate) fn extend(&mut self, change: Self) {
        for fact in change.withdrawn {
            if !self.added.remove(&fact) { self.withdrawn.insert(fact); }
        }
        for fact in change.added {
            if !self.withdrawn.remove(&fact) { self.added.insert(fact); }
        }
        for entity in change.deleted {
            if !self.created.remove(&entity) { self.deleted.insert(entity); }
        }
        for entity in change.created {
            if !self.deleted.remove(&entity) { self.created.insert(entity); }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub from: u64,
    pub until: Option<u64>,
}

impl Span {
    pub fn contains(&self, revision: u64) -> bool {
        self.from <= revision && self.until.map_or(true, |until| revision < until)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    pub facts: BTreeMap<Tuple, Vec<Span>>,
    pub live: BTreeMap<Entity, Vec<Span>>,
    pub next: Vec<u64>,
    pub changes: Vec<Change>,
}

impl History {
    pub fn new(state: &State) -> Self {
        let initial = || vec![Span { from: 0, until: None }];
        Self {
            facts: state.facts.iter().cloned().map(|fact| (fact, initial())).collect(),
            live: state.live.iter().cloned().map(|entity| (entity, initial())).collect(),
            next: vec![state.next],
            changes: Vec::new(),
        }
    }

    pub(crate) fn append(&mut self, change: Change, next: u64) {
        let revision = self.next.len() as u64;
        for fact in &change.withdrawn {
            self.facts.get_mut(fact).and_then(|spans| spans.last_mut()).expect("a withdrawn fact has a span").until = Some(revision);
        }
        for fact in &change.added {
            self.facts.entry(fact.clone()).or_default().push(Span { from: revision, until: None });
        }
        for entity in &change.deleted {
            self.live.get_mut(entity).and_then(|spans| spans.last_mut()).expect("a deleted entity has a span").until = Some(revision);
        }
        for entity in &change.created {
            self.live.entry(entity.clone()).or_default().push(Span { from: revision, until: None });
        }
        self.next.push(next);
        self.changes.push(change);
    }

    pub fn as_of(&self, revision: u64) -> Option<State> {
        let next = *self.next.get(usize::try_from(revision).ok()?)?;
        Some(State {
            facts: self.facts.iter().filter(|(_, spans)| spans.iter().any(|span| span.contains(revision))).map(|(fact, _)| fact.clone()).collect(),
            live: self.live.iter().filter(|(_, spans)| spans.iter().any(|span| span.contains(revision))).map(|(entity, _)| entity.clone()).collect(),
            next,
        })
    }
}
