use crate::check::nested_order;
use crate::*;
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

type Binding = BTreeMap<Variable, Value>;

struct Meter {
    limit: u64,
    spent: u64,
}

impl Meter {
    fn charge(&mut self, at: &str) -> Result<(), EvalError> {
        self.spent += 1;
        if self.spent > self.limit {
            return Err(Reason::OutOfBudget {
                resource: Resource::Work,
                at: at.to_string(),
            }
            .into());
        }
        Ok(())
    }
}

pub fn evaluate(program: &Program, state: &[Fact], budget: Budget) -> Result<Model, EvalError> {
    let checked = check(program).map_err(EvalError::Static)?;
    let mut meter = Meter {
        limit: budget.work,
        spent: 0,
    };
    let mut relations: BTreeMap<RelationId, Tuples> = BTreeMap::new();
    for fact in program.facts.iter().chain(state) {
        meter.charge("stated facts")?;
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
                let mut context = Context {
                    program,
                    model: &model,
                    meter: &mut meter,
                    at: &rule.name,
                };
                let body: Vec<&Literal> = checked.schedules[index]
                    .iter()
                    .map(|&position| &rule.body[position])
                    .collect();
                for binding in context.solve(&body, Binding::new())? {
                    context.meter.charge(&rule.name)?;
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
                grew |= model.relations.entry(relation).or_default().insert(tuple);
            }
            if !grew {
                break;
            }
        }
    }
    model.work = meter.spent;
    Ok(model)
}

struct Context<'a> {
    program: &'a Program,
    model: &'a Model,
    meter: &'a mut Meter,
    at: &'a str,
}

impl Context<'_> {
    fn solve(&mut self, body: &[&Literal], binding: Binding) -> Result<Vec<Binding>, EvalError> {
        let Some((literal, rest)) = body.split_first() else {
            return Ok(vec![binding]);
        };
        let mut results = Vec::new();
        for extended in self.literal(literal, &binding)? {
            results.extend(self.solve(rest, extended)?);
        }
        Ok(results)
    }

    fn literal(&mut self, literal: &Literal, binding: &Binding) -> Result<Vec<Binding>, EvalError> {
        match literal {
            Literal::Positive(atom) => {
                let mut results = Vec::new();
                for tuple in self.model.tuples(atom.relation) {
                    self.meter.charge(self.at)?;
                    if let Some(extended) = self.unify(&atom.arguments, tuple, binding)? {
                        results.push(extended);
                    }
                }
                Ok(results)
            }
            Literal::Negative(atom) => {
                self.meter.charge(self.at)?;
                let tuple = self.exprs(&atom.arguments, binding)?;
                Ok(self.pass(!self.model.contains(atom.relation, &tuple), binding))
            }
            Literal::Compare { op, left, right } => {
                self.meter.charge(self.at)?;
                let left = self.expr(left, binding)?;
                let right = self.expr(right, binding)?;
                let holds = self.compare(*op, &left, &right)?;
                Ok(self.pass(holds, binding))
            }
            Literal::Boolean(expression) => {
                self.meter.charge(self.at)?;
                let holds = self.boolean(expression, binding)?;
                Ok(self.pass(holds, binding))
            }
            Literal::Bind { variable, value } => {
                self.meter.charge(self.at)?;
                let value = self.expr(value, binding)?;
                Ok(bind(binding, variable, value).into_iter().collect())
            }
            Literal::Member { variable, list } => {
                let Value::List(items) = self.expr(list, binding)? else {
                    return Err(self.type_error("membership needs a list"));
                };
                let mut results = Vec::new();
                for item in items {
                    self.meter.charge(self.at)?;
                    results.extend(bind(binding, variable, item));
                }
                Ok(results)
            }
            Literal::Aggregate {
                variable,
                op,
                value,
                body,
            } => {
                self.meter.charge(self.at)?;
                let rows = self.distinct(body, &[value], binding)?;
                let mut values = Vec::new();
                for row in &rows {
                    values.push(self.expr(value, row)?);
                }
                let result = match op {
                    AggregateOp::Count => Some(Value::Int(
                        i64::try_from(rows.len()).map_err(|_| self.overflow())?,
                    )),
                    AggregateOp::Sum => Some(self.exact_sum(&values)?),
                    AggregateOp::Min => values.into_iter().min(),
                    AggregateOp::Max => values.into_iter().max(),
                };
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
                self.meter.charge(self.at)?;
                let rows = self.distinct(body, &[candidate, key], binding)?;
                let mut ranked = BTreeSet::new();
                for row in &rows {
                    ranked.insert((self.expr(key, row)?, self.expr(candidate, row)?));
                }
                Ok(ranked
                    .pop_first()
                    .and_then(|(_, winner)| bind(binding, variable, winner))
                    .into_iter()
                    .collect())
            }
        }
    }

    fn distinct(
        &mut self,
        body: &[Literal],
        extras: &[&Expr],
        binding: &Binding,
    ) -> Result<BTreeSet<Binding>, EvalError> {
        let bound = binding.keys().cloned().collect();
        let order = nested_order(self.program, body, extras, &bound);
        let literals: Vec<&Literal> = order.iter().map(|&position| &body[position]).collect();
        let mut rows = BTreeSet::new();
        for row in self.solve(&literals, binding.clone())? {
            self.meter.charge(self.at)?;
            rows.insert(row);
        }
        Ok(rows)
    }

    fn exact_sum(&self, values: &[Value]) -> Result<Value, EvalError> {
        let mut total: i128 = 0;
        for value in values {
            let Value::Int(value) = value else {
                return Err(self.type_error("sum adds Int values only"));
            };
            total = total
                .checked_add(i128::from(*value))
                .ok_or_else(|| self.overflow())?;
        }
        Ok(Value::Int(i64::try_from(total).map_err(|_| self.overflow())?))
    }

    fn query(&mut self, query: &Query, binding: &Binding) -> Result<Value, EvalError> {
        self.meter.charge(self.at)?;
        let sort: Vec<&Expr> = query.sorted_by.iter().map(|sort| sort.as_ref()).collect();
        let rows = self.distinct(&query.body, &sort, binding)?;
        let declared = match &query.kind {
            QueryKind::All(variables) => variables.as_slice(),
            QueryKind::The(variable) => std::slice::from_ref(variable),
        };
        let mut ranked = BTreeSet::new();
        for row in &rows {
            let mut values = Vec::new();
            for variable in declared {
                values.push(self.expr(&Expr::Variable(variable.clone()), row)?);
            }
            let projected = match <[Value; 1]>::try_from(values) {
                Ok([value]) => value,
                Err(values) => Value::List(values),
            };
            let key = match &query.sorted_by {
                Some(sort) => self.expr(sort, row)?,
                None => projected.clone(),
            };
            ranked.insert((key, projected));
        }
        let mut seen = BTreeSet::new();
        let results: Vec<Value> = ranked
            .into_iter()
            .map(|(_, projected)| projected)
            .filter(|projected| seen.insert(projected.clone()))
            .collect();
        match query.kind {
            QueryKind::All(_) => Ok(Value::List(results)),
            QueryKind::The(_) => match <[Value; 1]>::try_from(results) {
                Ok([value]) => Ok(value),
                Err(results) if results.is_empty() => Ok(Value::Null),
                Err(results) => Err(self.type_error(&format!(
                    "the found {} bindings where a mode allows at most one",
                    results.len()
                ))),
            },
        }
    }

    fn pass(&self, holds: bool, binding: &Binding) -> Vec<Binding> {
        if holds {
            vec![binding.clone()]
        } else {
            Vec::new()
        }
    }

    fn unify(
        &mut self,
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

    fn exprs(&mut self, expressions: &[Expr], binding: &Binding) -> Result<Vec<Value>, EvalError> {
        expressions
            .iter()
            .map(|expression| self.expr(expression, binding))
            .collect()
    }

    fn boolean(&mut self, expression: &Expr, binding: &Binding) -> Result<bool, EvalError> {
        match self.expr(expression, binding)? {
            Value::Bool(value) => Ok(value),
            _ => Err(self.type_error("a condition must be a Bool")),
        }
    }

    fn expr(&mut self, expression: &Expr, binding: &Binding) -> Result<Value, EvalError> {
        Ok(match expression {
            Expr::Value(value) => value.clone(),
            Expr::Variable(variable) => match binding.get(variable) {
                Some(value) => value.clone(),
                None => return Err(self.type_error(&format!("?{} is not bound", variable.0))),
            },
            Expr::List(items) => Value::List(self.exprs(items, binding)?),
            Expr::Record(fields) => Value::Record(self.fields(fields, binding)?),
            Expr::Attrs(fields) => Value::Attrs(self.fields(fields, binding)?),
            Expr::Field { value, field } => match self.expr(value, binding)? {
                Value::Record(fields) | Value::Attrs(fields) => match fields.get(field) {
                    Some(value) => value.clone(),
                    None => return Err(self.type_error(&format!("no field {field}"))),
                },
                Value::Entity(_) => return Err(self.unsupported("an entity field")),
                _ => return Err(self.type_error(&format!("no field {field}"))),
            },
            Expr::Unary { op, value } => match (op, self.expr(value, binding)?) {
                (UnaryOp::Not, Value::Bool(value)) => Value::Bool(!value),
                (UnaryOp::Negate, Value::Int(value)) => {
                    Value::Int(value.checked_neg().ok_or_else(|| self.overflow())?)
                }
                _ => return Err(self.type_error("wrong operand for a unary operator")),
            },
            Expr::Binary { op, left, right } => self.binary(*op, left, right, binding)?,
            Expr::If {
                condition,
                then_value,
                else_value,
            } => {
                if self.boolean(condition, binding)? {
                    self.expr(then_value, binding)?
                } else {
                    self.expr(else_value, binding)?
                }
            }
            Expr::Call {
                function,
                arguments,
            } => {
                let Some(definition) = self
                    .program
                    .definitions
                    .iter()
                    .find(|definition| &definition.name == function)
                else {
                    return Err(self.unsupported(&format!("the built-in {function}")));
                };
                if definition.parameters.len() != arguments.len() {
                    return Err(self.type_error(&format!("{function} takes {} arguments", definition.parameters.len())));
                }
                let mut scope = Binding::new();
                for (parameter, argument) in definition.parameters.iter().zip(arguments) {
                    let value = self.expr(argument, binding)?;
                    scope.insert(Variable::new(parameter.name.clone()), value);
                }
                self.expr(&definition.body, &scope)?
            }
            Expr::Lambda { .. } | Expr::Apply { .. } => {
                return Err(self.unsupported("a function value"))
            }
            Expr::Query(query) => self.query(query, binding)?,
        })
    }

    fn fields(
        &mut self,
        fields: &BTreeMap<String, Expr>,
        binding: &Binding,
    ) -> Result<BTreeMap<String, Value>, EvalError> {
        fields
            .iter()
            .map(|(name, expression)| Ok((name.clone(), self.expr(expression, binding)?)))
            .collect()
    }

    fn binary(
        &mut self,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        binding: &Binding,
    ) -> Result<Value, EvalError> {
        if let BinaryOp::And | BinaryOp::Or = op {
            let left = self.boolean(left, binding)?;
            return Ok(Value::Bool(match op {
                BinaryOp::And => left && self.boolean(right, binding)?,
                _ => left || self.boolean(right, binding)?,
            }));
        }
        let left = self.expr(left, binding)?;
        let right = self.expr(right, binding)?;
        Ok(match (op, left, right) {
            (BinaryOp::Compare(op), left, right) => Value::Bool(self.compare(op, &left, &right)?),
            (BinaryOp::Add, Value::Int(a), Value::Int(b)) => {
                Value::Int(a.checked_add(b).ok_or_else(|| self.overflow())?)
            }
            (BinaryOp::Subtract, Value::Int(a), Value::Int(b)) => {
                Value::Int(a.checked_sub(b).ok_or_else(|| self.overflow())?)
            }
            (BinaryOp::Multiply, Value::Int(a), Value::Int(b)) => {
                Value::Int(a.checked_mul(b).ok_or_else(|| self.overflow())?)
            }
            (BinaryOp::Divide, Value::Int(_), Value::Int(0)) => Value::Null,
            (BinaryOp::Divide, Value::Int(a), Value::Int(b)) => {
                Value::Int(a.checked_div(b).ok_or_else(|| self.overflow())?)
            }
            (BinaryOp::Concat, Value::Text(a), Value::Text(b)) => Value::Text(a + &b),
            (BinaryOp::Concat, Value::List(mut a), Value::List(b)) => {
                a.extend(b);
                Value::List(a)
            }
            (BinaryOp::Merge, Value::Attrs(mut a), Value::Attrs(b)) => {
                a.extend(b);
                Value::Attrs(a)
            }
            (BinaryOp::Merge, Value::Record(mut a), Value::Record(b)) => {
                a.extend(b);
                Value::Record(a)
            }
            _ => return Err(self.type_error("wrong operands for a binary operator")),
        })
    }

    fn compare(&self, op: CompareOp, left: &Value, right: &Value) -> Result<bool, EvalError> {
        let ordered = matches!(
            (left, right),
            (Value::Int(_), Value::Int(_)) | (Value::Text(_), Value::Text(_))
        );
        Ok(match op {
            CompareOp::Equal => left == right,
            CompareOp::NotEqual => left != right,
            _ if !ordered => return Err(self.type_error("ordering compares Int or Text only")),
            CompareOp::Less => left < right,
            CompareOp::LessEqual => left <= right,
            CompareOp::Greater => left > right,
            CompareOp::GreaterEqual => left >= right,
        })
    }

    fn unsupported(&self, construct: &str) -> EvalError {
        Reason::Unsupported {
            construct: construct.to_string(),
            at: self.at.to_string(),
        }
        .into()
    }
    fn overflow(&self) -> EvalError {
        EvalError::Overflow {
            at: self.at.to_string(),
        }
    }
    fn type_error(&self, message: &str) -> EvalError {
        EvalError::Type {
            at: self.at.to_string(),
            message: message.to_string(),
        }
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
