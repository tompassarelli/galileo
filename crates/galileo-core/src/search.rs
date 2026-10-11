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
    pub sampled: Option<bool>,
    pub work: u64,
    pub admitted: usize,
    pub rejected: usize,
}

struct Node {
    state: State,
    depth: usize,
    predecessor: Option<(usize, Tuple)>,
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

fn holds(program: &Program, state: &State, claim: &StateClaim, meter: &mut crate::expr::Meter) -> Result<bool, EvalError> {
    let mut current = program.clone();
    let state_types: BTreeSet<_> = program.entity_types.iter().filter(|t| t.state).map(|t| &t.name).collect();
    current.entities.retain(|entity| !state_types.contains(&entity.entity_type));
    current.entities.extend(state.live.iter().cloned());
    current.initially = state.facts.iter().map(|(relation, values)| Fact::new(*relation, values.clone())).collect();
    current.requirements = vec![Requirement::new("search", Claim::State(claim.clone()))];
    let (results, work) = crate::claims::check_claims_spending(&current, Budget::work(meter.limit.saturating_sub(meter.spent))).map_err(|error| match error {
        ClaimError::Eval(error) => error,
        other => EvalError::Type { at: "search".into(), message: other.to_string() },
    })?;
    meter.spent += work;
    match &results[0].verdict {
        Verdict::Holds(_) => Ok(true),
        Verdict::Fails(_) => Ok(false),
        Verdict::Unknown(reason) => Err(EvalError::Unknown(reason.clone())),
    }
}

pub fn search(program: &Program, claim: &Claim, limits: Limits) -> Result<SearchResult, EvalError> {
    let (condition, existential, negative) = match claim {
        Claim::Eventually(condition) => (condition, true, false),
        Claim::Always(condition) => (condition, false, false),
        Claim::Never(condition) => (condition, false, true),
        _ => return Err(EvalError::Type { at: "search".into(), message: "search takes possibly, always or never".into() }),
    };
    let initial = State::initial(program);
    let mut result = SearchResult {
        verdict: SearchVerdict::Unknown(Vec::new()),
        scope: Scope::Incomplete { states: 0, depth: 0 },
        alphabet: alphabet(program, &initial),
        witness: None,
        sampled: None,
        work: 0,
        admitted: 0,
        rejected: 0,
    };
    let mut reasons: Vec<_> = result.alphabet.iter().flat_map(|event| event.slots.iter().filter(|slot| slot.open).map(|slot| SearchReason::OpenDomain { event: event.relation, slot: slot.name.clone() })).collect();
    if limits.states == 0 {
        reasons.push(SearchReason::States);
        result.verdict = SearchVerdict::Unknown(reasons);
        return Ok(result);
    }
    let mut meter = crate::expr::Meter { limit: limits.work, spent: 0 };
    let mut nodes = vec![Node { state: initial, depth: 0, predecessor: None }];
    let mut seen = BTreeMap::from([(nodes[0].state.hash(program), 0)]);
    let mut index = 0;
    let mut depth = 0;
    let mut decisive = None;
    let outcome = (|| -> Result<(), EvalError> {
        while index < nodes.len() {
            meter.charge("search state")?;
            depth = depth.max(nodes[index].depth);
            let yes = holds(program, &nodes[index].state, condition, &mut meter)?;
            if (existential && yes) || (!existential && yes == negative) {
                decisive = Some(if existential { SearchVerdict::Holds } else { SearchVerdict::Fails });
                result.witness = Some(trace(&nodes, index));
                return Ok(());
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
                            let hash = transition.state.hash(program);
                            if !seen.contains_key(&hash) {
                                if nodes[index].depth == limits.depth {
                                    if !reasons.contains(&SearchReason::Depth) { reasons.push(SearchReason::Depth); }
                                } else if nodes.len() == limits.states {
                                    if !reasons.contains(&SearchReason::States) { reasons.push(SearchReason::States); }
                                } else {
                                    seen.insert(hash, nodes.len());
                                    nodes.push(Node { state: transition.state, depth: nodes[index].depth + 1, predecessor: Some((index, (event.relation, arguments))) });
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
    result.scope = if complete { Scope::Complete { states: nodes.len() } } else { Scope::Incomplete { states: index + usize::from(index < nodes.len()), depth } };
    let sampled_complete = index == nodes.len() && reasons.iter().all(|reason| matches!(reason, SearchReason::OpenDomain { .. }));
    if sampled_complete { result.sampled = Some(!existential); }
    result.verdict = decisive.unwrap_or_else(|| if complete {
        if existential { SearchVerdict::Fails } else { SearchVerdict::Holds }
    } else { SearchVerdict::Unknown(reasons) });
    Ok(result)
}
