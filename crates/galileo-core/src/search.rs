use crate::state::{State, StepError, Tuple};
use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub states: usize,
    pub depth: usize,
    pub work: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self { states: 100_000, depth: 100, work: Budget::default().work }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchReason {
    States,
    Depth,
    Work,
    OpenDomain { event: RelationId, slot: String },
    Evaluation(Reason),
}

impl fmt::Display for SearchReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::States => write!(f, "state budget ran out"),
            Self::Depth => write!(f, "depth budget ran out"),
            Self::Work => write!(f, "work budget ran out"),
            Self::OpenDomain { slot, .. } => write!(f, "open domain: {slot}"),
            Self::Evaluation(reason) => write!(f, "{reason}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchVerdict {
    Holds,
    Fails,
    Unknown(Vec<SearchReason>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    Complete { states: usize },
    Incomplete { states: usize, depth: usize },
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Complete { states } => write!(f, "across all {states} reachable states (complete)"),
            Self::Incomplete { states, depth } => write!(f, "explored {states} states to depth {depth} (incomplete)"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotAlphabet {
    pub name: String,
    pub open: bool,
    pub values: BTreeSet<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventAlphabet {
    pub relation: RelationId,
    pub slots: Vec<SlotAlphabet>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult {
    pub verdict: SearchVerdict,
    pub scope: Scope,
    pub alphabet: Vec<EventAlphabet>,
    pub witness: Option<Vec<Tuple>>,
    pub binding: Option<crate::claims::Instance>,
    pub sampled: Option<bool>,
    pub work: u64,
    pub admitted: usize,
    pub rejected: usize,
    pub vacuous: bool,
}

struct Node {
    state: State,
    visited: BTreeSet<Vec<Value>>,
    depth: usize,
    predecessor: Option<(usize, Tuple)>,
}

enum Goal {
    State { condition: StateClaim, existential: bool, negative: bool },
    Passes { program: Program, target: RelationId, checkpoint: RelationId, shared: Vec<usize> },
    When { event: (RelationId, Vec<crate::claims::Hole>), condition: StateClaim },
    InState(Claim),
}

fn alphabet(program: &Program, state: &State) -> Vec<EventAlphabet> {
    let mut events: Vec<_> = program.events.iter().collect();
    events.sort_by_key(|event| &program.relations[event.relation.0].name);
    events.into_iter().map(|event| EventAlphabet {
        relation: event.relation,
        slots: program.relations[event.relation.0].slots.iter().enumerate().map(|(index, slot)| {
            let (open, values) = match event.domains.get(&index) {
                Some(ArgumentDomain::Only(values)) => (false, values.iter().cloned().collect()),
                Some(ArgumentDomain::Sampled(values)) => (true, values.iter().cloned().collect()),
                None => match &slot.ty {
                    Type::Entity(name) => {
                        let state_type = program.entity_types.iter().any(|t| t.name == *name && t.state);
                        let values = if state_type { state.live.iter().collect::<Vec<_>>() } else { program.entities.iter().collect() };
                        (false, values.into_iter().filter(|entity| entity.entity_type == *name).cloned().map(Value::Entity).collect())
                    }
                    Type::TextUnion(values) => (false, values.iter().cloned().map(Value::Text).collect()),
                    Type::Bool => (false, BTreeSet::from([Value::Bool(false), Value::Bool(true)])),
                    _ => (true, BTreeSet::new()),
                },
            };
            SlotAlphabet { name: slot.name.clone(), open, values }
        }).collect(),
    }).collect()
}

fn trace(nodes: &[Node], mut index: usize) -> Vec<Tuple> {
    let mut events = Vec::new();
    while let Some((previous, event)) = &nodes[index].predecessor {
        events.push(event.clone());
        index = *previous;
    }
    events.reverse();
    events
}

fn at(program: &Program, state: &State) -> Program {
    let mut current = program.clone();
    let state_types: BTreeSet<_> = program.entity_types.iter().filter(|t| t.state).map(|t| &t.name).collect();
    current.entities.retain(|entity| !state_types.contains(&entity.entity_type));
    current.entities.extend(state.live.iter().cloned());
    current.initially = state.facts.iter().map(|(relation, values)| Fact::new(*relation, values.clone())).collect();
    current
}

fn tuples(program: &Program, state: &State, relations: [RelationId; 2], meter: &mut crate::expr::Meter) -> Result<[BTreeSet<Vec<Value>>; 2], EvalError> {
    let current = at(program, state);
    let model = crate::eval::evaluate(&current, &current.initially, Budget::work(meter.limit.saturating_sub(meter.spent)))?;
    meter.spent += model.work;
    Ok(relations.map(|relation| model.tuples(relation).cloned().collect()))
}

fn sentence_body(claim: &StateClaim) -> Option<Vec<Literal>> {
    match claim {
        StateClaim::Holds(atom) => Some(vec![atom.clone().positive()]),
        StateClaim::Some { body, .. } => Some(body.clone()),
        _ => None,
    }
}

fn goal(program: &Program, claim: &Claim) -> Result<Goal, EvalError> {
    let unsupported = |message: &str| EvalError::Type { at: "search".into(), message: message.into() };
    Ok(match claim {
        Claim::Eventually(condition) => Goal::State { condition: condition.clone(), existential: true, negative: false },
        Claim::Always(condition) => Goal::State { condition: condition.clone(), existential: false, negative: false },
        Claim::Never(condition) => Goal::State { condition: condition.clone(), existential: false, negative: true },
        Claim::EveryTracePasses { variables, target, checkpoint } => {
            let (Some(target), Some(checkpoint)) = (sentence_body(target), sentence_body(checkpoint)) else {
                return Err(unsupported("`only after` takes a sentence on each side"));
            };
            let free = crate::claims::free_variables(&checkpoint);
            let shared: Vec<usize> = (0..variables.len()).filter(|&index| free.contains(&variables[index])).collect();
            let heads: Vec<Variable> = shared.iter().map(|&index| variables[index].clone()).collect();
            let mut augmented = program.clone();
            let target = crate::claims::auxiliary(&mut augmented, "search", "target", variables, target);
            let checkpoint = crate::claims::auxiliary(&mut augmented, "search", "checkpoint", &heads, checkpoint);
            Goal::Passes { program: augmented, target, checkpoint, shared }
        }
        Claim::EveryEventWhen { event, condition } => {
            let event = crate::claims::pattern(event, &BTreeMap::new()).ok_or_else(|| unsupported("`only when` takes an event pattern without computed arguments"))?;
            Goal::When { event, condition: condition.clone() }
        }
        Claim::Requires { .. } | Claim::NoSinglePointOfFailure { .. } => Goal::InState(claim.clone()),
        _ => return Err(unsupported("search takes possibly, always, never, only after, only when, requires or a single point of failure")),
    })
}

fn judge(program: &Program, state: &State, claim: &Claim, meter: &mut crate::expr::Meter) -> Result<Result<crate::claims::Evidence, crate::claims::Instance>, EvalError> {
    let mut current = at(program, state);
    current.requirements = vec![Requirement::new("search", claim.clone())];
    let (mut results, work) = crate::claims::check_in_state(&current, Budget::work(meter.limit.saturating_sub(meter.spent))).map_err(|error| match error {
        ClaimError::Eval(error) => error,
        other => EvalError::Type { at: "search".into(), message: other.to_string() },
    })?;
    meter.spent += work;
    match results.remove(0).verdict {
        Verdict::Holds(evidence) => Ok(Ok(evidence)),
        Verdict::Fails(instance) => Ok(Err(instance)),
        Verdict::Unknown(reason) => Err(EvalError::Unknown(reason)),
    }
}

fn holds(program: &Program, state: &State, claim: &StateClaim, meter: &mut crate::expr::Meter) -> Result<bool, EvalError> {
    Ok(judge(program, state, &Claim::State(claim.clone()), meter)?.is_ok())
}

pub fn search(program: &Program, claim: &Claim, limits: Limits) -> Result<SearchResult, EvalError> {
    let goal = goal(program, claim)?;
    let existential = matches!(goal, Goal::State { existential: true, .. });
    let initial = State::initial(program);
    let mut result = SearchResult {
        verdict: SearchVerdict::Unknown(Vec::new()),
        scope: Scope::Incomplete { states: 0, depth: 0 },
        alphabet: alphabet(program, &initial),
        witness: None,
        binding: None,
        sampled: None,
        work: 0,
        admitted: 0,
        rejected: 0,
        vacuous: matches!(goal, Goal::InState(_)),
    };
    let mut reasons: Vec<_> = result.alphabet.iter().flat_map(|event| event.slots.iter().filter(|slot| slot.open).map(|slot| SearchReason::OpenDomain { event: event.relation, slot: slot.name.clone() })).collect();
    if limits.states == 0 {
        reasons.push(SearchReason::States);
        result.verdict = SearchVerdict::Unknown(reasons);
        return Ok(result);
    }
    let mut meter = crate::expr::Meter { limit: limits.work, spent: 0 };
    let mut nodes = vec![Node { state: initial, visited: BTreeSet::new(), depth: 0, predecessor: None }];
    let mut seen = BTreeMap::from([((nodes[0].state.hash(program), BTreeSet::new()), 0)]);
    let mut distinct = BTreeSet::from([nodes[0].state.hash(program)]);
    let mut index = 0;
    let mut depth = 0;
    let mut decisive = None;
    let outcome = (|| -> Result<(), EvalError> {
        while index < nodes.len() {
            meter.charge("search state")?;
            depth = depth.max(nodes[index].depth);
            let mut visited = nodes[index].visited.clone();
            match &goal {
                Goal::State { condition, existential, negative } => {
                    let yes = holds(program, &nodes[index].state, condition, &mut meter)?;
                    if (*existential && yes) || (!*existential && yes == *negative) {
                        decisive = Some(if *existential { SearchVerdict::Holds } else { SearchVerdict::Fails });
                        result.witness = Some(trace(&nodes, index));
                        return Ok(());
                    }
                }
                Goal::Passes { program: augmented, target, checkpoint, shared } => {
                    let [targets, checkpoints] = tuples(augmented, &nodes[index].state, [*target, *checkpoint], &mut meter)?;
                    visited.extend(checkpoints);
                    if targets.iter().any(|tuple| !visited.contains(&shared.iter().map(|&slot| tuple[slot].clone()).collect::<Vec<_>>())) {
                        decisive = Some(SearchVerdict::Fails);
                        result.witness = Some(trace(&nodes, index));
                        return Ok(());
                    }
                }
                Goal::InState(claim) => {
                    match judge(program, &nodes[index].state, claim, &mut meter)? {
                        Err(instance) => {
                            decisive = Some(SearchVerdict::Fails);
                            result.witness = Some(trace(&nodes, index));
                            result.binding = Some(instance);
                            return Ok(());
                        }
                        Ok(evidence) => result.vacuous &= evidence.vacuous(),
                    }
                }
                Goal::When { .. } => {}
            }
            let events = alphabet(program, &nodes[index].state);
            for (recorded, event) in result.alphabet.iter_mut().zip(&events) {
                for (recorded, slot) in recorded.slots.iter_mut().zip(&event.slots) { recorded.values.extend(slot.values.iter().cloned()); }
            }
            for event in events {
                let values: Vec<Vec<_>> = event.slots.iter().map(|slot| slot.values.iter().cloned().collect()).collect();
                if values.iter().any(Vec::is_empty) { continue; }
                let mut positions = vec![0; values.len()];
                loop {
                    meter.charge("search occurrence")?;
                    let arguments: Vec<_> = values.iter().zip(&positions).map(|(values, &position)| values[position].clone()).collect();
                    match crate::state::step_search(program, &nodes[index].state, event.relation, &arguments, &mut meter) {
                        Ok(transition) => {
                            result.admitted += 1;
                            if let Goal::When { event: pattern, condition } = &goal {
                                if let Some(binding) = crate::claims::matches(pattern, event.relation, &arguments) {
                                    let bound = crate::claims::bind_state(condition, &binding);
                                    if !holds(program, &nodes[index].state, &bound, &mut meter)? {
                                        decisive = Some(SearchVerdict::Fails);
                                        let mut witness = trace(&nodes, index);
                                        witness.push((event.relation, arguments));
                                        result.witness = Some(witness);
                                        return Ok(());
                                    }
                                }
                            }
                            let hash = (transition.state.hash(program), visited.clone());
                            if !seen.contains_key(&hash) {
                                if nodes[index].depth == limits.depth {
                                    if !reasons.contains(&SearchReason::Depth) { reasons.push(SearchReason::Depth); }
                                } else if nodes.len() == limits.states {
                                    if !reasons.contains(&SearchReason::States) { reasons.push(SearchReason::States); }
                                } else {
                                    distinct.insert(hash.0.clone());
                                    seen.insert(hash, nodes.len());
                                    nodes.push(Node { state: transition.state, visited: visited.clone(), depth: nodes[index].depth + 1, predecessor: Some((index, (event.relation, arguments))) });
                                }
                            }
                        }
                        Err(StepError::Rejected(_)) => result.rejected += 1,
                        Err(StepError::Eval(error)) => return Err(error),
                    }
                    let mut slot = positions.len();
                    while slot > 0 {
                        slot -= 1;
                        positions[slot] += 1;
                        if positions[slot] < values[slot].len() { break; }
                        positions[slot] = 0;
                    }
                    if positions.iter().all(|position| *position == 0) { break; }
                }
            }
            index += 1;
        }
        Ok(())
    })();
    match outcome {
        Ok(()) => {}
        Err(EvalError::Unknown(Reason::OutOfBudget { .. })) => { meter.spent = limits.work; reasons.push(SearchReason::Work); }
        Err(EvalError::Unknown(reason)) => reasons.push(SearchReason::Evaluation(reason)),
        Err(error) => return Err(error),
    }
    result.work = meter.spent;
    let complete = decisive.is_none() && index == nodes.len() && reasons.is_empty();
    result.scope = if complete { Scope::Complete { states: distinct.len() } } else { Scope::Incomplete { states: index + usize::from(index < nodes.len()), depth } };
    let sampled_complete = index == nodes.len() && reasons.iter().all(|reason| matches!(reason, SearchReason::OpenDomain { .. }));
    if sampled_complete { result.sampled = Some(!existential); }
    result.verdict = decisive.unwrap_or_else(|| if complete {
        if existential { SearchVerdict::Fails } else { SearchVerdict::Holds }
    } else { SearchVerdict::Unknown(reasons) });
    Ok(result)
}
