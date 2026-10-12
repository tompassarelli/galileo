use super::execute::{plain, variables};
use super::store::Store;
use crate::check::nested_order;
use crate::expr::{self, Binding, Evaluator, Meter, Queries, Scope};
use crate::*;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Reader<'a, 'm> {
    program: &'a Program,
    stores: RefCell<&'a mut BTreeMap<RelationId, Store>>,
    meter: RefCell<&'m mut Meter>,
    at: &'a str,
}

impl<'a, 'm> Reader<'a, 'm> {
    pub(crate) fn new(program: &'a Program, stores: &'a mut BTreeMap<RelationId, Store>, meter: &'m mut Meter, at: &'a str) -> Self {
        Self { program, stores: RefCell::new(stores), meter: RefCell::new(meter), at }
    }

    fn eval(&self) -> Evaluator<'_, 'm> {
        Evaluator { program: self.program, at: self.at, queries: Some(self), meter: &self.meter }
    }

    fn charge(&self) -> Result<(), EvalError> {
        self.meter.borrow_mut().charge(self.at)
    }

    pub(crate) fn value(&self, expression: &Expr, binding: &Binding) -> Result<Value, EvalError> {
        self.eval().expr(expression, binding)
    }

    pub(crate) fn contains(&self, relation: RelationId, tuple: &[Value]) -> bool {
        self.stores.borrow().get(&relation).is_some_and(|store| store.count(tuple) > 0)
    }

    pub(crate) fn matches(&self, body: &[Literal], binding: &Binding) -> Result<Vec<Binding>, EvalError> {
        let bound = binding.keys().cloned().collect();
        let order = nested_order(self.program, body, &[], &bound);
        let planned = self.rewrite(body, binding);
        let planned_order = nested_order(self.program, &planned, &[], &bound);
        let mut rows = Vec::new();
        self.step(&planned, &planned_order, &mut binding.clone(), &mut rows)?;
        for row in &mut rows { row.retain(|variable, _| !variable.0.starts_with("$condition-")); }
        let key = |row: &Binding| -> Vec<Vec<Value>> {
            order.iter().filter_map(|&position| match &body[position] {
                Literal::Positive(atom) => Some(atom.arguments.iter().map(|argument| match argument {
                    Expr::Value(value) => value.clone(),
                    Expr::Variable(variable) => row.get(variable).cloned().unwrap_or(Value::Null),
                    _ => Value::Null,
                }).collect()),
                Literal::Member { variable, .. } => Some(vec![row.get(variable).cloned().unwrap_or(Value::Null)]),
                _ => None,
            }).collect()
        };
        rows.sort_by_cached_key(key);
        Ok(rows)
    }

    fn rewrite(&self, body: &[Literal], binding: &Binding) -> Vec<Literal> {
        let program = self.program;
        let mut types = BTreeMap::new();
        for literal in body {
            if let Literal::Positive(atom) = literal {
                for (argument, slot) in atom.arguments.iter().zip(&program.relations[atom.relation.0].slots) {
                    if let (Expr::Variable(v), Type::Entity(t)) = (argument, &slot.ty) { types.insert(v.clone(), t.clone()); }
                }
            }
        }
        let field = |expression: &Expr| -> Option<(Variable, RelationId)> {
            let Expr::Field { value, field } = expression else { return None };
            let Expr::Variable(x) = value.as_ref() else { return None };
            let relation = program.field_relation(types.get(x)?, field)?;
            let definition = &program.relations[relation.0];
            if definition.slots.len() != 2 || !definition.modes.iter().any(|m| matches!(m, Mode::ExactlyOne { .. })) { return None; }
            Some((x.clone(), relation))
        };
        let mut binds = Vec::new();
        let mut planned = Vec::with_capacity(body.len());
        for (position, literal) in body.iter().enumerate() {
            let rewritten = match literal {
                Literal::Boolean(expression) => field(expression)
                    .filter(|(_, relation)| program.relations[relation.0].slots[1].ty == Type::Bool)
                    .map(|(x, relation)| Literal::Positive(Atom::new(relation, [Expr::Variable(x), Expr::Value(Value::Bool(true))]))),
                Literal::Compare { op: CompareOp::Equal, left, right } => [(left, right), (right, left)].into_iter().find_map(|(this, other)| {
                    let (x, relation) = field(this)?;
                    let mut mentions = false;
                    variables(other, &mut |v| mentions |= *v == x);
                    if mentions { return None; }
                    let argument = match other {
                        Expr::Value(_) => other.clone(),
                        Expr::Variable(v) if !v.is_anonymous() => other.clone(),
                        Expr::Field { value, .. } if matches!(value.as_ref(), Expr::Variable(v) if binding.contains_key(v)) && field(other).is_some() => {
                            let fresh = Variable::new(format!("$condition-{position}"));
                            binds.push(Literal::Bind { variable: fresh.clone(), value: other.clone() });
                            Expr::Variable(fresh)
                        }
                        _ => return None,
                    };
                    Some(Literal::Positive(Atom::new(relation, [Expr::Variable(x), argument])))
                }),
                _ => None,
            };
            planned.push(rewritten.unwrap_or_else(|| literal.clone()));
        }
        binds.extend(planned);
        binds
    }

    fn distinct(&self, body: &[Literal], extras: &[&Expr], binding: &Binding) -> Result<BTreeSet<Binding>, EvalError> {
        let bound = binding.keys().cloned().collect();
        let order = nested_order(self.program, body, extras, &bound);
        let mut found = Vec::new();
        self.step(body, &order, &mut binding.clone(), &mut found)?;
        let mut rows = BTreeSet::new();
        for row in found {
            self.charge()?;
            rows.insert(row);
        }
        Ok(rows)
    }

    fn matching_len(&self, relation: RelationId, columns: &[usize], key: &[Value]) -> usize {
        self.stores.borrow_mut().entry(relation).or_default().matching_len(columns, key)
    }

    fn matching(&self, relation: RelationId, columns: &[usize], key: &[Value]) -> Vec<Vec<Value>> {
        self.stores.borrow_mut().entry(relation).or_default().matching(columns, key).cloned().collect()
    }

    fn known(argument: &Expr, binding: &Binding) -> Option<Value> {
        match argument {
            Expr::Value(value) => Some(value.clone()),
            Expr::Variable(variable) if !variable.is_anonymous() => binding.get(variable).cloned(),
            _ => None,
        }
    }

    fn step(&self, body: &[Literal], remaining: &[usize], binding: &mut Binding, out: &mut Vec<Binding>) -> Result<(), EvalError> {
        if remaining.is_empty() {
            out.push(binding.clone());
            return Ok(());
        }
        let end = remaining.iter().position(|&position| !plain(&body[position])).unwrap_or(remaining.len());
        let chosen = if end == 0 { 0 } else {
            let mut best = None;
            for (index, &position) in remaining[..end].iter().enumerate() {
                let (atom, negative) = match &body[position] {
                    Literal::Positive(atom) => (atom, false),
                    Literal::Negative(atom) => (atom, true),
                    _ => unreachable!(),
                };
                let mut columns = Vec::new();
                let mut key = Vec::new();
                let mut ready = true;
                for (column, argument) in atom.arguments.iter().enumerate() {
                    if matches!(argument, Expr::Variable(variable) if variable.is_anonymous()) { continue; }
                    match Self::known(argument, binding) {
                        Some(value) => { columns.push(column); key.push(value); }
                        None => ready &= !negative,
                    }
                }
                if !ready { continue; }
                self.charge()?;
                let candidate = (self.matching_len(atom.relation, &columns, &key), position, index);
                if best.is_none_or(|best| candidate < best) { best = Some(candidate); }
            }
            best.expect("the checked schedule has a ready sentence").2
        };
        let position = remaining[chosen];
        let mut rest = remaining.to_vec();
        rest.remove(chosen);
        match &body[position] {
            Literal::Positive(atom) => {
                let mut columns = Vec::new();
                let mut key = Vec::new();
                for (column, argument) in atom.arguments.iter().enumerate() {
                    if let Some(value) = Self::known(argument, binding) { columns.push(column); key.push(value); }
                }
                for row in self.matching(atom.relation, &columns, &key) {
                    self.charge()?;
                    let mut bound = Vec::new();
                    let mut matched = true;
                    for (column, argument) in atom.arguments.iter().enumerate() {
                        if columns.contains(&column) { continue; }
                        match argument {
                            Expr::Variable(variable) if variable.is_anonymous() => {}
                            Expr::Variable(variable) => match binding.get(variable) {
                                Some(value) if *value != row[column] => { matched = false; break; }
                                Some(_) => {}
                                None => { binding.insert(variable.clone(), row[column].clone()); bound.push(variable.clone()); }
                            },
                            other => match self.value(other, binding) {
                                Ok(value) if value == row[column] => {}
                                Ok(_) => { matched = false; break; }
                                Err(error) => { for variable in &bound { binding.remove(variable); } return Err(error); }
                            },
                        }
                    }
                    let result = if matched { self.step(body, &rest, binding, out) } else { Ok(()) };
                    for variable in bound { binding.remove(&variable); }
                    result?;
                }
            }
            Literal::Negative(atom) => {
                self.charge()?;
                let mut columns = Vec::new();
                let mut key = Vec::new();
                for (column, argument) in atom.arguments.iter().enumerate() {
                    if matches!(argument, Expr::Variable(variable) if variable.is_anonymous()) { continue; }
                    columns.push(column);
                    key.push(self.eval().exprs(std::slice::from_ref(argument), binding)?.remove(0));
                }
                let exists = if columns.len() == atom.arguments.len() {
                    self.contains(atom.relation, &key)
                } else {
                    let exists = self.matching_len(atom.relation, &columns, &key) > 0;
                    if exists { self.charge()?; }
                    exists
                };
                if !exists { self.step(body, &rest, binding, out)?; }
            }
            Literal::Compare { op, left, right } => {
                self.charge()?;
                let left = self.value(left, binding)?;
                let right = self.value(right, binding)?;
                if self.eval().compare(*op, &left, &right)? { self.step(body, &rest, binding, out)?; }
            }
            Literal::Boolean(expression) => {
                self.charge()?;
                if self.eval().boolean(expression, binding)? { self.step(body, &rest, binding, out)?; }
            }
            Literal::Bind { variable, value } => {
                self.charge()?;
                let value = self.value(value, binding)?;
                self.bind(body, &rest, binding, variable, value, out)?;
            }
            Literal::Member { variable, list } => {
                self.charge()?;
                let Value::List(items) = self.value(list, binding)? else {
                    return Err(self.eval().type_error("membership needs a list"));
                };
                for item in items {
                    self.charge()?;
                    self.bind(body, &rest, binding, variable, item, out)?;
                }
            }
            Literal::Aggregate { variable, op, body: inner } => {
                self.charge()?;
                let extras: Vec<&Expr> = op.value().into_iter().collect();
                let rows = self.distinct(inner, &extras, binding)?;
                if let Some(value) = expr::aggregate(op, &rows, &mut |e, row| self.value(e, row), self.at)? {
                    self.bind(body, &rest, binding, variable, value, out)?;
                }
            }
            Literal::ArgMin { direction, variable, candidate, key, body: inner } => {
                self.charge()?;
                let rows = self.distinct(inner, &[candidate, key], binding)?;
                if let Some(value) = expr::argmin(*direction, candidate, key, &rows, &mut |e, row| self.value(e, row), self.at)? {
                    self.bind(body, &rest, binding, variable, value, out)?;
                }
            }
        }
        Ok(())
    }

    fn bind(&self, body: &[Literal], rest: &[usize], binding: &mut Binding, variable: &Variable, value: Value, out: &mut Vec<Binding>) -> Result<(), EvalError> {
        match binding.get(variable) {
            Some(existing) => if *existing == value { self.step(body, rest, binding, out)?; },
            None => {
                binding.insert(variable.clone(), value);
                let result = self.step(body, rest, binding, out);
                binding.remove(variable);
                result?;
            }
        }
        Ok(())
    }
}

impl Queries for Reader<'_, '_> {
    fn query(&self, query: &Query, scope: &dyn Scope) -> Result<Value, EvalError> {
        self.charge()?;
        let binding = scope.all();
        let rows = self.distinct(&query.body, &expr::query_extras(query), &binding)?;
        expr::query(query, &rows, &mut |e, row| self.value(e, row), self.at)
    }

    fn field(&self, relation: RelationId, entity: &Entity) -> Option<Value> {
        self.stores.borrow_mut().entry(relation).or_default().matching(&[0], &[Value::Entity(entity.clone())]).next().map(|row| row[1].clone())
    }
}

pub(super) fn expressions(literal: &Literal) -> Vec<&Expr> {
    match literal {
        Literal::Bind { value: Expr::Query(query), .. } => expr::query_extras(query),
        _ => super::execute::expressions(literal).unwrap(),
    }
}

pub(super) fn value(query: &Query, rows: &expr::Rows, cached: &RefCell<Option<Result<Value, EvalError>>>, evaluate: &mut dyn FnMut(&Expr, &Binding) -> Result<Value, EvalError>, at: &str) -> Result<Value, EvalError> {
    if let Some(result) = cached.borrow().as_ref() { return result.clone(); }
    let result = expr::query(query, rows, evaluate, at);
    if !matches!(result, Err(EvalError::Unknown(_))) { *cached.borrow_mut() = Some(result.clone()); }
    result
}
