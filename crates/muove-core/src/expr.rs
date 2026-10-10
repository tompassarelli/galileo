use crate::*;
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
    let mut ranked: BTreeSet<(Value, Value)> = ordered(&keys, "an argmin key", at)?
        .zip(candidates)
        .collect();
    Ok(ranked.pop_first().map(|(_, winner)| winner))
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
    let mut ranked = BTreeSet::new();
    for row in rows {
        let mut values = Vec::new();
        for variable in declared {
            values.push(eval(&Expr::Variable(variable.clone()), row)?);
        }
        let key = match &query.sorted_by {
            Some(sort) => Some(eval(sort, row)?),
            None => None,
        };
        ranked.insert((key, values));
    }
    let mut seen = BTreeSet::new();
    let results: Vec<Value> = ranked
        .into_iter()
        .filter(|(_, values)| seen.insert(values.clone()))
        .map(|(_, values)| match <[Value; 1]>::try_from(values) {
            Ok([value]) => value,
            Err(values) => Value::Record(
                declared
                    .iter()
                    .map(|variable| variable.0.clone())
                    .zip(values)
                    .collect(),
            ),
        })
        .collect();
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
    query.sorted_by.iter().map(|sort| sort.as_ref()).collect()
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
