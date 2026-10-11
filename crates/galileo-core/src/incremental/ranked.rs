use super::{execute, store::Store, Delta};
use crate::expr::Meter;
use crate::*;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(super) struct Plan {
    pub(super) op: CompareOp,
    pub(super) key: Expr,
    pub(super) threshold: Expr,
}

pub(super) fn shape(literal: &Literal, outer: &BTreeSet<Variable>) -> Option<(Plan, Vec<Literal>)> {
    let Literal::Aggregate { op: AggregateOp::Count | AggregateOp::Sum(_), body, .. } = literal else { return None; };
    let mut comparison = None;
    for (position, literal) in body.iter().enumerate() {
        if execute::plain(literal) { continue; }
        let Literal::Compare { op, left, right } = literal else { return None; };
        if comparison.is_some() || !matches!(op, CompareOp::Less | CompareOp::LessEqual | CompareOp::Greater | CompareOp::GreaterEqual) { return None; }
        comparison = Some((position, *op, left, right));
    }
    let (position, mut op, left, right) = comparison?;
    let vars = |expr| { let mut vars = BTreeSet::new(); execute::variables(expr, &mut |v| { vars.insert(v.clone()); }); vars };
    let left_vars = vars(left);
    let right_vars = vars(right);
    let local = |vars: &BTreeSet<Variable>| !vars.is_empty() && vars.is_disjoint(outer);
    let bound = |vars: &BTreeSet<Variable>| !vars.is_empty() && vars.is_subset(outer);
    let (key, threshold, threshold_vars) = if local(&left_vars) && bound(&right_vars) {
        (left, right, right_vars)
    } else if bound(&left_vars) && local(&right_vars) {
        op = match op { CompareOp::Less => CompareOp::Greater, CompareOp::LessEqual => CompareOp::GreaterEqual, CompareOp::Greater => CompareOp::Less, CompareOp::GreaterEqual => CompareOp::LessEqual, _ => unreachable!() };
        (right, left, left_vars)
    } else { return None; };
    let body: Vec<_> = body.iter().enumerate().filter(|(i, _)| *i != position).map(|(_, l)| l.clone()).collect();
    let mut used = BTreeSet::new();
    for expr in body.iter().flat_map(|l| execute::expressions(l).unwrap()).chain(execute::expressions(literal).unwrap()) {
        execute::variables(expr, &mut |v| { used.insert(v.clone()); });
    }
    if !used.is_disjoint(&threshold_vars) { return None; }
    Some((Plan { op, key: key.clone(), threshold: threshold.clone() }, body))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Node {
    key: Value,
    count: usize,
    weight: i128,
    height: usize,
    rows: usize,
    sum: i128,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

impl Node {
    fn height(node: &Option<Box<Self>>) -> usize { node.as_ref().map_or(0, |n| n.height) }
    fn rows(node: &Option<Box<Self>>) -> usize { node.as_ref().map_or(0, |n| n.rows) }
    fn sum(node: &Option<Box<Self>>) -> i128 { node.as_ref().map_or(0, |n| n.sum) }
    fn refresh(&mut self) {
        self.height = 1 + Self::height(&self.left).max(Self::height(&self.right));
        self.rows = self.count + Self::rows(&self.left) + Self::rows(&self.right);
        self.sum = self.weight + Self::sum(&self.left) + Self::sum(&self.right);
    }
    fn rotate(mut node: Box<Self>, left: bool) -> Box<Self> {
        let mut root = if left { node.right.take().unwrap() } else { node.left.take().unwrap() };
        if left { node.right = root.left.take(); node.refresh(); root.left = Some(node); }
        else { node.left = root.right.take(); node.refresh(); root.right = Some(node); }
        root.refresh();
        root
    }
    fn balance(mut node: Box<Self>) -> Box<Self> {
        node.refresh();
        if Self::height(&node.left) > Self::height(&node.right) + 1 {
            let child = node.left.as_ref().unwrap();
            if Self::height(&child.right) > Self::height(&child.left) { node.left = Some(Self::rotate(node.left.take().unwrap(), true)); }
            return Self::rotate(node, false);
        }
        if Self::height(&node.right) > Self::height(&node.left) + 1 {
            let child = node.right.as_ref().unwrap();
            if Self::height(&child.left) > Self::height(&child.right) { node.right = Some(Self::rotate(node.right.take().unwrap(), false)); }
            return Self::rotate(node, true);
        }
        node
    }
    fn remove(mut node: Box<Self>, key: &Value) -> Option<Box<Self>> {
        match key.cmp(&node.key) {
            std::cmp::Ordering::Less => node.left = Self::remove(node.left.take().unwrap(), key),
            std::cmp::Ordering::Greater => node.right = Self::remove(node.right.take().unwrap(), key),
            std::cmp::Ordering::Equal => {
                if node.left.is_none() { return node.right; }
                if node.right.is_none() { return node.left; }
                let mut next = node.right.as_ref().unwrap();
                while let Some(left) = &next.left { next = left; }
                node.key = next.key.clone(); node.count = next.count; node.weight = next.weight;
                node.right = Self::remove(node.right.take().unwrap(), &node.key);
            }
        }
        Some(Self::balance(node))
    }
    fn change(node: Option<Box<Self>>, key: &Value, weight: i128, adding: bool) -> Option<Box<Self>> {
        let Some(mut node) = node else {
            assert!(adding);
            return Some(Box::new(Self { key: key.clone(), count: 1, weight, height: 1, rows: 1, sum: weight, left: None, right: None }));
        };
        match key.cmp(&node.key) {
            std::cmp::Ordering::Less => node.left = Self::change(node.left.take(), key, weight, adding),
            std::cmp::Ordering::Greater => node.right = Self::change(node.right.take(), key, weight, adding),
            std::cmp::Ordering::Equal => {
                if !adding && node.count == 1 { return Self::remove(node, key); }
                if adding { node.count += 1; node.weight += weight; } else { node.count -= 1; node.weight -= weight; }
            }
        }
        Some(Self::balance(node))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Multiset(Option<Box<Node>>);
impl Multiset {
    pub(super) fn change(&mut self, key: &Value, weight: i128, adding: bool) { self.0 = Node::change(self.0.take(), key, weight, adding); }
    pub(super) fn valid(&self, threshold: &Value) -> bool {
        self.0.as_ref().is_none_or(|root| matches!((&root.key, threshold), (Value::Int(_), Value::Int(_)) | (Value::Text(_), Value::Text(_))))
    }
    pub(super) fn sum(&self) -> i128 { Node::sum(&self.0) }
    pub(super) fn value(&self, op: CompareOp, threshold: &Value) -> i128 {
        let inclusive = matches!(op, CompareOp::LessEqual | CompareOp::Greater);
        let mut node = self.0.as_ref();
        let mut prefix = 0;
        while let Some(current) = node {
            if current.key < *threshold || (inclusive && current.key == *threshold) {
                prefix += Node::sum(&current.left) + current.weight;
                node = current.right.as_ref();
            } else { node = current.left.as_ref(); }
        }
        if matches!(op, CompareOp::Greater | CompareOp::GreaterEqual) { self.sum() - prefix } else { prefix }
    }
}

pub(super) fn operator(op: CompareOp) -> usize {
    match op { CompareOp::Less => 0, CompareOp::LessEqual => 1, CompareOp::Greater => 2, CompareOp::GreaterEqual => 3, _ => unreachable!() }
}

#[derive(Clone, Debug)]
pub(super) struct Index {
    pub(super) rule: usize,
    position: usize,
    relation: RelationId,
    shared: usize,
    variables: Vec<Variable>,
    start: usize,
    prefix: Rule,
    schedule: Vec<usize>,
}

pub(super) fn indexes(program: &Program, plans: &[super::aggregate::Plan], schedules: &[Vec<usize>], delta: impl Fn(usize) -> bool) -> Vec<Index> {
    let mut indexes = Vec::new();
    for plan in plans {
        let Some(Plan { threshold: Expr::Variable(threshold), .. }) = &plan.ranked else { continue };
        if threshold.is_anonymous() || !delta(plan.rule) { continue; }
        let rule = &program.rules[plan.rule];
        let schedule = &schedules[plan.rule];
        let Some(start) = schedule.iter().position(|&p| p == plan.position) else { continue };
        let mut positions = schedule[..start].to_vec();
        if positions.iter().any(|&p| p > plan.position || matches!(rule.body[p], Literal::Aggregate { .. } | Literal::ArgMin { .. } | Literal::Bind { value: Expr::Query(_), .. })) { continue; }
        positions.sort_unstable();
        let mut named = BTreeSet::new();
        for &position in &positions {
            let literal = &rule.body[position];
            for expression in execute::expressions(literal).unwrap() {
                execute::variables(expression, &mut |v| { if !v.is_anonymous() { named.insert(v.clone()); } });
            }
            if let Literal::Bind { variable, .. } | Literal::Member { variable, .. } = literal { named.insert(variable.clone()); }
        }
        let group = &plan.variables[..plan.shared];
        if !group.iter().chain([threshold]).all(|v| named.contains(v)) { continue; }
        let variables: Vec<Variable> = group.iter().chain([threshold]).cloned()
            .chain(named.iter().filter(|v| !group.contains(v) && *v != threshold).cloned()).collect();
        let relation = RelationId(program.relations.len() + indexes.len());
        let mut prefix = rule.clone();
        prefix.head = Atom::new(relation, variables.iter().cloned().map(Expr::Variable));
        prefix.body = positions.iter().map(|&p| rule.body[p].clone()).collect();
        if rule.body_sources.len() == rule.body.len() {
            prefix.body_sources = positions.iter().map(|&p| rule.body_sources[p].clone()).collect();
        }
        let schedule = schedule[..start].iter().map(|p| positions.binary_search(p).unwrap()).collect();
        indexes.push(Index { rule: plan.rule, position: plan.position, relation, shared: plan.shared, variables, start, prefix, schedule });
    }
    indexes
}

impl Index {
    pub(super) fn relation(&self) -> RelationId { self.relation }
}

pub(super) type Changes = BTreeMap<RelationId, BTreeMap<Vec<Value>, BTreeMap<Value, (i128, isize)>>>;

pub(super) struct Kept<'a> {
    index: &'a Index,
    gained: BTreeMap<Vec<Value>, usize>,
    swept: BTreeMap<Vec<Value>, BTreeMap<Vec<Value>, usize>>,
    pub(super) visits: Cell<usize>,
}

impl Kept<'_> {
    pub(super) fn position(&self) -> usize { self.index.position }
    pub(super) fn variables(&self) -> &[Variable] { &self.index.variables }
    pub(super) fn start(&self) -> usize { self.index.start }
    fn kept(&self, store: &Store, row: &[Value]) -> usize { store.count(row) - self.gained.get(row).copied().unwrap_or(0) }
    pub(super) fn bindings(&self, stores: &mut BTreeMap<RelationId, Store>, group: &[Value]) -> Vec<(Vec<Value>, usize)> {
        if let Some(swept) = self.swept.get(group) { return swept.iter().map(|(row, count)| (row.clone(), *count)).collect(); }
        let store = stores.entry(self.index.relation).or_default();
        let columns: Vec<usize> = (0..self.index.shared).collect();
        let rows: Vec<Vec<Value>> = store.matching(&columns, group).cloned().collect();
        rows.into_iter().filter_map(|row| {
            let kept = self.kept(store, &row);
            (kept > 0).then_some((row, kept))
        }).collect()
    }
    pub(super) fn unswept(&self, stores: &BTreeMap<RelationId, Store>, row: &[Value]) -> bool {
        self.swept.get(&row[..self.index.shared]).is_some_and(|swept| !swept.contains_key(row))
            && stores.get(&self.index.relation).is_some_and(|store| self.kept(store, row) > 0)
    }
    pub(super) fn sweep(&mut self, stores: &mut BTreeMap<RelationId, Store>, group: &[Value], op: CompareOp, changes: &BTreeMap<Value, i128>) -> usize {
        let shared = self.index.shared;
        let columns: Vec<usize> = (0..shared).collect();
        let store = stores.entry(self.index.relation).or_default();
        store.index(&columns);
        let store = &*store;
        let ascending = matches!(op, CompareOp::Less | CompareOp::LessEqual);
        let keys: Vec<(&Value, i128)> = if ascending { changes.iter().map(|(k, w)| (k, *w)).collect() } else { changes.iter().rev().map(|(k, w)| (k, *w)).collect() };
        let mut swept = BTreeMap::new();
        let rows = store.rows(&columns, group);
        if let (Some(rows), Some((first, _))) = (rows, keys.first()) {
            let mut probe = group.to_vec();
            probe.push((*first).clone());
            let range: Box<dyn Iterator<Item = &Vec<Value>>> = if ascending {
                Box::new(rows.range(probe..))
            } else {
                Box::new(rows.range(probe.clone()..).take_while(|row| row[shared] == **first).chain(rows.range(..probe).rev()))
            };
            let (mut next, mut running) = (0, 0i128);
            for row in range {
                let threshold = &row[shared];
                while next < keys.len() && passes(op, keys[next].0, threshold) { running += keys[next].1; next += 1; }
                if next == keys.len() && running == 0 { break; }
                if running == 0 { continue; }
                let kept = self.kept(store, row);
                if kept > 0 { swept.insert(row.clone(), kept); }
            }
        }
        let gained = self.gained.iter().filter(|(row, count)| row[..shared] == *group && store.count(row) == **count).count();
        let unvisited = rows.map_or(0, BTreeSet::len) - gained - swept.len();
        self.swept.insert(group.to_vec(), swept);
        unvisited
    }
}

fn passes(op: CompareOp, key: &Value, threshold: &Value) -> bool {
    match op { CompareOp::Less => key < threshold, CompareOp::LessEqual => key <= threshold, CompareOp::Greater => key > threshold, CompareOp::GreaterEqual => key >= threshold, _ => unreachable!() }
}

pub(super) struct Maintenance<'a> {
    pub(super) kept: Kept<'a>,
    pub(super) changed: bool,
    pub(super) replaced: bool,
}

pub(super) fn maintain<'a>(program: &Program, index: &'a Index, stores: &mut BTreeMap<RelationId, Store>, deltas: &mut BTreeMap<RelationId, Delta>, fill: bool, meter: &mut Meter) -> Result<Maintenance<'a>, EvalError> {
    let mut gained = BTreeMap::new();
    let (mut changed, mut replaced) = (false, false);
    if fill {
        let rows = execute::rule(program, &index.prefix, &index.schedule, stores, meter)?;
        let store = stores.entry(index.relation).or_default();
        for row in rows { store.insert(row); }
    } else {
        let lost = execute::pass(program, &index.prefix, &index.schedule, stores, deltas, true, meter)?;
        let added = execute::pass(program, &index.prefix, &index.schedule, stores, deltas, false, meter)?;
        changed = !lost.is_empty() || !added.is_empty();
        let column = index.shared;
        let rest = |row: &Vec<Value>| { let mut rest = row.clone(); rest.remove(column); rest };
        let mut thresholds: BTreeMap<Vec<Value>, BTreeSet<Value>> = BTreeMap::new();
        for row in &lost { thresholds.entry(rest(row)).or_default().insert(row[column].clone()); }
        replaced = added.iter().any(|row| thresholds.get(&rest(row)).is_some_and(|old| old.iter().any(|t| *t != row[column])));
        let store = stores.entry(index.relation).or_default();
        for row in &added { store.insert(row.clone()); }
        for row in &lost { store.remove(row); }
        for row in added { *gained.entry(row).or_insert(0) += 1; }
    }
    Ok(Maintenance { kept: Kept { index, gained, swept: BTreeMap::new(), visits: Cell::new(0) }, changed, replaced })
}
