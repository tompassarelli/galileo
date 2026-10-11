use super::store::Store;
use super::Delta;
use crate::expr::{Binding, Evaluator, Meter, Queries, Scope};
use crate::*;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn eligible(program: &Program, rule: &Rule) -> bool {
    rule.head
        .arguments
        .iter()
        .all(|e| supported(program, e, &mut BTreeSet::new()))
        && rule.body.iter().all(|literal| {
            expressions(literal).is_some_and(|expressions| {
                expressions
                    .into_iter()
                    .all(|e| supported(program, e, &mut BTreeSet::new()))
            })
        })
}

pub(crate) fn expressions(literal: &Literal) -> Option<Vec<&Expr>> {
    Some(match literal {
        Literal::Positive(atom) | Literal::Negative(atom) => atom.arguments.iter().collect(),
        Literal::Compare { left, right, .. } => vec![left, right],
        Literal::Boolean(value)
        | Literal::Bind { value, .. }
        | Literal::Member { list: value, .. } => vec![value],
        Literal::Aggregate { .. } | Literal::ArgMin { .. } => return None,
    })
}

fn supported(program: &Program, expression: &Expr, visiting: &mut BTreeSet<String>) -> bool {
    match expression {
        Expr::Value(_) | Expr::Variable(_) => true,
        Expr::List(items) => items.iter().all(|e| supported(program, e, visiting)),
        Expr::Record(fields) | Expr::Attrs(fields) => {
            fields.values().all(|e| supported(program, e, visiting))
        }
        Expr::Field { value, .. } | Expr::Unary { value, .. } => {
            supported(program, value, visiting)
        }
        Expr::Binary { left, right, .. } => {
            supported(program, left, visiting) && supported(program, right, visiting)
        }
        Expr::If {
            condition,
            then_value,
            else_value,
        } => [condition, then_value, else_value]
            .into_iter()
            .all(|e| supported(program, e, visiting)),
        Expr::Call {
            function,
            arguments,
        } => {
            if !arguments.iter().all(|e| supported(program, e, visiting)) {
                return false;
            }
            let Some(definition) = program
                .definitions
                .iter()
                .find(|definition| definition.name == *function)
            else {
                return true;
            };
            if !visiting.insert(function.clone()) {
                return false;
            }
            let result = supported(program, &definition.body, visiting);
            visiting.remove(function);
            result
        }
        Expr::Lambda { .. } | Expr::Apply { .. } | Expr::Query(_) => false,
    }
}

fn variables(expression: &Expr, visit: &mut impl FnMut(&Variable)) {
    match expression {
        Expr::Variable(variable) => visit(variable),
        Expr::List(items)
        | Expr::Call {
            arguments: items, ..
        } => {
            for item in items {
                variables(item, visit);
            }
        }
        Expr::Record(fields) | Expr::Attrs(fields) => {
            for item in fields.values() {
                variables(item, visit);
            }
        }
        Expr::Field { value, .. } | Expr::Unary { value, .. } => variables(value, visit),
        Expr::Binary { left, right, .. } => {
            variables(left, visit);
            variables(right, visit);
        }
        Expr::If {
            condition,
            then_value,
            else_value,
        } => {
            for e in [condition, then_value, else_value] {
                variables(e, visit);
            }
        }
        _ => {}
    }
}

fn plain(literal: &Literal) -> bool {
    matches!(literal, Literal::Positive(atom) | Literal::Negative(atom) if atom.arguments.iter().all(|argument| matches!(argument, Expr::Value(_) | Expr::Variable(_))))
}

pub(crate) fn rule(
    program: &Program,
    rule: &Rule,
    schedule: &[usize],
    stores: &mut BTreeMap<RelationId, Store>,
    meter: &mut Meter,
) -> Result<Vec<Vec<Value>>, EvalError> {
    enumerate(program, rule, schedule, stores, None, false, meter)
}

pub(crate) fn pass(
    program: &Program,
    rule: &Rule,
    schedule: &[usize],
    stores: &mut BTreeMap<RelationId, Store>,
    deltas: &mut BTreeMap<RelationId, Delta>,
    before: bool,
    meter: &mut Meter,
) -> Result<Vec<Vec<Value>>, EvalError> {
    let mut rows = Vec::new();
    for (position, literal) in rule.body.iter().enumerate() {
        let (atom, negative) = match literal {
            Literal::Positive(atom) => (atom, false),
            Literal::Negative(atom) => (atom, true),
            _ => continue,
        };
        let Some(delta) = deltas.get(&atom.relation) else {
            continue;
        };
        let source = if negative != before {
            &delta.deleted
        } else {
            &delta.inserted
        };
        let seeds: BTreeSet<_> = source.presence().map(|row| row.iter().enumerate().map(|(column, value)| {
            if negative && matches!(&atom.arguments[column], Expr::Variable(variable) if variable.is_anonymous()) { Value::Null } else { value.clone() }
        }).collect::<Vec<_>>()).collect();
        for seed in seeds {
            meter.charge(&rule.name)?;
            rows.extend(enumerate(
                program,
                rule,
                schedule,
                stores,
                Some((position, &seed, deltas)),
                before,
                meter,
            )?);
        }
    }
    Ok(rows)
}

fn enumerate(
    program: &Program,
    rule: &Rule,
    schedule: &[usize],
    stores: &mut BTreeMap<RelationId, Store>,
    delta: Option<(usize, &[Value], &mut BTreeMap<RelationId, Delta>)>,
    before: bool,
    meter: &mut Meter,
) -> Result<Vec<Vec<Value>>, EvalError> {
    let mut slots = BTreeMap::new();
    let mut collect = |variable: &Variable| {
        let next = slots.len();
        slots.entry(variable.clone()).or_insert(next);
    };
    for expression in rule.head.arguments.iter().chain(
        rule.body
            .iter()
            .flat_map(|literal| expressions(literal).unwrap()),
    ) {
        variables(expression, &mut collect);
    }
    for literal in &rule.body {
        if let Literal::Bind { variable, .. } | Literal::Member { variable, .. } = literal {
            collect(variable);
        }
    }
    let mut search = Search {
        program,
        rule,
        slots,
        env: Vec::new(),
        stores: RefCell::new(stores),
        meter: RefCell::new(meter),
        rows: Vec::new(),
        pivot: delta.as_ref().map(|(position, row, _)| (*position, *row)),
        deltas: RefCell::new(delta.map(|(_, _, deltas)| deltas)),
        before,
    };
    search.env.resize(search.slots.len(), None);
    if let Some((position, row)) = search.pivot {
        let atom = match &rule.body[position] {
            Literal::Positive(atom) | Literal::Negative(atom) => atom,
            _ => unreachable!(),
        };
        for (argument, value) in atom.arguments.iter().zip(row) {
            match argument {
                Expr::Variable(variable) if !variable.is_anonymous() => {
                    let slot = search.slots[variable];
                    if search.env[slot]
                        .as_ref()
                        .is_some_and(|existing| existing != value)
                    {
                        return Ok(Vec::new());
                    }
                    search.env[slot] = Some(value.clone());
                }
                Expr::Value(expected) if expected != value => return Ok(Vec::new()),
                _ => {}
            }
        }
    }
    search.step(schedule)?;
    Ok(search.rows)
}

struct Search<'a> {
    program: &'a Program,
    rule: &'a Rule,
    slots: BTreeMap<Variable, usize>,
    env: Vec<Option<Value>>,
    stores: RefCell<&'a mut BTreeMap<RelationId, Store>>,
    meter: RefCell<&'a mut Meter>,
    rows: Vec<Vec<Value>>,
    pivot: Option<(usize, &'a [Value])>,
    deltas: RefCell<Option<&'a mut BTreeMap<RelationId, Delta>>>,
    before: bool,
}

impl<'a> Search<'a> {
    fn eval(&self) -> Evaluator<'_, 'a> {
        Evaluator {
            program: self.program,
            at: &self.rule.name,
            queries: Some(self),
            meter: &self.meter,
        }
    }
    fn charge(&self) -> Result<(), EvalError> {
        self.meter.borrow_mut().charge(&self.rule.name)
    }
    fn ground(&self, expression: &Expr) -> bool {
        let mut grounded = true;
        variables(expression, &mut |variable| {
            grounded &= self.get(variable).is_some()
        });
        grounded
    }
    fn key(
        &self,
        atom: &Atom,
        negative: bool,
        evaluate: bool,
    ) -> Result<(Vec<usize>, Vec<Value>), EvalError> {
        let mut columns = Vec::new();
        let mut key = Vec::new();
        for (column, argument) in atom.arguments.iter().enumerate() {
            if matches!(argument, Expr::Variable(variable) if variable.is_anonymous()) {
                continue;
            }
            if negative || self.ground(argument) {
                columns.push(column);
                key.push(if evaluate {
                    self.eval().expr(argument, self)?
                } else {
                    match argument {
                        Expr::Value(value) => value.clone(),
                        Expr::Variable(variable) => self.get(variable).unwrap().clone(),
                        _ => unreachable!(),
                    }
                });
            }
        }
        Ok((columns, key))
    }
    fn changed_len(&self, relation: RelationId, deleted: bool, columns: &[usize], key: &[Value]) -> usize {
        self.deltas
            .borrow_mut()
            .as_mut()
            .and_then(|deltas| deltas.get_mut(&relation))
            .map_or(0, |delta| {
                if deleted {
                    &mut delta.deleted
                } else {
                    &mut delta.inserted
                }
                .matching_len(columns, key)
            })
    }
    fn matching_len(&self, relation: RelationId, columns: &[usize], key: &[Value]) -> usize {
        let len = self
            .stores
            .borrow_mut()
            .entry(relation)
            .or_default()
            .matching_len(columns, key);
        if self.before {
            len - self.changed_len(relation, false, columns, key)
                + self.changed_len(relation, true, columns, key)
        } else {
            len
        }
    }
    fn matching(&self, relation: RelationId, columns: &[usize], key: &[Value]) -> Vec<Vec<Value>> {
        let mut rows: Vec<_> = self
            .stores
            .borrow_mut()
            .entry(relation)
            .or_default()
            .matching(columns, key)
            .cloned()
            .collect();
        if self.before {
            if let Some(delta) = self
                .deltas
                .borrow_mut()
                .as_mut()
                .and_then(|deltas| deltas.get_mut(&relation))
            {
                rows.retain(|row| delta.inserted.count(row) == 0);
                rows.extend(delta.deleted.matching(columns, key).cloned());
                rows.sort();
            }
        }
        rows
    }
    fn step(&mut self, remaining: &[usize]) -> Result<(), EvalError> {
        if remaining.is_empty() {
            self.charge()?;
            self.rows.push(
                self.rule
                    .head
                    .arguments
                    .iter()
                    .map(|e| self.eval().expr(e, self))
                    .collect::<Result<_, _>>()?,
            );
            return Ok(());
        }
        let end = remaining
            .iter()
            .position(|&position| !plain(&self.rule.body[position]))
            .unwrap_or(remaining.len());
        let chosen = if end == 0 {
            0
        } else {
            let mut best = None;
            for (index, &position) in remaining[..end].iter().enumerate() {
                let (atom, negative) = match &self.rule.body[position] {
                    Literal::Positive(atom) => (atom, false),
                    Literal::Negative(atom) => (atom, true),
                    _ => unreachable!(),
                };
                if negative
                    && atom.arguments.iter().any(|e| {
                        !matches!(e, Expr::Variable(v) if v.is_anonymous()) && !self.ground(e)
                    })
                {
                    continue;
                }
                let (columns, key) = self.key(atom, negative, false)?;
                self.charge()?;
                let size = self.matching_len(atom.relation, &columns, &key);
                let candidate = (size, position, index);
                if best.is_none_or(|best| candidate < best) {
                    best = Some(candidate);
                }
            }
            best.expect("the checked schedule has a ready sentence").2
        };
        let position = remaining[chosen];
        let literal = &self.rule.body[position];
        let mut rest = remaining.to_vec();
        rest.remove(chosen);
        match literal {
            Literal::Positive(atom) => {
                let (columns, key) = match self.key(atom, false, true) {
                    Ok(key) => key,
                    Err(error @ EvalError::Unknown(_)) => return Err(error),
                    Err(_) => (Vec::new(), Vec::new()),
                };
                let rows = self.matching(atom.relation, &columns, &key);
                for row in rows {
                    if let Some((pivot, seed)) = self.pivot {
                        if position == pivot && row != seed {
                            continue;
                        }
                        if position < pivot
                            && self
                                .deltas
                                .borrow()
                                .as_ref()
                                .and_then(|deltas| deltas.get(&atom.relation))
                                .is_some_and(|delta| {
                                    if self.before { &delta.deleted } else { &delta.inserted }
                                        .count(&row)
                                        > 0
                                })
                        {
                            continue;
                        }
                    }
                    self.charge()?;
                    let mut bound = Vec::new();
                    let mut matched = true;
                    for (column, argument) in atom.arguments.iter().enumerate() {
                        if columns.contains(&column) {
                            continue;
                        }
                        if let Expr::Variable(variable) = argument {
                            if variable.is_anonymous() {
                                continue;
                            }
                            let slot = self.slots[variable];
                            match &self.env[slot] {
                                Some(value) if *value != row[column] => {
                                    matched = false;
                                    break;
                                }
                                Some(_) => {}
                                None => {
                                    self.env[slot] = Some(row[column].clone());
                                    bound.push(slot);
                                }
                            }
                        } else if self.eval().expr(argument, self)? != row[column] {
                            matched = false;
                            break;
                        }
                    }
                    let result = if matched { self.step(&rest) } else { Ok(()) };
                    for slot in bound {
                        self.env[slot] = None;
                    }
                    result?;
                }
            }
            Literal::Negative(atom) => {
                self.charge()?;
                let mut columns = Vec::new();
                let mut key = Vec::new();
                for (column, argument) in atom.arguments.iter().enumerate() {
                    if matches!(argument, Expr::Variable(variable) if variable.is_anonymous()) {
                        continue;
                    }
                    columns.push(column);
                    key.push(
                        self.eval()
                            .exprs(std::slice::from_ref(argument), self)?
                            .remove(0),
                    );
                }
                if let Some((pivot, seed)) = self.pivot {
                    if position == pivot
                        && columns
                            .iter()
                            .zip(&key)
                            .any(|(&column, value)| &seed[column] != value)
                    {
                        return Ok(());
                    }
                    if position < pivot
                        && self.changed_len(atom.relation, !self.before, &columns, &key) > 0
                    {
                        return Ok(());
                    }
                }
                let exists = self.matching_len(atom.relation, &columns, &key) > 0;
                if columns.len() != atom.arguments.len() && exists {
                    self.charge()?;
                }
                if !exists {
                    self.step(&rest)?;
                }
            }
            Literal::Compare { op, left, right } => {
                self.charge()?;
                if self.eval().compare(
                    *op,
                    &self.eval().expr(left, self)?,
                    &self.eval().expr(right, self)?,
                )? {
                    self.step(&rest)?;
                }
            }
            Literal::Boolean(expression) => {
                self.charge()?;
                if self.eval().boolean(expression, self)? {
                    self.step(&rest)?;
                }
            }
            Literal::Bind { variable, value } => {
                self.charge()?;
                let value = self.eval().expr(value, self)?;
                self.bind(variable, value, &rest)?;
            }
            Literal::Member { variable, list } => {
                self.charge()?;
                let Value::List(items) = self.eval().expr(list, self)? else {
                    return Err(self.eval().type_error("membership needs a list"));
                };
                for item in items {
                    self.charge()?;
                    self.bind(variable, item, &rest)?;
                }
            }
            Literal::Aggregate { .. } | Literal::ArgMin { .. } => unreachable!(),
        }
        Ok(())
    }
    fn bind(&mut self, variable: &Variable, value: Value, rest: &[usize]) -> Result<(), EvalError> {
        let slot = self.slots[variable];
        if let Some(existing) = &self.env[slot] {
            if *existing == value {
                self.step(rest)?;
            }
        } else {
            self.env[slot] = Some(value);
            let result = self.step(rest);
            self.env[slot] = None;
            result?;
        }
        Ok(())
    }
}

impl Scope for Search<'_> {
    fn get(&self, variable: &Variable) -> Option<&Value> {
        self.slots
            .get(variable)
            .and_then(|&slot| self.env[slot].as_ref())
    }
    fn all(&self) -> Binding {
        self.slots
            .iter()
            .filter_map(|(variable, &slot)| Some((variable.clone(), self.env[slot].clone()?)))
            .collect()
    }
}

impl Queries for Search<'_> {
    fn query(&self, _: &Query, _: &dyn Scope) -> Result<Value, EvalError> {
        unreachable!("query expressions use Recompute")
    }
    fn field(&self, relation: RelationId, entity: &Entity) -> Option<Value> {
        self.stores
            .borrow_mut()
            .entry(relation)
            .or_default()
            .matching(&[0], &[Value::Entity(entity.clone())])
            .next()
            .map(|row| row[1].clone())
    }
}
