use crate::state::{State, Tuple};
use crate::{Entity, Expr, Literal, Program, Query, RelationId, Type};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Change {
    pub added: BTreeSet<Tuple>,
    pub withdrawn: BTreeSet<Tuple>,
    pub created: BTreeSet<Entity>,
    pub deleted: BTreeSet<Entity>,
    pub requested: Vec<Tuple>,
    pub settled: BTreeSet<usize>,
}

impl Change {
    pub(crate) fn extend(&mut self, change: Self, pending: usize) {
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
        let retained = pending - self.settled.len();
        for index in change.settled.into_iter().rev() {
            if index >= retained {
                self.requested.remove(index - retained);
            } else {
                let original = (0..pending).filter(|index| !self.settled.contains(index)).nth(index).expect("a settled request is pending");
                self.settled.insert(original);
            }
        }
        self.requested.extend(change.requested);
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
    pub pending: Vec<(Tuple, Span)>,
    pub next: Vec<u64>,
    pub changes: Vec<Change>,
}

impl History {
    pub fn new(state: &State) -> Self {
        let initial = || vec![Span { from: 0, until: None }];
        Self {
            facts: state.facts.iter().cloned().map(|fact| (fact, initial())).collect(),
            live: state.live.iter().cloned().map(|entity| (entity, initial())).collect(),
            pending: state.pending.iter().cloned().map(|request| (request, Span { from: 0, until: None })).collect(),
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
        for (index, (_, span)) in self.pending.iter_mut().filter(|(_, span)| span.until.is_none()).enumerate() {
            if change.settled.contains(&index) { span.until = Some(revision); }
        }
        self.pending.extend(change.requested.iter().cloned().map(|request| (request, Span { from: revision, until: None })));
        self.next.push(next);
        self.changes.push(change);
    }

    pub fn as_of(&self, revision: u64) -> Option<State> {
        let next = *self.next.get(usize::try_from(revision).ok()?)?;
        Some(State {
            facts: self.facts.iter().filter(|(_, spans)| spans.iter().any(|span| span.contains(revision))).map(|(fact, _)| fact.clone()).collect(),
            live: self.live.iter().filter(|(_, spans)| spans.iter().any(|span| span.contains(revision))).map(|(entity, _)| entity.clone()).collect(),
            next,
            pending: self.pending.iter().filter(|(_, span)| span.contains(revision)).map(|(request, _)| request.clone()).collect(),
        })
    }

    pub fn query_revisions(&self, program: &Program, query: &Query, since: u64, until: u64) -> BTreeSet<u64> {
        fn expression(program: &Program, value: &Expr, relations: &mut BTreeSet<RelationId>, calls: &mut BTreeSet<String>) {
            match value {
                Expr::Query(query) => collect(program, query, relations, calls),
                Expr::Call { function, arguments } => {
                    for argument in arguments { expression(program, argument, relations, calls); }
                    if calls.insert(function.clone()) {
                        if let Some(definition) = program.definitions.iter().find(|d| d.name == *function) {
                            expression(program, &definition.body, relations, calls);
                        }
                        if let Some(query) = program.queries.iter().find(|q| q.name == *function) {
                            collect(program, &query.query, relations, calls);
                        }
                    }
                }
                Expr::List(values) => {
                    for value in values { expression(program, value, relations, calls); }
                }
                Expr::Record(values) | Expr::Attrs(values) => {
                    for value in values.values() { expression(program, value, relations, calls); }
                }
                Expr::Field { value, field } => {
                    expression(program, value, relations, calls);
                    relations.extend(crate::check::field_relations(program, field));
                }
                Expr::Unary { value, .. } | Expr::Lambda { body: value, .. } => expression(program, value, relations, calls),
                Expr::Binary { left, right, .. } => {
                    expression(program, left, relations, calls);
                    expression(program, right, relations, calls);
                }
                Expr::If { condition, then_value, else_value } => {
                    for value in [condition, then_value, else_value] { expression(program, value, relations, calls); }
                }
                Expr::Apply { function, arguments } => {
                    expression(program, function, relations, calls);
                    for argument in arguments { expression(program, argument, relations, calls); }
                }
                Expr::Value(_) | Expr::Variable(_) => {}
            }
        }
        fn body(program: &Program, literals: &[Literal], relations: &mut BTreeSet<RelationId>, calls: &mut BTreeSet<String>) {
            for literal in literals {
                match literal {
                    Literal::Positive(atom) | Literal::Negative(atom) => {
                        relations.insert(atom.relation);
                        for argument in &atom.arguments { expression(program, argument, relations, calls); }
                    }
                    Literal::Compare { left, right, .. } => {
                        expression(program, left, relations, calls);
                        expression(program, right, relations, calls);
                    }
                    Literal::Boolean(value) | Literal::Bind { value, .. } | Literal::Member { list: value, .. } => expression(program, value, relations, calls),
                    Literal::Aggregate { op, body: literals, .. } => {
                        body(program, literals, relations, calls);
                        if let Some(value) = op.value() { expression(program, value, relations, calls); }
                    }
                    Literal::ArgMin { candidate, key, body: literals, .. } => {
                        body(program, literals, relations, calls);
                        expression(program, candidate, relations, calls);
                        expression(program, key, relations, calls);
                    }
                }
            }
        }
        fn collect(program: &Program, query: &Query, relations: &mut BTreeSet<RelationId>, calls: &mut BTreeSet<String>) {
            body(program, &query.body, relations, calls);
            if let Some(sort) = &query.sorted_by { expression(program, sort, relations, calls); }
        }
        let mut relations = BTreeSet::new();
        let mut calls = BTreeSet::new();
        collect(program, query, &mut relations, &mut calls);
        let mut visited = BTreeSet::new();
        while let Some(relation) = relations.difference(&visited).next().copied() {
            visited.insert(relation);
            for rule in program.rules.iter().filter(|rule| rule.head.relation == relation) {
                body(program, &rule.body, &mut relations, &mut calls);
                for argument in &rule.head.arguments { expression(program, argument, &mut relations, &mut calls); }
            }
        }
        let live_types: BTreeSet<_> = relations.iter().flat_map(|id| &program.relations[id.0].slots).filter_map(|slot| {
            if let Type::Entity(name) = &slot.ty { Some(name) } else { None }
        }).collect();
        self.facts.iter().filter(|((relation, _), _)| relations.contains(relation)).flat_map(|(_, spans)| spans)
            .chain(self.live.iter().filter(|(entity, _)| live_types.contains(&entity.entity_type)).flat_map(|(_, spans)| spans))
            .flat_map(|span| std::iter::once(span.from).chain(span.until))
            .filter(|revision| since < *revision && *revision <= until).collect()
    }
}
