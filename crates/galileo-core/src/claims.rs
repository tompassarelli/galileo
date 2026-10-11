use crate::*;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub type Instance = BTreeMap<Variable, Value>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Evidence {
    Witness(Instance),
    Complete,
    Every { instances: usize },
}

impl Evidence {
    pub fn vacuous(&self) -> bool {
        matches!(self, Self::Every { instances: 0 })
    }
}

pub type ClaimVerdict = Verdict<Evidence, Instance>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimResult {
    pub requirement: String,
    pub verdict: ClaimVerdict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimFailures {
    pub verdict: ClaimVerdict,
    pub instances: Vec<Instance>,
    pub incomplete: Option<Reason>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClaimError {
    Unquantified {
        requirement: String,
        variables: Vec<Variable>,
        form: Option<&'static str>,
        source: Option<SourceSpan>,
    },
    InvalidForm {
        requirement: String,
        form: String,
        source: Option<SourceSpan>,
    },
    Eval(EvalError),
}

impl fmt::Display for ClaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unquantified {
                requirement,
                variables,
                form,
                source,
            } => {
                let names: Vec<String> = variables.iter().map(|v| format!("?{}", v.0)).collect();
                let names = names.join(", ");
                if let Some(source) = source {
                    write!(f, "{}:{}: ", source.file, source.line)?;
                }
                if let Some(form) = form {
                    return write!(f, "{requirement}: `{form}` takes a sentence without variables: {names} is not bound");
                }
                write!(
                    f,
                    "{requirement}: {names} is not quantified. Write `some {names}:`, `every {names}:` or `no {names}:`"
                )
            }
            Self::InvalidForm { requirement, form, source } => {
                if let Some(source) = source {
                    write!(f, "{}:{}: ", source.file, source.line)?;
                }
                write!(f, "{requirement}: {form}")
            }
            Self::Eval(EvalError::Static(errors)) => {
                let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
                write!(f, "{}", lines.join("\n"))
            }
            Self::Eval(error) => write!(f, "{error:?}"),
        }
    }
}

pub fn unbound(claim: &Claim, outer: &BTreeSet<Variable>) -> BTreeSet<Variable> {
    fn state(claim: &StateClaim) -> BTreeSet<Variable> {
        match claim {
            StateClaim::Holds(atom) => free_variables(&[atom.clone().positive()]),
            StateClaim::Some { variables, body } | StateClaim::No { variables, body } => {
                &free_variables(body) - &variables.iter().cloned().collect()
            }
            StateClaim::Every { variables, domain, body } => {
                &free_variables(domain.iter().chain(body)) - &variables.iter().cloned().collect()
            }
            StateClaim::Before(claim) | StateClaim::After(claim) => state(claim),
        }
    }
    let free = match claim {
        Claim::State(claim) | Claim::Always(claim) | Claim::Eventually(claim) => state(claim),
        Claim::NoSinglePointOfFailure { claim, .. } => match claim.as_ref() {
            Claim::State(StateClaim::Holds(_)) => BTreeSet::new(),
            claim => unbound(claim, outer),
        },
        Claim::Never(_) | Claim::Requires { .. } | Claim::EveryTracePasses { .. }
        | Claim::EveryEventWhen { .. } => BTreeSet::new(),
    };
    free.difference(outer).filter(|variable| !variable.is_anonymous()).cloned().collect()
}

enum Plan {
    Some(Vec<Variable>, RelationId),
    No(Vec<Variable>, RelationId),
    Every {
        variables: Vec<Variable>,
        domain: RelationId,
        bad: RelationId,
    },
    Requires {
        variables: Vec<Variable>,
        subject: RelationId,
        condition: Atom,
    },
    Robust {
        variables: Vec<Variable>,
        subject: RelationId,
        among: Atom,
    },
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    pub modes: Vec<crate::modes::ModeFailure>,
    pub claims: Vec<ClaimResult>,
}

pub fn check_claims(program: &Program, budget: Budget) -> Result<Vec<ClaimResult>, ClaimError> {
    check_program(program, budget).map(|checked| checked.claims)
}

pub fn check_program(program: &Program, budget: Budget) -> Result<Checked, ClaimError> {
    let pool = Pool { budget, capped: false, spent: Cell::new(0) };
    claims_from(program, &pool)
}

pub(crate) fn check_claims_spending(program: &Program, budget: Budget) -> Result<(Vec<ClaimResult>, u64), ClaimError> {
    let pool = Pool { budget, capped: true, spent: Cell::new(0) };
    let results = claims_from(program, &pool)?.claims;
    Ok((results, pool.spent.get()))
}

struct Pool {
    budget: Budget,
    capped: bool,
    spent: Cell<u64>,
}

impl Pool {
    fn left(&self) -> u64 {
        self.budget.work.saturating_sub(self.spent.get())
    }

    fn charge(&self, outcome: &Result<Model, EvalError>) {
        match outcome {
            Ok(model) => self.spent.set(self.spent.get() + model.work),
            Err(EvalError::Unknown(Reason::OutOfBudget { .. })) => self.spent.set(self.spent.get().max(self.budget.work)),
            Err(_) => {}
        }
    }

    fn claim(&self, base: &Model) -> Ledger<'_> {
        Ledger { pool: self, left: Cell::new(self.budget.work.saturating_sub(base.work)) }
    }
}

struct Ledger<'a> {
    pool: &'a Pool,
    left: Cell<u64>,
}

impl Ledger<'_> {
    fn next(&self) -> Budget {
        let left = self.left.get();
        Budget::work(if self.pool.capped { left.min(self.pool.left()) } else { left })
    }

    fn tally(&self, outcome: Result<Model, EvalError>) -> Result<Model, EvalError> {
        match &outcome {
            Ok(model) => self.left.set(self.left.get().saturating_sub(model.work)),
            Err(EvalError::Unknown(Reason::OutOfBudget { .. })) => self.left.set(0),
            Err(_) => {}
        }
        self.pool.charge(&outcome);
        outcome
    }
}

fn invalid_form(claim: &Claim) -> Option<String> {
    fn state(claim: &StateClaim) -> Option<String> {
        match claim {
            StateClaim::Before(_) => Some("`before` is only allowed in a contract or `ensures`".into()),
            StateClaim::After(_) => Some("`after` is only allowed in a contract or `ensures`".into()),
            _ => None,
        }
    }
    match claim {
        Claim::State(claim) | Claim::Always(claim) | Claim::Eventually(claim) | Claim::Never(claim) => state(claim),
        Claim::NoSinglePointOfFailure { claim, .. } => {
            let form = match claim.as_ref() {
                Claim::State(StateClaim::Some { .. }) => Some("some"),
                Claim::State(StateClaim::No { .. }) => Some("no"),
                Claim::State(StateClaim::Every { .. }) => Some("every"),
                Claim::Requires { .. } => Some("requires"),
                Claim::NoSinglePointOfFailure { .. } => Some("has no single point of failure"),
                _ => None,
            };
            match form {
                Some(form) => Some(format!("`has no single point of failure` cannot apply to `{form}`; it takes a sentence or a temporal claim")),
                None => invalid_form(claim),
            }
        }
        Claim::EveryTracePasses { target, checkpoint, .. } => state(target).or_else(|| state(checkpoint)),
        Claim::EveryEventWhen { condition, .. } => state(condition),
        Claim::Requires { .. } => None,
    }
}

fn plans(program: &Program) -> Result<Vec<(Program, Plan)>, ClaimError> {
    check(program).map_err(|errors| ClaimError::Eval(EvalError::Static(errors)))?;
    let temporal_ok = program.events.is_empty() && program.handlers.is_empty();
    let mut plans = Vec::new();
    for requirement in &program.requirements {
        if let Some(form) = invalid_form(&requirement.claim) {
            return Err(ClaimError::InvalidForm {
                requirement: requirement.name.clone(),
                form,
                source: requirement.source.clone(),
            });
        }
        let variables = unbound(&requirement.claim, &BTreeSet::new());
        if !variables.is_empty() {
            let mut claim = &requirement.claim;
            while let Claim::NoSinglePointOfFailure { claim: left, .. } = claim {
                claim = left;
            }
            let form = match claim {
                Claim::Always(_) => Some("always"),
                Claim::Eventually(_) => Some("possibly"),
                _ => None,
            };
            return Err(ClaimError::Unquantified {
                requirement: requirement.name.clone(),
                variables: variables.into_iter().collect(),
                form,
                source: requirement.source.clone(),
            });
        }
        let mut augmented = program.clone();
        let plan = match &requirement.claim {
            Claim::State(claim) => plan(&mut augmented, requirement, claim)?,
            Claim::Always(claim) | Claim::Eventually(claim) if temporal_ok => {
                plan(&mut augmented, requirement, claim)?
            }
            Claim::Never(claim) if temporal_ok => {
                match plan(&mut augmented, requirement, claim)? {
                    Plan::Some(variables, yes) => Plan::No(variables, yes),
                    Plan::No(variables, yes) => Plan::Some(variables, yes),
                    _ => Plan::Unsupported("`never` over an `every` claim".into()),
                }
            }
            Claim::Always(_) | Claim::Eventually(_) | Claim::Never(_) => {
                Plan::Unsupported("a temporal claim over events".into())
            }
            Claim::Requires { target, condition } => {
                let (variables, subject) = subject(&mut augmented, &requirement.name, target);
                Plan::Requires {
                    variables,
                    subject,
                    condition: condition.clone(),
                }
            }
            Claim::NoSinglePointOfFailure { claim, among } => match claim.as_ref() {
                Claim::State(StateClaim::Holds(target)) => {
                    let (variables, subject) = subject(&mut augmented, &requirement.name, target);
                    Plan::Robust {
                        variables,
                        subject,
                        among: among.clone(),
                    }
                }
                Claim::Always(StateClaim::Holds(target))
                | Claim::Eventually(StateClaim::Holds(target))
                    if temporal_ok =>
                {
                    let (variables, subject) = subject(&mut augmented, &requirement.name, target);
                    Plan::Robust {
                        variables,
                        subject,
                        among: among.clone(),
                    }
                }
                _ => Plan::Unsupported(
                    "`has no single point of failure` over anything but a sentence".into(),
                ),
            },
            Claim::EveryTracePasses { .. } => Plan::Unsupported("`every trace to`".into()),
            Claim::EveryEventWhen { .. } => Plan::Unsupported("`every … happens when`".into()),
        };
        for rule in augmented.rules.iter_mut().skip(program.rules.len()) {
            rule.source = requirement.source.clone();
        }
        check(&augmented).map_err(|errors| ClaimError::Eval(EvalError::Static(errors)))?;
        plans.push((augmented, plan));
    }
    Ok(plans)
}

pub(crate) struct Invariants {
    pub(crate) program: Program,
    pub(crate) layers: Vec<std::ops::Range<usize>>,
    pub(crate) decisions: Vec<(String, RelationId, bool)>,
}

pub(crate) fn invariants(program: &Program) -> Option<Invariants> {
    let mut claimed = program.clone();
    claimed.requirements.retain(|requirement| requirement.invariant);
    if claimed.requirements.is_empty() {
        return None;
    }
    plans(&claimed).ok()?;
    let mut augmented = program.clone();
    let (mut layers, mut decisions) = (Vec::new(), Vec::new());
    for requirement in &claimed.requirements {
        let Claim::State(claim) = &requirement.claim else { return None };
        let (start, rules) = (augmented.relations.len(), augmented.rules.len());
        let (relation, holds_when_empty) = match plan(&mut augmented, requirement, claim).ok()? {
            Plan::Some(_, yes) => (yes, false),
            Plan::No(_, yes) => (yes, true),
            Plan::Every { bad, .. } => (bad, true),
            _ => return None,
        };
        for rule in &mut augmented.rules[rules..] {
            rule.source = requirement.source.clone();
        }
        layers.push(start..augmented.relations.len());
        decisions.push((requirement.name.clone(), relation, holds_when_empty));
    }
    Some(Invariants { program: augmented, layers, decisions })
}

fn claims_from(program: &Program, pool: &Pool) -> Result<Checked, ClaimError> {
    let plans = plans(program)?;
    let mut claims = Vec::new();
    let evaluations: Vec<(&Program, Option<&Plan>)> = if plans.is_empty() {
        vec![(program, None)]
    } else {
        plans.iter().map(|(program, plan)| (program, Some(plan))).collect()
    };
    for (index, (augmented, plan)) in evaluations.into_iter().enumerate() {
        let refuting = match plan {
            Some(Plan::No(_, yes)) => Some(*yes),
            Some(Plan::Every { bad, .. }) => Some(*bad),
            _ => None,
        };
        let found: RefCell<BTreeMap<RelationId, Vec<Value>>> = RefCell::default();
        let record = |relation: RelationId, tuple: &[Value]| {
            if refuting == Some(relation) {
                found.borrow_mut().entry(relation).or_insert_with(|| tuple.to_vec());
            }
            false
        };
        let budget = if pool.capped { Budget::work(pool.left()) } else { pool.budget };
        let base = evaluate_blocking(augmented, &program.initially, budget, &record);
        pool.charge(&base);
        let model = match base {
            Ok(model) => Ok(model),
            Err(EvalError::Unknown(reason)) => Err(reason),
            Err(EvalError::Mode(modes)) => {
                let conflict: Vec<String> = modes.iter().map(|f| crate::modes::describe(program, f)).collect();
                let reason = Reason::ModeFails {
                    conflict: conflict.join("; "),
                };
                let claims = program
                    .requirements
                    .iter()
                    .map(|requirement| ClaimResult {
                        requirement: requirement.name.clone(),
                        verdict: Verdict::Unknown(reason.clone()),
                    })
                    .collect();
                return Ok(Checked { modes, claims });
            }
            Err(error) => return Err(ClaimError::Eval(error)),
        };
        let Some(plan) = plan else { continue };
        let requirement = &program.requirements[index];
        let verdict = match plan {
            Plan::Requires {
                variables,
                subject,
                condition,
            } => requires(augmented, &model, pool, &requirement.name, variables, *subject, condition, None)?,
            Plan::Robust {
                variables,
                subject,
                among,
            } => robust(augmented, &model, pool, &requirement.name, variables, *subject, among, None)?,
            _ => decide(plan, &model, &found.take(), &requirement.name),
        };
        claims.push(ClaimResult {
            requirement: requirement.name.clone(),
            verdict,
        });
    }
    Ok(Checked { modes: Vec::new(), claims })
}

pub fn enumerate_claim_failures(program: &Program, budget: Budget) -> Result<Vec<ClaimFailures>, ClaimError> {
    let pool = Pool { budget, capped: false, spent: Cell::new(0) };
    let plans = plans(program)?;
    program.requirements.iter().zip(plans).map(|(requirement, (augmented, plan))| {
        let outcome = evaluate(&augmented, &program.initially, budget);
        pool.charge(&outcome);
        let model = match outcome {
            Ok(model) => Ok(model),
            Err(EvalError::Unknown(reason)) => Err(reason),
            Err(EvalError::Mode(modes)) => Err(Reason::ModeFails {
                conflict: modes.iter().map(|f| crate::modes::describe(program, f)).collect::<Vec<_>>().join("; "),
            }),
            Err(error) => return Err(ClaimError::Eval(error)),
        };
        let mut instances = Vec::new();
        let verdict = match &plan {
            Plan::Requires { variables, subject, condition } => requires(
                &augmented, &model, &pool, &requirement.name, variables, *subject, condition, Some(&mut instances),
            )?,
            Plan::Robust { variables, subject, among } => robust(
                &augmented, &model, &pool, &requirement.name, variables, *subject, among, Some(&mut instances),
            )?,
            _ => {
                let verdict = decide(&plan, &model, &BTreeMap::new(), &requirement.name);
                if matches!(verdict, Verdict::Fails(_)) {
                    if let (Plan::Every { variables, bad, .. }, Ok(model)) = (&plan, &model) {
                        instances.extend(model.tuples(*bad).map(|tuple| variables.iter().cloned().zip(tuple.iter().cloned()).collect()));
                    } else {
                        instances.push(Instance::new());
                    }
                }
                verdict
            }
        };
        let incomplete = match &verdict {
            Verdict::Unknown(reason) => Some(reason.clone()),
            _ => None,
        };
        Ok(ClaimFailures { verdict, instances, incomplete })
    }).collect()
}

fn decide(
    plan: &Plan,
    model: &Result<Model, Reason>,
    found: &BTreeMap<RelationId, Vec<Value>>,
    at: &str,
) -> ClaimVerdict {
    if let Plan::Unsupported(construct) = plan {
        return Verdict::Unknown(Reason::Unsupported {
            construct: construct.clone(),
            at: at.to_string(),
        });
    }
    let model = match model {
        Ok(model) => model,
        Err(reason) => {
            let refuted = match plan {
                Plan::No(variables, relation) | Plan::Every { variables, bad: relation, .. } => found
                    .get(relation)
                    .map(|tuple| (variables, tuple)),
                _ => None,
            };
            return match refuted {
                Some((variables, tuple)) => {
                    Verdict::Fails(variables.iter().cloned().zip(tuple.iter().cloned()).collect())
                }
                None => Verdict::Unknown(reason.clone()),
            };
        }
    };
    let first = |variables: &[Variable], relation: RelationId| {
        model
            .tuples(relation)
            .next()
            .map(|tuple| variables.iter().cloned().zip(tuple.iter().cloned()).collect::<Instance>())
    };
    match plan {
        Plan::Some(variables, yes) => match first(variables, *yes) {
            Some(instance) => Verdict::Holds(Evidence::Witness(instance)),
            None => Verdict::Fails(Instance::new()),
        },
        Plan::No(variables, yes) => match first(variables, *yes) {
            Some(instance) => Verdict::Fails(instance),
            None => Verdict::Holds(Evidence::Complete),
        },
        Plan::Every {
            variables,
            domain,
            bad,
        } => match first(variables, *bad) {
            Some(instance) => Verdict::Fails(instance),
            None => Verdict::Holds(Evidence::Every {
                instances: model.tuples(*domain).count(),
            }),
        },
        _ => unreachable!(),
    }
}

fn subject(program: &mut Program, requirement: &str, target: &Atom) -> (Vec<Variable>, RelationId) {
    let body = vec![target.clone().positive()];
    let variables: Vec<Variable> = free_variables(&body).into_iter().collect();
    let id = auxiliary(program, requirement, "subject", &variables, body);
    (variables, id)
}

#[derive(Clone)]
enum Hole {
    Is(Value),
    Any(Variable),
}

fn pattern(atom: &Atom, instance: &Instance) -> Option<(RelationId, Vec<Hole>)> {
    let slots = atom
        .arguments
        .iter()
        .map(|argument| match argument {
            Expr::Value(value) => Some(Hole::Is(value.clone())),
            Expr::Variable(variable) => Some(match instance.get(variable) {
                Some(value) => Hole::Is(value.clone()),
                None => Hole::Any(variable.clone()),
            }),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some((atom.relation, slots))
}

fn matches(pattern: &(RelationId, Vec<Hole>), relation: RelationId, values: &[Value]) -> Option<Instance> {
    if pattern.0 != relation || pattern.1.len() != values.len() {
        return None;
    }
    let mut bound = Instance::new();
    for (slot, value) in pattern.1.iter().zip(values) {
        match slot {
            Hole::Is(expected) if expected != value => return None,
            Hole::Is(_) => {}
            Hole::Any(variable) => {
                if bound.insert(variable.clone(), value.clone()).is_some_and(|old| &old != value) {
                    return None;
                }
            }
        }
    }
    Some(bound)
}

struct Search<'a> {
    requirement: &'a str,
    instances: usize,
    unknown: Option<Reason>,
}

impl Search<'_> {
    fn still_holds(
        &mut self,
        outcome: Result<Model, EvalError>,
        subject: RelationId,
        tuple: &[Value],
    ) -> Result<Option<bool>, ClaimError> {
        match outcome {
            Ok(model) => Ok(Some(model.contains(subject, tuple))),
            Err(EvalError::Unknown(reason)) => {
                let reason = match reason {
                    Reason::OutOfBudget { resource, at } => Reason::OutOfBudget {
                        resource,
                        at: format!("{at}, in a counterfactual of {}", self.requirement),
                    },
                    other => other,
                };
                self.unknown.get_or_insert(reason);
                Ok(None)
            }
            Err(error) => Err(ClaimError::Eval(error)),
        }
    }

    fn finish(self) -> ClaimVerdict {
        match self.unknown {
            Some(reason) => Verdict::Unknown(reason),
            None => Verdict::Holds(Evidence::Every {
                instances: self.instances,
            }),
        }
    }
}

fn unsupported_pattern(at: &str) -> ClaimVerdict {
    Verdict::Unknown(Reason::Unsupported {
        construct: "a computed argument in a `requires` or `among` pattern".into(),
        at: at.to_string(),
    })
}

fn requires(
    program: &Program,
    model: &Result<Model, Reason>,
    pool: &Pool,
    at: &str,
    variables: &[Variable],
    subject: RelationId,
    condition: &Atom,
    mut failures: Option<&mut Vec<Instance>>,
) -> Result<ClaimVerdict, ClaimError> {
    let model = match model {
        Ok(model) => model,
        Err(reason) => return Ok(Verdict::Unknown(reason.clone())),
    };
    let mut search = Search {
        requirement: at,
        instances: 0,
        unknown: None,
    };
    let ledger = pool.claim(model);
    for tuple in model.tuples(subject) {
        search.instances += 1;
        let instance: Instance = variables.iter().cloned().zip(tuple.iter().cloned()).collect();
        let Some(pattern) = pattern(condition, &instance) else {
            return Ok(unsupported_pattern(at));
        };
        let outcome = ledger.tally(evaluate_blocking(program, &program.initially, ledger.next(), &|relation, values| {
            matches(&pattern, relation, values).is_some()
        }));
        match search.still_holds(outcome, subject, tuple)? {
            Some(true) => match &mut failures {
                Some(failures) => failures.push(instance),
                None => return Ok(Verdict::Fails(instance)),
            },
            Some(false) => {}
            None => return Ok(search.finish()),
        }
    }
    Ok(finish_failures(search, failures))
}

fn robust(
    program: &Program,
    model: &Result<Model, Reason>,
    pool: &Pool,
    at: &str,
    variables: &[Variable],
    subject: RelationId,
    among: &Atom,
    mut failures: Option<&mut Vec<Instance>>,
) -> Result<ClaimVerdict, ClaimError> {
    let model = match model {
        Ok(model) => model,
        Err(reason) => return Ok(Verdict::Unknown(reason.clone())),
    };
    let mut search = Search {
        requirement: at,
        instances: 0,
        unknown: None,
    };
    let ledger = pool.claim(model);
    for tuple in model.tuples(subject) {
        search.instances += 1;
        let instance: Instance = variables.iter().cloned().zip(tuple.iter().cloned()).collect();
        let Some(pattern) = pattern(among, &instance) else {
            return Ok(unsupported_pattern(at));
        };
        let mut tried = BTreeSet::new();
        for fact in program.facts.iter().chain(&program.initially) {
            let Some(bound) = matches(&pattern, fact.relation, &fact.values) else {
                continue;
            };
            if !tried.insert((fact.relation, fact.values.clone())) {
                continue;
            }
            let other = |stated: &Fact| stated.relation != fact.relation || stated.values != fact.values;
            let mut reduced = program.clone();
            reduced.facts.retain(other);
            reduced.initially.retain(other);
            let outcome = ledger.tally(evaluate(&reduced, &reduced.initially, ledger.next()));
            match search.still_holds(outcome, subject, tuple)? {
                Some(false) => {
                    if let Some(failures) = &mut failures {
                        failures.push(instance.clone());
                        break;
                    }
                    let mut counterexample = instance.clone();
                    counterexample.extend(bound);
                    return Ok(Verdict::Fails(counterexample));
                }
                Some(true) => {}
                None => return Ok(search.finish()),
            }
        }
    }
    Ok(finish_failures(search, failures))
}

fn finish_failures(search: Search<'_>, failures: Option<&mut Vec<Instance>>) -> ClaimVerdict {
    if search.unknown.is_none() {
        if let Some(instance) = failures.and_then(|failures| failures.first().cloned()) {
            return Verdict::Fails(instance);
        }
    }
    search.finish()
}

fn plan(
    program: &mut Program,
    requirement: &Requirement,
    claim: &StateClaim,
) -> Result<Plan, ClaimError> {
    let name = &requirement.name;
    Ok(match claim {
        StateClaim::Holds(atom) => {
            let body = vec![atom.clone().positive()];
            let variables: Vec<Variable> = free_variables(&body).into_iter().collect();
            Plan::Some(variables.clone(), auxiliary(program, name, "yes", &variables, body))
        }
        StateClaim::Some { variables, body } => {
            Plan::Some(variables.clone(), auxiliary(program, name, "yes", variables, body.clone()))
        }
        StateClaim::No { variables, body } => {
            Plan::No(variables.clone(), auxiliary(program, name, "yes", variables, body.clone()))
        }
        StateClaim::Every {
            variables,
            domain,
            body,
        } => {
            let domain_relation = auxiliary(program, name, "where", variables, domain.clone());
            let both = domain.iter().chain(body).cloned().collect();
            let yes = auxiliary(program, name, "yes", variables, both);
            let arguments: Vec<Expr> = variables.iter().cloned().map(Expr::Variable).collect();
            let bad = auxiliary(
                program,
                name,
                "fails",
                variables,
                vec![
                    Atom::new(domain_relation, arguments.clone()).positive(),
                    Atom::new(yes, arguments).negative(),
                ],
            );
            Plan::Every {
                variables: variables.clone(),
                domain: domain_relation,
                bad,
            }
        }
        StateClaim::Before(_) | StateClaim::After(_) => {
            Plan::Unsupported("a two-state claim outside a contract".into())
        }
    })
}

fn auxiliary(
    program: &mut Program,
    requirement: &str,
    part: &str,
    variables: &[Variable],
    body: Vec<Literal>,
) -> RelationId {
    let id = RelationId(program.relations.len());
    let name = format!("claim {requirement} {part}");
    program.relations.push(Relation::new(
        name.clone(),
        RelationKind::Derived,
        variables.iter().map(|v| Slot::new(v.0.clone(), Type::Any)),
    ));
    let head = Atom::new(id, variables.iter().cloned().map(Expr::Variable));
    program.rules.push(Rule::new(name, head).when(body));
    id
}

fn free_variables<'a>(body: impl IntoIterator<Item = &'a Literal>) -> BTreeSet<Variable> {
    let mut mentioned = BTreeSet::new();
    let mut local = BTreeSet::new();
    for literal in body {
        match literal {
            Literal::Positive(atom) | Literal::Negative(atom) => {
                atom.arguments.iter().for_each(|a| mentions(a, &mut mentioned))
            }
            Literal::Compare { left, right, .. } => {
                mentions(left, &mut mentioned);
                mentions(right, &mut mentioned);
            }
            Literal::Boolean(value) => mentions(value, &mut mentioned),
            Literal::Bind { variable, value } => {
                mentions(value, &mut mentioned);
                local.insert(variable.clone());
            }
            Literal::Member { variable, list } => {
                mentioned.insert(variable.clone());
                mentions(list, &mut mentioned);
            }
            Literal::Aggregate { variable, .. } | Literal::ArgMin { variable, .. } => {
                local.insert(variable.clone());
            }
        }
    }
    &mentioned - &local
}

fn mentions(expression: &Expr, into: &mut BTreeSet<Variable>) {
    match expression {
        Expr::Variable(variable) => {
            into.insert(variable.clone());
        }
        Expr::Value(_) | Expr::Query(_) => {}
        Expr::Lambda { parameters, body } => {
            let mut free = BTreeSet::new();
            mentions(body, &mut free);
            for parameter in parameters { free.remove(&Variable::new(&parameter.name)); }
            into.extend(free);
        }
        Expr::List(items) | Expr::Call { arguments: items, .. } => {
            items.iter().for_each(|item| mentions(item, into))
        }
        Expr::Record(fields) | Expr::Attrs(fields) => {
            fields.values().for_each(|item| mentions(item, into))
        }
        Expr::Field { value, .. } | Expr::Unary { value, .. } => mentions(value, into),
        Expr::Binary { left, right, .. } => {
            mentions(left, into);
            mentions(right, into);
        }
        Expr::If {
            condition,
            then_value,
            else_value,
        } => {
            mentions(condition, into);
            mentions(then_value, into);
            mentions(else_value, into);
        }
        Expr::Apply {
            function,
            arguments,
        } => {
            mentions(function, into);
            arguments.iter().for_each(|item| mentions(item, into));
        }
    }
}

pub fn exit_code(results: &[ClaimResult]) -> i32 {
    if results.iter().any(|r| matches!(r.verdict, Verdict::Fails(_))) {
        1
    } else if results.iter().any(|r| matches!(r.verdict, Verdict::Unknown(_))) {
        2
    } else {
        0
    }
}

pub fn sentence(result: &ClaimResult) -> String {
    let name = &result.requirement;
    match &result.verdict {
        Verdict::Holds(Evidence::Witness(instance)) if instance.is_empty() => {
            format!("{name} holds.")
        }
        Verdict::Holds(Evidence::Witness(instance)) => {
            format!("{name} holds: {} is a witness.", show(instance))
        }
        Verdict::Holds(Evidence::Complete) => {
            format!("{name} holds: a complete search found no instance.")
        }
        Verdict::Holds(Evidence::Every { instances: 0 }) => format!(
            "{name} holds vacuously: nothing satisfies its `where`, so there was no instance to check."
        ),
        Verdict::Holds(Evidence::Every { instances }) => {
            format!("{name} holds for all {instances} of its instances.")
        }
        Verdict::Fails(instance) if instance.is_empty() => format!("{name} fails: it is false."),
        Verdict::Fails(instance) => {
            format!("{name} fails: {} is a counterexample.", show(instance))
        }
        Verdict::Unknown(Reason::OutOfBudget { resource, at }) if at.starts_with("claim ") => {
            let reason = Reason::OutOfBudget {
                resource: *resource,
                at: format!("claim {name}"),
            };
            format!("{name} is unknown: {reason}. Unknown is not an approval.")
        }
        Verdict::Unknown(reason) => {
            format!("{name} is unknown: {reason}. Unknown is not an approval.")
        }
    }
}

fn show(instance: &Instance) -> String {
    let parts: Vec<String> = instance
        .iter()
        .map(|(variable, value)| format!("?{} = {}", variable.0, show_value(value)))
        .collect();
    parts.join(", ")
}

fn show_value(value: &Value) -> String {
    match value {
        Value::Entity(entity) => entity.name.clone(),
        Value::Text(text) => format!("{text:?}"),
        Value::Int(int) => int.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::OptionPath(path) => path.join("."),
        other => format!("{other:?}"),
    }
}

pub fn failure_sentence(program: &Program, requirement: &Requirement, instance: &Instance) -> String {
    fn state(program: &Program, claim: &StateClaim, instance: &Instance) -> String {
        let body = |body: &[Literal]| body.iter().map(|literal| check::render_literal(program, &bind(literal, instance))).collect::<Vec<_>>().join(" and ");
        match claim {
            StateClaim::Holds(atom) => body(&[atom.clone().positive()]),
            StateClaim::Some { body: literals, .. } => format!("some: {}", body(literals)),
            StateClaim::No { body: literals, .. } => format!("no: {}", body(literals)),
            StateClaim::Every { domain, body: literals, .. } => format!("{}, but not {}", body(domain), body(literals)),
            StateClaim::Before(inner) => format!("before {}", state(program, inner, instance)),
            StateClaim::After(inner) => format!("after {}", state(program, inner, instance)),
        }
    }
    fn claim(program: &Program, value: &Claim, instance: &Instance) -> String {
        let atom = |atom: &Atom| check::render_literal(program, &bind(&atom.clone().positive(), instance));
        match value {
            Claim::State(s) => state(program, s, instance),
            Claim::Always(s) => format!("always {}", state(program, s, instance)),
            Claim::Eventually(s) => format!("possibly {}", state(program, s, instance)),
            Claim::Never(s) => format!("never {}", state(program, s, instance)),
            Claim::Requires { target, condition } => format!("{} requires {}", atom(target), atom(condition)),
            Claim::NoSinglePointOfFailure { claim: inner, among } => format!("{} has no single point of failure among {}", claim(program, inner, instance), atom(among)),
            Claim::EveryTracePasses { target, checkpoint, .. } => format!("every trace to {} passes {}", state(program, target, instance), state(program, checkpoint, instance)),
            Claim::EveryEventWhen { event, condition } => format!("every {} happens when {}", atom(event), state(program, condition, instance)),
        }
    }
    claim(program, &requirement.claim, instance)
}

/// Names a failing `every` instance as sentences, with the requirement's file and line.
pub fn counterexample(program: &Program, result: &ClaimResult) -> Option<String> {
    let Verdict::Fails(instance) = &result.verdict else {
        return None;
    };
    let requirement = program.requirements.iter().find(|r| r.name == result.requirement)?;
    let Claim::State(StateClaim::Every { domain, body, .. }) = &requirement.claim else {
        return None;
    };
    let show = |literals: &[Literal]| {
        literals
            .iter()
            .map(|literal| check::render_literal(program, &bind(literal, instance)))
            .collect::<Vec<_>>()
            .join(" and ")
    };
    let at = requirement
        .source
        .as_ref()
        .map(|s| format!("{}:{}: ", s.file, s.line))
        .unwrap_or_default();
    Some(format!("{at}{}, but not {}.", show(domain), show(body)))
}

fn bind(literal: &Literal, instance: &Instance) -> Literal {
    fn expression(value: &Expr, instance: &Instance) -> Expr {
        let mut value = value.clone();
        match &mut value {
            Expr::Variable(variable) => {
                if let Some(bound) = instance.get(variable) {
                    return Expr::Value(bound.clone());
                }
            }
            Expr::Value(_) => {}
            Expr::List(values) | Expr::Call { arguments: values, .. } => {
                for value in values { *value = expression(value, instance); }
            }
            Expr::Record(fields) | Expr::Attrs(fields) => {
                for value in fields.values_mut() { *value = expression(value, instance); }
            }
            Expr::Field { value, .. } | Expr::Unary { value, .. } => **value = expression(value, instance),
            Expr::Binary { left, right, .. } => {
                **left = expression(left, instance);
                **right = expression(right, instance);
            }
            Expr::If { condition, then_value, else_value } => {
                **condition = expression(condition, instance);
                **then_value = expression(then_value, instance);
                **else_value = expression(else_value, instance);
            }
            Expr::Lambda { parameters, body } => {
                let mut outer = instance.clone();
                for parameter in parameters { outer.remove(&Variable(parameter.name.clone())); }
                **body = expression(body, &outer);
            }
            Expr::Apply { function, arguments } => {
                **function = expression(function, instance);
                for value in arguments { *value = expression(value, instance); }
            }
            Expr::Query(query) => {
                let mut outer = instance.clone();
                match &query.kind {
                    QueryKind::All(variables) => { for variable in variables { outer.remove(variable); } }
                    QueryKind::The(variable) => { outer.remove(variable); }
                }
                query.body = query.body.iter().map(|literal| bind(literal, &outer)).collect();
                if let Some(head) = &mut query.head { **head = expression(head, &outer); }
                if let Some(key) = &mut query.sorted_by { **key = expression(key, &outer); }
            }
        }
        value
    }
    let atom = |atom: &Atom| Atom {
        arguments: atom
            .arguments
            .iter()
            .map(|argument| expression(argument, instance))
            .collect(),
        ..atom.clone()
    };
    match literal {
        Literal::Positive(a) => Literal::Positive(atom(a)),
        Literal::Negative(a) => Literal::Negative(atom(a)),
        Literal::Compare { op, left, right } => Literal::Compare { op: *op, left: expression(left, instance), right: expression(right, instance) },
        Literal::Boolean(value) => Literal::Boolean(expression(value, instance)),
        Literal::Bind { variable, value } => Literal::Bind { variable: variable.clone(), value: expression(value, instance) },
        Literal::Member { variable, list } => Literal::Member { variable: variable.clone(), list: expression(list, instance) },
        Literal::Aggregate { variable, op, body } => {
            let op = match op {
                AggregateOp::Sum(value) => AggregateOp::Sum(expression(value, instance)),
                AggregateOp::Min(value) => AggregateOp::Min(expression(value, instance)),
                AggregateOp::Max(value) => AggregateOp::Max(expression(value, instance)),
                other => other.clone(),
            };
            Literal::Aggregate { variable: variable.clone(), op, body: body.iter().map(|literal| bind(literal, instance)).collect() }
        }
        Literal::ArgMin { direction, variable, candidate, key, body } => Literal::ArgMin { direction: *direction, variable: variable.clone(), candidate: expression(candidate, instance), key: expression(key, instance), body: body.iter().map(|literal| bind(literal, instance)).collect() },
    }
}
