use crate::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;

type Row = Vec<Value>;

#[derive(Default)]
struct Store {
    rows: Vec<Row>,
    set: HashSet<Row>,
    indexes: HashMap<Vec<usize>, HashMap<Row, Vec<usize>>>,
}

impl Store {
    fn insert(&mut self, tuple: Row) -> bool {
        if self.set.contains(&tuple) {
            return false;
        }
        let position = self.rows.len();
        for (columns, index) in &mut self.indexes {
            let key = columns.iter().map(|&column| tuple[column].clone()).collect();
            index.entry(key).or_default().push(position);
        }
        self.set.insert(tuple.clone());
        self.rows.push(tuple);
        true
    }

    fn matching<'s>(
        &'s self,
        columns: &[usize],
        key: &[Value],
        range: Range<usize>,
    ) -> Box<dyn Iterator<Item = &'s Row> + 's> {
        if columns.is_empty() {
            return Box::new(self.rows[range].iter());
        }
        let positions = self.indexes[columns]
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let low = positions.partition_point(|&p| p < range.start);
        let high = positions.partition_point(|&p| p < range.end);
        Box::new(positions[low..high].iter().map(|&p| &self.rows[p]))
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
    rule: &'p Rule,
    slots: HashMap<Variable, usize>,
    steps: Vec<Step<'p>>,
}

struct Meter {
    limit: u64,
    spent: u64,
}

impl Meter {
    fn charge(&mut self, at: &str) -> Result<(), EvalError> {
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

pub fn evaluate(program: &Program, state: &[Fact], budget: Budget) -> Result<Model, EvalError> {
    let checked = check(program).map_err(EvalError::Static)?;
    let plans: Vec<Plan> = program
        .rules
        .iter()
        .zip(&checked.schedules)
        .map(|(rule, schedule)| plan(rule, schedule))
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
    for fact in program.facts.iter().chain(state) {
        meter.charge("stated facts")?;
        store(&mut stores, fact.relation);
        stores[fact.relation.0].insert(fact.values.clone());
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
            for (relation, tuple) in derived.drain(..) {
                stores[relation.0].insert(tuple);
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
    let relations = stores
        .into_iter()
        .enumerate()
        .filter(|(_, store)| !store.rows.is_empty())
        .map(|(id, store)| (RelationId(id), store.rows.into_iter().collect()))
        .collect();
    Ok(Model {
        relations,
        work: meter.spent,
    })
}

fn plan<'p>(rule: &'p Rule, schedule: &[usize]) -> Plan<'p> {
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
    Plan { rule, slots, steps }
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
    derived: &mut Vec<(RelationId, Row)>,
) -> Result<(), EvalError> {
    let mut search = Search {
        program,
        plan,
        stores,
        ranges,
        meter,
        derived,
        env: vec![None; plan.slots.len()],
        at: &plan.rule.name,
    };
    search.step(0)
}

struct Search<'a> {
    program: &'a Program,
    plan: &'a Plan<'a>,
    stores: &'a [Store],
    ranges: &'a [Range<usize>],
    meter: &'a mut Meter,
    derived: &'a mut Vec<(RelationId, Row)>,
    env: Vec<Option<Value>>,
    at: &'a str,
}

impl Search<'_> {
    fn step(&mut self, index: usize) -> Result<(), EvalError> {
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
            self.derived.push((plan.rule.head.relation, tuple));
            return Ok(());
        };
        match step {
            Step::Scan { atom, key_columns } => {
                let key = key_columns
                    .iter()
                    .map(|&column| self.eval().expr(&atom.arguments[column], &self.scope()))
                    .collect::<Result<Row, _>>()?;
                let stores = self.stores;
                let rows = stores[atom.relation.0].matching(
                    key_columns,
                    &key,
                    self.ranges[index].clone(),
                );
                for row in rows {
                    self.meter.charge(self.at)?;
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
                        self.step(index + 1)?;
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
        let holds = match literal {
            Literal::Positive(_) => unreachable!("positive sentences are scans"),
            Literal::Negative(atom) => {
                self.meter.charge(self.at)?;
                let tuple = atom
                    .arguments
                    .iter()
                    .map(|argument| self.eval().expr(argument, &self.scope()))
                    .collect::<Result<Row, _>>()?;
                !self
                    .stores
                    .get(atom.relation.0)
                    .is_some_and(|store| store.set.contains(&tuple))
            }
            Literal::Compare { op, left, right } => {
                self.meter.charge(self.at)?;
                let left = self.eval().expr(left, &self.scope())?;
                let right = self.eval().expr(right, &self.scope())?;
                self.eval().compare(*op, &left, &right)?
            }
            Literal::Boolean(expression) => {
                self.meter.charge(self.at)?;
                self.eval().boolean(expression, &self.scope())?
            }
            Literal::Bind { variable, value } => {
                self.meter.charge(self.at)?;
                let value = self.eval().expr(value, &self.scope())?;
                return self.bind(variable, value, index);
            }
            Literal::Member { variable, list } => {
                let Value::List(items) = self.eval().expr(list, &self.scope())? else {
                    return Err(self.eval().type_error("membership needs a list"));
                };
                for item in items {
                    self.meter.charge(self.at)?;
                    self.bind(variable, item, index)?;
                }
                return Ok(());
            }
            Literal::Aggregate { .. } => return Err(self.eval().unsupported("an aggregate")),
            Literal::ArgMin { .. } => return Err(self.eval().unsupported("an argmin")),
        };
        if holds {
            self.step(index + 1)?;
        }
        Ok(())
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

trait Scope {
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

struct Evaluator<'a> {
    program: &'a Program,
    at: &'a str,
}

impl Evaluator<'_> {
    fn boolean(&self, expression: &Expr, scope: &dyn Scope) -> Result<bool, EvalError> {
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

    fn expr(&self, expression: &Expr, scope: &dyn Scope) -> Result<Value, EvalError> {
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

    fn compare(&self, op: CompareOp, left: &Value, right: &Value) -> Result<bool, EvalError> {
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
