use crate::check::nested_order;
use crate::expr::{self, Binding, Evaluator, Meter, Queries, Scope};
use crate::*;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict<Evidence, Counterexample> {
    Holds(Evidence),
    Fails(Counterexample),
    Unknown(Reason),
}

impl<E, C> Verdict<E, C> {
    pub fn approves(&self) -> bool {
        matches!(self, Self::Holds(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    Work,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    OutOfBudget { resource: Resource, at: String },
    Unsupported { construct: String, at: String },
    ModeFails { conflict: String },
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBudget {
                resource: Resource::Work,
                at,
            } => write!(f, "work budget ran out in {at}"),
            Self::Unsupported { construct, at } => {
                write!(f, "{construct} is not supported yet, in {at}")
            }
            Self::ModeFails { conflict } => write!(f, "the model fails a mode: {conflict}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    pub work: u64,
}

impl Budget {
    pub fn work(work: u64) -> Self {
        Self { work }
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self { work: 10_000_000 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvalError {
    Static(Vec<StaticError>),
    Unknown(Reason),
    Overflow { at: String },
    Type { at: String, message: String },
    Mode(Vec<crate::modes::ModeFailure>),
}

impl From<Reason> for EvalError {
    fn from(reason: Reason) -> Self {
        Self::Unknown(reason)
    }
}

pub type Tuples = BTreeSet<Vec<Value>>;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Model {
    pub relations: BTreeMap<RelationId, Tuples>,
    pub work: u64,
}

impl Model {
    pub fn tuples(&self, relation: RelationId) -> impl Iterator<Item = &Vec<Value>> {
        self.relations.get(&relation).into_iter().flatten()
    }
    pub fn contains(&self, relation: RelationId, tuple: &[Value]) -> bool {
        self.relations
            .get(&relation)
            .is_some_and(|tuples| tuples.contains(tuple))
    }
}

pub fn evaluate(program: &Program, state: &[Fact], budget: Budget) -> Result<Model, EvalError> {
    evaluate_blocking(program, state, budget, &|_, _| false)
}

pub fn evaluate_blocking(
    program: &Program,
    state: &[Fact],
    budget: Budget,
    blocked: &dyn Fn(RelationId, &[Value]) -> bool,
) -> Result<Model, EvalError> {
    let checked = check(program).map_err(EvalError::Static)?;
    let mut meter = Meter {
        limit: budget.work,
        spent: 0,
    };
    let mut relations: BTreeMap<RelationId, Tuples> = BTreeMap::new();
    for fact in program.facts.iter().chain(state) {
        meter.charge("stated facts")?;
        if blocked(fact.relation, &fact.values) {
            continue;
        }
        relations
            .entry(fact.relation)
            .or_default()
            .insert(fact.values.clone());
    }
    let mut model = Model {
        relations,
        work: 0,
    };
    let top = checked.strata.iter().copied().max().unwrap_or(0);
    for stratum in 0..=top {
        let rules: Vec<usize> = (0..program.rules.len())
            .filter(|&index| checked.strata[program.rules[index].head.relation.0] == stratum)
            .collect();
        loop {
            let mut derived = Vec::new();
            for &index in &rules {
                let rule = &program.rules[index];
                let context = Context {
                    program,
                    model: &model,
                    meter: RefCell::new(&mut meter),
                    at: &rule.name,
                };
                let body: Vec<&Literal> = checked.schedules[index]
                    .iter()
                    .map(|&position| &rule.body[position])
                    .collect();
                for binding in context.solve(&body, Binding::new())? {
                    context.charge()?;
                    let tuple = rule
                        .head
                        .arguments
                        .iter()
                        .map(|argument| context.expr(argument, &binding))
                        .collect::<Result<Vec<_>, _>>()?;
                    derived.push((rule.head.relation, tuple));
                }
            }
            let mut grew = false;
            for (relation, tuple) in derived {
                if blocked(relation, &tuple) {
                    continue;
                }
                grew |= model.relations.entry(relation).or_default().insert(tuple);
            }
            if !grew {
                break;
            }
        }
        let failures = crate::modes::check_stratum(program, &checked.strata, stratum, &|relation, each| {
            model.tuples(relation).for_each(|tuple| each(tuple))
        });
        if !failures.is_empty() {
            return Err(EvalError::Mode(failures));
        }
    }
    model.work = meter.spent;
    Ok(model)
}

/// Answers a query against an evaluated model, with the enclosing scope's names already bound.
pub fn answer(
    program: &Program,
    model: &Model,
    query: &Query,
    outer: &[(Variable, Value)],
    budget: Budget,
) -> Result<Value, EvalError> {
    let mut meter = Meter {
        limit: budget.work,
        spent: 0,
    };
    let context = Context {
        program,
        model,
        meter: RefCell::new(&mut meter),
        at: "query",
    };
    context.query(query, &outer.iter().cloned().collect())
}

pub(crate) fn matches(
    program: &Program,
    model: &Model,
    body: &[Literal],
    binding: &Binding,
    budget: Budget,
) -> Result<Vec<Binding>, EvalError> {
    let mut meter = Meter { limit: budget.work, spent: 0 };
    let context = Context { program, model, meter: RefCell::new(&mut meter), at: "handler clause" };
    let bound = binding.keys().cloned().collect();
    let order = nested_order(program, body, &[], &bound);
    let literals: Vec<&Literal> = order.iter().map(|&position| &body[position]).collect();
    context.solve(&literals, binding.clone())
}

pub(crate) fn value(
    program: &Program,
    model: &Model,
    expression: &Expr,
    binding: &Binding,
    budget: Budget,
) -> Result<Value, EvalError> {
    let mut meter = Meter { limit: budget.work, spent: 0 };
    let context = Context { program, model, meter: RefCell::new(&mut meter), at: "handler action" };
    context.expr(expression, binding)
}

struct Context<'a> {
    program: &'a Program,
    model: &'a Model,
    meter: RefCell<&'a mut Meter>,
    at: &'a str,
}

impl<'a> Context<'a> {
    fn solve(&self, body: &[&Literal], binding: Binding) -> Result<Vec<Binding>, EvalError> {
        let Some((literal, rest)) = body.split_first() else {
            return Ok(vec![binding]);
        };
        let mut results = Vec::new();
        for extended in self.literal(literal, &binding)? {
            results.extend(self.solve(rest, extended)?);
        }
        Ok(results)
    }

    fn literal(&self, literal: &Literal, binding: &Binding) -> Result<Vec<Binding>, EvalError> {
        match literal {
            Literal::Positive(atom) => {
                let mut results = Vec::new();
                for tuple in self.model.tuples(atom.relation) {
                    self.charge()?;
                    if let Some(extended) = self.unify(&atom.arguments, tuple, binding)? {
                        results.push(extended);
                    }
                }
                Ok(results)
            }
            Literal::Negative(atom) => {
                self.charge()?;
                let tuple = self.exprs(&atom.arguments, binding)?;
                Ok(self.pass(!self.model.contains(atom.relation, &tuple), binding))
            }
            Literal::Compare { op, left, right } => {
                self.charge()?;
                let left = self.expr(left, binding)?;
                let right = self.expr(right, binding)?;
                let holds = self.eval().compare(*op, &left, &right)?;
                Ok(self.pass(holds, binding))
            }
            Literal::Boolean(expression) => {
                self.charge()?;
                let holds = self.boolean(expression, binding)?;
                Ok(self.pass(holds, binding))
            }
            Literal::Bind { variable, value } => {
                self.charge()?;
                let value = self.expr(value, binding)?;
                Ok(bind(binding, variable, value).into_iter().collect())
            }
            Literal::Member { variable, list } => {
                self.charge()?;
                let Value::List(items) = self.expr(list, binding)? else {
                    return Err(self.eval().type_error("membership needs a list"));
                };
                let mut results = Vec::new();
                for item in items {
                    self.charge()?;
                    results.extend(bind(binding, variable, item));
                }
                Ok(results)
            }
            Literal::Aggregate { variable, op, body } => {
                self.charge()?;
                let extras: Vec<&Expr> = op.value().into_iter().collect();
                let rows = self.distinct(body, &extras, binding)?;
                let at = self.at;
                let result = expr::aggregate(op, &rows, &mut |e, row| self.expr(e, row), at)?;
                Ok(result
                    .and_then(|value| bind(binding, variable, value))
                    .into_iter()
                    .collect())
            }
            Literal::ArgMin {
                variable,
                candidate,
                key,
                body,
            } => {
                self.charge()?;
                let rows = self.distinct(body, &[candidate, key], binding)?;
                let at = self.at;
                let winner = expr::argmin(candidate, key, &rows, &mut |e, row| self.expr(e, row), at)?;
                Ok(winner
                    .and_then(|winner| bind(binding, variable, winner))
                    .into_iter()
                    .collect())
            }
        }
    }

    fn distinct(
        &self,
        body: &[Literal],
        extras: &[&Expr],
        binding: &Binding,
    ) -> Result<BTreeSet<Binding>, EvalError> {
        let bound = binding.keys().cloned().collect();
        let order = nested_order(self.program, body, extras, &bound);
        let literals: Vec<&Literal> = order.iter().map(|&position| &body[position]).collect();
        let mut rows = BTreeSet::new();
        for row in self.solve(&literals, binding.clone())? {
            self.charge()?;
            rows.insert(row);
        }
        Ok(rows)
    }

    fn query(&self, query: &Query, binding: &Binding) -> Result<Value, EvalError> {
        self.charge()?;
        let rows = self.distinct(&query.body, &expr::query_extras(query), binding)?;
        let at = self.at;
        expr::query(query, &rows, &mut |e, row| self.expr(e, row), at)
    }

    fn pass(&self, holds: bool, binding: &Binding) -> Vec<Binding> {
        if holds {
            vec![binding.clone()]
        } else {
            Vec::new()
        }
    }

    fn unify(
        &self,
        arguments: &[Expr],
        tuple: &[Value],
        binding: &Binding,
    ) -> Result<Option<Binding>, EvalError> {
        let mut extended = binding.clone();
        for (argument, value) in arguments.iter().zip(tuple) {
            if let Expr::Variable(variable) = argument {
                match bind(&extended, variable, value.clone()) {
                    Some(next) => extended = next,
                    None => return Ok(None),
                }
            } else if self.expr(argument, &extended)? != *value {
                return Ok(None);
            }
        }
        Ok(Some(extended))
    }

    fn charge(&self) -> Result<(), EvalError> {
        self.meter.borrow_mut().charge(self.at)
    }

    fn eval(&self) -> Evaluator<'_, 'a> {
        Evaluator {
            program: self.program,
            at: self.at,
            queries: Some(self),
            meter: &self.meter,
        }
    }

    fn exprs(&self, expressions: &[Expr], binding: &Binding) -> Result<Vec<Value>, EvalError> {
        self.eval().exprs(expressions, binding)
    }

    fn boolean(&self, expression: &Expr, binding: &Binding) -> Result<bool, EvalError> {
        self.eval().boolean(expression, binding)
    }

    fn expr(&self, expression: &Expr, binding: &Binding) -> Result<Value, EvalError> {
        self.eval().expr(expression, binding)
    }
}

impl Queries for Context<'_> {
    fn query(&self, query: &Query, scope: &dyn Scope) -> Result<Value, EvalError> {
        Context::query(self, query, &scope.all())
    }
}

fn bind(binding: &Binding, variable: &Variable, value: Value) -> Option<Binding> {
    match binding.get(variable) {
        Some(existing) => (*existing == value).then(|| binding.clone()),
        None => {
            let mut extended = binding.clone();
            extended.insert(variable.clone(), value);
            Some(extended)
        }
    }
}
