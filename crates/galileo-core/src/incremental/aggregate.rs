use super::{execute, query, store::Store, Delta};
use crate::expr::{self, Binding, Evaluator, Meter, Rows, Scope};
use crate::*;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(super) struct Plan {
    pub(super) rule: usize,
    pub(super) position: usize,
    pub(super) relation: RelationId,
    pub(super) reads: BTreeSet<RelationId>,
    unkeyed: BTreeSet<RelationId>,
    keyed: BTreeSet<(usize, String)>,
    pub(super) variables: Vec<Variable>,
    pub(super) shared: usize,
    literal: Literal,
    pub(super) ranked: Option<super::ranked::Plan>,
    at: String,
}

pub(super) fn expand(program: &mut Program) -> Vec<Plan> {
    let mut plans = Vec::new();
    for index in 0..program.rules.len() {
        let rule = rewrite(program, &program.rules[index], index);
        program.rules[index] = rule.clone();
        let rule = &rule;
        let mut outer = BTreeSet::new();
        let mut collect = |v: &Variable| { if !v.is_anonymous() { outer.insert(v.clone()); } };
        for expression in &rule.head.arguments { execute::variables(expression, &mut collect); }
        for literal in &rule.body {
            match literal {
                Literal::Aggregate { variable, .. } | Literal::ArgMin { variable, .. } | Literal::Bind { variable, value: Expr::Query(_), .. } => collect(variable),
                _ => {
                    for expression in query::expressions(literal) { execute::variables(expression, &mut collect); }
                    if let Literal::Bind { variable, .. } | Literal::Member { variable, .. } = literal { collect(variable); }
                }
            }
        }
        for (position, literal) in rule.body.iter().enumerate() {
            let body = match literal {
                Literal::Aggregate { body, .. } | Literal::ArgMin { body, .. } => body,
                Literal::Bind { value: Expr::Query(q), .. } => &q.body,
                _ => continue,
            };
            let ranked = super::ranked::shape(literal, &outer);
            let body = if let Some((_, body)) = &ranked { body } else { body };
            if !body.iter().all(execute::plain) { continue; }
            let mut types = BTreeMap::new();
            for literal in body {
                if let Literal::Positive(atom) = literal {
                    for (argument, slot) in atom.arguments.iter().zip(&program.relations[atom.relation.0].slots) {
                        if let Expr::Variable(v) = argument {
                            if !v.is_anonymous() { types.insert(v.clone(), slot.ty.clone()); }
                        }
                    }
                }
            }
            let mut mentioned = BTreeSet::new();
            for expression in body.iter().flat_map(|l| execute::expressions(l).unwrap())
                .chain(query::expressions(literal)).chain(ranked.as_ref().map(|(p, _)| &p.key)) {
                execute::variables(expression, &mut |v| { if !v.is_anonymous() { mentioned.insert(v.clone()); } });
            }
            let shared: BTreeSet<_> = outer.intersection(&mentioned).cloned().collect();
            if !mentioned.iter().all(|v| types.contains_key(v))
                || !query::expressions(literal).into_iter().chain(ranked.as_ref().map(|(p, _)| &p.key)).chain(ranked.as_ref().map(|(p, _)| &p.threshold)).all(|e| execute::supported(program, e, &mut BTreeSet::new())) { continue; }
            let variables: Vec<_> = shared.iter().chain(mentioned.difference(&shared)).cloned().collect();
            let relation = RelationId(program.relations.len());
            program.relations.push(Relation::new(format!("$aggregate-{index}-{position}"), RelationKind::Derived,
                variables.iter().map(|v| Slot::new(v.0.clone(), types[v].clone()))));
            program.rules.push(Rule::new(format!("$aggregate-{index}-{position}"),
                Atom::new(relation, variables.iter().cloned().map(Expr::Variable))).when(body.clone()));
            let read: Vec<&Expr> = query::expressions(literal).into_iter().chain(ranked.as_ref().map(|(p, _)| &p.key)).collect();
            let reads = read.iter().flat_map(|e| crate::check::expression_reads(program, e)).collect();
            let mut unkeyed: BTreeSet<_> = read.iter().flat_map(|e| crate::check::expression_reads(program, &execute::unkeyed(e))).collect();
            let mut keyed = BTreeSet::new();
            for e in &read {
                execute::keyed(e, &mut |v, field| match variables.iter().position(|w| w == v) {
                    Some(column) => { keyed.insert((column, field.to_string())); }
                    None => unkeyed.extend(crate::check::field_relations(program, field)),
                });
            }
            plans.push(Plan { reads, unkeyed, keyed, rule: index, position, relation, variables, shared: shared.len(), literal: literal.clone(), at: rule.name.clone(), ranked: ranked.map(|(p, _)| p) });
        }
    }
    plans
}

fn rewrite(program: &Program, rule: &Rule, index: usize) -> Rule {
    let mut rule = rule.clone();
    for literal in &mut rule.body {
        let (Literal::Aggregate { body, .. } | Literal::ArgMin { body, .. }) = literal else { continue };
        let mut types = BTreeMap::new();
        for literal in body.iter() {
            if let Literal::Positive(atom) = literal {
                for (argument, slot) in atom.arguments.iter().zip(&program.relations[atom.relation.0].slots) {
                    if let (Expr::Variable(v), Type::Entity(t)) = (argument, &slot.ty) { types.insert(v.clone(), t.clone()); }
                }
            }
        }
        for literal in body.iter_mut() {
            let Literal::Boolean(Expr::Field { value, field }) = literal else { continue };
            let Expr::Variable(x) = value.as_ref() else { continue };
            let Some(relation) = types.get(x).and_then(|t| program.field_relation(t, field)) else { continue };
            let relation_def = &program.relations[relation.0];
            if !relation_def.modes.iter().any(|m| matches!(m, Mode::ExactlyOne { .. })) || relation_def.slots.len() != 2 || relation_def.slots[1].ty != Type::Bool { continue; }
            *literal = Literal::Positive(Atom::new(relation, [Expr::Variable(x.clone()), Expr::Value(Value::Bool(true))]));
        }
    }
    let mut outer = BTreeSet::new();
    for literal in &rule.body {
        if let Literal::Positive(atom) = literal {
            for (argument, slot) in atom.arguments.iter().zip(&program.relations[atom.relation.0].slots) {
                if let (Expr::Variable(v), Type::Entity(_)) = (argument, &slot.ty) { outer.insert(v.clone()); }
            }
        }
    }
    let mut position = 0;
    while position < rule.body.len() {
        let literal = &mut rule.body[position];
        let Literal::Aggregate { op: op @ (AggregateOp::Count | AggregateOp::Sum(_)), body, .. } = literal else { position += 1; continue };
        let ordering = |l: &Literal| matches!(l, Literal::Compare { op: CompareOp::Less | CompareOp::LessEqual | CompareOp::Greater | CompareOp::GreaterEqual, .. });
        if body.iter().filter(|l| ordering(l)).count() != 1 { position += 1; continue; }
        let at = body.iter().position(ordering).unwrap();
        let mut found = None;
        for side in 0..2 {
            let Literal::Compare { left, right, .. } = &body[at] else { unreachable!() };
            let (this, other) = if side == 0 { (left, right) } else { (right, left) };
            let Expr::Field { value, .. } = this else { continue };
            let Expr::Variable(y) = value.as_ref() else { continue };
            if !outer.contains(y) { continue; }
            let mut elsewhere = false;
            let mut visit = |v: &Variable| { if v == y { elsewhere = true; } };
            execute::variables(other, &mut visit);
            for (i, l) in body.iter().enumerate() {
                if i != at { for e in execute::expressions(l).unwrap_or_default() { execute::variables(e, &mut visit); } }
            }
            if let Some(e) = op.value() { execute::variables(e, &mut visit); }
            if !elsewhere { found = Some(side); break; }
        }
        let Some(side) = found else { position += 1; continue };
        let fresh = Variable::new(format!("$threshold-{index}-{position}"));
        let Literal::Compare { left, right, .. } = &mut body[at] else { unreachable!() };
        let this = if side == 0 { left } else { right };
        let value = std::mem::replace(this, Expr::Variable(fresh.clone()));
        rule.body.insert(position, Literal::Bind { variable: fresh, value });
        if position < rule.body_sources.len() { let source = rule.body_sources[position].clone(); rule.body_sources.insert(position, source); }
        position += 2;
    }
    rule
}

pub(super) fn eligible(program: &Program, rule: &Rule, index: usize, plans: &[Plan]) -> bool {
    let mut plain = rule.clone();
    for (position, literal) in rule.body.iter().enumerate() {
        if matches!(literal, Literal::Aggregate { .. } | Literal::ArgMin { .. } | Literal::Bind { value: Expr::Query(_), .. }) {
            if !plans.iter().any(|p| p.rule == index && p.position == position) { return false; }
            plain.body[position] = Literal::Boolean(Expr::value(true));
        }
    }
    execute::eligible(program, &plain)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Group {
    rows: BTreeMap<Vec<Value>, Result<(Value, Option<Value>), EvalError>>,
    sum: i128,
    values: BTreeMap<Value, usize>,
    pairs: BTreeMap<(Value, Value), usize>,
    tally: [usize; 3],
    raised: usize,
    ranked: super::ranked::Multiset,
    invalid_weights: usize,
    query: RefCell<Option<Result<Value, EvalError>>>,
}

impl Group {
    fn count(&mut self, result: &Result<(Value, Option<Value>), EvalError>, adding: bool, ranked: bool) {
        let change = |n: &mut usize| { if adding { *n += 1; } else { *n -= 1; } };
        let Ok((value, candidate)) = result else { change(&mut self.raised); return; };
        change(&mut self.tally[match value { Value::Int(_) => 0, Value::Text(_) => 1, _ => 2 }]);
        if let Value::Int(n) = value { self.sum += if adding { i128::from(*n) } else { -i128::from(*n) }; }
        let count = self.values.entry(value.clone()).or_default();
        change(count);
        if *count == 0 { self.values.remove(value); }
        if let Some(candidate) = candidate {
            if ranked { if let Value::Int(weight) = candidate {
                self.ranked.change(value, i128::from(*weight), adding);
            } else { change(&mut self.invalid_weights); } }
            let pair = (value.clone(), candidate.clone());
            let count = self.pairs.entry(pair.clone()).or_default();
            change(count);
            if *count == 0 { self.pairs.remove(&pair); }
        }
    }
}

type Entry = Result<(Value, Option<Value>), EvalError>;

#[derive(Clone, Debug)]
pub(super) struct Record {
    existed: bool,
    len: usize,
    sum: i128,
    tally: [usize; 3],
    raised: usize,
    invalid_weights: usize,
    ranked: bool,
    query: Option<RefCell<Option<Result<Value, EvalError>>>>,
    rows: BTreeMap<Vec<Value>, Option<Entry>>,
}

impl Record {
    fn new(group: Option<&Group>, ranked: bool) -> Self {
        let empty = Group::default();
        let g = group.unwrap_or(&empty);
        Record { existed: group.is_some(), len: g.rows.len(), sum: g.sum, tally: g.tally, raised: g.raised, invalid_weights: g.invalid_weights, ranked, query: None, rows: BTreeMap::new() }
    }

    fn change(&mut self, row: &[Value], old: Option<&Entry>, group: &mut Group, untracked: &mut usize) {
        if !self.rows.contains_key(row) { self.rows.insert(row.to_vec(), old.cloned()); *untracked += 1; }
        if self.query.is_none() { self.query = Some(RefCell::new(group.query.get_mut().take())); }
    }

    pub(super) fn undo(self, groups: &mut BTreeMap<Vec<Value>, Group>, key: Vec<Value>) {
        if !self.existed { groups.remove(&key); return; }
        let group = groups.entry(key).or_default();
        for (row, old) in self.rows {
            if let Some(current) = group.rows.remove(&row) { group.count(&current, false, self.ranked); }
            if let Some(old) = old { group.count(&old, true, self.ranked); group.rows.insert(row, old); }
        }
        if let Some(query) = self.query { group.query = query; }
    }
}

fn record<'t>(touched: &'t mut BTreeMap<Vec<Value>, Record>, groups: &BTreeMap<Vec<Value>, Group>, key: &[Value], ranked: bool) -> &'t mut Record {
    if !touched.contains_key(key) { touched.insert(key.to_vec(), Record::new(groups.get(key), ranked)); }
    touched.get_mut(key).unwrap()
}

fn walk<K: Ord + Clone>(current: impl Iterator<Item = (K, usize)>, delta: impl Iterator<Item = (K, isize)>, reverse: bool) -> Option<K> {
    let (mut current, mut delta) = (current.peekable(), delta.peekable());
    loop {
        let order = match (current.peek(), delta.peek()) {
            (None, None) => return None,
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some((a, _)), Some((b, _))) => if reverse { b.cmp(a) } else { a.cmp(b) },
        };
        let (key, count) = match order {
            std::cmp::Ordering::Less => { let (key, n) = current.next().unwrap(); (key, n as isize) }
            std::cmp::Ordering::Greater => delta.next().unwrap(),
            std::cmp::Ordering::Equal => { let (key, n) = current.next().unwrap(); (key, n as isize + delta.next().unwrap().1) }
        };
        if count > 0 { return Some(key); }
    }
}

fn counts(op: CompareOp, key: &Value, threshold: &Value) -> bool {
    let below = key < threshold || (matches!(op, CompareOp::LessEqual | CompareOp::Greater) && key == threshold);
    below != matches!(op, CompareOp::Greater | CompareOp::GreaterEqual)
}

#[derive(Clone, Copy)]
pub(super) struct GroupView<'a> {
    group: Option<&'a Group>,
    record: Option<&'a Record>,
}
impl<'a> GroupView<'a> {
    fn new(group: &'a Group) -> Self { GroupView { group: Some(group), record: None } }
    fn current(self) -> impl Iterator<Item = (&'a Vec<Value>, &'a Entry)> { self.group.into_iter().flat_map(|g| g.rows.iter()) }
    fn changed(self) -> impl Iterator<Item = (Option<&'a Entry>, Option<&'a Entry>)> {
        let group = self.group;
        self.record.into_iter().flat_map(|r| r.rows.iter()).map(move |(row, old)| (old.as_ref(), group.and_then(|g| g.rows.get(row))))
    }
    fn len(self) -> usize { self.record.map_or_else(|| self.group.map_or(0, |g| g.rows.len()), |r| r.len) }
    fn is_empty(self) -> bool { self.len() == 0 }
    fn sum(self) -> i128 { self.record.map_or_else(|| self.group.map_or(0, |g| g.sum), |r| r.sum) }
    fn tally(self) -> [usize; 3] { self.record.map_or_else(|| self.group.map_or([0; 3], |g| g.tally), |r| r.tally) }
    fn raised(self) -> usize { self.record.map_or_else(|| self.group.map_or(0, |g| g.raised), |r| r.raised) }
    fn invalid_weights(self) -> usize { self.record.map_or_else(|| self.group.map_or(0, |g| g.invalid_weights), |r| r.invalid_weights) }
    fn deltas<K: Ord>(self, key: impl Fn(&'a (Value, Option<Value>)) -> Option<K>) -> BTreeMap<K, isize> {
        let mut deltas = BTreeMap::new();
        for (old, now) in self.changed() {
            for (entry, sign) in [(old, 1), (now, -1)] {
                if let Some(k) = entry.and_then(|e| e.as_ref().ok()).and_then(&key) { *deltas.entry(k).or_default() += sign; }
            }
        }
        deltas
    }
    fn value(self, reverse: bool) -> Option<Value> {
        let deltas = self.deltas(|(value, _)| Some(value));
        let values = self.group.into_iter().flat_map(|g| g.values.iter());
        if reverse { walk(values.rev().map(|(k, n)| (k, *n)), deltas.into_iter().rev(), true) } else { walk(values.map(|(k, n)| (k, *n)), deltas.into_iter(), false) }.cloned()
    }
    fn first_value(self) -> Option<Value> { self.value(false) }
    fn last_value(self) -> Option<Value> { self.value(true) }
    fn pair_deltas(self) -> BTreeMap<(Value, Value), isize> { self.deltas(|(value, candidate)| candidate.as_ref().map(|c| (value.clone(), c.clone()))) }
    fn pairs(self) -> impl DoubleEndedIterator<Item = (&'a (Value, Value), &'a usize)> { self.group.into_iter().flat_map(|g| g.pairs.iter()) }
    fn argmin_candidate(self) -> Option<Value> {
        walk(self.pairs().map(|(k, n)| (k.clone(), *n)), self.pair_deltas().into_iter(), false).map(|(_, candidate)| candidate)
    }
    fn argmax_candidate(self) -> Option<Value> {
        let deltas = self.pair_deltas();
        let (key, _) = walk(self.pairs().rev().map(|(k, n)| (k.clone(), *n)), deltas.clone().into_iter().rev(), true)?;
        let from = (key, Value::Null);
        let pairs = self.group.into_iter().flat_map(|g| g.pairs.range(from.clone()..)).map(|(k, n)| (k.clone(), *n));
        walk(pairs, deltas.into_iter().filter(|(k, _)| *k >= from), false).map(|(_, candidate)| candidate)
    }
    fn ranked_valid(self, threshold: &Value) -> bool {
        match self.record {
            None => self.group.is_none_or(|g| g.ranked.valid(threshold)),
            Some(r) => r.len == 0 || matches!(threshold, Value::Int(_)) && r.tally[0] > 0 || matches!(threshold, Value::Text(_)) && r.tally[1] > 0,
        }
    }
    fn ranked_value(self, op: CompareOp, threshold: &Value) -> i128 {
        let weight = |entry: Option<&Entry>| match entry {
            Some(Ok((key, Some(Value::Int(weight))))) if counts(op, key, threshold) => i128::from(*weight),
            _ => 0,
        };
        self.group.map_or(0, |g| g.ranked.value(op, threshold)) + self.changed().map(|(old, now)| weight(old) - weight(now)).sum::<i128>()
    }
    fn query(self) -> &'a RefCell<Option<Result<Value, EvalError>>> {
        self.record.and_then(|r| r.query.as_ref()).unwrap_or_else(|| &self.group.expect("an unchanged group is current").query)
    }
    fn rows(self) -> Vec<&'a Vec<Value>> {
        let Some(record) = self.record else { return self.current().map(|(row, _)| row).collect(); };
        let mut rows: Vec<_> = self.current().map(|(row, _)| row).filter(|row| record.rows.get(*row).is_none_or(Option::is_some))
            .chain(record.rows.iter().filter(|(row, old)| old.is_some() && self.group.is_none_or(|g| !g.rows.contains_key(*row))).map(|(row, _)| row)).collect();
        rows.sort_unstable();
        rows
    }
    fn differs(self, current: GroupView<'_>) -> bool {
        let Some(record) = self.record else { return false; };
        let now = current.group;
        record.rows.iter().any(|(row, old)| old.as_ref() != now.and_then(|g| g.rows.get(row)))
            || record.query.as_ref().is_some_and(|query| *query.borrow() != *current.query().borrow())
    }
}

pub(super) type State = BTreeMap<RelationId, BTreeMap<Vec<Value>, Group>>;
pub(super) type Overlay = BTreeMap<RelationId, BTreeMap<Vec<Value>, Record>>;

#[allow(clippy::too_many_arguments)]
pub(super) fn update(program: &Program, plans: &[Plan], heads: &BTreeSet<RelationId>, changed: &BTreeSet<RelationId>,
    stores: &mut BTreeMap<RelationId, Store>, state: &mut State, meter: &mut Meter, deltas: &BTreeMap<RelationId, Delta>, built: bool, touched: &mut Overlay, keyed_refreshes: &mut [usize; 2], weights: &mut super::ranked::Changes, untracked: &mut usize) -> Result<(), EvalError> {
    for plan in plans.iter().filter(|p| heads.contains(&p.relation)) {
        let refresh = !built && !plan.unkeyed.is_disjoint(changed);
        let groups = state.entry(plan.relation).or_default();
        let touched = touched.entry(plan.relation).or_default();
        let weights = weights.entry(plan.relation).or_default();
        let mut weigh = |group: &[Value], result: &Result<(Value, Option<Value>), EvalError>, sign: i128| {
            if let (false, Some(_), Ok((key, Some(Value::Int(weight))))) = (built, &plan.ranked, result) {
                let change = weights.entry(group.to_vec()).or_default().entry(key.clone()).or_default();
                change.0 += sign * i128::from(*weight);
                change.1 += sign as isize;
            }
        };
        let empty = Delta::default();
        let delta = deltas.get(&plan.relation).unwrap_or(&empty);
        let mut kept = BTreeSet::new();
        if !refresh && !built {
            let store = stores.entry(plan.relation).or_default();
            for (column, field) in &plan.keyed {
                for entity in execute::changed_keys(program, field, deltas) {
                    kept.extend(store.matching(&[*column], &[Value::Entity(entity)]).filter(|row| delta.inserted.count(row) == 0).cloned());
                }
            }
            if !kept.is_empty() {
                keyed_refreshes[0] += 1;
                keyed_refreshes[1] += usize::from(plan.ranked.is_some());
            }
        }
        let deleted: Vec<_> = if refresh {
            groups.values().flat_map(|group| group.rows.keys().cloned()).collect()
        } else {
            delta.deleted.presence().chain(&kept).cloned().collect()
        };
        let inserted: Vec<_> = if refresh || built {
            stores.get(&plan.relation).into_iter().flat_map(|store| store.presence().cloned()).collect()
        } else {
            delta.inserted.presence().chain(&kept).cloned().collect()
        };
        for row in deleted {
            meter.charge(&plan.at)?;
            let mut undo = (!built).then(|| record(touched, groups, &row[..plan.shared], plan.ranked.is_some()));
            if let Some(group) = groups.get_mut(&row[..plan.shared]) {
                if let Some(value) = group.rows.remove(&row) {
                    if let Some(undo) = undo.as_deref_mut() { undo.change(&row, Some(&value), group, untracked); }
                    weigh(&row[..plan.shared], &value, -1); group.count(&value, false, plan.ranked.is_some()); *group.query.get_mut() = None;
                }
            }
        }
        for row in inserted {
            let mut undo = (!built).then(|| record(touched, groups, &row[..plan.shared], plan.ranked.is_some()));
            let group = groups.entry(row[..plan.shared].to_vec()).or_default();
            if group.rows.contains_key(&row) { continue; }
            meter.charge(&plan.at)?;
            let binding: Binding = plan.variables.iter().cloned().zip(row.iter().cloned()).collect();
            let reader = Fields(RefCell::new(&mut *stores));
            let meter = RefCell::new(&mut *meter);
            let eval = Evaluator { program, at: &plan.at, queries: Some(&reader), meter: &meter };
            let result = match &plan.literal {
                Literal::Aggregate { op, .. } if plan.ranked.is_some() => {
                    let ranked = plan.ranked.as_ref().unwrap();
                    eval.expr(&ranked.key, &binding).and_then(|key| {
                        op.value().map_or(Ok(Value::Int(1)), |e| eval.expr(e, &binding)).map(|weight| (key, Some(weight)))
                    })
                },
                Literal::Aggregate { op, .. } => op.value().map_or(Ok((Value::Int(0), None)), |e| eval.expr(e, &binding).map(|value| (value, None))),
                Literal::ArgMin { key, candidate, .. } => eval.expr(key, &binding).and_then(|key| eval.expr(candidate, &binding).map(|candidate| (key, Some(candidate)))),
                Literal::Bind { value: Expr::Query(_), .. } => Ok((Value::Int(0), None)),
                _ => unreachable!(),
            };
            if matches!(result, Err(EvalError::Unknown(Reason::OutOfBudget { .. }))) { return result.map(|_| ()); }
            if let Some(undo) = undo.as_deref_mut() { undo.change(&row, None, group, untracked); }
            weigh(&row[..plan.shared], &result, 1);
            group.count(&result, true, plan.ranked.is_some());
            group.rows.insert(row, result);
            *group.query.get_mut() = None;
        }
        groups.retain(|_, group| !group.rows.is_empty());
    }
    Ok(())
}

struct Fields<'a>(RefCell<&'a mut BTreeMap<RelationId, Store>>);
impl expr::Queries for Fields<'_> {
    fn query(&self, _: &Query, _: &dyn Scope) -> Result<Value, EvalError> { unreachable!() }
    fn field(&self, relation: RelationId, entity: &Entity) -> Option<Value> {
        self.0.borrow_mut().entry(relation).or_default().matching(&[0], &[Value::Entity(entity.clone())]).next().map(|r| r[1].clone())
    }
}

pub(super) struct Read<'a> {
    pub(super) plans: &'a [Plan],
    pub(super) state: &'a State,
    pub(super) overlay: Option<&'a Overlay>,
    pub(super) counts: RefCell<[usize; 8]>,
    pub(super) ranked_counts: RefCell<[usize; 4]>,
}
impl Read<'_> {
    pub(super) fn plan(&self, rule: usize, position: usize) -> &Plan {
        self.plans.iter().find(|p| p.rule == rule && p.position == position).unwrap()
    }

    pub(super) fn sweepable(&self, relation: RelationId, key: &[Value]) -> Option<usize> {
        let group = self.group(relation, key)?;
        let tally = group.tally();
        let typed = (tally[0] > 0, tally[1] > 0);
        (!group.is_empty() && group.raised() == 0 && group.invalid_weights() == 0 && tally[2] == 0 && typed.0 != typed.1).then_some(usize::from(typed.1))
    }

    fn group(&self, relation: RelationId, key: &[Value]) -> Option<GroupView<'_>> {
        let current = self.state.get(&relation).and_then(|groups| groups.get(key));
        match self.overlay.and_then(|overlay| overlay.get(&relation)).and_then(|groups| groups.get(key)) {
            Some(record) => record.existed.then_some(GroupView { group: current, record: Some(record) }),
            None => current.map(GroupView::new),
        }
    }

    pub(super) fn len(&self, relation: RelationId) -> usize {
        let current = self.state.get(&relation).map_or(0, BTreeMap::len);
        let Some(kept) = self.overlay.and_then(|overlay| overlay.get(&relation)) else { return current; };
        let now = |key: &Vec<Value>| self.state.get(&relation).is_some_and(|groups| groups.contains_key(key));
        current + kept.iter().filter(|(key, old)| old.existed && !now(key)).count()
            - kept.iter().filter(|(key, old)| !old.existed && now(key)).count()
    }

    pub(super) fn changes(&self, old: &Read<'_>) -> BTreeMap<RelationId, BTreeSet<Vec<Value>>> {
        self.plans.iter().map(|plan| {
            let keys: BTreeSet<Vec<Value>> = old.overlay.and_then(|overlay| overlay.get(&plan.relation)).into_iter().flat_map(BTreeMap::keys).cloned().collect();
            let changed = keys.into_iter().filter(|key| {
                match (old.group(plan.relation, key), self.group(plan.relation, key)) {
                    (Some(a), Some(b)) => match (self.result(plan, a), self.result(plan, b)) {
                        (Some(a), Some(b)) => a != b,
                        _ => a.differs(b),
                    },
                    (None, None) => false,
                    _ => true,
                }
            }).collect();
            (plan.relation, changed)
        }).collect()
    }

    fn result(&self, plan: &Plan, group: GroupView<'_>) -> Option<Result<Option<Value>, EvalError>> {
        if plan.ranked.is_some() || matches!(&plan.literal, Literal::Bind { value: Expr::Query(_), .. }) { return None; }
        let kind = Self::kind(plan);
        let tally = group.tally();
        let valid = match kind { 0 => true, 1 => tally[1] + tally[2] == 0, _ => tally[2] == 0 && (tally[0] == 0 || tally[1] == 0) };
        if group.raised() > 0 || !valid { return None; }
        Some((|| Ok(match kind {
            0 => Some(Value::Int(i64::try_from(group.len()).map_err(|_| EvalError::Overflow { at: plan.at.clone() })?)),
            1 => Some(Value::Int(i64::try_from(group.sum()).map_err(|_| EvalError::Overflow { at: plan.at.clone() })?)),
            2 => Some(group.first_value().unwrap_or(Value::Null)),
            3 => Some(group.last_value().unwrap_or(Value::Null)),
            4 => Some(group.argmin_candidate().unwrap_or(Value::Null)),
            5 => Some(group.argmax_candidate().unwrap_or(Value::Null)),
            _ => unreachable!(),
        }))())
    }

    pub(super) fn kind(plan: &Plan) -> usize {
        match &plan.literal {
            Literal::Aggregate { op, .. } => match op { AggregateOp::Count => 0, AggregateOp::Sum(_) => 1, AggregateOp::Min(_) => 2, AggregateOp::Max(_) => 3 },
            Literal::ArgMin { direction, .. } => if *direction == ArgMinDirection::Min { 4 } else { 5 },
            Literal::Bind { value: Expr::Query(q), .. } => match q.kind { QueryKind::All(_) => 6, QueryKind::The(_) => 7 },
            _ => unreachable!(),
        }
    }

    pub(super) fn value(&self, rule: usize, position: usize, scope: &dyn Scope, eval: &Evaluator<'_, '_>) -> Result<Option<Value>, EvalError> {
        let plan = self.plan(rule, position);
        let key: Vec<_> = plan.variables[..plan.shared].iter().map(|v| scope.get(v).unwrap().clone()).collect();
        let empty = Group::default();
        let group = self.group(plan.relation, &key).unwrap_or(GroupView::new(&empty));
        self.counts.borrow_mut()[Self::kind(plan)] += 1;
        if let Some(ranked) = &plan.ranked {
            if group.is_empty() { eval.meter.borrow_mut().charge(&plan.at)?; return Ok(Some(Value::Int(0))); }
            let tally = group.tally();
            if group.raised() == 0 && group.invalid_weights() == 0 && tally[2] == 0
                && (tally[0] == 0 || tally[1] == 0) {
                let threshold = eval.expr(&ranked.threshold, scope)?;
                if group.ranked_valid(&threshold) {
                    eval.meter.borrow_mut().charge(&plan.at)?;
                    self.ranked_counts.borrow_mut()[super::ranked::operator(ranked.op)] += 1;
                    let value = i64::try_from(group.ranked_value(ranked.op, &threshold)).map_err(|_| EvalError::Overflow { at: plan.at.clone() })?;
                    return Ok(Some(Value::Int(value)));
                }
            }
        }
        if let Some(result) = self.result(plan, group) { return result; }
        if matches!(&plan.literal, Literal::Bind { value: Expr::Query(_), .. }) {
            if let Some(result) = group.query().borrow().as_ref() { return result.clone().map(Some); }
        }
        {
            let mut rows = Rows::new();
            for row in group.rows() {
                eval.meter.borrow_mut().charge(&plan.at)?;
                let mut binding: Binding = scope.all();
                binding.extend(plan.variables.iter().cloned().zip(row.iter().cloned()));
                if let Some(ranked) = &plan.ranked {
                    let key = eval.expr(&ranked.key, &binding)?;
                    let threshold = eval.expr(&ranked.threshold, &binding)?;
                    if !eval.compare(ranked.op, &key, &threshold)? { continue; }
                }
                rows.insert(binding);
            }
            let mut evaluate = |e: &Expr, row: &Binding| eval.expr(e, row);
            return match &plan.literal {
                Literal::Aggregate { op, .. } => expr::aggregate(op, &rows, &mut evaluate, &plan.at),
                Literal::ArgMin { direction, candidate, key, .. } => expr::argmin(*direction, candidate, key, &rows, &mut evaluate, &plan.at),
                Literal::Bind { value: Expr::Query(q), .. } => query::value(q, &rows, group.query(), &mut evaluate, &plan.at).map(Some),
                _ => unreachable!(),
            };
        }

    }
}

pub(super) fn order(program: &Program, plans: &[Plan], strata: &[usize], checked: &mut CheckedProgram) {
    for plan in plans {
        let head = program.rules[plan.rule].head.relation;
        checked.dependencies.push(Dependency { head, body: plan.relation, kind: DependencyKind::Aggregate, rule: plan.rule });
        for &body in &plan.reads {
            checked.dependencies.push(Dependency { head: plan.relation, body, kind: DependencyKind::Positive, rule: plan.rule });
        }
    }
    for (target, source) in checked.strata.iter_mut().zip(strata) { *target = source * 2 + 1; }
    for plan in plans { checked.strata[plan.relation.0] = checked.strata[program.rules[plan.rule].head.relation.0] - 1; }
}
