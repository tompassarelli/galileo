use crate::check::{render_atom, render_expr, render_literal};
use std::cell::RefCell;
use crate::expr::Binding;
use crate::history::{Change, History};
use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

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
    model: Option<Model>,
    rule_reads: Option<BTreeSet<RelationId>>,
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
        Self { state: checkpoint.state, history: checkpoint.history, revision: checkpoint.revision, log: checkpoint.log, model: None, rule_reads: None, hash_sums: None }
    }

    pub fn new(program: &Program) -> Self {
        let state = State::initial(program);
        let hash_sums = Some(state.hash_sums(program));
        Self { history: History::new(&state), state, revision: 0, log: Vec::new(), model: None, rule_reads: None, hash_sums }
    }

    pub fn run(&mut self, program: &Program, event: RelationId, arguments: &[Value], budget: Budget) -> Result<&StepRecord, EvalError> {
        self.run_all(program, &[(event, arguments.to_vec())], budget)
    }

    pub fn run_all(&mut self, program: &Program, events: &[Tuple], budget: Budget) -> Result<&StepRecord, EvalError> {
        let mut hash_sums = self.hash_sums.unwrap_or_else(|| self.state.hash_sums(program));
        let pending = self.state.pending.len();
        let mut undo = Vec::new();
        let step_budget = StepBudget { ordinary: budget, meter: None, evaluator: crate::engine::evaluate };
        let mut model = self.model.clone();
        let mut rule_reads = self.rule_reads.clone();
        let mut change = Change::default();
        let mut effects = BTreeSet::new();
        let mut result = Ok(Vec::new());
        for (event, arguments) in events {
            if model.is_none() {
                model = Some(state_model(program, &self.state, &StepBudget { ordinary: Budget::default(), meter: None, evaluator: crate::engine::evaluate })?);
                rule_reads = Some(check(program).map_err(EvalError::Static)?.dependencies.into_iter().map(|dependency| dependency.body).collect());
            }
            match step_outcome(program, &self.state, model.as_ref().expect("the pre-state model was evaluated"), *event, arguments, &step_budget, rule_reads.as_ref(), None) {
                Ok((mut outcome, post_model)) => {
                    model = Some(post_model);
                    outcome.change.requested.retain(|(relation, values)| !effects.contains(&(program.relations[relation.0].name.clone(), values.clone(), *relation)));
                    change.extend(outcome.change.clone(), pending);
                    undo.push(self.state.apply(outcome.change, outcome.next));
                    effects.extend(outcome.effects.into_iter().map(|(relation, values)| (program.relations[relation.0].name.clone(), values, relation)));
                }
                Err(StepError::Rejected(rejection)) => {
                    result = Err(rejection);
                    break;
                }
                Err(StepError::Eval(error)) => {
                    for occurrence in undo.into_iter().rev() { self.state.undo(occurrence); }
                    return Err(error);
                }
            }
        }
        if result.is_ok() {
            for fact in &change.withdrawn { hash_sums.0 = hash_sums.0.wrapping_sub(fact_hash(program, fact)); }
            for fact in &change.added { hash_sums.0 = hash_sums.0.wrapping_add(fact_hash(program, fact)); }
            for entity in &change.deleted { hash_sums.1 = hash_sums.1.wrapping_sub(entity_hash(entity)); }
            for entity in &change.created { hash_sums.1 = hash_sums.1.wrapping_add(entity_hash(entity)); }
            self.model = model;
            self.rule_reads = rule_reads;
            self.revision += 1;
            self.history.append(change, self.state.next);
            result = Ok(effects.into_iter().map(|(_, values, relation)| (relation, values)).collect());
        } else {
            for occurrence in undo.into_iter().rev() { self.state.undo(occurrence); }
        }
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
        run.model = None;
        run.rule_reads = None;
        Ok(run)
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
    let budget = StepBudget { ordinary: budget, meter: meter.map(RefCell::new), evaluator: evaluate };
    let model = state_model(program, state, &budget)?;
    if let Some((_, work)) = &mut causes { **work += model.work; }
    step_outcome(program, state, &model, event, arguments, &budget, None, causes).map(|(outcome, _)| {
        let mut state = state.clone();
        state.apply(outcome.change.clone(), outcome.next);
        Transition { state, change: outcome.change, effects: outcome.effects, substitutions: outcome.substitutions }
    })
}

fn step_outcome(
    program: &Program,
    state: &State,
    model: &Model,
    event: RelationId,
    arguments: &[Value],
    budget: &StepBudget<'_>,
    rule_reads: Option<&BTreeSet<RelationId>>,
    mut causes: Option<(&mut BTreeMap<Tuple, (HandlerClause, Binding)>, &mut u64)>,
) -> Result<(StepOutcome, Model), StepError> {
    argument_domain(program, event, arguments)?;
    let state_types: BTreeSet<&str> = program.entity_types.iter().filter(|t| t.state).map(|t| t.name.as_str()).collect();

    let mut matched_ensures = Vec::new();
    let mut may_skip = false;
    let mut matched: BTreeMap<(String, Binding), &HandlerClause> = BTreeMap::new();
    for handler in program.handlers.iter().filter(|h| h.event.relation == event) {
        may_skip |= handler.may_skip;
        let Some(start) = bind_event(program, model, &handler.event, arguments, budget)? else {
            continue;
        };
        for clause in &handler.clauses {
            let text = clause_text(program, clause);
            let bindings = budget.matches(program, model, &clause.body, &start)?;
            for binding in bindings {
                matched_ensures.push((text.clone(), binding.clone(), clause));
                matched.insert((text.clone(), binding), clause);
            }
        }
    }

    if matched.is_empty() && !may_skip {
        let sentence = crate::why::sentence(program, event, arguments);
        if budget.meter.is_some() {
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
                    values.insert(name.clone(), budget.value(program, model, expression, binding)?);
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
                    let tuple = ground(program, model, atom, binding, budget)?;
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
                    let fact = (atom.relation, ground(program, model, atom, binding, budget)?);
                    if let Some((causes, _)) = &mut causes { causes.entry(fact.clone()).or_insert_with(|| ((*clause).clone(), key.1.clone())); }
                    included.insert(fact);
                }
                Action::Withdraw(atom) => {
                    let fact = (atom.relation, ground(program, model, atom, binding, budget)?);
                    if let Some((causes, _)) = &mut causes { causes.entry(fact.clone()).or_insert_with(|| ((*clause).clone(), key.1.clone())); }
                    withdrawn.insert(fact);
                }
                Action::Delete(expression) => match budget.value(program, model, expression, binding)? {
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
                    let tuple = ground(program, model, atom, binding, budget)?;
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
    let failures = crate::modes::check_stratum(program, &state_strata, 0, RelationKind::State, entities(), &|relation, each| {
        relation_facts(relation).for_each(|(_, tuple)| each(tuple));
    });
    if let Some(failure) = failures.first() {
        return Err(StepError::Rejected(Rejection {
            kind: if failure.tuples.is_empty() { RejectionKind::UnfilledExactlyOne } else { RejectionKind::ConflictingWrite },
            explanation: vec![format!("{}.", crate::modes::describe(program, failure))],
        }));
    }
    let evaluated = if rule_reads.is_some_and(|reads| changed.is_disjoint(reads)) {
        let mut post_model = model.clone();
        post_model.work = 0;
        for relation in &changed {
            let tuples: crate::eval::Tuples = program.facts.iter().filter(|fact| fact.relation == *relation).map(|fact| fact.values.clone())
                .chain(relation_facts(*relation).map(|(_, values)| values.clone())).collect();
            if tuples.is_empty() {
                post_model.relations.remove(relation);
            } else {
                post_model.relations.insert(*relation, Arc::new(tuples));
            }
        }
        let failures = if post_live.is_none() || live == &state.live { Vec::new() } else {
            crate::modes::check_stratum(program, &vec![0; program.relations.len()], 0, RelationKind::Derived, entities(), &|relation, each| {
                post_model.tuples(relation).for_each(|tuple| each(tuple));
            })
        };
        if failures.is_empty() { Ok(post_model) } else { Err(EvalError::Mode(failures)) }
    } else {
        let mut post_program = program.clone();
        post_program.entities = entities().cloned().collect();
        post_program.initially = facts().map(|(relation, values)| Fact::new(*relation, values.clone())).collect();
        budget.evaluate(&post_program, &post_program.initially)
    };
    let mut post_model = match evaluated {
        Err(EvalError::Mode(failures)) => {
            return Err(StepError::Rejected(Rejection {
                kind: RejectionKind::DerivedMode,
                explanation: vec![format!("{}.", crate::modes::describe(program, &failures[0]))],
            }));
        }
        other => other?,
    };
    for (relation, tuples) in &mut post_model.relations {
        if let Some(previous) = model.relations.get(relation).filter(|previous| *previous == tuples) {
            *tuples = previous.clone();
        }
    }
    if let Some((_, work)) = &mut causes { **work += post_model.work; }
    let invariants = if !program.requirements.iter().any(|r| r.invariant) { Vec::new() } else {
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
    for (text, original, clause) in matched_ensures {
        let binding = extended.get(&(text, original.clone())).unwrap_or(&original);
        for claim in &clause.ensures {
            let mut claim = claim;
            let mut claim_model = &post_model;
            loop {
                match claim {
                    StateClaim::Before(inner) => { claim_model = model; claim = inner; }
                    StateClaim::After(inner) => { claim_model = &post_model; claim = inner; }
                    _ => break,
                }
            }
            let (holds, sentence) = match claim {
                StateClaim::Holds(atom) => {
                    let tuple = ground(program, claim_model, atom, binding, budget)?;
                    (claim_model.contains(atom.relation, &tuple), crate::why::sentence(program, atom.relation, &tuple))
                }
                StateClaim::Some { body, .. } | StateClaim::No { body, .. } => {
                    let found = !budget.matches(program, claim_model, body, binding)?.is_empty();
                    (if matches!(claim, StateClaim::No { .. }) { !found } else { found }, body.iter().map(|l| render_literal(program, l)).collect::<Vec<_>>().join(", "))
                }
                StateClaim::Every { domain, body, .. } => {
                    let mut holds = true;
                    for instance in budget.matches(program, claim_model, domain, binding)? {
                        if budget.matches(program, claim_model, body, &instance)?.is_empty() { holds = false; break; }
                    }
                    (holds, body.iter().map(|l| render_literal(program, l)).collect::<Vec<_>>().join(", "))
                }
                StateClaim::Before(_) | StateClaim::After(_) => unreachable!(),
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
    }, post_model))
}

fn bind_event(
    program: &Program,
    model: &Model,
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
                if budget.value(program, model, other, &binding)? != *given {
                    return Ok(None);
                }
            }
        }
    }
    Ok(Some(binding))
}

fn ground(program: &Program, model: &Model, atom: &Atom, binding: &Binding, budget: &StepBudget<'_>) -> Result<Vec<Value>, EvalError> {
    atom.arguments
        .iter()
        .map(|argument| budget.value(program, model, argument, binding))
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
    let mut lines: Vec<String> = clause.body.iter().map(|l| format!("when {}", render_literal(program, l))).collect();
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
    lines.sort();
    lines.join("\n")
}
