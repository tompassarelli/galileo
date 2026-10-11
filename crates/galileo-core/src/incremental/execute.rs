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
            if matches!(literal, Literal::Aggregate { .. } | Literal::ArgMin { .. }) { return false; }
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
        Literal::Aggregate { op, .. } => op.value().into_iter().collect(),
        Literal::ArgMin { candidate, key, .. } => vec![candidate, key],
    })
}

pub(super) fn supported(program: &Program, expression: &Expr, visiting: &mut BTreeSet<String>) -> bool {
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

pub(super) fn variables(expression: &Expr, visit: &mut impl FnMut(&Variable)) {
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

pub(super) fn plain(literal: &Literal) -> bool {
    matches!(literal, Literal::Positive(atom) | Literal::Negative(atom) if atom.arguments.iter().all(|argument| matches!(argument, Expr::Value(_) | Expr::Variable(_))))
}

pub(crate) fn rule(
    program: &Program,
    rule: &Rule,
    schedule: &[usize],
    stores: &mut BTreeMap<RelationId, Store>,
    meter: &mut Meter,
) -> Result<Vec<Vec<Value>>, EvalError> {
    enumerate(program, rule, schedule, stores, None, false, meter, None, None, None)
}

pub(super) fn summarized(
    program: &Program,
    rule: &Rule,
    index: usize,
    schedule: &[usize],
    stores: &mut BTreeMap<RelationId, Store>,
    meter: &mut Meter,
    summaries: &super::aggregate::Read<'_>,
) -> Result<Vec<Vec<Value>>, EvalError> {
    enumerate(program, rule, schedule, stores, None, false, meter, Some((index, summaries)), None, None)
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
    summarized_pass(program, rule, schedule, stores, deltas, before, meter, None, None, &[])
}

pub(super) fn summarized_pass(
    program: &Program,
    rule: &Rule,
    schedule: &[usize],
    stores: &mut BTreeMap<RelationId, Store>,
    deltas: &mut BTreeMap<RelationId, Delta>,
    before: bool,
    meter: &mut Meter,
    summaries: Option<(usize, &super::aggregate::Read<'_>)>,
    groups: Option<&BTreeMap<RelationId, BTreeSet<Vec<Value>>>>,
    outer: &[super::ranked::Kept<'_>],
) -> Result<Vec<Vec<Value>>, EvalError> {
    let mut rows = Vec::new();
    for (position, literal) in rule.body.iter().enumerate() {
        let (atom, negative) = match literal {
            Literal::Positive(atom) => (atom, false),
            Literal::Negative(atom) => (atom, true),
            Literal::Aggregate { .. } | Literal::ArgMin { .. } | Literal::Bind { value: Expr::Query(_), .. } => {
                let (index, reader) = summaries.unwrap();
                let plan = reader.plan(index, position);
                let kept = outer.iter().find(|kept| kept.position() == position);
                for seed in groups.into_iter().flat_map(|g| g.get(&plan.relation)).flatten() {
                    meter.charge(&rule.name)?;
                    let Some(kept) = kept else {
                        rows.extend(enumerate(program, rule, schedule, stores, Some((position, 0, seed, deltas)), before, meter, summaries, groups, None)?);
                        continue;
                    };
                    for (binding, count) in kept.bindings(stores, seed) {
                        meter.charge(&rule.name)?;
                        kept.visits.set(kept.visits.get() + 1);
                        let found = enumerate(program, rule, schedule, stores, Some((position, 0, seed, deltas)), before, meter, summaries, groups, Some((kept.variables(), &binding, kept.start())))?;
                        for _ in 0..count { rows.extend(found.iter().cloned()); }
                    }
                }
                continue;
            }
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
                Some((position, 0, &seed, deltas)),
                before,
                meter,
                summaries,
                groups,
                None,
            )?);
        }
    }
    for (occurrence, keys) in keyed_seeds(program, rule, deltas) {
        for entity in keys {
            meter.charge(&rule.name)?;
            let seed = [Value::Entity(entity)];
            rows.extend(enumerate(program, rule, schedule, stores, Some((occurrence.position, occurrence.sub, &seed, deltas)), before, meter, summaries, groups, None)?);
        }
    }
    Ok(rows)
}

fn enumerate(
    program: &Program,
    rule: &Rule,
    schedule: &[usize],
    stores: &mut BTreeMap<RelationId, Store>,
    delta: Option<(usize, usize, &[Value], &mut BTreeMap<RelationId, Delta>)>,
    before: bool,
    meter: &mut Meter,
    summaries: Option<(usize, &super::aggregate::Read<'_>)>,
    groups: Option<&BTreeMap<RelationId, BTreeSet<Vec<Value>>>>,
    resume: Option<(&[Variable], &[Value], usize)>,
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
        if let Literal::Bind { variable, .. } | Literal::Member { variable, .. } | Literal::Aggregate { variable, .. } | Literal::ArgMin { variable, .. } = literal {
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
        pivot: delta.as_ref().map(|(position, sub, row, _)| (*position, *sub, *row)),
        earlier: delta.as_ref().map_or(Vec::new(), |(position, sub, _, _)| {
            occurrences(rule).into_iter().filter(|o| (o.position, o.sub) < (*position, *sub)).collect()
        }),
        deltas: RefCell::new(delta.map(|(_, _, _, deltas)| deltas)),
        before,
        summaries,
        groups,
    };
    search.env.resize(search.slots.len(), None);
    if let Some((position, sub, row)) = search.pivot {
        if sub > 0 {
            let occurrence = occurrences(rule).into_iter().find(|o| (o.position, o.sub) == (position, sub)).unwrap();
            let slot = search.slots[&occurrence.variable];
            search.env[slot] = Some(row[0].clone());
            search.step(schedule)?;
            return Ok(search.rows);
        }
        let atom = match &rule.body[position] {
            Literal::Positive(atom) | Literal::Negative(atom) => atom,
            Literal::Aggregate { .. } | Literal::ArgMin { .. } | Literal::Bind { value: Expr::Query(_), .. } => {
                let (index, reader) = summaries.unwrap();
                let plan = reader.plan(index, position);
                for (variable, value) in plan.variables[..plan.shared].iter().zip(row) {
                    search.env[search.slots[variable]] = Some(value.clone());
                }
                let Some((variables, binding, start)) = resume else {
                    search.step(schedule)?;
                    return Ok(search.rows);
                };
                for (variable, value) in variables.iter().zip(binding) {
                    search.env[search.slots[variable]] = Some(value.clone());
                }
                search.step(&schedule[start..])?;
                return Ok(search.rows);
            }
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

struct Search<'a, 's> {
    program: &'a Program,
    rule: &'a Rule,
    slots: BTreeMap<Variable, usize>,
    env: Vec<Option<Value>>,
    stores: RefCell<&'a mut BTreeMap<RelationId, Store>>,
    meter: RefCell<&'a mut Meter>,
    rows: Vec<Vec<Value>>,
    pivot: Option<(usize, usize, &'a [Value])>,
    earlier: Vec<Occurrence>,
    deltas: RefCell<Option<&'a mut BTreeMap<RelationId, Delta>>>,
    before: bool,
    summaries: Option<(usize, &'a super::aggregate::Read<'s>)>,
    groups: Option<&'a BTreeMap<RelationId, BTreeSet<Vec<Value>>>>,
}

impl<'a> Search<'a, '_> {
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
            for occurrence in &self.earlier {
                if let Some(Value::Entity(entity)) = self.get(&occurrence.variable) {
                    if let Some(relation) = self.program.field_relation(&entity.entity_type, &occurrence.field) {
                        let key = [Value::Entity(entity.clone())];
                        if self.changed_len(relation, false, &[0], &key) + self.changed_len(relation, true, &[0], &key) > 0 {
                            return Ok(());
                        }
                    }
                }
            }
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
                    if let Some((pivot, sub, seed)) = self.pivot {
                        if (position, 0) == (pivot, sub) && row != seed {
                            continue;
                        }
                        if (position, 0) < (pivot, sub)
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
                if let Some((pivot, sub, seed)) = self.pivot {
                    if (position, 0) == (pivot, sub)
                        && columns
                            .iter()
                            .zip(&key)
                            .any(|(&column, value)| &seed[column] != value)
                    {
                        return Ok(());
                    }
                    if (position, 0) < (pivot, sub)
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
            Literal::Bind { variable, value } if !matches!(value, Expr::Query(_)) => {
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
            Literal::Aggregate { variable, .. } | Literal::ArgMin { variable, .. } | Literal::Bind { variable, value: Expr::Query(_), .. } => {
                self.charge()?;
                let (index, summaries) = self.summaries.unwrap();
                if self.pivot.is_some_and(|(pivot, sub, _)| (position, 0) < (pivot, sub)) {
                    let plan = summaries.plan(index, position);
                    let key: Vec<_> = plan.variables[..plan.shared].iter().map(|v| self.get(v).unwrap().clone()).collect();
                    if self.groups.and_then(|g| g.get(&plan.relation)).is_some_and(|g| g.contains(&key)) { return Ok(()); }
                }
                if let Some(value) = summaries.value(index, position, self, &self.eval())? {
                    self.bind(variable, value, &rest)?;
                }
            },
            Literal::Bind { .. } => unreachable!(),
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

impl Scope for Search<'_, '_> {
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

impl Queries for Search<'_, '_> {
    fn query(&self, _: &Query, _: &dyn Scope) -> Result<Value, EvalError> {
        unreachable!("query expressions use Recompute")
    }
    fn field(&self, relation: RelationId, entity: &Entity) -> Option<Value> {
        self.matching(relation, &[0], &[Value::Entity(entity.clone())])
            .first()
            .map(|row| row[1].clone())
    }
}

pub(super) fn keyed(expression: &Expr, visit: &mut impl FnMut(&Variable, &str)) {
    match expression {
        Expr::Field { value, field } => match value.as_ref() {
            Expr::Variable(variable) => visit(variable, field),
            other => keyed(other, visit),
        },
        Expr::List(items) | Expr::Call { arguments: items, .. } => {
            for item in items {
                keyed(item, visit);
            }
        }
        Expr::Record(fields) | Expr::Attrs(fields) => {
            for item in fields.values() {
                keyed(item, visit);
            }
        }
        Expr::Unary { value, .. } => keyed(value, visit),
        Expr::Binary { left, right, .. } => {
            keyed(left, visit);
            keyed(right, visit);
        }
        Expr::If { condition, then_value, else_value } => {
            for e in [condition, then_value, else_value] {
                keyed(e, visit);
            }
        }
        _ => {}
    }
}

pub(super) fn unkeyed(expression: &Expr) -> Expr {
    let map = |items: &[Expr]| items.iter().map(unkeyed).collect();
    let fields = |fields: &BTreeMap<String, Expr>| fields.iter().map(|(k, v)| (k.clone(), unkeyed(v))).collect();
    match expression {
        Expr::Field { value, field } => match value.as_ref() {
            Expr::Variable(_) => value.as_ref().clone(),
            other => Expr::Field { value: Box::new(unkeyed(other)), field: field.clone() },
        },
        Expr::List(items) => Expr::List(map(items)),
        Expr::Call { function, arguments } => Expr::Call { function: function.clone(), arguments: map(arguments) },
        Expr::Record(f) => Expr::Record(fields(f)),
        Expr::Attrs(f) => Expr::Attrs(fields(f)),
        Expr::Unary { op, value } => Expr::Unary { op: *op, value: Box::new(unkeyed(value)) },
        Expr::Binary { op, left, right } => Expr::Binary { op: *op, left: Box::new(unkeyed(left)), right: Box::new(unkeyed(right)) },
        Expr::If { condition, then_value, else_value } => Expr::If {
            condition: Box::new(unkeyed(condition)),
            then_value: Box::new(unkeyed(then_value)),
            else_value: Box::new(unkeyed(else_value)),
        },
        other => other.clone(),
    }
}

pub(super) struct Occurrence {
    pub(super) position: usize,
    pub(super) sub: usize,
    variable: Variable,
    field: String,
}

impl Occurrence {
    pub(super) fn field(&self) -> &str {
        &self.field
    }
}

pub(super) fn occurrences(rule: &Rule) -> Vec<Occurrence> {
    let mut found = Vec::new();
    let literals = rule.body.iter().map(|literal| match literal {
        Literal::Aggregate { .. } | Literal::ArgMin { .. } => Vec::new(),
        literal => expressions(literal).unwrap(),
    });
    for (position, expressions) in literals.chain(std::iter::once(rule.head.arguments.iter().collect())).enumerate() {
        let mut sub = 0;
        for expression in expressions {
            keyed(expression, &mut |variable, field| {
                if !variable.is_anonymous() {
                    sub += 1;
                    found.push(Occurrence { position, sub, variable: variable.clone(), field: field.to_string() });
                }
            });
        }
    }
    found
}

pub(super) fn changed_keys(program: &Program, field: &str, deltas: &BTreeMap<RelationId, Delta>) -> BTreeSet<Entity> {
    let mut keys = BTreeSet::new();
    for relation in crate::check::field_relations(program, field) {
        let Some(delta) = deltas.get(&relation) else { continue };
        for row in delta.inserted.presence().chain(delta.deleted.presence()) {
            if let Some(Value::Entity(entity)) = row.first() {
                if program.field_relation(&entity.entity_type, field) == Some(relation) {
                    keys.insert(entity.clone());
                }
            }
        }
    }
    keys
}

pub(super) fn keyed_seeds(program: &Program, rule: &Rule, deltas: &BTreeMap<RelationId, Delta>) -> Vec<(Occurrence, BTreeSet<Entity>)> {
    occurrences(rule)
        .into_iter()
        .map(|occurrence| {
            let keys = changed_keys(program, &occurrence.field, deltas);
            (occurrence, keys)
        })
        .filter(|(_, keys)| !keys.is_empty())
        .collect()
}
