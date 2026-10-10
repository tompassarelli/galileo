use crate::check::render_literal;
use crate::engine::{derive, expr_variables, Derivation, Evaluator, Meter, Origin};
use crate::why::{fact_line, located, scope, sentence};
use crate::*;
use std::cmp::Reverse;
use std::collections::BTreeMap;

pub const DEPTH: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Via {
    pub rule: String,
    pub bindings: Vec<(Variable, Value)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt {
    pub rule: usize,
    pub conditions: Vec<Condition>,
    pub missing: usize,
    pub met: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    pub text: String,
    pub state: State,
    pub deeper: Vec<Attempt>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Met,
    Missing { stated: bool, scope: Vec<String> },
    Blocked { by: String },
    False,
    Waiting,
}

type Binding = BTreeMap<Variable, Value>;

pub fn why_not(
    program: &Program,
    state: &[Fact],
    relation: RelationId,
    values: &[Value],
    via: Option<&Via>,
    budget: Budget,
) -> Result<Verdict<(), Vec<Attempt>>, EvalError> {
    if let Some(via) = via {
        if !program.rules.iter().any(|rule| rule.name == via.rule) {
            return Err(EvalError::Type {
                at: via.rule.clone(),
                message: "no rule has this name".into(),
            });
        }
    }
    let derivation = match derive(program, state, budget) {
        Err(EvalError::Unknown(reason)) => return Ok(Verdict::Unknown(reason)),
        other => other?,
    };
    if derivation.origin(relation, values).is_some() {
        return Ok(Verdict::Holds(()));
    }
    let mut search = NearMiss {
        meter: Meter {
            limit: budget.work,
            spent: derivation.work,
        },
        derivation: &derivation,
    };
    let mut path = vec![(relation, values.to_vec())];
    match search.explain(relation, values, via, 0, &mut path) {
        Err(EvalError::Unknown(reason)) => Ok(Verdict::Unknown(reason)),
        other => Ok(Verdict::Fails(other?)),
    }
}

struct Best {
    missing: usize,
    met: usize,
    binding: Binding,
    states: Vec<State>,
}

struct NearMiss<'d, 'p> {
    derivation: &'d Derivation<'p>,
    meter: Meter,
}

impl NearMiss<'_, '_> {
    fn program(&self) -> &Program {
        self.derivation.program()
    }

    fn explain(
        &mut self,
        relation: RelationId,
        values: &[Value],
        via: Option<&Via>,
        depth: usize,
        path: &mut Vec<(RelationId, Vec<Value>)>,
    ) -> Result<Vec<Attempt>, EvalError> {
        let program = self.derivation.program();
        let mut attempts = Vec::new();
        for (index, rule) in program.rules.iter().enumerate() {
            if rule.head.relation != relation || via.is_some_and(|via| via.rule != rule.name) {
                continue;
            }
            let mut binding: Binding = via
                .map(|via| via.bindings.iter().cloned().collect())
                .unwrap_or_default();
            if !fits(&rule.head.arguments, values, &mut binding, None) {
                continue;
            }
            let mut best = None;
            let mut poisoned = Vec::new();
            let mut states = vec![State::Waiting; rule.body.len()];
            let mut done = vec![false; rule.body.len()];
            self.search(rule, values, &mut done, &mut binding, &mut poisoned, &mut states, 0, &mut best)?;
            let Some(best) = best else { continue };
            let mut conditions = Vec::new();
            for (literal, state) in rule.body.iter().zip(&best.states) {
                let mut deeper = Vec::new();
                if let (Literal::Positive(atom), State::Missing { stated: false, .. }, true) =
                    (literal, state, depth < DEPTH)
                {
                    if let Some(row) = ground(atom, &best.binding, program) {
                        let key = (atom.relation, row);
                        if !path.contains(&key) {
                            path.push(key.clone());
                            deeper = self.explain(key.0, &key.1, None, depth + 1, path)?;
                            path.pop();
                        }
                    }
                }
                conditions.push(Condition {
                    text: render_literal(program, &substitute_literal(literal, &best.binding)),
                    state: state.clone(),
                    deeper,
                });
            }
            attempts.push(Attempt {
                rule: index,
                conditions,
                missing: best.missing,
                met: best.met,
            });
        }
        attempts.sort_by_key(|a| (a.missing, Reverse(a.met), a.rule));
        Ok(attempts)
    }

    #[allow(clippy::too_many_arguments)]
    fn search(
        &mut self,
        rule: &Rule,
        target: &[Value],
        done: &mut Vec<bool>,
        binding: &mut Binding,
        poisoned: &mut Vec<Variable>,
        states: &mut Vec<State>,
        missing: usize,
        best: &mut Option<Best>,
    ) -> Result<(), EvalError> {
        self.meter.charge("why not")?;
        if best.as_ref().is_some_and(|b| missing > b.missing) {
            return Ok(());
        }
        let Some(i) = next_literal(&rule.body, done, binding, poisoned) else {
            if !head_agrees(self.program(), &rule.head.arguments, target, binding) {
                return Ok(());
            }
            for (state, done) in states.iter_mut().zip(done.iter()) {
                if !done {
                    *state = State::Waiting;
                }
            }
            let met = states.iter().filter(|s| **s == State::Met).count();
            if best
                .as_ref()
                .is_none_or(|b| (missing, Reverse(met)) < (b.missing, Reverse(b.met)))
            {
                *best = Some(Best {
                    missing,
                    met,
                    binding: binding.clone(),
                    states: states.clone(),
                });
            }
            return Ok(());
        };
        let literal = &rule.body[i];
        let program = self.derivation.program();
        let evaluator = Evaluator {
            program,
            at: &rule.name,
        };
        done[i] = true;
        self.step(literal, &evaluator, i, rule, target, done, binding, poisoned, states, missing, best)?;
        done[i] = false;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn step(
        &mut self,
        literal: &Literal,
        evaluator: &Evaluator,
        i: usize,
        rule: &Rule,
        target: &[Value],
        done: &mut Vec<bool>,
        binding: &mut Binding,
        poisoned: &mut Vec<Variable>,
        states: &mut Vec<State>,
        missing: usize,
        best: &mut Option<Best>,
    ) -> Result<(), EvalError> {
        let program = self.derivation.program();
        macro_rules! next {
            ($state:expr, $missing:expr) => {{
                states[i] = $state;
                self.search(rule, target, done, binding, poisoned, states, $missing, best)?;
            }};
        }
        match literal {
            Literal::Positive(atom) => {
                if waits(&atom.arguments, binding, poisoned) {
                    next!(State::Waiting, missing);
                    return Ok(());
                }
                let rows = self.derivation.rows(atom.relation);
                for row in rows {
                    self.meter.charge("why not")?;
                    let mut extended = binding.clone();
                    if fits(&atom.arguments, row, &mut extended, Some(evaluator)) {
                        let saved = std::mem::replace(binding, extended);
                        next!(State::Met, missing);
                        *binding = saved;
                    }
                }
                let fresh: Vec<Variable> = variables(&atom.arguments)
                    .filter(|v| !binding.contains_key(v) && !poisoned.contains(v))
                    .collect();
                let mark = poisoned.len();
                poisoned.extend(fresh);
                let stated = !program.rules.iter().any(|r| r.head.relation == atom.relation);
                let checked = scope(program, self.derivation.stated_facts(), atom.relation);
                next!(
                    State::Missing {
                        stated,
                        scope: checked
                    },
                    missing + 1
                );
                poisoned.truncate(mark);
            }
            Literal::Negative(atom) => match ground(atom, binding, program) {
                None => next!(State::Waiting, missing),
                Some(row) => match self.derivation.origin(atom.relation, &row) {
                    None => next!(State::Met, missing),
                    Some(origin) => {
                        let by = blocker(self.derivation, atom.relation, &row, origin);
                        next!(State::Blocked { by }, missing + 1);
                    }
                },
            },
            Literal::Compare { op, left, right } => {
                let outcome = evaluator
                    .expr(left, binding)
                    .and_then(|l| Ok((l, evaluator.expr(right, binding)?)))
                    .and_then(|(l, r)| evaluator.compare(*op, &l, &r));
                match settle(outcome)? {
                    Some(true) => next!(State::Met, missing),
                    Some(false) => next!(State::False, missing + 1),
                    None => next!(State::Waiting, missing),
                }
            }
            Literal::Boolean(expression) => match settle(evaluator.boolean(expression, binding))? {
                Some(true) => next!(State::Met, missing),
                Some(false) => next!(State::False, missing + 1),
                None => next!(State::Waiting, missing),
            },
            Literal::Bind { variable, value } => match settle(evaluator.expr(value, binding))? {
                Some(value) => match binding.get(variable) {
                    Some(existing) if *existing == value => next!(State::Met, missing),
                    Some(_) => next!(State::False, missing + 1),
                    None => {
                        binding.insert(variable.clone(), value);
                        next!(State::Met, missing);
                        binding.remove(variable);
                    }
                },
                None => {
                    poisoned.push(variable.clone());
                    next!(State::Waiting, missing);
                    poisoned.pop();
                }
            },
            Literal::Member { variable, list } => match settle(evaluator.expr(list, binding))? {
                Some(Value::List(items)) => {
                    for item in items {
                        self.meter.charge("why not")?;
                        match binding.get(variable) {
                            Some(existing) if *existing != item => continue,
                            Some(_) => next!(State::Met, missing),
                            None => {
                                binding.insert(variable.clone(), item);
                                next!(State::Met, missing);
                                binding.remove(variable);
                            }
                        }
                    }
                    poisoned.push(variable.clone());
                    next!(State::False, missing + 1);
                    poisoned.pop();
                }
                _ => {
                    poisoned.push(variable.clone());
                    next!(State::Waiting, missing);
                    poisoned.pop();
                }
            },
            Literal::Aggregate { variable, .. } | Literal::ArgMin { variable, .. } => {
                poisoned.push(variable.clone());
                next!(State::Waiting, missing);
                poisoned.pop();
            }
        }
        Ok(())
    }
}

fn settle<T>(outcome: Result<T, EvalError>) -> Result<Option<T>, EvalError> {
    match outcome {
        Ok(value) => Ok(Some(value)),
        Err(EvalError::Unknown(reason)) => Err(EvalError::Unknown(reason)),
        Err(_) => Ok(None),
    }
}

fn variables(arguments: &[Expr]) -> impl Iterator<Item = Variable> + '_ {
    arguments.iter().filter_map(|argument| match argument {
        Expr::Variable(variable) => Some(variable.clone()),
        _ => None,
    })
}

fn waits(arguments: &[Expr], binding: &Binding, poisoned: &[Variable]) -> bool {
    variables(arguments).any(|v| !binding.contains_key(&v) && poisoned.contains(&v))
}

fn fits(arguments: &[Expr], row: &[Value], binding: &mut Binding, evaluator: Option<&Evaluator>) -> bool {
    let mut extended = binding.clone();
    for (argument, value) in arguments.iter().zip(row) {
        let ok = match argument {
            Expr::Value(constant) => constant == value,
            Expr::Variable(variable) => extended.entry(variable.clone()).or_insert_with(|| value.clone()) == value,
            _ => true,
        };
        if !ok {
            return false;
        }
    }
    if let Some(evaluator) = evaluator {
        let computed = arguments.iter().zip(row).all(|(argument, value)| match argument {
            Expr::Value(_) | Expr::Variable(_) => true,
            complex => evaluator.expr(complex, &extended).is_ok_and(|v| v == *value),
        });
        if !computed {
            return false;
        }
    }
    *binding = extended;
    true
}

fn bound(expression: &Expr, binding: &Binding, also: &[Variable]) -> bool {
    let mut all = true;
    expr_variables(expression, &mut |v| all &= binding.contains_key(v) || also.contains(v));
    all
}

fn next_literal(body: &[Literal], done: &[bool], binding: &Binding, poisoned: &[Variable]) -> Option<usize> {
    let ready = |literal: &Literal| match literal {
        Literal::Positive(atom) => {
            let simple: Vec<Variable> = variables(&atom.arguments).collect();
            !waits(&atom.arguments, binding, poisoned) && atom.arguments.iter().all(|a| bound(a, binding, &simple))
        }
        Literal::Negative(atom) => atom.arguments.iter().all(|a| bound(a, binding, &[])),
        Literal::Compare { left, right, .. } => bound(left, binding, &[]) && bound(right, binding, &[]),
        Literal::Boolean(expression) => bound(expression, binding, &[]),
        Literal::Bind { value, .. } => bound(value, binding, &[]),
        Literal::Member { list, .. } => bound(list, binding, &[]),
        Literal::Aggregate { .. } | Literal::ArgMin { .. } => true,
    };
    let filter = |literal: &Literal| match literal {
        Literal::Positive(atom) => atom.arguments.iter().all(|a| bound(a, binding, &[])),
        _ => true,
    };
    let open = || (0..body.len()).filter(|&i| !done[i] && ready(&body[i]));
    open().find(|&i| filter(&body[i])).or_else(|| open().next())
}

fn head_agrees(program: &Program, arguments: &[Expr], target: &[Value], binding: &Binding) -> bool {
    let evaluator = Evaluator { program, at: "why not" };
    arguments.iter().zip(target).all(|(argument, value)| match argument {
        Expr::Value(_) | Expr::Variable(_) => true,
        complex => evaluator.expr(complex, binding).map_or(true, |v| v == *value),
    })
}

fn ground(atom: &Atom, binding: &Binding, program: &Program) -> Option<Vec<Value>> {
    let evaluator = Evaluator { program, at: "why not" };
    atom.arguments
        .iter()
        .map(|argument| evaluator.expr(argument, binding).ok())
        .collect()
}

fn blocker(derivation: &Derivation, relation: RelationId, row: &[Value], origin: Origin) -> String {
    let program = derivation.program();
    match origin {
        Origin::Stated(index) => fact_line(program, derivation.stated(index).expect("a recorded stated fact exists")),
        Origin::Derived { rule, .. } => {
            let rule = &program.rules[rule];
            let at = rule
                .source
                .as_ref()
                .map(|_| format!(" ({})", located(&rule.source)))
                .unwrap_or_default();
            format!("{}    ← {}{at}", sentence(program, relation, row), rule.name)
        }
    }
}

fn substitute(expression: &Expr, binding: &Binding) -> Expr {
    let all = |items: &[Expr]| items.iter().map(|e| substitute(e, binding)).collect();
    match expression {
        Expr::Variable(variable) => binding
            .get(variable)
            .map_or_else(|| expression.clone(), |value| Expr::Value(value.clone())),
        Expr::List(items) => Expr::List(all(items)),
        Expr::Field { value, field } => Expr::Field {
            value: Box::new(substitute(value, binding)),
            field: field.clone(),
        },
        Expr::Unary { op, value } => Expr::Unary {
            op: *op,
            value: Box::new(substitute(value, binding)),
        },
        Expr::Binary { op, left, right } => Expr::binary(*op, substitute(left, binding), substitute(right, binding)),
        Expr::Call { function, arguments } => Expr::call(function.clone(), all(arguments)),
        other => other.clone(),
    }
}

fn substitute_literal(literal: &Literal, binding: &Binding) -> Literal {
    let atom = |atom: &Atom| Atom {
        relation: atom.relation,
        arguments: atom.arguments.iter().map(|e| substitute(e, binding)).collect(),
        source: atom.source.clone(),
    };
    match literal {
        Literal::Positive(a) => Literal::Positive(atom(a)),
        Literal::Negative(a) => Literal::Negative(atom(a)),
        Literal::Compare { op, left, right } => Literal::Compare {
            op: *op,
            left: substitute(left, binding),
            right: substitute(right, binding),
        },
        Literal::Boolean(e) => Literal::Boolean(substitute(e, binding)),
        Literal::Bind { variable, value } => Literal::Bind {
            variable: variable.clone(),
            value: substitute(value, binding),
        },
        Literal::Member { variable, list } => Literal::Member {
            variable: variable.clone(),
            list: substitute(list, binding),
        },
        other => other.clone(),
    }
}

pub fn render(
    program: &Program,
    relation: RelationId,
    values: &[Value],
    via: Option<&Via>,
    verdict: &Verdict<(), Vec<Attempt>>,
    budget: Budget,
) -> String {
    let asked = sentence(program, relation, values);
    let rules = match via {
        Some(via) => format!("rule {}", via.rule),
        None => "each rule that concludes it".to_string(),
    };
    match verdict {
        Verdict::Holds(()) => format!("{asked} holds. Ask `why` for its proof."),
        Verdict::Fails(attempts) if attempts.is_empty() => {
            let checked = checked(&scope(program, program.facts.iter(), relation));
            match via {
                Some(via) => format!("{asked} does not hold, and rule {} cannot conclude it.", via.rule),
                None => format!(
                    "{asked} does not hold: nothing states it and no rule concludes it    (checked: {checked})"
                ),
            }
        }
        Verdict::Fails(attempts) => {
            let mut out = vec![format!(
                "{asked} does not hold. The closest binding of {rules}, with missing conditions explained {DEPTH} levels deep, within a work budget of {} units:",
                budget.work
            )];
            for attempt in attempts {
                out.push(String::new());
                block(program, attempt, "  ", &mut out);
            }
            out.join("\n")
        }
        Verdict::Unknown(reason) => format!(
            "Why not {asked} is unknown: {reason}, within a work budget of {} units over the whole program. Unknown is not an approval.",
            budget.work
        ),
    }
}

fn checked(scope: &[String]) -> String {
    if scope.is_empty() {
        "every stated fact of the program".into()
    } else {
        scope.join(", ")
    }
}

fn block(program: &Program, attempt: &Attempt, indent: &str, out: &mut Vec<String>) {
    let rule = &program.rules[attempt.rule];
    let at = rule
        .source
        .as_ref()
        .map(|_| format!(" ({})", located(&rule.source)))
        .unwrap_or_default();
    out.push(format!("{indent}{}{at} needs:", rule.name));
    for condition in &attempt.conditions {
        let (mark, note) = match &condition.state {
            State::Met => ("✓", String::new()),
            State::Missing { stated: true, scope } => {
                ("✗", format!("   (nothing states this; checked: {})", checked(scope)))
            }
            State::Missing { stated: false, .. } => ("✗", String::new()),
            State::Blocked { by } => ("✗", format!("   (blocked by: {by})")),
            State::False => ("✗", "   (false)".into()),
            State::Waiting => ("·", String::new()),
        };
        out.push(format!("{indent}  {mark} {}{note}", condition.text));
        if let Some(deeper) = condition.deeper.first() {
            block(program, deeper, &format!("{indent}      "), out);
        }
    }
}
