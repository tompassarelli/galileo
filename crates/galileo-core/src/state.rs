use crate::check::{render_atom, render_expr, render_literal};
use std::cell::{Cell, RefCell, RefMut};
use crate::incremental::{Maintained, Mark};
use crate::expr::Binding;
use crate::history::{Change, History};
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

pub type Tuple = (RelationId, Vec<Value>);

struct StepOutcome {
    change: Change,
    effects: Vec<Tuple>,
    next: u64,
    substitutions: usize,
}

struct StepUndo {
    change: Change,
    settled: Vec<(usize, Tuple)>,
    next: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct State {
    pub facts: BTreeSet<Tuple>,
    pub live: BTreeSet<Entity>,
    pub next: u64,
    pub pending: Vec<Tuple>,
}

impl State {
    fn apply(&mut self, change: Change, next: u64) -> StepUndo {
        for fact in &change.withdrawn { self.facts.remove(fact); }
        self.facts.extend(change.added.iter().cloned());
        for entity in &change.deleted { self.live.remove(entity); }
        self.live.extend(change.created.iter().cloned());
        let settled = change.settled.iter().rev().map(|&index| (index, self.pending.remove(index))).collect();
        self.pending.extend(change.requested.iter().cloned());
        let previous = std::mem::replace(&mut self.next, next);
        StepUndo { change, settled, next: previous }
    }

    fn undo(&mut self, undo: StepUndo) {
        for fact in &undo.change.added { self.facts.remove(fact); }
        self.facts.extend(undo.change.withdrawn);
        for entity in &undo.change.created { self.live.remove(entity); }
        self.live.extend(undo.change.deleted);
        self.pending.truncate(self.pending.len() - undo.change.requested.len());
        for (index, request) in undo.settled.into_iter().rev() { self.pending.insert(index, request); }
        self.next = undo.next;
    }

    pub fn hash(&self, program: &Program) -> String {
        self.hash_from_sums(program, self.hash_sums(program))
    }

    fn hash_sums(&self, program: &Program) -> (u64, u64) {
        let facts = self.facts.iter().fold(0_u64, |sum, fact| sum.wrapping_add(fact_hash(program, fact)));
        let live = self.live.iter().fold(0_u64, |sum, entity| sum.wrapping_add(entity_hash(entity)));
        (facts, live)
    }

    fn hash_from_sums(&self, program: &Program, (facts, live): (u64, u64)) -> String {
        let pending: Vec<_> = self.pending.iter().map(|(relation, values)| (&program.relations[relation.0].name, values)).collect();
        let hash = state_hash_bytes(&format!("{:?}", (facts, live, self.next, pending)));
        format!("{hash:016x}")
    }

    pub fn initial(program: &Program) -> Self {
        let state_types: BTreeSet<&str> = program
            .entity_types
            .iter()
            .filter(|t| t.state)
            .map(|t| t.name.as_str())
            .collect();
        Self {
            facts: program
                .initially
                .iter()
                .map(|f| (f.relation, f.values.clone()))
                .collect(),
            live: program
                .entities
                .iter()
                .filter(|e| state_types.contains(e.entity_type.as_str()))
                .cloned()
                .collect(),
            next: 0,
            pending: Vec::new(),
        }
    }
}

fn state_hash_bytes(bytes: &str) -> u64 {
    bytes.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3))
}

fn fact_hash(program: &Program, (relation, values): &Tuple) -> u64 {
    state_hash_bytes(&format!("{:?}", (&program.relations[relation.0].name, values)))
}

fn entity_hash(entity: &Entity) -> u64 {
    state_hash_bytes(&format!("{entity:?}"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transition {
    pub state: State,
    pub change: Change,
    pub effects: Vec<Tuple>,
    pub substitutions: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectionKind {
    NoMatch,
    ArgumentDomain,
    ConflictingWrite,
    UnfilledExactlyOne,
    DerivedMode,
    Invariant,
    Ensures,
    NonLiveInclude,
    CompetingEffects,
}

impl RejectionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoMatch => "no match",
            Self::ArgumentDomain => "argument domain",
            Self::ConflictingWrite => "conflicting write",
            Self::UnfilledExactlyOne => "unfilled exactly one",
            Self::DerivedMode => "derived mode",
            Self::Invariant => "invariant",
            Self::Ensures => "ensures",
            Self::NonLiveInclude => "non-live include",
            Self::CompetingEffects => "competing effects",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejection {
    pub kind: RejectionKind,
    pub explanation: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepRecord {
    pub events: Vec<Tuple>,
    pub revision: u64,
    pub hash: String,
    pub result: Result<Vec<Tuple>, Rejection>,
}

#[derive(Clone, Debug)]
pub struct Run {
    pub state: State,
    pub history: History,
    pub revision: u64,
    pub log: Vec<StepRecord>,
    pub last_work: u64,
    pub last_untracked: usize,
    build_work: Cell<u64>,
    maintained: RefCell<Option<Maintained>>,
    hash_sums: Option<(u64, u64)>,
}

impl PartialEq for Run {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state && self.history == other.history && self.revision == other.revision && self.log == other.log
    }
}

impl Eq for Run {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub program_identity: Option<u64>,
    state: State,
    history: History,
    revision: u64,
    log: Vec<StepRecord>,
}

impl Run {
    pub fn pending(&self) -> &[Tuple] {
        &self.state.pending
    }

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint { program_identity: None, state: self.state.clone(), history: self.history.clone(), revision: self.revision, log: self.log.clone() }
    }

    pub fn restore(checkpoint: Checkpoint) -> Self {
        Self { state: checkpoint.state, history: checkpoint.history, revision: checkpoint.revision, log: checkpoint.log, last_work: 0, last_untracked: 0, build_work: Cell::new(0), maintained: RefCell::new(None), hash_sums: None }
    }

    pub fn new(program: &Program) -> Self {
        let state = State::initial(program);
        let hash_sums = Some(state.hash_sums(program));
        Self { history: History::new(&state), state, revision: 0, log: Vec::new(), last_work: 0, last_untracked: 0, build_work: Cell::new(0), maintained: RefCell::new(None), hash_sums }
    }

    pub fn converted(state: State, revision: u64) -> Self {
        Self { history: History::from_revision(&state, revision), state, revision, log: Vec::new(), last_work: 0, last_untracked: 0, build_work: Cell::new(0), maintained: RefCell::new(None), hash_sums: None }
    }

    pub fn build_work(&self) -> u64 {
        self.build_work.get()
    }

    fn build(&self, program: &Program) -> Result<Maintained, EvalError> {
        let budget = if self.revision == 0 { Budget::default() } else { Budget::work(u64::MAX) };
        let layered = crate::claims::invariants(program).and_then(|invariants| {
            let mut maintained = Maintained::build(&invariants.program, &self.state.facts, &self.state.live, &invariants.layers, budget).ok()?;
            maintained.invariants = Some(invariants.decisions);
            Some(maintained)
        });
        let maintained = match layered {
            Some(maintained) => maintained,
            None => Maintained::build(program, &self.state.facts, &self.state.live, &[], budget)?,
        };
        self.build_work.set(maintained.work());
        Ok(maintained)
    }

    pub fn maintained(&self, program: &Program) -> Result<RefMut<'_, Maintained>, EvalError> {
        if self.maintained.borrow().is_none() {
            let maintained = self.build(program)?;
            *self.maintained.borrow_mut() = Some(maintained);
        }
        Ok(RefMut::map(self.maintained.borrow_mut(), |maintained| maintained.as_mut().expect("the model was built")))
    }

    pub fn run(&mut self, program: &Program, event: RelationId, arguments: &[Value], budget: Budget) -> Result<&StepRecord, EvalError> {
        self.run_all(program, &[(event, arguments.to_vec())], budget)
    }

    pub fn run_all(&mut self, program: &Program, events: &[Tuple], budget: Budget) -> Result<&StepRecord, EvalError> {
        let mut hash_sums = self.hash_sums.unwrap_or_else(|| self.state.hash_sums(program));
        let pending = self.state.pending.len();
        let mut undo = Vec::new();
        self.last_work = 0;
        self.last_untracked = 0;
        let mut meter = crate::expr::Meter { limit: budget.work, spent: 0 };
        let mut maintained = self.maintained.get_mut().take();
        let mut change = Change::default();
        let mut effects = BTreeSet::new();
        let mut result = Ok(Vec::new());
        for (event, arguments) in events {
            if maintained.is_none() {
                maintained = Some(self.build(program)?);
            }
            meter.spent = 0;
            let step_budget = StepBudget { ordinary: budget, meter: Some(RefCell::new(&mut meter)), evaluator: crate::engine::evaluate, mode_failures: None };
            let outcome = step_outcome(program, &self.state, Models::Maintained(maintained.as_mut().expect("the model was built")), *event, arguments, &step_budget, None);
            self.last_work = self.last_work.saturating_add(meter.spent.min(meter.limit));
            match outcome {
                Ok((mut outcome, mark)) => {
                    if mark.is_some() { self.last_untracked += maintained.as_ref().map_or(0, Maintained::untracked_rows); }
                    outcome.change.requested.retain(|(relation, values)| !effects.contains(&(program.relations[relation.0].name.clone(), values.clone(), *relation)));
                    change.extend(outcome.change.clone(), pending);
                    undo.push((self.state.apply(outcome.change, outcome.next), mark));
                    effects.extend(outcome.effects.into_iter().map(|(relation, values)| (program.relations[relation.0].name.clone(), values, relation)));
                }
                Err(StepError::Rejected(rejection)) => {
                    result = Err(rejection);
                    break;
                }
                Err(StepError::Eval(error)) => {
                    self.undo(undo, &mut maintained);
                    if let Some(maintained) = maintained.as_mut() { maintained.commit(); }
                    *self.maintained.get_mut() = maintained;
                    return Err(error);
                }
            }
        }
        if result.is_ok() {
            for fact in &change.withdrawn { hash_sums.0 = hash_sums.0.wrapping_sub(fact_hash(program, fact)); }
            for fact in &change.added { hash_sums.0 = hash_sums.0.wrapping_add(fact_hash(program, fact)); }
            for entity in &change.deleted { hash_sums.1 = hash_sums.1.wrapping_sub(entity_hash(entity)); }
            for entity in &change.created { hash_sums.1 = hash_sums.1.wrapping_add(entity_hash(entity)); }
            self.revision += 1;
            self.history.append(change, self.state.next);
            result = Ok(effects.into_iter().map(|(_, values, relation)| (relation, values)).collect());
        } else {
            self.undo(undo, &mut maintained);
        }
        if let Some(maintained) = maintained.as_mut() { maintained.commit(); }
        *self.maintained.get_mut() = maintained;
        self.hash_sums = Some(hash_sums);
        self.log.push(StepRecord { events: events.to_vec(), revision: self.revision, hash: self.state.hash_from_sums(program, hash_sums), result });
        Ok(self.log.last().expect("the occurrence was recorded"))
    }

    pub fn replay(program: &Program, log: &[StepRecord], budget: Budget) -> Result<Self, EvalError> {
        let mut run = Self::new(program);
        for (index, recorded) in log.iter().enumerate() {
            if run.run_all(program, &recorded.events, budget)? != recorded {
                return Err(EvalError::Type { at: format!("event log occurrence {}", index + 1), message: "The replay differs from the recorded admission, effects, revision or state hash.".into() });
            }
        }
        *run.maintained.get_mut() = None;
        Ok(run)
    }

    fn undo(&mut self, undo: Vec<(StepUndo, Option<Mark>)>, maintained: &mut Option<Maintained>) {
        for (occurrence, mark) in undo.into_iter().rev() {
            self.state.undo(occurrence);
            if let (Some(maintained), Some(mark)) = (maintained.as_mut(), mark) { maintained.undo(mark); }
        }
    }
}

enum Models<'m> {
    Reference(&'m Model),
    Maintained(&'m mut Maintained),
}

impl Models<'_> {
    fn matches(&mut self, program: &Program, body: &[Literal], binding: &Binding, budget: &StepBudget<'_>) -> Result<Vec<Binding>, EvalError> {
        match self {
            Models::Reference(model) => budget.matches(program, model, body, binding),
            Models::Maintained(maintained) => {
                let mut meter = budget.meter.as_ref().expect("a run step has a meter").borrow_mut();
                maintained.reader(program, &mut **meter, "handler clause").matches(body, binding)
            }
        }
    }

    fn value(&mut self, program: &Program, expression: &Expr, binding: &Binding, budget: &StepBudget<'_>) -> Result<Value, EvalError> {
        match self {
            Models::Reference(model) => budget.value(program, model, expression, binding),
            Models::Maintained(maintained) => {
                let mut meter = budget.meter.as_ref().expect("a run step has a meter").borrow_mut();
                maintained.reader(program, &mut **meter, "handler action").value(expression, binding)
            }
        }
    }

    fn contains(&self, relation: RelationId, tuple: &[Value]) -> bool {
        match self {
            Models::Reference(model) => model.contains(relation, tuple),
            Models::Maintained(maintained) => maintained.presence(relation).contains(tuple),
        }
    }
}

pub(crate) enum StepError {
    Rejected(Rejection),
    Eval(EvalError),
}

impl From<EvalError> for StepError {
    fn from(error: EvalError) -> Self {
        Self::Eval(error)
    }
}

pub fn step(
    program: &Program,
    state: &State,
    event: RelationId,
    arguments: &[Value],
    budget: Budget,
) -> Result<Transition, EvalError> {
    step_reference(program, state, event, arguments, budget, None, None).map_err(|error| match error {
        StepError::Eval(error) => error,
        StepError::Rejected(rejection) => EvalError::Type { at: "step".into(), message: rejection.explanation.join("\n") },
    })
}

pub(crate) fn step_causes(
    program: &Program,
    state: &State,
    event: RelationId,
    arguments: &[Value],
    meter: &mut crate::expr::Meter,
) -> Result<BTreeMap<Tuple, (HandlerClause, Binding)>, EvalError> {
    meter.charge("explaining an occurrence")?;
    let mut causes = BTreeMap::new();
    let mut work = 0;
    step_reference(program, state, event, arguments, Budget::work(meter.limit.saturating_sub(meter.spent)), Some((&mut causes, &mut work)), None)
        .map_err(|error| match error {
            StepError::Eval(error) => error,
            StepError::Rejected(rejection) => EvalError::Type { at: "chain of title".into(), message: rejection.explanation.join("\n") },
        })?;
    meter.spent += work;
    meter.charge("explaining an occurrence")?;
    Ok(causes)
}

pub(crate) fn step_search(program: &Program, state: &State, event: RelationId, arguments: &[Value], meter: &mut crate::expr::Meter) -> Result<Transition, StepError> {
    step_reference(program, state, event, arguments, Budget::work(meter.limit.saturating_sub(meter.spent)), None, Some(meter))
}

struct StepBudget<'a> {
    ordinary: Budget,
    meter: Option<RefCell<&'a mut crate::expr::Meter>>,
    evaluator: fn(&Program, &[Fact], Budget) -> Result<Model, EvalError>,
    mode_failures: Option<RefCell<Vec<crate::modes::ModeFailure>>>,
}

impl StepBudget<'_> {
    fn evaluate(&self, program: &Program, facts: &[Fact]) -> Result<Model, EvalError> {
        let Some(meter) = &self.meter else { return (self.evaluator)(program, facts, self.ordinary); };
        let mut meter = meter.borrow_mut();
        let outcome = (self.evaluator)(program, facts, Budget::work(meter.limit.saturating_sub(meter.spent)));
        match &outcome {
            Ok(model) => meter.spent += model.work,
            Err(EvalError::Unknown(Reason::OutOfBudget { .. })) => meter.spent = meter.limit,
            _ => {}
        }
        outcome
    }

    fn matches(&self, program: &Program, model: &Model, body: &[Literal], binding: &Binding) -> Result<Vec<Binding>, EvalError> {
        match &self.meter {
            Some(meter) => crate::eval::matches_metered(program, model, body, binding, &mut meter.borrow_mut()),
            None => crate::eval::matches(program, model, body, binding, self.ordinary),
        }
    }

    fn value(&self, program: &Program, model: &Model, expression: &Expr, binding: &Binding) -> Result<Value, EvalError> {
        match &self.meter {
            Some(meter) => crate::eval::value_metered(program, model, expression, binding, &mut meter.borrow_mut()),
            None => crate::eval::value(program, model, expression, binding, self.ordinary),
        }
    }

    fn modes<'a>(
        &self,
        program: &Program,
        strata: &[usize],
        entities: impl Iterator<Item = &'a Entity> + Clone,
        rows: &dyn Fn(RelationId, &mut dyn FnMut(&[Value])),
    ) -> Vec<crate::modes::ModeFailure> {
        let failures = crate::modes::check_stratum(program, strata, 0, RelationKind::State, entities, rows);
        if let Some(sink) = &self.mode_failures { *sink.borrow_mut() = failures.clone(); }
        failures
    }

    fn claims(&self, program: &Program) -> Result<Vec<ClaimResult>, ClaimError> {
        let Some(meter) = &self.meter else { return check_claims(program, self.ordinary); };
        let mut meter = meter.borrow_mut();
        let (results, work) = crate::claims::check_claims_spending(program, Budget::work(meter.limit.saturating_sub(meter.spent)))?;
        meter.spent += work;
        Ok(results)
    }
}

fn argument_domain(program: &Program, event: RelationId, arguments: &[Value]) -> Result<(), StepError> {
    if let Some(declaration) = program.events.iter().find(|declaration| declaration.relation == event) {
        for (&index, domain) in &declaration.domains {
            if let ArgumentDomain::Only(values) = domain {
                if arguments.get(index).is_none_or(|argument| !values.contains(argument)) {
                    return Err(StepError::Rejected(Rejection {
                        kind: RejectionKind::ArgumentDomain,
                        explanation: vec![format!("{} is outside the only in domain of {}.", crate::why::sentence(program, event, arguments), program.relations[event.0].slots[index].name)],
                    }));
                }
            }
        }
    }
    Ok(())
}

fn state_model(program: &Program, state: &State, budget: &StepBudget<'_>) -> Result<Model, EvalError> {
    let state_types: BTreeSet<&str> = program.entity_types.iter().filter(|t| t.state).map(|t| t.name.as_str()).collect();
    let mut program = program.clone();
    program.entities.retain(|e| !state_types.contains(e.entity_type.as_str()));
    program.entities.extend(state.live.iter().cloned());
    let facts: Vec<Fact> = state.facts.iter().map(|(relation, values)| Fact::new(*relation, values.clone())).collect();
    budget.evaluate(&program, &facts)
}

fn step_reference(
    program: &Program,
    state: &State,
    event: RelationId,
    arguments: &[Value],
    budget: Budget,
    mut causes: Option<(&mut BTreeMap<Tuple, (HandlerClause, Binding)>, &mut u64)>,
    meter: Option<&mut crate::expr::Meter>,
) -> Result<Transition, StepError> {
    argument_domain(program, event, arguments)?;
    let budget = StepBudget { ordinary: budget, meter: meter.map(RefCell::new), evaluator: evaluate, mode_failures: None };
    let model = state_model(program, state, &budget)?;
    if let Some((_, work)) = &mut causes { **work += model.work; }
    step_outcome(program, state, Models::Reference(&model), event, arguments, &budget, causes).map(|(outcome, _)| {
        let mut state = state.clone();
        state.apply(outcome.change.clone(), outcome.next);
        Transition { state, change: outcome.change, effects: outcome.effects, substitutions: outcome.substitutions }
    })
}

fn step_outcome(
    program: &Program,
    state: &State,
    mut models: Models<'_>,
    event: RelationId,
    arguments: &[Value],
    budget: &StepBudget<'_>,
    mut causes: Option<(&mut BTreeMap<Tuple, (HandlerClause, Binding)>, &mut u64)>,
) -> Result<(StepOutcome, Option<Mark>), StepError> {
    let maintained = matches!(models, Models::Maintained(_));
    argument_domain(program, event, arguments)?;
    let state_types: BTreeSet<&str> = program.entity_types.iter().filter(|t| t.state).map(|t| t.name.as_str()).collect();

    let mut matched_ensures = Vec::new();
    let mut may_skip = false;
    let mut matched: BTreeMap<(String, Binding), &HandlerClause> = BTreeMap::new();
    for handler in program.handlers.iter().filter(|h| h.event.relation == event) {
        may_skip |= handler.may_skip;
        let Some(start) = bind_event(program, &mut models, &handler.event, arguments, budget)? else {
            continue;
        };
        for clause in &handler.clauses {
            let text = clause_text(program, clause);
            let bindings = models.matches(program, &clause.body, &start, budget)?;
            for binding in bindings {
                matched_ensures.push((text.clone(), binding.clone(), clause));
                matched.insert((text.clone(), binding), clause);
            }
        }
    }

    if matched.is_empty() && !may_skip {
        let sentence = crate::why::sentence(program, event, arguments);
        if budget.meter.is_some() && !maintained {
            return Err(StepError::Rejected(Rejection { kind: RejectionKind::NoMatch, explanation: vec![format!("{sentence} has no matching clause.")] }));
        }
        let mut explanation = (*program).clone();
        explanation.entities.retain(|e| !state_types.contains(e.entity_type.as_str()));
        explanation.entities.extend(state.live.iter().cloned());
        let occurrence = RelationId(explanation.relations.len());
        let mut relation = program.relations[event.0].clone();
        if relation.template.is_empty() {
            relation.template = format!("{}({})", relation.name, relation.slots.iter().map(|s| format!("{{{}}}", s.name)).collect::<Vec<_>>().join(", "));
        }
        relation.name = "step occurrence".into();
        relation.kind = RelationKind::Static;
        explanation.relations.push(relation.clone());
        explanation.facts.push(Fact::new(occurrence, arguments.iter().cloned()));
        let goal = RelationId(explanation.relations.len());
        relation.name = "step match".into();
        relation.kind = RelationKind::Derived;
        explanation.relations.push(relation);
        for handler in program.handlers.iter().filter(|h| h.event.relation == event) {
            for clause in &handler.clauses {
                let mut event_atom = handler.event.clone();
                event_atom.relation = occurrence;
                let body = std::iter::once(event_atom.positive()).chain(clause.body.iter().cloned());
                let head = Atom::new(goal, arguments.iter().cloned().map(Expr::Value));
                let name = format!("clause {}", explanation.rules.len() - program.rules.len() + 1);
                explanation.rules.push(Rule::new(name, head).when(body));
            }
        }
        let facts: Vec<Fact> = state.facts.iter().map(|(relation, values)| Fact::new(*relation, values.clone())).collect();
        let verdict = crate::why_not::why_not(&explanation, &facts, goal, arguments, None, budget.ordinary)?;
        return Err(StepError::Rejected(Rejection {
            kind: RejectionKind::NoMatch,
            explanation: format!("{sentence} has no matching clause.\n{}", crate::why_not::render(&explanation, goal, arguments, None, &verdict, budget.ordinary)).split('\n').map(str::to_string).collect(),
        }));
    }

    let mut requests = Vec::new();
    for ((text, binding), clause) in &matched {
        for action in &clause.actions {
            if let Action::Create { variable, entity_type, fields } = action {
                let declared = program.entity_types.iter().find(|t| &t.name == entity_type);
                let mut values = BTreeMap::new();
                for (name, expression) in fields {
                    values.insert(name.clone(), models.value(program, expression, binding, budget)?);
                }
                let ordered: Vec<Value> = match declared {
                    Some(t) => t.fields.iter().map(|f| values.get(&f.name).cloned().unwrap_or(Value::Null)).collect(),
                    None => values.values().cloned().collect(),
                };
                requests.push(((entity_type.clone(), ordered, text.clone(), binding.clone(), variable.clone()), values));
            }
        }
    }
    requests.sort_by(|a, b| a.0.cmp(&b.0));

    let mut created: BTreeMap<Entity, BTreeMap<String, Value>> = BTreeMap::new();
    let mut extended: BTreeMap<(String, Binding), Binding> = BTreeMap::new();
    for (offset, ((entity_type, _, text, binding, variable), values)) in requests.into_iter().enumerate() {
        let id = state.next + offset as u64;
        let entity = Entity::created(entity_type.clone(), format!("{entity_type}#{id}"), id);
        extended
            .entry((text, binding.clone()))
            .or_insert(binding)
            .insert(variable, Value::Entity(entity.clone()));
        created.insert(entity, values);
    }

    let mut withdrawn: BTreeSet<Tuple> = BTreeSet::new();
    let mut included: BTreeSet<Tuple> = BTreeSet::new();
    let mut deleted: BTreeSet<Entity> = BTreeSet::new();
    let mut effects: BTreeSet<(String, Vec<Value>, RelationId)> = BTreeSet::new();
    for (key, clause) in &matched {
        let binding = extended.get(key).unwrap_or(&key.1);
        for action in &clause.actions {
            match action {
                Action::Set(atom) => {
                    let tuple = ground(program, &mut models, atom, binding, budget)?;
                    let keys: &[usize] = match program.relations[atom.relation.0].modes.as_slice() {
                        [Mode::ExactlyOne { key, .. }] => std::slice::from_ref(key),
                        [Mode::AtMostOne { keys, .. }] => keys,
                        _ => &[],
                    };
                    if !keys.is_empty() {
                        withdrawn.extend(
                            state
                                .facts
                                .range((atom.relation, Vec::new())..)
                                .take_while(|(relation, _)| *relation == atom.relation)
                                .filter(|(_, values)| keys.iter().all(|&k| values[k] == tuple[k]))
                                .cloned(),
                        );
                    }
                    let fact = (atom.relation, tuple);
                    if let Some((causes, _)) = &mut causes { causes.entry(fact.clone()).or_insert_with(|| ((*clause).clone(), key.1.clone())); }
                    included.insert(fact);
                }
                Action::Include(atom) => {
                    let fact = (atom.relation, ground(program, &mut models, atom, binding, budget)?);
                    if let Some((causes, _)) = &mut causes { causes.entry(fact.clone()).or_insert_with(|| ((*clause).clone(), key.1.clone())); }
                    included.insert(fact);
                }
                Action::Withdraw(atom) => {
                    let fact = (atom.relation, ground(program, &mut models, atom, binding, budget)?);
                    if let Some((causes, _)) = &mut causes { causes.entry(fact.clone()).or_insert_with(|| ((*clause).clone(), key.1.clone())); }
                    withdrawn.insert(fact);
                }
                Action::Delete(expression) => match models.value(program, expression, binding, budget)? {
                    Value::Entity(entity) => {
                        if let Some((causes, _)) = &mut causes {
                            let removed = BTreeSet::from([entity.clone()]);
                            for fact in &state.facts {
                                if fact.1.iter().any(|value| mentions(value, &removed)) { causes.entry(fact.clone()).or_insert_with(|| ((*clause).clone(), key.1.clone())); }
                            }
                        }
                        deleted.insert(entity);
                    }
                    _ => {
                        return Err(EvalError::Type {
                            at: "handler action".into(),
                            message: "a deletion needs an entity".into(),
                        }.into())
                    }
                },
                Action::Emit(atom) => {
                    let tuple = ground(program, &mut models, atom, binding, budget)?;
                    effects.insert((program.relations[atom.relation.0].name.clone(), tuple, atom.relation));
                }
                Action::Create { variable, .. } => {
                    if let (Some((causes, _)), Some(Value::Entity(entity))) = (&mut causes, binding.get(variable)) {
                        for (field, value) in &created[entity] {
                            if let Some(relation) = program.field_relation(&entity.entity_type, field) {
                                causes.entry((relation, vec![Value::Entity(entity.clone()), value.clone()])).or_insert_with(|| ((*clause).clone(), key.1.clone()));
                            }
                        }
                    }
                }
            }
        }
    }

    let mut change = Change::default();
    change.withdrawn.extend(withdrawn.intersection(&state.facts).filter(|fact| !included.contains(*fact)).cloned());
    if !deleted.is_empty() {
        change.withdrawn.extend(state.facts.iter().filter(|fact| !included.contains(*fact) && fact.1.iter().any(|v| mentions(v, &deleted))).cloned());
    }
    for (entity, values) in &created {
        for (field, value) in values {
            if let Some(relation) = program.field_relation(&entity.entity_type, field) {
                if value != &Value::Null
                    || !matches!(program.relations[relation.0].modes.as_slice(), [Mode::AtMostOne { .. }])
                {
                    let fact = (relation, vec![Value::Entity(entity.clone()), value.clone()]);
                    change.added.insert(fact);
                }
            }
        }
    }
    change.added.extend(included.iter().filter(|fact| !state.facts.contains(*fact)).cloned());
    change.created.extend(created.keys().cloned());
    change.deleted.extend(deleted.intersection(&state.live).cloned());
    let facts = || state.facts.iter().filter(|fact| !change.withdrawn.contains(*fact)).chain(change.added.iter());
    let relation_facts = |relation| state.facts.range((relation, Vec::new())..).take_while(move |(id, _)| *id == relation).filter(|fact| !change.withdrawn.contains(*fact))
        .chain(change.added.range((relation, Vec::new())..).take_while(move |(id, _)| *id == relation));
    let post_live = if created.is_empty() && deleted.is_empty() { None } else {
        let mut live: BTreeSet<Entity> = state.live.difference(&deleted).cloned().collect();
        live.extend(created.keys().cloned());
        Some(live)
    };
    let live = post_live.as_ref().unwrap_or(&state.live);
    for (relation, tuple) in &included {
        let mut mentioned = BTreeSet::new();
        let mut pending: Vec<&Value> = tuple.iter().collect();
        while let Some(value) = pending.pop() {
            match value {
                Value::Entity(entity) if state_types.contains(entity.entity_type.as_str()) && !live.contains(entity) => {
                    mentioned.insert(entity);
                }
                Value::List(items) => pending.extend(items),
                Value::Record(items) | Value::Attrs(items) => pending.extend(items.values()),
                _ => {}
            }
        }
        if let Some(entity) = mentioned.first() {
            return Err(StepError::Rejected(Rejection {
                kind: RejectionKind::NonLiveInclude,
                explanation: vec![format!("{} mentions {}, which is not live in the post-state.", crate::why::sentence(program, *relation, tuple), entity.name)],
            }));
        }
    }
    let changed: BTreeSet<RelationId> = withdrawn.iter().chain(&included).chain(&change.added).chain(&change.withdrawn).map(|(relation, _)| *relation).collect();
    let entities = || program.entities.iter().filter(|e| !state_types.contains(e.entity_type.as_str())).chain(live.iter());
    let state_strata: Vec<usize> = program.relations.iter().enumerate().map(|(index, relation)| {
        let new_key = relation.modes.iter().any(|mode| match mode {
            Mode::ExactlyOne { key, .. } => match &relation.slots[*key].ty {
                Type::Entity(name) => created.keys().chain(&deleted).any(|entity| &entity.entity_type == name),
                _ => false,
            },
            _ => false,
        });
        if changed.contains(&RelationId(index)) || new_key { 0 } else { usize::MAX }
    }).collect();
    let failures = if maintained {
        touched_modes(program, state, &change, &|entity| if state_types.contains(entity.entity_type.as_str()) { live.contains(entity) } else { program.entities.contains(entity) }, budget)?
    } else {
        budget.modes(program, &state_strata, entities(), &|relation, each| relation_facts(relation).for_each(|(_, tuple)| each(tuple)))
    };
    if let Some(failure) = failures.first() {
        return Err(StepError::Rejected(Rejection {
            kind: if failure.tuples.is_empty() { RejectionKind::UnfilledExactlyOne } else { RejectionKind::ConflictingWrite },
            explanation: vec![format!("{}.", crate::modes::describe(program, failure))],
        }));
    }
    let mut early = Vec::new();
    if maintained {
        for (text, original, clause) in &matched_ensures {
            let binding = extended.get(&(text.clone(), original.clone())).unwrap_or(original);
            for claim in &clause.ensures {
                let (before, claim) = innermost(claim);
                early.push(if before { Some(decide(program, &mut models, claim, binding, budget)) } else { None });
            }
        }
    }
    let (post_model, mark) = match &mut models {
        Models::Reference(_) => {
            let mut post_program = program.clone();
            post_program.entities = entities().cloned().collect();
            post_program.initially = facts().map(|(relation, values)| Fact::new(*relation, values.clone())).collect();
            let post_model = match budget.evaluate(&post_program, &post_program.initially) {
                Err(EvalError::Mode(failures)) => return Err(derived_mode(program, &failures)),
                other => other?,
            };
            if let Some((_, work)) = &mut causes { **work += post_model.work; }
            (Some(post_model), None)
        }
        Models::Maintained(model) => {
            let mut meter = budget.meter.as_ref().expect("a run step has a meter").borrow_mut();
            match model.advance(&change, Budget::work(meter.limit.saturating_sub(meter.spent))) {
                Ok((_, mark)) => {
                    meter.spent += model.work();
                    (None, Some(mark))
                }
                Err(EvalError::Mode(failures)) => return Err(derived_mode(program, &failures)),
                Err(error) => {
                    if matches!(error, EvalError::Unknown(Reason::OutOfBudget { .. })) { meter.spent = meter.limit; }
                    return Err(error.into());
                }
            }
        }
    };
    let pre = match &models { Models::Reference(model) => Some(*model), Models::Maintained(_) => None };
    let mut post = match (&post_model, models) {
        (Some(post_model), _) => Models::Reference(post_model),
        (None, models) => models,
    };
    let verdict = (|| -> Result<(), StepError> {
        if let Models::Maintained(model) = &post {
            if let Some(decisions) = &model.invariants {
                for (requirement, relation, holds_when_empty) in decisions {
                    if model.presence(*relation).is_empty() != *holds_when_empty {
                        return Err(StepError::Rejected(Rejection {
                            kind: RejectionKind::Invariant,
                            explanation: vec![format!("The invariant `{requirement}` fails in the post-state.")],
                        }));
                    }
                }
            }
        }
        let maintained_invariants = matches!(&post, Models::Maintained(model) if model.invariants.is_some());
        let invariants = if maintained_invariants || !program.requirements.iter().any(|r| r.invariant) { Vec::new() } else {
            let mut post_program = program.clone();
            post_program.entities = entities().cloned().collect();
            post_program.requirements.retain(|r| r.invariant);
            post_program.initially = facts().map(|(relation, values)| Fact::new(*relation, values.clone())).collect();
            budget.claims(&post_program).map_err(|error| match error {
                ClaimError::Eval(error) => error,
                other => EvalError::Type { at: "step".into(), message: other.to_string() },
            })?
        };
        for result in invariants {
            match result.verdict {
                Verdict::Holds(_) => {}
                Verdict::Unknown(reason) => return Err(EvalError::Unknown(reason).into()),
                Verdict::Fails(_) => return Err(StepError::Rejected(Rejection {
                    kind: RejectionKind::Invariant,
                    explanation: vec![format!("The invariant `{}` fails in the post-state.", result.requirement)],
                })),
            }
        }
        let mut early = early.into_iter();
        for (text, original, clause) in &matched_ensures {
            let binding = extended.get(&(text.clone(), original.clone())).unwrap_or(original);
            for claim in &clause.ensures {
                let (before, inner) = innermost(claim);
                let (holds, sentence) = match early.next().flatten() {
                    Some(decided) => decided?,
                    None => match (before, pre) {
                        (true, Some(model)) => decide(program, &mut Models::Reference(model), inner, binding, budget)?,
                        _ => decide(program, &mut post, inner, binding, budget)?,
                    },
                };
                if !holds {
                    return Err(StepError::Rejected(Rejection {
                        kind: RejectionKind::Ensures,
                        explanation: vec![format!("The ensures claim `{sentence}` fails.")],
                    }));
                }
            }
        }
        for effect in program.effects.iter().filter(|e| e.at_most_one_per_step) {
            let requests: Vec<_> = effects.iter().filter(|(_, _, relation)| *relation == effect.relation).collect();
            if requests.len() > 1 {
                return Err(StepError::Rejected(Rejection {
                    kind: RejectionKind::CompetingEffects,
                    explanation: vec![format!("The effect `{}` allows at most one request per step: {} and {} compete.", program.relations[effect.relation.0].name, crate::why::sentence(program, effect.relation, &requests[0].1), crate::why::sentence(program, effect.relation, &requests[1].1))],
                }));
            }
        }
        Ok(())
    })();
    if let Err(error) = verdict {
        if let (Models::Maintained(model), Some(mark)) = (post, mark) { model.undo(mark); }
        return Err(error);
    }
    if let Some((causes, _)) = causes { causes.retain(|fact, _| change.added.contains(fact) || change.withdrawn.contains(fact)); }
    if let Some(index) = state.pending.iter().position(|(relation, values)| {
        program.effects.iter().any(|effect| effect.relation == *relation && effect.settled_by == Some(event))
            && program.relations[relation.0].slots.iter().zip(values).all(|(slot, value)| {
                program.relations[event.0].slots.iter().position(|receipt| receipt.name == slot.name)
                    .is_none_or(|index| arguments.get(index) == Some(value))
            })
    }) {
        change.settled.insert(index);
    }
    let effects: Vec<Tuple> = effects.into_iter().map(|(_, values, relation)| (relation, values)).collect();
    change.requested.extend(effects.iter().filter(|(relation, _)| program.effects.iter().any(|effect| effect.relation == *relation && effect.settled_by.is_some())).cloned());
    let next = state.next + created.len() as u64;
    Ok((StepOutcome {
        next,
        change,
        effects,
        substitutions: matched.len(),
    }, mark))
}

pub fn step_mode_failures(program: &Program, state: &State, event: RelationId, arguments: &[Value], budget: Budget) -> Result<Vec<crate::modes::ModeFailure>, EvalError> {
    let budget = StepBudget { ordinary: budget, meter: None, evaluator: evaluate, mode_failures: Some(RefCell::new(Vec::new())) };
    let model = state_model(program, state, &budget)?;
    match step_outcome(program, state, Models::Reference(&model), event, arguments, &budget, None) {
        Err(StepError::Eval(error)) => Err(error),
        _ => Ok(budget.mode_failures.map(RefCell::into_inner).unwrap_or_default()),
    }
}

fn touched_modes(program: &Program, state: &State, change: &Change, known: &dyn Fn(&Entity) -> bool, budget: &StepBudget<'_>) -> Result<Vec<crate::modes::ModeFailure>, EvalError> {
    let charge = || match &budget.meter { Some(meter) => meter.borrow_mut().charge("mode tuples"), None => Ok(()) };
    let mut failures = Vec::new();
    for (index, relation) in program.relations.iter().enumerate() {
        if relation.kind != RelationKind::State { continue; }
        let id = RelationId(index);
        fn in_relation(facts: &BTreeSet<Tuple>, id: RelationId) -> impl Iterator<Item = &Vec<Value>> { facts.range((id, Vec::new())..).take_while(move |(other, _)| *other == id).map(|(_, tuple)| tuple) }
        for mode in &relation.modes {
            let (keys, values, total) = match mode {
                Mode::ExactlyOne { key, values } => (std::slice::from_ref(key), values, true),
                Mode::AtMostOne { keys, values } => (keys.as_slice(), values, false),
            };
            let key_of = |tuple: &[Value]| -> Vec<Value> { keys.iter().map(|&slot| tuple[slot].clone()).collect() };
            let mut touched: BTreeSet<Vec<Value>> = in_relation(&change.withdrawn, id).chain(in_relation(&change.added, id)).map(|tuple| key_of(tuple)).collect();
            let entity_type = match &relation.slots[keys[0]].ty { Type::Entity(name) if total => Some(name), _ => None };
            if let Some(name) = entity_type {
                touched.extend(change.created.iter().filter(|entity| &entity.entity_type == name).map(|entity| vec![Value::Entity(entity.clone())]));
            }
            let prefix = keys.iter().enumerate().all(|(at, &slot)| at == slot);
            for key in touched {
                let mut tuples: BTreeSet<Vec<Value>> = BTreeSet::new();
                let mut visit = |tuple: &Vec<Value>| -> Result<(), EvalError> {
                    charge()?;
                    if key_of(tuple) == key { tuples.insert(tuple.clone()); }
                    Ok(())
                };
                for facts in [&state.facts, &change.added] {
                    let start = (id, if prefix { key.clone() } else { Vec::new() });
                    for fact in facts.range(start..).take_while(|(other, tuple)| *other == id && (!prefix || tuple.starts_with(&key))) {
                        if std::ptr::eq(facts, &state.facts) && change.withdrawn.contains(fact) { continue; }
                        visit(&fact.1)?;
                    }
                }
                let required = match (entity_type, key.as_slice()) {
                    (Some(name), [Value::Entity(entity)]) => &entity.entity_type == name && known(entity),
                    _ => false,
                };
                if tuples.is_empty() && !required { continue; }
                failures.extend(crate::modes::check_key(id, key, values, total, tuples.iter()));
            }
        }
    }
    Ok(failures)
}

fn derived_mode(program: &Program, failures: &[crate::modes::ModeFailure]) -> StepError {
    StepError::Rejected(Rejection {
        kind: RejectionKind::DerivedMode,
        explanation: vec![format!("{}.", crate::modes::describe(program, &failures[0]))],
    })
}

fn innermost(mut claim: &StateClaim) -> (bool, &StateClaim) {
    let mut before = false;
    loop {
        match claim {
            StateClaim::Before(inner) => { before = true; claim = inner; }
            _ => return (before, claim),
        }
    }
}

fn decide(program: &Program, models: &mut Models<'_>, claim: &StateClaim, binding: &Binding, budget: &StepBudget<'_>) -> Result<(bool, String), EvalError> {
    Ok(match claim {
        StateClaim::Holds(atom) => {
            let tuple = ground(program, models, atom, binding, budget)?;
            (models.contains(atom.relation, &tuple), crate::why::sentence(program, atom.relation, &tuple))
        }
        StateClaim::Some { body, .. } | StateClaim::No { body, .. } => {
            let found = !models.matches(program, body, binding, budget)?.is_empty();
            (if matches!(claim, StateClaim::No { .. }) { !found } else { found }, body.iter().map(|l| render_literal(program, l)).collect::<Vec<_>>().join(", "))
        }
        StateClaim::Every { domain, body, .. } => {
            let mut holds = true;
            for instance in models.matches(program, domain, binding, budget)? {
                if models.matches(program, body, &instance, budget)?.is_empty() { holds = false; break; }
            }
            (holds, body.iter().map(|l| render_literal(program, l)).collect::<Vec<_>>().join(", "))
        }
        StateClaim::Before(_) => unreachable!(),
    })
}

fn bind_event(
    program: &Program,
    models: &mut Models<'_>,
    event: &Atom,
    arguments: &[Value],
    budget: &StepBudget<'_>,
) -> Result<Option<Binding>, EvalError> {
    let mut binding = Binding::new();
    for (argument, given) in event.arguments.iter().zip(arguments) {
        match argument {
            Expr::Variable(variable) => match binding.get(variable) {
                Some(bound) if bound != given => return Ok(None),
                _ => {
                    binding.insert(variable.clone(), given.clone());
                }
            },
            other => {
                if models.value(program, other, &binding, budget)? != *given {
                    return Ok(None);
                }
            }
        }
    }
    Ok(Some(binding))
}

fn ground(program: &Program, models: &mut Models<'_>, atom: &Atom, binding: &Binding, budget: &StepBudget<'_>) -> Result<Vec<Value>, EvalError> {
    atom.arguments
        .iter()
        .map(|argument| models.value(program, argument, binding, budget))
        .collect()
}

fn mentions(value: &Value, entities: &BTreeSet<Entity>) -> bool {
    match value {
        Value::Entity(entity) => entities.contains(entity),
        Value::List(items) => items.iter().any(|v| mentions(v, entities)),
        Value::Record(items) | Value::Attrs(items) => items.values().any(|v| mentions(v, entities)),
        _ => false,
    }
}

pub(crate) fn clause_text(program: &Program, clause: &HandlerClause) -> String {
    let (body, written) = crate::check::inline_hoisted(program, &clause.body);
    let mut lines: Vec<String> = body.into_iter().map(|l| format!("when {l}")).collect();
    for action in &clause.actions {
        lines.push(match action {
            Action::Set(atom) | Action::Include(atom) | Action::Emit(atom) => format!("then {}", render_atom(program, atom)),
            Action::Withdraw(atom) => format!("then not {}", render_atom(program, atom)),
            Action::Create { variable, entity_type, fields } => {
                let values: Vec<String> = fields.iter().map(|(n, e)| format!("{n}: {}", render_expr(program, e))).collect();
                format!("then ?{} is a new {entity_type} {{{}}}", variable.0, values.join(", "))
            }
            Action::Delete(expression) => format!("then not {} is deleted", render_expr(program, expression)),
        });
    }
    for line in &mut lines {
        for (name, aggregate) in written.iter().rev() {
            *line = line.replace(name.as_str(), aggregate);
        }
    }
    lines.sort();
    lines.join("\n")
}
