use super::{execute, store::Store, Delta};
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
    pub(super) variables: Vec<Variable>,
    pub(super) shared: usize,
    literal: Literal,
    pub(super) ranked: Option<super::ranked::Plan>,
    at: String,
}

pub(super) fn expand(program: &mut Program) -> Vec<Plan> {
    let mut plans = Vec::new();
    for (index, rule) in program.rules.clone().iter().enumerate() {
        let mut outer = BTreeSet::new();
        let mut collect = |v: &Variable| { if !v.is_anonymous() { outer.insert(v.clone()); } };
        for expression in &rule.head.arguments { execute::variables(expression, &mut collect); }
        for literal in &rule.body {
            match literal {
                Literal::Aggregate { variable, .. } | Literal::ArgMin { variable, .. } => collect(variable),
                _ => {
                    for expression in execute::expressions(literal).unwrap() { execute::variables(expression, &mut collect); }
                    if let Literal::Bind { variable, .. } | Literal::Member { variable, .. } = literal { collect(variable); }
                }
            }
        }
        for (position, literal) in rule.body.iter().enumerate() {
            let body = match literal {
                Literal::Aggregate { body, .. } | Literal::ArgMin { body, .. } => body,
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
                .chain(execute::expressions(literal).unwrap()).chain(ranked.as_ref().map(|(p, _)| &p.key)) {
                execute::variables(expression, &mut |v| { if !v.is_anonymous() { mentioned.insert(v.clone()); } });
            }
            let shared: BTreeSet<_> = outer.intersection(&mentioned).cloned().collect();
            if !mentioned.iter().all(|v| types.contains_key(v))
                || !execute::expressions(literal).unwrap().into_iter().chain(ranked.as_ref().map(|(p, _)| &p.key)).chain(ranked.as_ref().map(|(p, _)| &p.threshold)).all(|e| execute::supported(program, e, &mut BTreeSet::new())) { continue; }
            let variables: Vec<_> = shared.iter().chain(mentioned.difference(&shared)).cloned().collect();
            let relation = RelationId(program.relations.len());
            program.relations.push(Relation::new(format!("$aggregate-{index}-{position}"), RelationKind::Derived,
                variables.iter().map(|v| Slot::new(v.0.clone(), types[v].clone()))));
            program.rules.push(Rule::new(format!("$aggregate-{index}-{position}"),
                Atom::new(relation, variables.iter().cloned().map(Expr::Variable))).when(body.clone()));
            let reads = execute::expressions(literal).unwrap().into_iter().chain(ranked.as_ref().map(|(p, _)| &p.key)).flat_map(|e| crate::check::expression_reads(program, e)).collect();
            plans.push(Plan { reads, rule: index, position, relation, variables, shared: shared.len(), literal: literal.clone(), at: rule.name.clone(), ranked: ranked.map(|(p, _)| p) });
        }
    }
    plans
}

pub(super) fn eligible(program: &Program, rule: &Rule, index: usize, plans: &[Plan]) -> bool {
    let mut plain = rule.clone();
    for (position, literal) in rule.body.iter().enumerate() {
        if matches!(literal, Literal::Aggregate { .. } | Literal::ArgMin { .. }) {
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

pub(super) type State = BTreeMap<RelationId, BTreeMap<Vec<Value>, Group>>;
pub(super) type Overlay = BTreeMap<RelationId, BTreeMap<Vec<Value>, Option<Group>>>;

pub(super) fn update(program: &Program, plans: &[Plan], heads: &BTreeSet<RelationId>, changed: &BTreeSet<RelationId>,
    stores: &mut BTreeMap<RelationId, Store>, state: &mut State, meter: &mut Meter, live_changed: bool, deltas: &BTreeMap<RelationId, Delta>, built: bool, touched: &mut Overlay) -> Result<(), EvalError> {
    for plan in plans.iter().filter(|p| heads.contains(&p.relation)) {
        let refresh = live_changed || !plan.reads.is_disjoint(changed);
        let groups = state.entry(plan.relation).or_default();
        let touched = touched.entry(plan.relation).or_default();
        let mut keep = |groups: &BTreeMap<Vec<Value>, Group>, key: &[Value]| {
            if !built && !touched.contains_key(key) { touched.insert(key.to_vec(), groups.get(key).cloned()); }
        };
        let deleted: Vec<_> = if refresh {
            groups.values().flat_map(|group| group.rows.keys().cloned()).collect()
        } else {
            deltas.get(&plan.relation).into_iter().flat_map(|delta| delta.deleted.presence().cloned()).collect()
        };
        let inserted: Vec<_> = if refresh || built {
            stores.get(&plan.relation).into_iter().flat_map(|store| store.presence().cloned()).collect()
        } else {
            deltas.get(&plan.relation).into_iter().flat_map(|delta| delta.inserted.presence().cloned()).collect()
        };
        for row in deleted {
            meter.charge(&plan.at)?;
            keep(groups, &row[..plan.shared]);
            if let Some(group) = groups.get_mut(&row[..plan.shared]) {
                if let Some(value) = group.rows.remove(&row) { group.count(&value, false, plan.ranked.is_some()); }
            }
        }
        for row in inserted {
            keep(groups, &row[..plan.shared]);
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
                _ => unreachable!(),
            };
            if matches!(result, Err(EvalError::Unknown(Reason::OutOfBudget { .. }))) { return result.map(|_| ()); }
            group.count(&result, true, plan.ranked.is_some());
            group.rows.insert(row, result);
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
    pub(super) counts: RefCell<[usize; 6]>,
    pub(super) ranked_counts: RefCell<[usize; 4]>,
}
impl Read<'_> {
    pub(super) fn plan(&self, rule: usize, position: usize) -> &Plan {
        self.plans.iter().find(|p| p.rule == rule && p.position == position).unwrap()
    }

    fn group(&self, relation: RelationId, key: &[Value]) -> Option<&Group> {
        match self.overlay.and_then(|overlay| overlay.get(&relation)).and_then(|groups| groups.get(key)) {
            Some(kept) => kept.as_ref(),
            None => self.state.get(&relation).and_then(|groups| groups.get(key)),
        }
    }

    pub(super) fn len(&self, relation: RelationId) -> usize {
        let current = self.state.get(&relation).map_or(0, BTreeMap::len);
        let Some(kept) = self.overlay.and_then(|overlay| overlay.get(&relation)) else { return current; };
        let now = |key: &Vec<Value>| self.state.get(&relation).is_some_and(|groups| groups.contains_key(key));
        current + kept.iter().filter(|(key, old)| old.is_some() && !now(key)).count()
            - kept.iter().filter(|(key, old)| old.is_none() && now(key)).count()
    }

    pub(super) fn changes(&self, old: &Read<'_>) -> BTreeMap<RelationId, BTreeSet<Vec<Value>>> {
        self.plans.iter().map(|plan| {
            let keys: BTreeSet<Vec<Value>> = old.overlay.and_then(|overlay| overlay.get(&plan.relation)).into_iter().flat_map(BTreeMap::keys).cloned().collect();
            let changed = keys.into_iter().filter(|key| {
                match (old.group(plan.relation, key), self.group(plan.relation, key)) {
                    (Some(a), Some(b)) => match (self.result(plan, a), self.result(plan, b)) {
                        (Some(a), Some(b)) => a != b,
                        _ => a != b,
                    },
                    (None, None) => false,
                    _ => true,
                }
            }).collect();
            (plan.relation, changed)
        }).collect()
    }

    fn result(&self, plan: &Plan, group: &Group) -> Option<Result<Option<Value>, EvalError>> {
        if plan.ranked.is_some() { return None; }
        let kind = Self::kind(plan);
        let valid = match kind { 0 => true, 1 => group.tally[1] + group.tally[2] == 0, _ => group.tally[2] == 0 && (group.tally[0] == 0 || group.tally[1] == 0) };
        if group.raised > 0 || !valid { return None; }
        Some((|| Ok(match kind {
            0 => Some(Value::Int(i64::try_from(group.rows.len()).map_err(|_| EvalError::Overflow { at: plan.at.clone() })?)),
            1 => Some(Value::Int(i64::try_from(group.sum).map_err(|_| EvalError::Overflow { at: plan.at.clone() })?)),
            2 => group.values.first_key_value().map(|(value, _)| value.clone()),
            3 => group.values.last_key_value().map(|(value, _)| value.clone()),
            4 => group.pairs.first_key_value().map(|((_, candidate), _)| candidate.clone()),
            5 => group.pairs.last_key_value().and_then(|((key, _), _)| group.pairs.range((key.clone(), Value::Null)..).next().map(|((_, candidate), _)| candidate.clone())),
            _ => unreachable!(),
        }))())
    }

    pub(super) fn kind(plan: &Plan) -> usize {
        match &plan.literal {
            Literal::Aggregate { op, .. } => match op { AggregateOp::Count => 0, AggregateOp::Sum(_) => 1, AggregateOp::Min(_) => 2, AggregateOp::Max(_) => 3 },
            Literal::ArgMin { direction, .. } => if *direction == ArgMinDirection::Min { 4 } else { 5 },
            _ => unreachable!(),
        }
    }

    pub(super) fn value(&self, rule: usize, position: usize, scope: &dyn Scope, eval: &Evaluator<'_, '_>) -> Result<Option<Value>, EvalError> {
        let plan = self.plan(rule, position);
        let key: Vec<_> = plan.variables[..plan.shared].iter().map(|v| scope.get(v).unwrap().clone()).collect();
        let empty = Group::default();
        let group = self.group(plan.relation, &key).unwrap_or(&empty);
        self.counts.borrow_mut()[Self::kind(plan)] += 1;
        if let Some(ranked) = &plan.ranked {
            if group.rows.is_empty() { eval.meter.borrow_mut().charge(&plan.at)?; return Ok(Some(Value::Int(0))); }
            if group.raised == 0 && group.invalid_weights == 0 && group.tally[2] == 0
                && (group.tally[0] == 0 || group.tally[1] == 0) {
                let threshold = eval.expr(&ranked.threshold, scope)?;
                if group.ranked.valid(&threshold) {
                    eval.meter.borrow_mut().charge(&plan.at)?;
                    self.ranked_counts.borrow_mut()[super::ranked::operator(ranked.op)] += 1;
                    let value = i64::try_from(group.ranked.value(ranked.op, &threshold)).map_err(|_| EvalError::Overflow { at: plan.at.clone() })?;
                    return Ok(Some(Value::Int(value)));
                }
            }
        }
        if let Some(result) = self.result(plan, group) { return result; }
        {
            let mut rows = Rows::new();
            for row in group.rows.keys() {
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
                _ => unreachable!(),
            };
        }

    }
}

pub(super) fn order(program: &Program, plans: &[Plan], checked: &mut CheckedProgram) {
    for plan in plans {
        let head = program.rules[plan.rule].head.relation;
        checked.dependencies.push(Dependency { head, body: plan.relation, kind: DependencyKind::Aggregate, rule: plan.rule });
        for &body in &plan.reads {
            checked.dependencies.push(Dependency { head: plan.relation, body, kind: DependencyKind::Positive, rule: plan.rule });
        }
    }
    checked.strata = crate::check::strata(program.relations.len(), &checked.components, &checked.dependencies);
}
