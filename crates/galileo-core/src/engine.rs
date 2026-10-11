use crate::check::nested_order;
use crate::expr::{self, Binding, Evaluator, Meter, Queries, Rows, Scope};
use crate::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

type Row = Vec<Value>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Stated(usize),
    Derived { rule: usize, height: u32 },
}

impl Origin {
    pub fn height(self) -> u32 {
        match self {
            Self::Stated(_) => 0,
            Self::Derived { height, .. } => height,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Premise {
    Present(RelationId, Row),
    Absent(RelationId, Vec<Option<Value>>),
    Compare(CompareOp, Value, Value),
    Condition(Value),
    Bound(Variable, Value),
    Member(Variable, Value),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Firing {
    pub rule: usize,
    pub premises: Vec<Premise>,
}

#[derive(Default)]
struct Store {
    rows: Vec<Row>,
    origins: Vec<Origin>,
    stated: Vec<usize>,
    set: HashMap<Row, usize>,
    indexes: HashMap<Vec<usize>, HashMap<Row, Vec<usize>>>,
    improved: Vec<usize>,
}

pub(crate) fn pattern(
    arguments: &[Expr],
    mut expr: impl FnMut(&Expr) -> Result<Value, EvalError>,
) -> Result<Vec<Option<Value>>, EvalError> {
    arguments
        .iter()
        .map(|argument| match argument {
            Expr::Variable(variable) if variable.is_anonymous() => Ok(None),
            argument => expr(argument).map(Some),
        })
        .collect()
}

pub(crate) fn agrees(pattern: &[Option<Value>], row: &[Value]) -> bool {
    pattern.iter().zip(row).all(|(slot, value)| slot.as_ref().is_none_or(|slot| slot == value))
}

fn absent(
    store: Option<&Store>,
    pattern: &[Option<Value>],
    mut charge: impl FnMut() -> Result<(), EvalError>,
) -> Result<bool, EvalError> {
    let Some(store) = store else {
        return Ok(true);
    };
    if let Some(row) = pattern.iter().cloned().collect::<Option<Row>>() {
        return Ok(!store.set.contains_key(&row));
    }
    for row in &store.rows {
        charge()?;
        if agrees(pattern, row) {
            return Ok(false);
        }
    }
    Ok(true)
}

impl Store {
    fn insert(&mut self, tuple: Row, origin: Origin) -> bool {
        if let Some(&position) = self.set.get(&tuple) {
            if origin.height() < self.origins[position].height() {
                self.origins[position] = origin;
                self.improved.push(position);
            }
            return false;
        }
        let position = self.rows.len();
        for (columns, index) in &mut self.indexes {
            let key = columns.iter().map(|&column| tuple[column].clone()).collect();
            index.entry(key).or_default().push(position);
        }
        self.set.insert(tuple.clone(), position);
        self.rows.push(tuple);
        self.origins.push(origin);
        true
    }

    fn matching<'s>(
        &'s self,
        columns: &[usize],
        key: &[Value],
        range: Range<usize>,
    ) -> Box<dyn Iterator<Item = usize> + 's> {
        if columns.is_empty() {
            return Box::new(range);
        }
        let positions = self.indexes[columns]
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let low = positions.partition_point(|&p| p < range.start);
        let high = positions.partition_point(|&p| p < range.end);
        Box::new(positions[low..high].iter().copied())
    }
}

#[derive(Default)]
pub(crate) struct Lazy(
    RefCell<HashMap<(usize, Vec<usize>), (usize, HashMap<Row, Vec<usize>>)>>,
    RefCell<HashMap<(usize, Binding), Option<Ranked>>>,
);

struct Ranked {
    keys: Vec<Value>,
    totals: Vec<i128>,
}

impl Lazy {
    fn positions(&self, store: &Store, relation: usize, columns: &[usize], key: &[Value]) -> Vec<usize> {
        if columns.is_empty() {
            return (0..store.rows.len()).collect();
        }
        let mut indexes = self.0.borrow_mut();
        let (built, index) = indexes.entry((relation, columns.to_vec())).or_default();
        for position in *built..store.rows.len() {
            let row = &store.rows[position];
            let key = columns.iter().map(|&column| row[column].clone()).collect();
            index.entry(key).or_default().push(position);
        }
        *built = store.rows.len();
        index.get(key).cloned().unwrap_or_default()
    }
}

enum Step<'p> {
    Scan {
        atom: &'p Atom,
        key_columns: Vec<usize>,
    },
    Test(&'p Literal),
}

struct Plan<'p> {
    index: usize,
    rule: &'p Rule,
    slots: HashMap<Variable, usize>,
    steps: Vec<Step<'p>>,
}

pub struct Derivation<'p> {
    program: &'p Program,
    state: &'p [Fact],
    plans: Vec<Plan<'p>>,
    stores: Vec<Store>,
    lazy: Lazy,
    pub work: u64,
    pub modes: Vec<crate::modes::ModeFailure>,
}

impl<'p> Derivation<'p> {
    pub fn program(&self) -> &'p Program {
        self.program
    }

    pub fn origin(&self, relation: RelationId, tuple: &[Value]) -> Option<Origin> {
        let store = self.stores.get(relation.0)?;
        store.set.get(tuple).map(|&position| store.origins[position])
    }

    pub fn stated(&self, index: usize) -> Option<&'p Fact> {
        if index < self.program.facts.len() {
            self.program.facts.get(index)
        } else {
            self.state.get(index - self.program.facts.len())
        }
    }

    pub(crate) fn rows(&self, relation: RelationId) -> &[Row] {
        self.stores.get(relation.0).map_or(&[], |store| &store.rows)
    }

    pub fn stated_facts(&self) -> impl Iterator<Item = &'p Fact> {
        self.program.facts.iter().chain(self.state)
    }

    pub(crate) fn stated_for_relation(&self, relation: RelationId) -> impl Iterator<Item = &'p Fact> + '_ {
        self.stores
            .get(relation.0)
            .into_iter()
            .flat_map(|store| store.stated.iter())
            .filter_map(move |&index| self.stated(index))
    }

    pub(crate) fn firings(
        &self,
        relation: RelationId,
        tuple: &[Value],
        recorded: Option<(usize, u32)>,
        meter: &mut Meter,
    ) -> Result<Vec<Firing>, EvalError> {
        let mut found = Vec::new();
        for plan in &self.plans {
            let below = match recorded {
                Some((rule, height)) if rule == plan.index => height,
                Some(_) => continue,
                None if plan.rule.head.relation == relation => u32::MAX,
                None => continue,
            };
            let ranges: Vec<Range<usize>> = plan
                .steps
                .iter()
                .map(|step| match step {
                    Step::Scan { atom, .. } => 0..self.stores[atom.relation.0].rows.len(),
                    Step::Test(_) => 0..0,
                })
                .collect();
            let mut derived = Vec::new();
            let mut search = Search::new(self.program, plan, &self.stores, &self.lazy, &ranges, meter, &mut derived);
            search.goal = Some(Goal {
                row: tuple,
                below,
                all: recorded.is_none(),
            });
            search.step(0)?;
            found.append(&mut search.found);
        }
        Ok(found)
    }

    pub fn model(self) -> Model {
        let relations = self
            .stores
            .into_iter()
            .enumerate()
            .filter(|(_, store)| !store.rows.is_empty())
            .map(|(id, store)| (RelationId(id), Arc::new(store.rows.into_iter().collect())))
            .collect();
        Model {
            relations,
            work: self.work,
        }
    }
}

pub fn evaluate(program: &Program, state: &[Fact], budget: Budget) -> Result<Model, EvalError> {
    evaluate_blocking(program, state, budget, &|_, _| false)
}

pub fn evaluate_blocking(
    program: &Program,
    state: &[Fact],
    budget: Budget,
    blocked: &dyn Fn(RelationId, &[Value]) -> bool,
) -> Result<Model, EvalError> {
    let mut derivation = derive_blocking(program, state, budget, blocked)?;
    if !derivation.modes.is_empty() {
        return Err(EvalError::Mode(std::mem::take(&mut derivation.modes)));
    }
    Ok(derivation.model())
}

pub fn derive<'p>(
    program: &'p Program,
    state: &'p [Fact],
    budget: Budget,
) -> Result<Derivation<'p>, EvalError> {
    derive_blocking(program, state, budget, &|_, _| false)
}

pub fn derive_blocking<'p>(
    program: &'p Program,
    state: &'p [Fact],
    budget: Budget,
    blocked: &dyn Fn(RelationId, &[Value]) -> bool,
) -> Result<Derivation<'p>, EvalError> {
    let checked = check(program).map_err(EvalError::Static)?;
    let plans: Vec<Plan> = program
        .rules
        .iter()
        .zip(&checked.schedules)
        .enumerate()
        .map(|(index, (rule, schedule))| plan(index, rule, schedule))
        .collect();
    let mut stores: Vec<Store> = Vec::new();
    let store = |stores: &mut Vec<Store>, id: RelationId| {
        if stores.len() <= id.0 {
            stores.resize_with(id.0 + 1, Store::default);
        }
    };
    for plan in &plans {
        for step in &plan.steps {
            if let Step::Scan { atom, key_columns } = step {
                store(&mut stores, atom.relation);
                if !key_columns.is_empty() {
                    stores[atom.relation.0]
                        .indexes
                        .entry(key_columns.clone())
                        .or_default();
                }
            }
        }
        store(&mut stores, plan.rule.head.relation);
    }
    let mut meter = Meter {
        limit: budget.work,
        spent: 0,
    };
    let lazy = Lazy::default();
    for (index, fact) in program.facts.iter().chain(state).enumerate() {
        meter.charge("stated facts")?;
        store(&mut stores, fact.relation);
        if blocked(fact.relation, &fact.values) {
            continue;
        }
        stores[fact.relation.0].stated.push(index);
        stores[fact.relation.0].insert(fact.values.clone(), Origin::Stated(index));
    }
    let top = checked.strata.iter().copied().max().unwrap_or(0);
    let mut modes = Vec::new();
    for stratum in 0..=top {
        let rules: Vec<&Plan> = plans
            .iter()
            .filter(|plan| checked.strata[plan.rule.head.relation.0] == stratum)
            .collect();
        let mut derived = Vec::new();
        for plan in &rules {
            let ranges: Vec<Range<usize>> = plan
                .steps
                .iter()
                .map(|step| match step {
                    Step::Scan { atom, .. } => 0..stores[atom.relation.0].rows.len(),
                    Step::Test(_) => 0..0,
                })
                .collect();
            run(program, plan, &stores, &lazy, &ranges, &mut meter, &mut derived)?;
        }
        loop {
            let before: Vec<usize> = stores.iter().map(|store| store.rows.len()).collect();
            derived.sort_by_key(|(_, _, origin): &(RelationId, Row, Origin)| origin.height());
            for (relation, tuple, origin) in derived.drain(..) {
                if blocked(relation, &tuple) {
                    continue;
                }
                stores[relation.0].insert(tuple, origin);
            }
            let after: Vec<usize> = stores.iter().map(|store| store.rows.len()).collect();
            let improved: Vec<Vec<usize>> = stores
                .iter_mut()
                .map(|store| {
                    let mut positions = std::mem::take(&mut store.improved);
                    positions.sort_unstable();
                    positions.dedup();
                    positions
                })
                .collect();
            if before == after && improved.iter().all(Vec::is_empty) {
                break;
            }
            for plan in &rules {
                for (delta, step) in plan.steps.iter().enumerate() {
                    let Step::Scan { atom, .. } = step else {
                        continue;
                    };
                    for &position in &improved[atom.relation.0] {
                        let ranges: Vec<Range<usize>> = plan
                            .steps
                            .iter()
                            .enumerate()
                            .map(|(index, step)| match step {
                                Step::Scan { .. } if index == delta => position..position + 1,
                                Step::Scan { atom, .. } => 0..after[atom.relation.0],
                                Step::Test(_) => 0..0,
                            })
                            .collect();
                        run(program, plan, &stores, &lazy, &ranges, &mut meter, &mut derived)?;
                    }
                    if before[atom.relation.0] == after[atom.relation.0] {
                        continue;
                    }
                    let ranges: Vec<Range<usize>> = plan
                        .steps
                        .iter()
                        .enumerate()
                        .map(|(position, step)| match step {
                            Step::Scan { atom, .. } => {
                                let relation = atom.relation.0;
                                if position < delta {
                                    0..before[relation]
                                } else if position == delta {
                                    before[relation]..after[relation]
                                } else {
                                    0..after[relation]
                                }
                            }
                            Step::Test(_) => 0..0,
                        })
                        .collect();
                    run(program, plan, &stores, &lazy, &ranges, &mut meter, &mut derived)?;
                }
            }
        }
        modes = crate::modes::check_stratum(program, &checked.strata, stratum, RelationKind::Derived, program.entities.iter(), &|relation, each| {
            stores.get(relation.0).into_iter().flat_map(|store| &store.rows).for_each(|row| each(row))
        });
        if !modes.is_empty() {
            break;
        }
    }
    Ok(Derivation {
        program,
        state,
        plans,
        stores,
        lazy,
        work: meter.spent,
        modes,
    })
}

fn plan<'p>(index: usize, rule: &'p Rule, schedule: &[usize]) -> Plan<'p> {
    let mut slots = HashMap::new();
    let mut grounded = HashSet::new();
    let mut steps = Vec::new();
    for &position in schedule {
        let literal = &rule.body[position];
        match literal {
            Literal::Positive(atom) => {
                let key_columns = atom
                    .arguments
                    .iter()
                    .enumerate()
                    .filter(|(_, argument)| ground(argument, &grounded))
                    .map(|(column, _)| column)
                    .collect();
                for argument in &atom.arguments {
                    if let Expr::Variable(variable) = argument {
                        grounded.insert(variable.clone());
                    }
                }
                steps.push(Step::Scan { atom, key_columns });
            }
            Literal::Bind { variable, .. } | Literal::Member { variable, .. } => {
                grounded.insert(variable.clone());
                steps.push(Step::Test(literal));
            }
            _ => steps.push(Step::Test(literal)),
        }
    }
    let mut collect = |variable: &Variable| {
        let next = slots.len();
        slots.entry(variable.clone()).or_insert(next);
    };
    for literal in &rule.body {
        literal_variables(literal, &mut collect);
    }
    for argument in &rule.head.arguments {
        expr_variables(argument, &mut collect);
    }
    Plan {
        index,
        rule,
        slots,
        steps,
    }
}

fn ground(expression: &Expr, grounded: &HashSet<Variable>) -> bool {
    let mut all = true;
    let keyable = expr_variables(expression, &mut |variable| {
        all &= grounded.contains(variable);
    });
    keyable && all
}

fn literal_variables(literal: &Literal, visit: &mut dyn FnMut(&Variable)) {
    match literal {
        Literal::Positive(atom) | Literal::Negative(atom) => {
            for argument in &atom.arguments {
                expr_variables(argument, visit);
            }
        }
        Literal::Compare { left, right, .. } => {
            expr_variables(left, visit);
            expr_variables(right, visit);
        }
        Literal::Boolean(value) => {
            expr_variables(value, visit);
        }
        Literal::Bind { variable, value } | Literal::Member { variable, list: value } => {
            visit(variable);
            expr_variables(value, visit);
        }
        Literal::Aggregate { variable, .. } | Literal::ArgMin { variable, .. } => visit(variable),
    }
}

pub(crate) fn expr_variables(expression: &Expr, visit: &mut dyn FnMut(&Variable)) -> bool {
    match expression {
        Expr::Value(_) => true,
        Expr::Variable(variable) => {
            visit(variable);
            true
        }
        Expr::List(items) | Expr::Call { arguments: items, .. } => {
            items.iter().fold(true, |all, item| expr_variables(item, visit) && all)
        }
        Expr::Record(fields) | Expr::Attrs(fields) => fields
            .values()
            .fold(true, |all, item| expr_variables(item, visit) && all),
        Expr::Field { value, .. } | Expr::Unary { value, .. } => expr_variables(value, visit),
        Expr::Binary { left, right, .. } => {
            let left = expr_variables(left, visit);
            expr_variables(right, visit) && left
        }
        Expr::If {
            condition,
            then_value,
            else_value,
        } => {
            let condition = expr_variables(condition, visit);
            let then_value = expr_variables(then_value, visit);
            expr_variables(else_value, visit) && condition && then_value
        }
        Expr::Lambda { .. } | Expr::Apply { .. } | Expr::Query(_) => false,
    }
}

fn run(
    program: &Program,
    plan: &Plan,
    stores: &[Store],
    lazy: &Lazy,
    ranges: &[Range<usize>],
    meter: &mut Meter,
    derived: &mut Vec<(RelationId, Row, Origin)>,
) -> Result<(), EvalError> {
    Search::new(program, plan, stores, lazy, ranges, meter, derived).step(0)
}

struct Goal<'a> {
    row: &'a [Value],
    below: u32,
    all: bool,
}

struct Search<'a> {
    program: &'a Program,
    plan: &'a Plan<'a>,
    stores: &'a [Store],
    lazy: &'a Lazy,
    ranges: &'a [Range<usize>],
    meter: RefCell<&'a mut Meter>,
    derived: &'a mut Vec<(RelationId, Row, Origin)>,
    env: Vec<Option<Value>>,
    at: &'a str,
    heights: Vec<u32>,
    goal: Option<Goal<'a>>,
    trail: Vec<Premise>,
    found: Vec<Firing>,
}

impl<'a> Search<'a> {
    fn new(
        program: &'a Program,
        plan: &'a Plan<'a>,
        stores: &'a [Store],
        lazy: &'a Lazy,
        ranges: &'a [Range<usize>],
        meter: &'a mut Meter,
        derived: &'a mut Vec<(RelationId, Row, Origin)>,
    ) -> Self {
        Search {
            program,
            plan,
            stores,
            lazy,
            ranges,
            meter: RefCell::new(meter),
            derived,
            env: vec![None; plan.slots.len()],
            at: &plan.rule.name,
            heights: Vec::new(),
            goal: None,
            trail: Vec::new(),
            found: Vec::new(),
        }
    }

    fn witnessing(&self) -> bool {
        self.goal.is_some()
    }

    fn step(&mut self, index: usize) -> Result<(), EvalError> {
        if self.goal.as_ref().is_some_and(|goal| !goal.all) && !self.found.is_empty() {
            return Ok(());
        }
        let plan = self.plan;
        let Some(step) = plan.steps.get(index) else {
            self.charge()?;
            let tuple = plan
                .rule
                .head
                .arguments
                .iter()
                .map(|argument| self.eval().expr(argument, &self.scope()))
                .collect::<Result<Row, _>>()?;
            if let Some(goal) = &self.goal {
                if tuple.as_slice() == goal.row {
                    self.found.push(Firing {
                        rule: plan.index,
                        premises: self.trail.clone(),
                    });
                }
                return Ok(());
            }
            let height = 1 + self.heights.iter().copied().max().unwrap_or(0);
            self.derived.push((
                plan.rule.head.relation,
                tuple,
                Origin::Derived {
                    rule: plan.index,
                    height,
                },
            ));
            return Ok(());
        };
        match step {
            Step::Scan { atom, key_columns } => {
                let (key_columns, key) = match key_columns
                    .iter()
                    .map(|&column| self.eval().expr(&atom.arguments[column], &self.scope()))
                    .collect::<Result<Row, _>>()
                {
                    Ok(key) => (key_columns.as_slice(), key),
                    Err(error @ EvalError::Unknown(Reason::OutOfBudget { .. })) => return Err(error),
                    Err(_) => (&[][..], Row::new()),
                };
                let stores = self.stores;
                let rows = stores[atom.relation.0].matching(
                    key_columns,
                    &key,
                    self.ranges[index].clone(),
                );
                for position in rows {
                    self.charge()?;
                    let store = &stores[atom.relation.0];
                    let row = &store.rows[position];
                    let height = store.origins[position].height();
                    if self.goal.as_ref().is_some_and(|goal| height >= goal.below) {
                        continue;
                    }
                    let mut bound = Vec::new();
                    let mut matched = true;
                    for (column, argument) in atom.arguments.iter().enumerate() {
                        if key_columns.contains(&column) {
                            continue;
                        }
                        if let Expr::Variable(variable) = argument {
                            let slot = plan.slots[variable];
                            match &self.env[slot] {
                                None => {
                                    self.env[slot] = Some(row[column].clone());
                                    bound.push(slot);
                                    continue;
                                }
                                Some(value) => {
                                    if *value != row[column] {
                                        matched = false;
                                        break;
                                    }
                                    continue;
                                }
                            }
                        }
                        if self.eval().expr(argument, &self.scope())? != row[column] {
                            matched = false;
                            break;
                        }
                    }
                    if matched {
                        self.heights.push(height);
                        if self.witnessing() {
                            self.trail.push(Premise::Present(atom.relation, row.clone()));
                        }
                        let result = self.step(index + 1);
                        self.heights.pop();
                        if self.witnessing() {
                            self.trail.pop();
                        }
                        result?;
                    }
                    for slot in bound {
                        self.env[slot] = None;
                    }
                }
                Ok(())
            }
            Step::Test(literal) => self.test(literal, index),
        }
    }

    fn test(&mut self, literal: &Literal, index: usize) -> Result<(), EvalError> {
        let (holds, premise) = match literal {
            Literal::Positive(_) => unreachable!("positive sentences are scans"),
            Literal::Negative(atom) => {
                self.charge()?;
                let scope = self.scope();
                let tuple = pattern(&atom.arguments, |argument| {
                    Ok(self.eval().exprs(std::slice::from_ref(argument), &scope)?.remove(0))
                })?;
                let holds = absent(self.stores.get(atom.relation.0), &tuple, || self.charge())?;
                (holds, Premise::Absent(atom.relation, tuple))
            }
            Literal::Compare { op, left, right } => {
                self.charge()?;
                let left = self.eval().expr(left, &self.scope())?;
                let right = self.eval().expr(right, &self.scope())?;
                let holds = self.eval().compare(*op, &left, &right)?;
                (holds, Premise::Compare(*op, left, right))
            }
            Literal::Boolean(expression) => {
                self.charge()?;
                let holds = self.eval().boolean(expression, &self.scope())?;
                (holds, Premise::Condition(Value::Bool(holds)))
            }
            Literal::Bind { variable, value } => {
                self.charge()?;
                let value = self.eval().expr(value, &self.scope())?;
                let premise = Premise::Bound(variable.clone(), value.clone());
                return self.witnessed(premise, |search| search.bind(variable, value, index));
            }
            Literal::Member { variable, list } => {
                self.charge()?;
                let Value::List(items) = self.eval().expr(list, &self.scope())? else {
                    return Err(self.eval().type_error("membership needs a list"));
                };
                for item in items {
                    self.charge()?;
                    let premise = Premise::Member(variable.clone(), item.clone());
                    self.witnessed(premise, |search| search.bind(variable, item, index))?;
                }
                return Ok(());
            }
            Literal::Aggregate { variable, .. } | Literal::ArgMin { variable, .. } => {
                let Some(value) = self.reduce(literal, &self.scope().all())? else {
                    return Ok(());
                };
                let premise = Premise::Bound(variable.clone(), value.clone());
                return self.witnessed(premise, |search| search.bind(variable, value, index));
            }
        };
        if holds {
            self.witnessed(premise, |search| search.step(index + 1))?;
        }
        Ok(())
    }

    fn witnessed(
        &mut self,
        premise: Premise,
        next: impl FnOnce(&mut Self) -> Result<(), EvalError>,
    ) -> Result<(), EvalError> {
        if !self.witnessing() {
            return next(self);
        }
        self.trail.push(premise);
        let result = next(self);
        self.trail.pop();
        result
    }

    fn bind(&mut self, variable: &Variable, value: Value, index: usize) -> Result<(), EvalError> {
        let slot = self.plan.slots[variable];
        match &self.env[slot] {
            Some(existing) => {
                if *existing == value {
                    self.step(index + 1)?;
                }
            }
            None => {
                self.env[slot] = Some(value);
                let result = self.step(index + 1);
                self.env[slot] = None;
                result?;
            }
        }
        Ok(())
    }

    fn eval(&self) -> Evaluator<'_, 'a> {
        Evaluator {
            program: self.program,
            at: self.at,
            queries: Some(self),
            meter: &self.meter,
        }
    }

    fn charge(&self) -> Result<(), EvalError> {
        self.meter.borrow_mut().charge(self.at)
    }

    fn reduce(&self, literal: &Literal, binding: &Binding) -> Result<Option<Value>, EvalError> {
        self.charge()?;
        let mut eval = |e: &Expr, row: &Binding| self.eval().expr(e, row);
        match literal {
            Literal::Aggregate { op, body, .. } => {
                if let Some(value) = self.ranked(literal, op, body, binding)? {
                    return Ok(Some(value));
                }
                let extras: Vec<&Expr> = op.value().into_iter().collect();
                let rows = self.rows(body, &extras, binding)?;
                expr::aggregate(op, &rows, &mut eval, self.at)
            }
            Literal::ArgMin {
                direction,
                candidate,
                key,
                body,
                ..
            } => {
                let rows = self.rows(body, &[candidate, key], binding)?;
                expr::argmin(*direction, candidate, key, &rows, &mut eval, self.at)
            }
            _ => unreachable!("only aggregates and argmin reduce"),
        }
    }

    fn ranked(
        &self,
        literal: &Literal,
        aggregate: &AggregateOp,
        body: &[Literal],
        binding: &Binding,
    ) -> Result<Option<Value>, EvalError> {
        if !matches!(aggregate, AggregateOp::Count | AggregateOp::Sum(_)) {
            return Ok(None);
        }
        let mut comparisons =
            body.iter()
                .enumerate()
                .filter_map(|(position, literal)| match literal {
                    Literal::Compare { op, left, right }
                        if matches!(
                            op,
                            CompareOp::Less
                                | CompareOp::LessEqual
                                | CompareOp::Greater
                                | CompareOp::GreaterEqual
                        ) =>
                    {
                        Some((position, *op, left, right))
                    }
                    _ => None,
                });
        let Some((position, mut op, left, right)) = comparisons.next() else {
            return Ok(None);
        };
        if comparisons.next().is_some() {
            return Ok(None);
        }
        let variables = |expression: &Expr| {
            let mut variables = HashSet::new();
            expr_variables(expression, &mut |variable| {
                variables.insert(variable.clone());
            })
            .then_some(variables)
        };
        let (Some(left_variables), Some(right_variables)) = (variables(left), variables(right))
        else {
            return Ok(None);
        };
        let local = |variables: &HashSet<Variable>| {
            !variables.is_empty()
                && variables
                    .iter()
                    .all(|variable| !binding.contains_key(variable))
        };
        let outer = |variables: &HashSet<Variable>| {
            !variables.is_empty()
                && variables
                    .iter()
                    .all(|variable| binding.contains_key(variable))
        };
        let (key, threshold, threshold_variables) =
            if local(&left_variables) && outer(&right_variables) {
                (left, right, right_variables)
            } else if outer(&left_variables) && local(&right_variables) {
                op = match op {
                    CompareOp::Less => CompareOp::Greater,
                    CompareOp::LessEqual => CompareOp::GreaterEqual,
                    CompareOp::Greater => CompareOp::Less,
                    CompareOp::GreaterEqual => CompareOp::LessEqual,
                    _ => unreachable!(),
                };
                (right, left, left_variables)
            } else {
                return Ok(None);
            };
        let mut used = HashSet::new();
        for (index, literal) in body.iter().enumerate() {
            if index == position {
                continue;
            }
            let expressions: Vec<&Expr> = match literal {
                Literal::Positive(atom) | Literal::Negative(atom) => {
                    atom.arguments.iter().collect()
                }
                Literal::Compare { left, right, .. } => vec![left, right],
                Literal::Boolean(value) => vec![value],
                Literal::Bind { variable, value }
                | Literal::Member {
                    variable,
                    list: value,
                } => {
                    used.insert(variable.clone());
                    vec![value]
                }
                Literal::Aggregate { .. } | Literal::ArgMin { .. } => return Ok(None),
            };
            for expression in expressions {
                if !expr_variables(expression, &mut |variable| {
                    used.insert(variable.clone());
                }) {
                    return Ok(None);
                }
            }
        }
        if let Some(value) = aggregate.value() {
            if !expr_variables(value, &mut |variable| {
                used.insert(variable.clone());
            }) {
                return Ok(None);
            }
        }
        if !used.is_disjoint(&threshold_variables) {
            return Ok(None);
        }
        let group: Binding = binding
            .iter()
            .filter(|(variable, _)| used.contains(*variable))
            .map(|(variable, value)| (variable.clone(), value.clone()))
            .collect();
        let cache_key = (literal as *const Literal as usize, group.clone());
        let spent = self.meter.borrow().spent;
        let cached: HashSet<_> = self.lazy.1.borrow().keys().cloned().collect();
        let fallback = || {
            self.meter.borrow_mut().spent = spent;
            self.lazy
                .1
                .borrow_mut()
                .retain(|key, _| cached.contains(key));
        };
        if !cached.contains(&cache_key) {
            let build = || -> Result<Option<Ranked>, EvalError> {
                let unranked: Vec<Literal> = body
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != position)
                    .map(|(_, literal)| literal.clone())
                    .collect();
                let mut extras = vec![key];
                extras.extend(aggregate.value());
                let rows = self.rows(&unranked, &extras, &group)?;
                let mut values = Vec::with_capacity(rows.len());
                for row in rows {
                    self.charge()?;
                    let key = self.eval().expr(key, &row)?;
                    if !matches!(key, Value::Int(_) | Value::Text(_))
                        || values.first().is_some_and(|(first, _): &(Value, i128)| {
                            std::mem::discriminant(first) != std::mem::discriminant(&key)
                        })
                    {
                        return Ok(None);
                    }
                    let value = match aggregate {
                        AggregateOp::Count => 1,
                        AggregateOp::Sum(expression) => {
                            let Value::Int(value) = self.eval().expr(expression, &row)? else {
                                return Ok(None);
                            };
                            i128::from(value)
                        }
                        _ => unreachable!(),
                    };
                    values.push((key, value));
                }
                values.sort_by(|(left, _), (right, _)| left.cmp(right));
                let mut keys = Vec::with_capacity(values.len());
                let mut totals = vec![0i128];
                for (key, value) in values {
                    let Some(total) = totals.last().unwrap().checked_add(value) else {
                        return Ok(None);
                    };
                    keys.push(key);
                    totals.push(total);
                }
                Ok(Some(Ranked { keys, totals }))
            };
            let ranked = match build() {
                Ok(Some(ranked)) => Some(ranked),
                Err(error @ EvalError::Unknown(Reason::OutOfBudget { .. })) => return Err(error),
                Ok(None) | Err(_) => {
                    fallback();
                    None
                }
            };
            self.lazy.1.borrow_mut().insert(cache_key.clone(), ranked);
        }
        {
            let cache = self.lazy.1.borrow();
            let Some(ranked) = cache[&cache_key].as_ref() else {
                return Ok(None);
            };
            if ranked.keys.is_empty() {
                self.charge()?;
                return Ok(Some(Value::Int(0)));
            }
        }
        let threshold = match self.eval().expr(threshold, binding) {
            Ok(value) => value,
            Err(error @ EvalError::Unknown(Reason::OutOfBudget { .. })) => return Err(error),
            Err(_) => {
                fallback();
                return Ok(None);
            }
        };
        let cache = self.lazy.1.borrow();
        let ranked = cache[&cache_key].as_ref().unwrap();
        if !matches!(
            (&ranked.keys[0], &threshold),
            (Value::Int(_), Value::Int(_)) | (Value::Text(_), Value::Text(_))
        ) {
            drop(cache);
            fallback();
            return Ok(None);
        }
        self.charge()?;
        let boundary = ranked.keys.partition_point(|key| match op {
            CompareOp::Less | CompareOp::GreaterEqual => key < &threshold,
            CompareOp::LessEqual | CompareOp::Greater => key <= &threshold,
            _ => unreachable!(),
        });
        let total = match op {
            CompareOp::Less | CompareOp::LessEqual => ranked.totals[boundary],
            CompareOp::Greater | CompareOp::GreaterEqual => {
                ranked.totals.last().unwrap() - ranked.totals[boundary]
            }
            _ => unreachable!(),
        };
        let value = i64::try_from(total).map_err(|_| EvalError::Overflow {
            at: self.at.to_string(),
        })?;
        Ok(Some(Value::Int(value)))
    }

    fn rows(&self, body: &[Literal], extras: &[&Expr], binding: &Binding) -> Result<Rows, EvalError> {
        let bound = binding.keys().cloned().collect();
        let order = nested_order(self.program, body, extras, &bound);
        let literals: Vec<&Literal> = order.iter().map(|&position| &body[position]).collect();
        let mut rows = Rows::new();
        self.nested(&literals, binding.clone(), &mut rows)?;
        Ok(rows)
    }

    fn nested(&self, body: &[&Literal], binding: Binding, rows: &mut Rows) -> Result<(), EvalError> {
        let Some((literal, rest)) = body.split_first() else {
            self.charge()?;
            rows.insert(binding);
            return Ok(());
        };
        for extended in self.nested_literal(literal, &binding)? {
            self.nested(rest, extended, rows)?;
        }
        Ok(())
    }

    fn nested_literal(&self, literal: &Literal, binding: &Binding) -> Result<Vec<Binding>, EvalError> {
        let eval = self.eval();
        let pass = |holds: bool| if holds { vec![binding.clone()] } else { Vec::new() };
        Ok(match literal {
            Literal::Positive(atom) => {
                let Some(store) = self.stores.get(atom.relation.0) else {
                    return Ok(Vec::new());
                };
                let columns: Vec<usize> = atom
                    .arguments
                    .iter()
                    .enumerate()
                    .filter(|(_, argument)| {
                        let mut all = true;
                        let keyable = expr_variables(argument, &mut |variable| {
                            all &= binding.contains_key(variable);
                        });
                        keyable && all
                    })
                    .map(|(column, _)| column)
                    .collect();
                let (columns, key) = match columns
                    .iter()
                    .map(|&column| eval.expr(&atom.arguments[column], binding))
                    .collect::<Result<Row, _>>()
                {
                    Ok(key) => (columns, key),
                    Err(error @ EvalError::Unknown(Reason::OutOfBudget { .. })) => return Err(error),
                    Err(_) => (Vec::new(), Row::new()),
                };
                let mut results = Vec::new();
                for position in self.lazy.positions(store, atom.relation.0, &columns, &key) {
                    self.charge()?;
                    let row = &store.rows[position];
                    let mut extended = binding.clone();
                    let mut matched = true;
                    for (column, argument) in atom.arguments.iter().enumerate() {
                        if columns.contains(&column) {
                            continue;
                        }
                        if let Expr::Variable(variable) = argument {
                            match extended.get(variable) {
                                Some(value) if *value != row[column] => {
                                    matched = false;
                                    break;
                                }
                                Some(_) => {}
                                None => {
                                    extended.insert(variable.clone(), row[column].clone());
                                }
                            }
                        } else if eval.expr(argument, &extended)? != row[column] {
                            matched = false;
                            break;
                        }
                    }
                    if matched {
                        results.push(extended);
                    }
                }
                results
            }
            Literal::Negative(atom) => {
                self.charge()?;
                let tuple = pattern(&atom.arguments, |argument| {
                    Ok(eval.exprs(std::slice::from_ref(argument), binding)?.remove(0))
                })?;
                pass(absent(self.stores.get(atom.relation.0), &tuple, || self.charge())?)
            }
            Literal::Compare { op, left, right } => {
                self.charge()?;
                let left = eval.expr(left, binding)?;
                let right = eval.expr(right, binding)?;
                pass(eval.compare(*op, &left, &right)?)
            }
            Literal::Boolean(expression) => {
                self.charge()?;
                pass(eval.boolean(expression, binding)?)
            }
            Literal::Bind { variable, value } => {
                self.charge()?;
                let value = eval.expr(value, binding)?;
                extend(binding, variable, value).into_iter().collect()
            }
            Literal::Member { variable, list } => {
                self.charge()?;
                let Value::List(items) = eval.expr(list, binding)? else {
                    return Err(eval.type_error("membership needs a list"));
                };
                let mut results = Vec::new();
                for item in items {
                    self.charge()?;
                    results.extend(extend(binding, variable, item));
                }
                results
            }
            Literal::Aggregate { variable, .. } | Literal::ArgMin { variable, .. } => self
                .reduce(literal, binding)?
                .and_then(|value| extend(binding, variable, value))
                .into_iter()
                .collect(),
        })
    }

    fn scope(&self) -> RuleScope<'_> {
        RuleScope {
            slots: &self.plan.slots,
            env: &self.env,
        }
    }
}

impl Queries for Search<'_> {
    fn query(&self, query: &Query, scope: &dyn Scope) -> Result<Value, EvalError> {
        self.charge()?;
        let rows = self.rows(&query.body, &expr::query_extras(query), &scope.all())?;
        expr::query(query, &rows, &mut |e, row| self.eval().expr(e, row), self.at)
    }

    fn field(&self, relation: RelationId, entity: &Entity) -> Option<Value> {
        let store = &self.stores[relation.0];
        let positions =
            self.lazy
                .positions(store, relation.0, &[0], &[Value::Entity(entity.clone())]);
        positions
            .first()
            .map(|&position| store.rows[position][1].clone())
    }
}

fn extend(binding: &Binding, variable: &Variable, value: Value) -> Option<Binding> {
    match binding.get(variable) {
        Some(existing) => (*existing == value).then(|| binding.clone()),
        None => {
            let mut extended = binding.clone();
            extended.insert(variable.clone(), value);
            Some(extended)
        }
    }
}

struct RuleScope<'a> {
    slots: &'a HashMap<Variable, usize>,
    env: &'a [Option<Value>],
}

impl Scope for RuleScope<'_> {
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
