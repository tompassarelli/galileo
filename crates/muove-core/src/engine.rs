use crate::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;

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
    Absent(RelationId, Row),
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
    set: HashMap<Row, usize>,
    indexes: HashMap<Vec<usize>, HashMap<Row, Vec<usize>>>,
}

impl Store {
    fn insert(&mut self, tuple: Row, origin: Origin) -> bool {
        if self.set.contains_key(&tuple) {
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

pub struct Derivation<'p> {
    program: &'p Program,
    state: &'p [Fact],
    plans: Vec<Plan<'p>>,
    stores: Vec<Store>,
    pub work: u64,
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
        self.program.facts.iter().chain(self.state).nth(index)
    }

    pub(crate) fn rows(&self, relation: RelationId) -> &[Row] {
        self.stores.get(relation.0).map_or(&[], |store| &store.rows)
    }

    pub fn stated_facts(&self) -> impl Iterator<Item = &'p Fact> {
        self.program.facts.iter().chain(self.state)
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
            let mut search = Search::new(self.program, plan, &self.stores, &ranges, meter, &mut derived);
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
            .map(|(id, store)| (RelationId(id), store.rows.into_iter().collect()))
            .collect();
        Model {
            relations,
            work: self.work,
        }
    }
}

pub fn evaluate(program: &Program, state: &[Fact], budget: Budget) -> Result<Model, EvalError> {
    Ok(derive(program, state, budget)?.model())
}

pub fn derive<'p>(
    program: &'p Program,
    state: &'p [Fact],
    budget: Budget,
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
    for (index, fact) in program.facts.iter().chain(state).enumerate() {
        meter.charge("stated facts")?;
        store(&mut stores, fact.relation);
        stores[fact.relation.0].insert(fact.values.clone(), Origin::Stated(index));
    }
    let top = checked.strata.iter().copied().max().unwrap_or(0);
    for stratum in 0..=top {
        let rules: Vec<&Plan> = plans
            .iter()
            .filter(|plan| checked.strata[plan.rule.head.relation.0] == stratum)
            .collect();
        if rules.is_empty() {
            continue;
        }
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
            run(program, plan, &stores, &ranges, &mut meter, &mut derived)?;
        }
        loop {
            let before: Vec<usize> = stores.iter().map(|store| store.rows.len()).collect();
            derived.sort_by_key(|(_, _, origin): &(RelationId, Row, Origin)| origin.height());
            for (relation, tuple, origin) in derived.drain(..) {
                stores[relation.0].insert(tuple, origin);
            }
            let after: Vec<usize> = stores.iter().map(|store| store.rows.len()).collect();
            if before == after {
                break;
            }
            for plan in &rules {
                for (delta, step) in plan.steps.iter().enumerate() {
                    let Step::Scan { atom, .. } = step else {
                        continue;
                    };
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
                    run(program, plan, &stores, &ranges, &mut meter, &mut derived)?;
                }
            }
        }
    }
    Ok(Derivation {
        program,
        state,
        plans,
        stores,
        work: meter.spent,
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

fn expr_variables(expression: &Expr, visit: &mut dyn FnMut(&Variable)) -> bool {
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
    ranges: &[Range<usize>],
    meter: &mut Meter,
    derived: &mut Vec<(RelationId, Row, Origin)>,
) -> Result<(), EvalError> {
    Search::new(program, plan, stores, ranges, meter, derived).step(0)
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
    ranges: &'a [Range<usize>],
    meter: &'a mut Meter,
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
        ranges: &'a [Range<usize>],
        meter: &'a mut Meter,
        derived: &'a mut Vec<(RelationId, Row, Origin)>,
    ) -> Self {
        Search {
            program,
            plan,
            stores,
            ranges,
            meter,
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
            self.meter.charge(self.at)?;
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
                    Err(_) => (&[][..], Row::new()),
                };
                let stores = self.stores;
                let rows = stores[atom.relation.0].matching(
                    key_columns,
                    &key,
                    self.ranges[index].clone(),
                );
                for position in rows {
                    self.meter.charge(self.at)?;
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
                self.meter.charge(self.at)?;
                let tuple = atom
                    .arguments
                    .iter()
                    .map(|argument| self.eval().expr(argument, &self.scope()))
                    .collect::<Result<Row, _>>()?;
                let holds = !self
                    .stores
                    .get(atom.relation.0)
                    .is_some_and(|store| store.set.contains_key(&tuple));
                (holds, Premise::Absent(atom.relation, tuple))
            }
            Literal::Compare { op, left, right } => {
                self.meter.charge(self.at)?;
                let left = self.eval().expr(left, &self.scope())?;
                let right = self.eval().expr(right, &self.scope())?;
                let holds = self.eval().compare(*op, &left, &right)?;
                (holds, Premise::Compare(*op, left, right))
            }
            Literal::Boolean(expression) => {
                self.meter.charge(self.at)?;
                let holds = self.eval().boolean(expression, &self.scope())?;
                (holds, Premise::Condition(Value::Bool(holds)))
            }
            Literal::Bind { variable, value } => {
                self.meter.charge(self.at)?;
                let value = self.eval().expr(value, &self.scope())?;
                let premise = Premise::Bound(variable.clone(), value.clone());
                return self.witnessed(premise, |search| search.bind(variable, value, index));
            }
            Literal::Member { variable, list } => {
                let Value::List(items) = self.eval().expr(list, &self.scope())? else {
                    return Err(self.eval().type_error("membership needs a list"));
                };
                for item in items {
                    self.meter.charge(self.at)?;
                    let premise = Premise::Member(variable.clone(), item.clone());
                    self.witnessed(premise, |search| search.bind(variable, item, index))?;
                }
                return Ok(());
            }
            Literal::Aggregate { .. } => return Err(self.eval().unsupported("an aggregate")),
            Literal::ArgMin { .. } => return Err(self.eval().unsupported("an argmin")),
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

    fn eval(&self) -> Evaluator<'_> {
        Evaluator {
            program: self.program,
            at: self.at,
        }
    }

    fn scope(&self) -> RuleScope<'_> {
        RuleScope {
            slots: &self.plan.slots,
            env: &self.env,
        }
    }
}

pub(crate) trait Scope {
    fn get(&self, variable: &Variable) -> Option<&Value>;
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
}

impl Scope for BTreeMap<Variable, Value> {
    fn get(&self, variable: &Variable) -> Option<&Value> {
        BTreeMap::get(self, variable)
    }
}

pub(crate) struct Evaluator<'a> {
    pub(crate) program: &'a Program,
    pub(crate) at: &'a str,
}

impl Evaluator<'_> {
    pub(crate) fn boolean(&self, expression: &Expr, scope: &dyn Scope) -> Result<bool, EvalError> {
        match self.expr(expression, scope)? {
            Value::Bool(value) => Ok(value),
            _ => Err(self.type_error("a condition must be a Bool")),
        }
    }

    fn exprs(&self, expressions: &[Expr], scope: &dyn Scope) -> Result<Vec<Value>, EvalError> {
        expressions
            .iter()
            .map(|expression| self.expr(expression, scope))
            .collect()
    }

    fn fields(
        &self,
        fields: &BTreeMap<String, Expr>,
        scope: &dyn Scope,
    ) -> Result<BTreeMap<String, Value>, EvalError> {
        fields
            .iter()
            .map(|(name, expression)| Ok((name.clone(), self.expr(expression, scope)?)))
            .collect()
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
                Value::Entity(_) => return Err(self.unsupported("an entity field")),
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
                let Some(definition) = self
                    .program
                    .definitions
                    .iter()
                    .find(|definition| &definition.name == function)
                else {
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
            Expr::Query(_) => return Err(self.unsupported("a query in a rule")),
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
    fn type_error(&self, message: &str) -> EvalError {
        EvalError::Type {
            at: self.at.to_string(),
            message: message.to_string(),
        }
    }
}
