use crate::check::{render_atom, render_expr, render_literal};
use crate::eval::{matches, value};
use crate::expr::Binding;
use crate::history::{Change, History};
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

pub type Tuple = (RelationId, Vec<Value>);

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct State {
    pub facts: BTreeSet<Tuple>,
    pub live: BTreeSet<Entity>,
    pub next: u64,
}

impl State {
    pub fn hash(&self, program: &Program) -> String {
        let mut facts: Vec<_> = self.facts.iter().map(|(relation, values)| (&program.relations[relation.0].name, values)).collect();
        facts.sort();
        let bytes = format!("{:?}", (facts, &self.live, self.next));
        let hash = bytes.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3));
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
        }
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub state: State,
    pub history: History,
    pub revision: u64,
    pub log: Vec<StepRecord>,
}

impl Run {
    pub fn new(program: &Program) -> Self {
        let state = State::initial(program);
        Self { history: History::new(&state), state, revision: 0, log: Vec::new() }
    }

    pub fn run(&mut self, program: &Program, event: RelationId, arguments: &[Value], budget: Budget) -> Result<&StepRecord, EvalError> {
        self.run_all(program, &[(event, arguments.to_vec())], budget)
    }

    pub fn run_all(&mut self, program: &Program, events: &[Tuple], budget: Budget) -> Result<&StepRecord, EvalError> {
        let mut state = self.state.clone();
        let mut change = Change::default();
        let mut effects = BTreeSet::new();
        let mut result = Ok(Vec::new());
        for (event, arguments) in events {
            match step_outcome(program, &state, *event, arguments, budget, None) {
                Ok(transition) => {
                    change.extend(transition.change);
                    state = transition.state;
                    effects.extend(transition.effects.into_iter().map(|(relation, values)| (program.relations[relation.0].name.clone(), values, relation)));
                }
                Err(StepError::Rejected(rejection)) => {
                    result = Err(rejection);
                    break;
                }
                Err(StepError::Eval(error)) => return Err(error),
            }
        }
        if result.is_ok() {
            self.state = state;
            self.revision += 1;
            self.history.append(change, self.state.next);
            result = Ok(effects.into_iter().map(|(_, values, relation)| (relation, values)).collect());
        }
        self.log.push(StepRecord { events: events.to_vec(), revision: self.revision, hash: self.state.hash(program), result });
        Ok(self.log.last().expect("the occurrence was recorded"))
    }

    pub fn replay(program: &Program, log: &[StepRecord], budget: Budget) -> Result<Self, EvalError> {
        let mut run = Self::new(program);
        for (index, recorded) in log.iter().enumerate() {
            if run.run_all(program, &recorded.events, budget)? != recorded {
                return Err(EvalError::Type { at: format!("event log occurrence {}", index + 1), message: "The replay differs from the recorded admission, effects, revision or state hash.".into() });
            }
        }
        Ok(run)
    }
}

enum StepError {
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
    step_outcome(program, state, event, arguments, budget, None).map_err(|error| match error {
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
    step_outcome(program, state, event, arguments, Budget::work(meter.limit.saturating_sub(meter.spent)), Some((&mut causes, &mut work)))
        .map_err(|error| match error {
            StepError::Eval(error) => error,
            StepError::Rejected(rejection) => EvalError::Type { at: "chain of title".into(), message: rejection.explanation.join("\n") },
        })?;
    meter.spent += work;
    meter.charge("explaining an occurrence")?;
    Ok(causes)
}

fn step_outcome(
    program: &Program,
    state: &State,
    event: RelationId,
    arguments: &[Value],
    budget: Budget,
    mut causes: Option<(&mut BTreeMap<Tuple, (HandlerClause, Binding)>, &mut u64)>,
) -> Result<Transition, StepError> {
    let state_types: BTreeSet<&str> = program.entity_types.iter().filter(|t| t.state).map(|t| t.name.as_str()).collect();
    let mut pre_program = program.clone();
    pre_program.entities.retain(|e| !state_types.contains(e.entity_type.as_str()));
    pre_program.entities.extend(state.live.iter().cloned());
    let program = &pre_program;
    let facts: Vec<Fact> = state
        .facts
        .iter()
        .map(|(relation, values)| Fact::new(*relation, values.clone()))
        .collect();
    let model = evaluate(program, &facts, budget)?;
    if let Some((_, work)) = &mut causes { **work += model.work; }

    let mut matched_ensures = Vec::new();
    let mut may_skip = false;
    let mut matched: BTreeMap<(String, Binding), &HandlerClause> = BTreeMap::new();
    for handler in program.handlers.iter().filter(|h| h.event.relation == event) {
        may_skip |= handler.may_skip;
        let Some(start) = bind_event(program, &model, &handler.event, arguments, budget)? else {
            continue;
        };
        for clause in &handler.clauses {
            let text = clause_text(program, clause);
            let bindings = matches(program, &model, &clause.body, &start, budget)?;
            for binding in bindings {
                matched_ensures.push((text.clone(), binding.clone(), clause));
                matched.insert((text.clone(), binding), clause);
            }
        }
    }

    if matched.is_empty() && !may_skip {
        let sentence = crate::why::sentence(program, event, arguments);
        let mut explanation = (*program).clone();
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
        let verdict = crate::why_not::why_not(&explanation, &facts, goal, arguments, None, budget)?;
        return Err(StepError::Rejected(Rejection {
            kind: RejectionKind::NoMatch,
            explanation: format!("{sentence} has no matching clause.\n{}", crate::why_not::render(&explanation, goal, arguments, None, &verdict, budget)).split('\n').map(str::to_string).collect(),
        }));
    }

    let mut requests = Vec::new();
    for ((text, binding), clause) in &matched {
        for action in &clause.actions {
            if let Action::Create { variable, entity_type, fields } = action {
                let declared = program.entity_types.iter().find(|t| &t.name == entity_type);
                let mut values = BTreeMap::new();
                for (name, expression) in fields {
                    values.insert(name.clone(), value(program, &model, expression, binding, budget)?);
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
                    let tuple = ground(program, &model, atom, binding, budget)?;
                    let keys: &[usize] = match program.relations[atom.relation.0].modes.as_slice() {
                        [Mode::ExactlyOne { key, .. }] => std::slice::from_ref(key),
                        [Mode::AtMostOne { keys, .. }] => keys,
                        _ => &[],
                    };
                    if !keys.is_empty() {
                        withdrawn.extend(
                            state
                                .facts
                                .iter()
                                .filter(|(relation, values)| {
                                    *relation == atom.relation && keys.iter().all(|&k| values[k] == tuple[k])
                                })
                                .cloned(),
                        );
                    }
                    let fact = (atom.relation, tuple);
                    if let Some((causes, _)) = &mut causes { causes.entry(fact.clone()).or_insert_with(|| ((*clause).clone(), key.1.clone())); }
                    included.insert(fact);
                }
                Action::Include(atom) => {
                    let fact = (atom.relation, ground(program, &model, atom, binding, budget)?);
                    if let Some((causes, _)) = &mut causes { causes.entry(fact.clone()).or_insert_with(|| ((*clause).clone(), key.1.clone())); }
                    included.insert(fact);
                }
                Action::Withdraw(atom) => {
                    let fact = (atom.relation, ground(program, &model, atom, binding, budget)?);
                    if let Some((causes, _)) = &mut causes { causes.entry(fact.clone()).or_insert_with(|| ((*clause).clone(), key.1.clone())); }
                    withdrawn.insert(fact);
                }
                Action::Delete(expression) => match value(program, &model, expression, binding, budget)? {
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
                    let tuple = ground(program, &model, atom, binding, budget)?;
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
    let mut facts: BTreeSet<Tuple> = state
        .facts
        .iter()
        .filter(|fact| {
            let removed = withdrawn.contains(*fact) || fact.1.iter().any(|v| mentions(v, &deleted));
            if removed && !included.contains(*fact) { change.withdrawn.insert((*fact).clone()); }
            !removed
        })
        .cloned()
        .collect();
    for (entity, values) in &created {
        for (field, value) in values {
            if let Some(relation) = program.field_relation(&entity.entity_type, field) {
                if value != &Value::Null
                    || !matches!(program.relations[relation.0].modes.as_slice(), [Mode::AtMostOne { .. }])
                {
                    let fact = (relation, vec![Value::Entity(entity.clone()), value.clone()]);
                    change.added.insert(fact.clone());
                    facts.insert(fact);
                }
            }
        }
    }
    facts.extend(included.iter().cloned());
    change.added.extend(included.iter().filter(|fact| !state.facts.contains(*fact)).cloned());
    change.created.extend(created.keys().cloned());
    change.deleted.extend(deleted.intersection(&state.live).cloned());
    let mut live: BTreeSet<Entity> = state.live.difference(&deleted).cloned().collect();
    live.extend(created.keys().cloned());
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
    let mut post_program = (*program).clone();
    post_program.entities.retain(|e| !state_types.contains(e.entity_type.as_str()));
    post_program.entities.extend(live.iter().cloned());
    for relation in &mut post_program.relations {
        relation.kind = if relation.kind == RelationKind::State { RelationKind::Derived } else { RelationKind::Static };
    }
    let failures = crate::modes::check_stratum(&post_program, &vec![0; program.relations.len()], 0, &|relation, each| {
        facts.iter().filter(|(id, _)| *id == relation).for_each(|(_, tuple)| each(tuple));
    });
    if let Some(failure) = failures.first() {
        return Err(StepError::Rejected(Rejection {
            kind: if failure.tuples.is_empty() { RejectionKind::UnfilledExactlyOne } else { RejectionKind::ConflictingWrite },
            explanation: vec![format!("{}.", crate::modes::describe(program, failure))],
        }));
    }
    post_program.relations = program.relations.clone();
    post_program.initially = facts.iter().map(|(relation, values)| Fact::new(*relation, values.clone())).collect();
    let post_model = match evaluate(&post_program, &post_program.initially, budget) {
        Err(EvalError::Mode(failures)) => {
            return Err(StepError::Rejected(Rejection {
                kind: RejectionKind::DerivedMode,
                explanation: vec![format!("{}.", crate::modes::describe(program, &failures[0]))],
            }));
        }
        other => other?,
    };
    if let Some((_, work)) = &mut causes { **work += post_model.work; }
    post_program.requirements.retain(|r| r.invariant);
    let invariants = if post_program.requirements.is_empty() { Vec::new() } else { check_claims(&post_program, budget).map_err(|error| match error {
        ClaimError::Eval(error) => error,
        other => EvalError::Type { at: "step".into(), message: other.to_string() },
    })? };
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
                    StateClaim::Before(inner) => { claim_model = &model; claim = inner; }
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
                    let found = !matches(program, claim_model, body, binding, budget)?.is_empty();
                    (if matches!(claim, StateClaim::No { .. }) { !found } else { found }, body.iter().map(|l| render_literal(program, l)).collect::<Vec<_>>().join(", "))
                }
                StateClaim::Every { domain, body, .. } => {
                    let mut holds = true;
                    for instance in matches(program, claim_model, domain, binding, budget)? {
                        if matches(program, claim_model, body, &instance, budget)?.is_empty() { holds = false; break; }
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
    let next = state.next + created.len() as u64;
    Ok(Transition {
        state: State { facts, live, next },
        change,
        effects: effects.into_iter().map(|(_, values, relation)| (relation, values)).collect(),
        substitutions: matched.len(),
    })
}

fn bind_event(
    program: &Program,
    model: &Model,
    event: &Atom,
    arguments: &[Value],
    budget: Budget,
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
                if value(program, model, other, &binding, budget)? != *given {
                    return Ok(None);
                }
            }
        }
    }
    Ok(Some(binding))
}

fn ground(program: &Program, model: &Model, atom: &Atom, binding: &Binding, budget: Budget) -> Result<Vec<Value>, EvalError> {
    atom.arguments
        .iter()
        .map(|argument| value(program, model, argument, binding, budget))
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

fn clause_text(program: &Program, clause: &HandlerClause) -> String {
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
