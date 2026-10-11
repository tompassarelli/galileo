use crate::*;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) type Binding = BTreeMap<Variable, Value>;

pub(crate) type Rows = BTreeSet<Binding>;

pub(crate) type Eval<'e> = dyn FnMut(&Expr, &Binding) -> Result<Value, EvalError> + 'e;

pub(crate) fn aggregate(
    op: &AggregateOp,
    rows: &Rows,
    eval: &mut Eval,
    at: &str,
) -> Result<Option<Value>, EvalError> {
    let mut values = Vec::new();
    if let Some(value) = op.value() {
        for row in rows {
            values.push(eval(value, row)?);
        }
    }
    Ok(match op {
        AggregateOp::Count => Some(Value::Int(
            i64::try_from(rows.len()).map_err(|_| overflow(at))?,
        )),
        AggregateOp::Sum(_) => Some(exact_sum(&values, at)?),
        AggregateOp::Min(_) => ordered(&values, "min", at)?.min(),
        AggregateOp::Max(_) => ordered(&values, "max", at)?.max(),
    })
}

pub(crate) fn argmin(
    direction: ArgMinDirection,
    candidate: &Expr,
    key: &Expr,
    rows: &Rows,
    eval: &mut Eval,
    at: &str,
) -> Result<Option<Value>, EvalError> {
    let mut keys = Vec::new();
    let mut candidates = Vec::new();
    for row in rows {
        keys.push(eval(key, row)?);
        candidates.push(eval(candidate, row)?);
    }
    let ranked = ordered(&keys, "an argmin or argmax key", at)?.zip(candidates);
    Ok(ranked
        .min_by(|(left_key, left_candidate), (right_key, right_candidate)| {
            let key_order = match direction {
                ArgMinDirection::Min => left_key.cmp(right_key),
                ArgMinDirection::Max => right_key.cmp(left_key),
            };
            key_order.then_with(|| left_candidate.cmp(right_candidate))
        })
        .map(|(_, winner)| winner))
}

pub(crate) fn query(
    query: &Query,
    rows: &Rows,
    eval: &mut Eval,
    at: &str,
) -> Result<Value, EvalError> {
    let declared = match &query.kind {
        QueryKind::All(variables) => variables.as_slice(),
        QueryKind::The(variable) => std::slice::from_ref(variable),
    };
    let mut seen = BTreeSet::new();
    let mut ranked = Vec::new();
    for row in rows {
        let mut values = Vec::new();
        for variable in declared {
            values.push(eval(&Expr::Variable(variable.clone()), row)?);
        }
        if !seen.insert(values.clone()) { continue; }
        let key = match &query.sorted_by {
            Some(sort) => Some(eval(sort, row)?),
            None => None,
        };
        let value = match &query.head {
            Some(head) => eval(head, row)?,
            None => match <[Value; 1]>::try_from(values.clone()) {
                Ok([value]) => value,
                Err(values) => Value::Record(declared.iter().map(|variable| variable.0.clone()).zip(values).collect()),
            },
        };
        ranked.push((key, if query.head.is_some() { vec![value.clone()] } else { values }, value));
    }
    ranked.sort();
    let results: Vec<Value> = ranked.into_iter().map(|(_, _, value)| value).collect();
    match query.kind {
        QueryKind::All(_) => Ok(Value::List(results)),
        QueryKind::The(_) => match <[Value; 1]>::try_from(results) {
            Ok([value]) => Ok(value),
            Err(results) if results.is_empty() => Ok(Value::Null),
            Err(results) => Err(type_error(
                at,
                &format!(
                    "the found {} bindings where a mode allows at most one",
                    results.len()
                ),
            )),
        },
    }
}

pub(crate) fn query_extras(query: &Query) -> Vec<&Expr> {
    query.head.iter().chain(query.sorted_by.iter()).map(|expression| expression.as_ref()).collect()
}

fn ordered<'v>(
    values: &'v [Value],
    what: &str,
    at: &str,
) -> Result<impl Iterator<Item = Value> + 'v, EvalError> {
    if values.iter().all(|value| matches!(value, Value::Int(_)))
        || values.iter().all(|value| matches!(value, Value::Text(_)))
    {
        Ok(values.iter().cloned())
    } else {
        Err(type_error(at, &format!("{what} orders Int or Text values only")))
    }
}

fn exact_sum(values: &[Value], at: &str) -> Result<Value, EvalError> {
    let mut total: i128 = 0;
    for value in values {
        let Value::Int(value) = value else {
            return Err(type_error(at, "sum adds Int values only"));
        };
        total = total
            .checked_add(i128::from(*value))
            .ok_or_else(|| overflow(at))?;
    }
    Ok(Value::Int(i64::try_from(total).map_err(|_| overflow(at))?))
}

fn overflow(at: &str) -> EvalError {
    EvalError::Overflow { at: at.to_string() }
}

fn type_error(at: &str, message: &str) -> EvalError {
    EvalError::Type {
        at: at.to_string(),
        message: message.to_string(),
    }
}

pub(crate) struct Meter {
    pub(crate) limit: u64,
    pub(crate) spent: u64,
}

impl Meter {
    pub(crate) fn charge(&mut self, at: &str) -> Result<(), EvalError> {
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

pub(crate) trait Queries {
    fn query(&self, query: &Query, scope: &dyn Scope) -> Result<Value, EvalError>;
    fn field(&self, relation: RelationId, entity: &Entity) -> Option<Value>;
}

pub(crate) trait Scope {
    fn get(&self, variable: &Variable) -> Option<&Value>;
    fn all(&self) -> Binding;
}

impl Scope for BTreeMap<Variable, Value> {
    fn get(&self, variable: &Variable) -> Option<&Value> {
        BTreeMap::get(self, variable)
    }
    fn all(&self) -> Binding {
        self.clone()
    }
}

pub(crate) struct Evaluator<'a, 'm> {
    pub(crate) program: &'a Program,
    pub(crate) at: &'a str,
    pub(crate) queries: Option<&'a dyn Queries>,
    pub(crate) meter: &'a RefCell<&'m mut Meter>,
}

impl Evaluator<'_, '_> {
    pub(crate) fn boolean(&self, expression: &Expr, scope: &dyn Scope) -> Result<bool, EvalError> {
        match self.expr(expression, scope)? {
            Value::Bool(value) => Ok(value),
            _ => Err(self.type_error("a condition must be a Bool")),
        }
    }

    pub(crate) fn exprs(&self, expressions: &[Expr], scope: &dyn Scope) -> Result<Vec<Value>, EvalError> {
        let mut values = Vec::new();
        for expression in expressions {
            self.meter.borrow_mut().charge(self.at)?;
            values.push(self.expr(expression, scope)?);
        }
        Ok(values)
    }

    fn fields(
        &self,
        fields: &BTreeMap<String, Expr>,
        scope: &dyn Scope,
    ) -> Result<BTreeMap<String, Value>, EvalError> {
        let mut values = BTreeMap::new();
        for (name, expression) in fields {
            self.meter.borrow_mut().charge(self.at)?;
            values.insert(name.clone(), self.expr(expression, scope)?);
        }
        Ok(values)
    }

    pub(crate) fn expr(&self, expression: &Expr, scope: &dyn Scope) -> Result<Value, EvalError> {
        Ok(match expression {
            Expr::Value(value) => value.clone(),
            Expr::Variable(variable) => match scope.get(variable) {
                Some(value) => value.clone(),
                None => return Err(self.type_error(&format!("?{} is not bound", variable.0))),
            },
            Expr::List(items) => Value::List(self.exprs(items, scope)?),
            Expr::Record(fields) => Value::Record(self.fields(fields, scope)?),
            Expr::Attrs(fields) => Value::Attrs(self.fields(fields, scope)?),
            Expr::Field { value, field } => match self.expr(value, scope)? {
                Value::Record(fields) | Value::Attrs(fields) => match fields.get(field) {
                    Some(value) => value.clone(),
                    None => return Err(self.type_error(&format!("no field {field}"))),
                },
                Value::Entity(entity) => {
                    let Some(relation) = self.program.field_relation(&entity.entity_type, field) else {
                        return Err(self.type_error(&format!("no field {field}")));
                    };
                    let Some(queries) = self.queries else {
                        return Err(self.unsupported("a field outside an evaluated state"));
                    };
                    self.meter.borrow_mut().charge(self.at)?;
                    queries.field(relation, &entity).unwrap_or(Value::Null)
                },
                _ => return Err(self.type_error(&format!("no field {field}"))),
            },
            Expr::Unary { op, value } => match (op, self.expr(value, scope)?) {
                (UnaryOp::Not, Value::Bool(value)) => Value::Bool(!value),
                (UnaryOp::Negate, Value::Int(value)) => {
                    Value::Int(value.checked_neg().ok_or_else(|| self.overflow())?)
                }
                _ => return Err(self.type_error("wrong operand for a unary operator")),
            },
            Expr::Binary { op, left, right } => self.binary(*op, left, right, scope)?,
            Expr::If {
                condition,
                then_value,
                else_value,
            } => {
                if self.boolean(condition, scope)? {
                    self.expr(then_value, scope)?
                } else {
                    self.expr(else_value, scope)?
                }
            }
            Expr::Call {
                function,
                arguments,
            } => {
                self.meter.borrow_mut().charge(self.at)?;
                let Some(definition) = self
                    .program
                    .definitions
                    .iter()
                    .find(|definition| &definition.name == function)
                else {
                    if matches!(
                        function.as_str(),
                        "lowercase" | "contains" | "starts-with" | "trim" | "first-word" | "rest-words" | "length"
                    ) {
                        return match (function.as_str(), self.exprs(arguments, scope)?.as_slice()) {
                            ("length", [Value::Text(text)]) => Ok(Value::Int(
                                i64::try_from(text.chars().count()).map_err(|_| self.overflow())?,
                            )),
                            ("lowercase", [Value::Text(text)]) => Ok(Value::Text(text.to_lowercase())),
                            ("contains", [Value::Text(text), Value::Text(query)]) => {
                                Ok(Value::Bool(text.contains(query)))
                            }
                            ("starts-with", [Value::Text(text), Value::Text(query)]) => {
                                Ok(Value::Bool(text.starts_with(query)))
                            }
                            ("trim", [Value::Text(text)]) => Ok(Value::Text(text.trim().into())),
                            ("first-word", [Value::Text(text)]) => Ok(Value::Text(
                                text.split_whitespace().next().unwrap_or("").into(),
                            )),
                            ("rest-words", [Value::Text(text)]) => {
                                let text = text.trim_start();
                                let rest = text
                                    .find(char::is_whitespace)
                                    .map(|at| text[at..].trim_start())
                                    .unwrap_or("");
                                Ok(Value::Text(rest.into()))
                            }
                            _ => Err(self.type_error(&format!("wrong arguments for {function}"))),
                        };
                    }
                    return Err(self.unsupported(&format!("the built-in {function}")));
                };
                if definition.parameters.len() != arguments.len() {
                    return Err(self.type_error(&format!(
                        "{function} takes {} arguments",
                        definition.parameters.len()
                    )));
                }
                let mut inner = BTreeMap::new();
                for (parameter, argument) in definition.parameters.iter().zip(arguments) {
                    self.meter.borrow_mut().charge(self.at)?;
                    inner.insert(
                        Variable::new(parameter.name.clone()),
                        self.expr(argument, scope)?,
                    );
                }
                self.expr(&definition.body, &inner)?
            }
            Expr::Lambda { .. } | Expr::Apply { .. } => {
                return Err(self.unsupported("a function value"))
            }
            Expr::Query(query) => match self.queries {
                Some(queries) => queries.query(query, scope)?,
                None => return Err(self.unsupported("a query here")),
            },
        })
    }

    fn binary(
        &self,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        scope: &dyn Scope,
    ) -> Result<Value, EvalError> {
        if let BinaryOp::And | BinaryOp::Or = op {
            let left = self.boolean(left, scope)?;
            return Ok(Value::Bool(match op {
                BinaryOp::And => left && self.boolean(right, scope)?,
                _ => left || self.boolean(right, scope)?,
            }));
        }
        let left = self.expr(left, scope)?;
        let right = self.expr(right, scope)?;
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

    pub(crate) fn compare(&self, op: CompareOp, left: &Value, right: &Value) -> Result<bool, EvalError> {
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
    pub(crate) fn type_error(&self, message: &str) -> EvalError {
        EvalError::Type {
            at: self.at.to_string(),
            message: message.to_string(),
        }
    }
}
