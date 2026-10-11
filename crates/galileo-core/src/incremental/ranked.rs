use super::execute;
use crate::*;
use std::collections::BTreeSet;

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
