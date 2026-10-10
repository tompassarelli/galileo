use crate::*;
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
pub enum ClaimError {
    Unquantified {
        requirement: String,
        variables: Vec<Variable>,
    },
    Eval(EvalError),
}

impl fmt::Display for ClaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unquantified {
                requirement,
                variables,
            } => {
                let names: Vec<String> = variables.iter().map(|v| format!("?{}", v.0)).collect();
                let names = names.join(", ");
                write!(
                    f,
                    "{requirement}: {names} is not quantified. Write `some {names}:`, `every {names}:` or `no {names}:`"
                )
            }
            Self::Eval(EvalError::Static(errors)) => {
                let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
                write!(f, "{}", lines.join("\n"))
            }
            Self::Eval(error) => write!(f, "{error:?}"),
        }
    }
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

pub fn check_claims(program: &Program, budget: Budget) -> Result<Vec<ClaimResult>, ClaimError> {
    let mut augmented = program.clone();
    let temporal_ok = program.events.is_empty() && program.handlers.is_empty();
    let mut plans = Vec::new();
    for requirement in &program.requirements {
        let plan = match &requirement.claim {
            Claim::State(claim) => plan(&mut augmented, requirement, claim, false)?,
            Claim::Always(claim) | Claim::Eventually(claim) if temporal_ok => {
                plan(&mut augmented, requirement, claim, false)?
            }
            Claim::Never(claim) if temporal_ok => {
                match plan(&mut augmented, requirement, claim, true)? {
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
        plans.push(plan);
    }
    let model = match evaluate(&augmented, &program.initially, budget) {
        Ok(model) => Ok(model),
        Err(EvalError::Unknown(reason)) => Err(reason),
        Err(error) => return Err(ClaimError::Eval(error)),
    };
    program
        .requirements
        .iter()
        .zip(plans)
        .map(|(requirement, plan)| {
            let verdict = match &plan {
                Plan::Requires {
                    variables,
                    subject,
                    condition,
                } => requires(&augmented, &model, budget, &requirement.name, variables, *subject, condition)?,
                Plan::Robust {
                    variables,
                    subject,
                    among,
                } => robust(&augmented, &model, budget, &requirement.name, variables, *subject, among)?,
                _ => decide(&plan, &model, &requirement.name),
            };
            Ok(ClaimResult {
                requirement: requirement.name.clone(),
                verdict,
            })
        })
        .collect()
}

fn decide(plan: &Plan, model: &Result<Model, Reason>, at: &str) -> ClaimVerdict {
    if let Plan::Unsupported(construct) = plan {
        return Verdict::Unknown(Reason::Unsupported {
            construct: construct.clone(),
            at: at.to_string(),
        });
    }
    let model = match model {
        Ok(model) => model,
        Err(reason) => return Verdict::Unknown(reason.clone()),
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
    budget: Budget,
    at: &str,
    variables: &[Variable],
    subject: RelationId,
    condition: &Atom,
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
    for tuple in model.tuples(subject) {
        search.instances += 1;
        let instance: Instance = variables.iter().cloned().zip(tuple.iter().cloned()).collect();
        let Some(pattern) = pattern(condition, &instance) else {
            return Ok(unsupported_pattern(at));
        };
        let outcome = evaluate_blocking(program, &program.initially, budget, &|relation, values| {
            matches(&pattern, relation, values).is_some()
        });
        if search.still_holds(outcome, subject, tuple)? == Some(true) {
            return Ok(Verdict::Fails(instance));
        }
    }
    Ok(search.finish())
}

fn robust(
    program: &Program,
    model: &Result<Model, Reason>,
    budget: Budget,
    at: &str,
    variables: &[Variable],
    subject: RelationId,
    among: &Atom,
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
    let stated = program.facts.len();
    for tuple in model.tuples(subject) {
        search.instances += 1;
        let instance: Instance = variables.iter().cloned().zip(tuple.iter().cloned()).collect();
        let Some(pattern) = pattern(among, &instance) else {
            return Ok(unsupported_pattern(at));
        };
        for (index, fact) in program.facts.iter().chain(&program.initially).enumerate() {
            let Some(bound) = matches(&pattern, fact.relation, &fact.values) else {
                continue;
            };
            let mut reduced = program.clone();
            if index < stated {
                reduced.facts.remove(index);
            } else {
                reduced.initially.remove(index - stated);
            }
            let outcome = evaluate(&reduced, &reduced.initially, budget);
            if search.still_holds(outcome, subject, tuple)? == Some(false) {
                let mut counterexample = instance.clone();
                counterexample.extend(bound);
                return Ok(Verdict::Fails(counterexample));
            }
        }
    }
    Ok(search.finish())
}

fn plan(
    program: &mut Program,
    requirement: &Requirement,
    claim: &StateClaim,
    existential: bool,
) -> Result<Plan, ClaimError> {
    let name = &requirement.name;
    let quantified = |variables: &[Variable], literals: &[&[Literal]]| {
        let allowed: BTreeSet<&Variable> = variables.iter().collect();
        let free: Vec<Variable> = literals
            .iter()
            .flat_map(|body| free_variables(body))
            .filter(|variable| !allowed.contains(variable))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if free.is_empty() {
            Ok(())
        } else {
            Err(ClaimError::Unquantified {
                requirement: name.clone(),
                variables: free,
            })
        }
    };
    Ok(match claim {
        StateClaim::Holds(atom) => {
            let body = vec![atom.clone().positive()];
            let variables: Vec<Variable> = free_variables(&body).into_iter().collect();
            if !existential {
                quantified(&[], &[&body])?;
            }
            Plan::Some(variables.clone(), auxiliary(program, name, "yes", &variables, body))
        }
        StateClaim::Some { variables, body } => {
            quantified(variables, &[body])?;
            Plan::Some(variables.clone(), auxiliary(program, name, "yes", variables, body.clone()))
        }
        StateClaim::No { variables, body } => {
            quantified(variables, &[body])?;
            Plan::No(variables.clone(), auxiliary(program, name, "yes", variables, body.clone()))
        }
        StateClaim::Every {
            variables,
            domain,
            body,
        } => {
            quantified(variables, &[domain, body])?;
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

fn free_variables(body: &[Literal]) -> BTreeSet<Variable> {
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
            Literal::Bind { variable, value } | Literal::Member { variable, list: value } => {
                mentions(value, &mut mentioned);
                local.insert(variable.clone());
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
        Expr::Value(_) | Expr::Query(_) | Expr::Lambda { .. } => {}
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
