use crate::*;
use std::collections::BTreeSet;
use std::fmt;

type Variables = BTreeSet<Variable>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependencyKind {
    Positive,
    Negation,
    Aggregate,
    Query,
}

impl fmt::Display for DependencyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Positive => "positive",
            Self::Negation => "not",
            Self::Aggregate => "aggregate",
            Self::Query => "query",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub head: RelationId,
    pub body: RelationId,
    pub kind: DependencyKind,
    pub rule: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StaticErrorKind {
    Grounding {
        literal: String,
        variables: Vec<Variable>,
    },
    Stratification {
        cycle: Vec<(String, DependencyKind, String)>,
    },
    ValueInvention {
        slot: usize,
        expression: String,
    },
    InvalidRelation(RelationId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticError {
    pub owner: String,
    pub source: Option<SourceSpan>,
    pub kind: StaticErrorKind,
}

impl fmt::Display for StaticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(source) = &self.source {
            write!(f, "{}:{}:{}: ", source.file, source.line, source.column)?;
        }
        write!(f, "{}: ", self.owner)?;
        match &self.kind {
            StaticErrorKind::Grounding { literal, variables } => {
                write!(f, "no grounding schedule for {literal}; ungrounded ")?;
                for (index, variable) in variables.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "?{}", variable.0)?;
                }
                Ok(())
            }
            StaticErrorKind::Stratification { cycle } => {
                f.write_str("stratification cycle: ")?;
                for (index, (head, kind, body)) in cycle.iter().enumerate() {
                    if index == 0 {
                        f.write_str(head)?;
                    }
                    write!(f, " -{kind}-> {body}")?;
                }
                Ok(())
            }
            StaticErrorKind::ValueInvention { slot, expression } => write!(
                f,
                "value invention in recursion at head slot {slot}: {expression}"
            ),
            StaticErrorKind::InvalidRelation(id) => write!(f, "unknown relation {}", id.0),
        }
    }
}
impl std::error::Error for StaticError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckedProgram {
    pub schedules: Vec<Vec<usize>>,
    pub strata: Vec<usize>,
    pub components: Vec<usize>,
    pub dependencies: Vec<Dependency>,
}

pub fn check(program: &Program) -> Result<CheckedProgram, Vec<StaticError>> {
    let mut errors = Vec::new();
    let mut schedules = Vec::new();
    let mut dependencies = Vec::new();
    for (index, rule) in program.rules.iter().enumerate() {
        if program.relation(rule.head.relation).is_none() {
            errors.push(StaticError {
                owner: rule.name.clone(),
                source: rule.source.clone(),
                kind: StaticErrorKind::InvalidRelation(rule.head.relation),
            });
        }
        let mut scope = body_scope(&rule.body);
        let head_variables: Variables = rule
            .head
            .arguments
            .iter()
            .flat_map(|argument| expr_vars(argument, &scope))
            .collect();
        scope.extend(head_variables.iter().cloned());
        match schedule(&rule.body, &head_variables, &Variables::new(), &scope) {
            Ok(order) => schedules.push(order),
            Err((literal, variables)) => {
                errors.push(StaticError {
                    owner: rule.name.clone(),
                    source: rule.source.clone(),
                    kind: StaticErrorKind::Grounding {
                        literal,
                        variables: variables.into_iter().collect(),
                    },
                });
                schedules.push(Vec::new());
            }
        }
        let grounded = body_scope(&rule.body);
        for argument in &rule.head.arguments {
            if head_variables.is_subset(&grounded) && !expression_ready(argument, &grounded, &scope)
            {
                errors.push(StaticError {
                    owner: rule.name.clone(),
                    source: rule.head.source.clone().or_else(|| rule.source.clone()),
                    kind: StaticErrorKind::Grounding {
                        literal: render_expr(argument),
                        variables: expr_vars(argument, &scope)
                            .difference(&grounded)
                            .cloned()
                            .collect(),
                    },
                });
            }
        }
        let mut collector = Collector {
            program,
            rule: index,
            head: rule.head.relation,
            dependencies: &mut dependencies,
            visiting: BTreeSet::new(),
        };
        collector.body(&rule.body, DependencyKind::Positive);
        for argument in &rule.head.arguments {
            collector.expression(argument);
        }
    }
    for query in &program.queries {
        let required = query_vars(&query.query);
        let scope = body_scope(&query.query.body)
            .union(&required)
            .cloned()
            .collect();
        if let Err((literal, variables)) =
            schedule(&query.query.body, &required, &Variables::new(), &scope)
        {
            errors.push(StaticError {
                owner: format!("query {}", query.name),
                source: None,
                kind: StaticErrorKind::Grounding {
                    literal,
                    variables: variables.into_iter().collect(),
                },
            });
        }
    }
    for dependency in &dependencies {
        for id in [dependency.head, dependency.body] {
            if program.relation(id).is_none() {
                errors.push(StaticError {
                    owner: program.rules[dependency.rule].name.clone(),
                    source: program.rules[dependency.rule].source.clone(),
                    kind: StaticErrorKind::InvalidRelation(id),
                });
            }
        }
    }
    if errors
        .iter()
        .any(|error| matches!(error.kind, StaticErrorKind::InvalidRelation(_)))
    {
        return Err(errors);
    }
    let components = components(program.relations.len(), &dependencies);
    for dependency in &dependencies {
        if dependency.kind != DependencyKind::Positive
            && components[dependency.head.0] == components[dependency.body.0]
        {
            let mut cycle = vec![(
                name(program, dependency.head),
                dependency.kind,
                name(program, dependency.body),
            )];
            for edge in path(dependency.body, dependency.head, &dependencies).unwrap_or_default() {
                cycle.push((
                    name(program, edge.head),
                    edge.kind,
                    name(program, edge.body),
                ));
            }
            let rule = &program.rules[dependency.rule];
            errors.push(StaticError {
                owner: rule.name.clone(),
                source: rule.source.clone(),
                kind: StaticErrorKind::Stratification { cycle },
            });
        }
    }
    let mut strata = vec![0; program.relations.len()];
    for _ in 0..program.relations.len() {
        let mut changed = false;
        for dependency in &dependencies {
            if components[dependency.head.0] != components[dependency.body.0] {
                let next = strata[dependency.body.0] + 1;
                if strata[dependency.head.0] < next {
                    let component = components[dependency.head.0];
                    for (relation, stratum) in strata.iter_mut().enumerate() {
                        if components[relation] == component {
                            *stratum = next;
                        }
                    }
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for (index, rule) in program.rules.iter().enumerate() {
        if !dependencies.iter().any(|edge| {
            edge.rule == index
                && edge.kind == DependencyKind::Positive
                && components[edge.head.0] == components[edge.body.0]
        }) {
            continue;
        }
        let mut relational = Variables::new();
        let mut lower = Variables::new();
        for literal in &rule.body {
            if let Literal::Positive(atom) = literal {
                let direct = atom.arguments.iter().filter_map(|argument| match argument {
                    Expr::Variable(variable) => Some(variable.clone()),
                    _ => None,
                });
                for variable in direct {
                    relational.insert(variable.clone());
                    if strata[atom.relation.0] < strata[rule.head.relation.0] {
                        lower.insert(variable);
                    }
                }
            }
        }
        let scope = body_scope(&rule.body)
            .union(&atom_vars(&rule.head))
            .cloned()
            .collect();
        loop {
            let before = lower.len();
            for literal in &rule.body {
                let output = match literal {
                    Literal::Bind { variable, value } => Some((variable, expr_vars(value, &scope))),
                    Literal::Member { variable, list } => Some((variable, expr_vars(list, &scope))),
                    Literal::Aggregate {
                        variable,
                        value,
                        body,
                        ..
                    } => Some((variable, nested_outside(body, &[value], &scope))),
                    Literal::ArgMin {
                        variable,
                        candidate,
                        key,
                        body,
                    } => Some((variable, nested_outside(body, &[candidate, key], &scope))),
                    _ => None,
                };
                if let Some((variable, input)) = output {
                    if input.is_subset(&lower)
                        && !literal_reads_component(
                            literal,
                            program,
                            rule.head.relation,
                            &components,
                        )
                    {
                        lower.insert(variable.clone());
                    }
                }
            }
            if lower.len() == before {
                break;
            }
        }
        for (slot, expression) in rule.head.arguments.iter().enumerate() {
            let safe = match expression {
                Expr::Value(_) => true,
                Expr::Variable(variable) if relational.contains(variable) => true,
                _ => {
                    expr_vars(expression, &scope).is_subset(&lower)
                        && !expression_reads_component(
                            expression,
                            program,
                            rule.head.relation,
                            &components,
                        )
                }
            };
            if !safe {
                errors.push(StaticError {
                    owner: rule.name.clone(),
                    source: rule.head.source.clone().or_else(|| rule.source.clone()),
                    kind: StaticErrorKind::ValueInvention {
                        slot,
                        expression: render_expr(expression),
                    },
                });
            }
        }
    }
    if errors.is_empty() {
        Ok(CheckedProgram {
            schedules,
            strata,
            components,
            dependencies,
        })
    } else {
        Err(errors)
    }
}

fn name(program: &Program, relation: RelationId) -> String {
    program.relations[relation.0].name.clone()
}

struct Collector<'a> {
    program: &'a Program,
    rule: usize,
    head: RelationId,
    dependencies: &'a mut Vec<Dependency>,
    visiting: BTreeSet<String>,
}
impl Collector<'_> {
    fn body(&mut self, body: &[Literal], context: DependencyKind) {
        for literal in body {
            match literal {
                Literal::Positive(atom) | Literal::Negative(atom) => {
                    let kind = if context != DependencyKind::Positive {
                        context
                    } else if matches!(literal, Literal::Negative(_)) {
                        DependencyKind::Negation
                    } else {
                        DependencyKind::Positive
                    };
                    self.dependencies.push(Dependency {
                        head: self.head,
                        body: atom.relation,
                        kind,
                        rule: self.rule,
                    });
                    for argument in &atom.arguments {
                        self.expression(argument);
                    }
                }
                Literal::Compare { left, right, .. } => {
                    self.expression(left);
                    self.expression(right);
                }
                Literal::Boolean(value)
                | Literal::Bind { value, .. }
                | Literal::Member { list: value, .. } => self.expression(value),
                Literal::Aggregate { value, body, .. } => {
                    self.body(body, DependencyKind::Aggregate);
                    self.expression(value);
                }
                Literal::ArgMin {
                    candidate,
                    key,
                    body,
                    ..
                } => {
                    self.body(body, DependencyKind::Aggregate);
                    self.expression(candidate);
                    self.expression(key);
                }
            }
        }
    }
    fn expression(&mut self, expression: &Expr) {
        match expression {
            Expr::Query(query) => {
                self.body(&query.body, DependencyKind::Query);
                if let Some(sort) = &query.sorted_by {
                    self.expression(sort);
                }
            }
            Expr::Call {
                function,
                arguments,
            } => {
                for argument in arguments {
                    self.expression(argument);
                }
                if self.visiting.insert(function.clone()) {
                    if let Some(definition) = self
                        .program
                        .definitions
                        .iter()
                        .find(|definition| definition.name == *function)
                    {
                        self.expression(&definition.body);
                    }
                    if let Some(query) = self
                        .program
                        .queries
                        .iter()
                        .find(|query| query.name == *function)
                    {
                        self.body(&query.query.body, DependencyKind::Query);
                        if let Some(sort) = &query.query.sorted_by {
                            self.expression(sort);
                        }
                    }
                    self.visiting.remove(function);
                }
            }
            _ => {
                for child in children(expression) {
                    self.expression(child);
                }
            }
        }
    }
}

fn expression_reads_component(
    expression: &Expr,
    program: &Program,
    head: RelationId,
    components: &[usize],
) -> bool {
    let mut dependencies = Vec::new();
    Collector {
        program,
        rule: 0,
        head,
        dependencies: &mut dependencies,
        visiting: BTreeSet::new(),
    }
    .expression(expression);
    dependencies
        .iter()
        .any(|edge| components[edge.body.0] == components[head.0])
}
fn literal_reads_component(
    literal: &Literal,
    program: &Program,
    head: RelationId,
    components: &[usize],
) -> bool {
    let mut dependencies = Vec::new();
    Collector {
        program,
        rule: 0,
        head,
        dependencies: &mut dependencies,
        visiting: BTreeSet::new(),
    }
    .body(std::slice::from_ref(literal), DependencyKind::Positive);
    dependencies
        .iter()
        .any(|edge| components[edge.body.0] == components[head.0])
}

fn children(expression: &Expr) -> Vec<&Expr> {
    match expression {
        Expr::List(values)
        | Expr::Call {
            arguments: values, ..
        } => values.iter().collect(),
        Expr::Record(values) | Expr::Attrs(values) => values.values().collect(),
        Expr::Field { value, .. }
        | Expr::Unary { value, .. }
        | Expr::Lambda { body: value, .. } => vec![value],
        Expr::Binary { left, right, .. } => vec![left, right],
        Expr::If {
            condition,
            then_value,
            else_value,
        } => vec![condition, then_value, else_value],
        Expr::Apply {
            function,
            arguments,
        } => std::iter::once(function.as_ref())
            .chain(arguments.iter())
            .collect(),
        Expr::Value(_) | Expr::Variable(_) | Expr::Query(_) => Vec::new(),
    }
}
fn atom_vars(atom: &Atom) -> Variables {
    atom.arguments
        .iter()
        .flat_map(|argument| expr_vars(argument, &Variables::new()))
        .collect()
}
fn query_vars(query: &Query) -> Variables {
    let mut variables: Variables = match &query.kind {
        QueryKind::All(variables) => variables.iter().cloned().collect(),
        QueryKind::The(variable) => BTreeSet::from([variable.clone()]),
    };
    if let Some(sort) = &query.sorted_by {
        variables.extend(expr_vars(sort, &body_scope(&query.body)));
    }
    variables
}
fn expr_vars(expression: &Expr, scope: &Variables) -> Variables {
    match expression {
        Expr::Variable(variable) => BTreeSet::from([variable.clone()]),
        Expr::Query(query) => {
            let extras: Vec<&Expr> = query.sorted_by.iter().map(|sort| sort.as_ref()).collect();
            let mut variables = nested_outside(&query.body, &extras, scope);
            variables.extend(query_vars(query).intersection(scope).cloned());
            variables
        }
        Expr::Lambda { parameters, body } => {
            let mut variables = expr_vars(body, scope);
            for parameter in parameters {
                variables.remove(&Variable::new(&parameter.name));
            }
            variables
        }
        _ => children(expression)
            .into_iter()
            .flat_map(|child| expr_vars(child, scope))
            .collect(),
    }
}
fn body_scope(body: &[Literal]) -> Variables {
    body.iter()
        .flat_map(|literal| match literal {
            Literal::Positive(atom) | Literal::Negative(atom) => atom_vars(atom),
            Literal::Compare { left, right, .. } => expr_vars(left, &Variables::new())
                .union(&expr_vars(right, &Variables::new()))
                .cloned()
                .collect(),
            Literal::Boolean(value) => expr_vars(value, &Variables::new()),
            Literal::Bind { variable, value }
            | Literal::Member {
                variable,
                list: value,
            } => {
                let mut variables = expr_vars(value, &Variables::new());
                variables.insert(variable.clone());
                variables
            }
            Literal::Aggregate { variable, .. } | Literal::ArgMin { variable, .. } => {
                BTreeSet::from([variable.clone()])
            }
        })
        .collect()
}
fn nested_outside(body: &[Literal], extra: &[&Expr], scope: &Variables) -> Variables {
    let mut variables = body_scope(body);
    for literal in body {
        match literal {
            Literal::Positive(atom) | Literal::Negative(atom) => {
                for argument in &atom.arguments {
                    variables.extend(expr_vars(argument, scope));
                }
            }
            Literal::Compare { left, right, .. } => {
                variables.extend(expr_vars(left, scope));
                variables.extend(expr_vars(right, scope));
            }
            Literal::Boolean(value)
            | Literal::Bind { value, .. }
            | Literal::Member { list: value, .. } => variables.extend(expr_vars(value, scope)),
            Literal::Aggregate { value, body, .. } => {
                variables.extend(nested_outside(body, &[value], scope))
            }
            Literal::ArgMin {
                candidate,
                key,
                body,
                ..
            } => variables.extend(nested_outside(body, &[candidate, key], scope)),
        }
    }
    for expression in extra {
        variables.extend(expr_vars(expression, scope));
    }
    variables.intersection(scope).cloned().collect()
}
fn expression_ready(expression: &Expr, grounded: &Variables, scope: &Variables) -> bool {
    if !expr_vars(expression, scope).is_subset(grounded) {
        return false;
    }
    match expression {
        Expr::Query(query) => {
            let mut nested_scope = body_scope(&query.body);
            nested_scope.extend(query_vars(query));
            if schedule(&query.body, &query_vars(query), grounded, &nested_scope).is_err() {
                return false;
            }
            let mut inside = grounded.clone();
            inside.extend(body_scope(&query.body));
            query
                .sorted_by
                .as_ref()
                .is_none_or(|sort| expression_ready(sort, &inside, &nested_scope))
        }
        Expr::Lambda { parameters, body } => {
            let mut inside = grounded.clone();
            inside.extend(
                parameters
                    .iter()
                    .map(|parameter| Variable::new(&parameter.name)),
            );
            expression_ready(body, &inside, scope)
        }
        _ => children(expression)
            .into_iter()
            .all(|child| expression_ready(child, grounded, scope)),
    }
}
fn schedule(
    body: &[Literal],
    required: &Variables,
    initial: &Variables,
    scope: &Variables,
) -> Result<Vec<usize>, (String, Variables)> {
    let mut grounded = initial.clone();
    let mut order = Vec::new();
    let mut remaining: BTreeSet<usize> = (0..body.len()).collect();
    while !remaining.is_empty() {
        let next = remaining.iter().copied().find(|index| match &body[*index] {
            Literal::Positive(atom) => atom.arguments.iter().all(|argument| {
                matches!(argument, Expr::Variable(_))
                    || expression_ready(argument, &grounded, scope)
            }),
            Literal::Negative(atom) => atom
                .arguments
                .iter()
                .all(|argument| expression_ready(argument, &grounded, scope)),
            Literal::Compare { left, right, .. } => {
                expression_ready(left, &grounded, scope)
                    && expression_ready(right, &grounded, scope)
            }
            Literal::Boolean(value)
            | Literal::Bind { value, .. }
            | Literal::Member { list: value, .. } => expression_ready(value, &grounded, scope),
            Literal::Aggregate { value, body, .. } => {
                nested_ready(body, &[value], &grounded, scope)
            }
            Literal::ArgMin {
                candidate,
                key,
                body,
                ..
            } => nested_ready(body, &[candidate, key], &grounded, scope),
        });
        let Some(index) = next else {
            let index = *remaining.first().unwrap();
            if let Some(failure) = nested_failure(&body[index], &grounded, scope) {
                return Err(failure);
            }
            let variables = body_scope(std::slice::from_ref(&body[index]))
                .difference(&grounded)
                .cloned()
                .collect();
            return Err((render_literal(&body[index]), variables));
        };
        match &body[index] {
            Literal::Positive(atom) => {
                grounded.extend(atom.arguments.iter().filter_map(|argument| match argument {
                    Expr::Variable(variable) => Some(variable.clone()),
                    _ => None,
                }))
            }
            Literal::Bind { variable, .. }
            | Literal::Member { variable, .. }
            | Literal::Aggregate { variable, .. }
            | Literal::ArgMin { variable, .. } => {
                grounded.insert(variable.clone());
            }
            _ => {}
        }
        order.push(index);
        remaining.remove(&index);
    }
    let missing: Variables = required.difference(&grounded).cloned().collect();
    if missing.is_empty() {
        Ok(order)
    } else {
        Err(("head or query result".into(), missing))
    }
}
fn nested_failure(
    literal: &Literal,
    grounded: &Variables,
    scope: &Variables,
) -> Option<(String, Variables)> {
    fn expression_failure(
        expression: &Expr,
        grounded: &Variables,
        scope: &Variables,
    ) -> Option<(String, Variables)> {
        if !expr_vars(expression, scope).is_subset(grounded) {
            return None;
        }
        if let Expr::Query(query) = expression {
            let required = query_vars(query);
            let mut inside = body_scope(&query.body);
            inside.extend(required.iter().cloned());
            if let Err(failure) = schedule(&query.body, &required, grounded, &inside) {
                return Some(failure);
            }
        }
        children(expression)
            .into_iter()
            .find_map(|child| expression_failure(child, grounded, scope))
    }
    match literal {
        Literal::Aggregate { value, body, .. } => failure_in_body(body, &[value], grounded, scope),
        Literal::ArgMin {
            candidate,
            key,
            body,
            ..
        } => failure_in_body(body, &[candidate, key], grounded, scope),
        Literal::Boolean(value)
        | Literal::Bind { value, .. }
        | Literal::Member { list: value, .. } => expression_failure(value, grounded, scope),
        Literal::Positive(atom) | Literal::Negative(atom) => atom
            .arguments
            .iter()
            .find_map(|value| expression_failure(value, grounded, scope)),
        Literal::Compare { left, right, .. } => expression_failure(left, grounded, scope)
            .or_else(|| expression_failure(right, grounded, scope)),
    }
}
fn failure_in_body(
    body: &[Literal],
    extra: &[&Expr],
    grounded: &Variables,
    scope: &Variables,
) -> Option<(String, Variables)> {
    if !nested_outside(body, extra, scope).is_subset(grounded) {
        return None;
    }
    let mut inside = body_scope(body);
    let required: Variables = extra
        .iter()
        .flat_map(|expression| expr_vars(expression, &inside))
        .collect();
    inside.extend(required.iter().cloned());
    schedule(body, &required, grounded, &inside).err()
}
fn nested_ready(
    body: &[Literal],
    extra: &[&Expr],
    grounded: &Variables,
    scope: &Variables,
) -> bool {
    if !nested_outside(body, extra, scope).is_subset(grounded) {
        return false;
    }
    let mut inside = body_scope(body);
    let required: Variables = extra
        .iter()
        .flat_map(|expression| expr_vars(expression, &inside))
        .collect();
    inside.extend(required.iter().cloned());
    if schedule(body, &required, grounded, &inside).is_err() {
        return false;
    }
    let mut complete = grounded.clone();
    complete.extend(body_scope(body));
    extra
        .iter()
        .all(|expression| expression_ready(expression, &complete, &inside))
}

fn components(count: usize, dependencies: &[Dependency]) -> Vec<usize> {
    fn visit(node: usize, graph: &[Vec<usize>], seen: &mut [bool], order: &mut Vec<usize>) {
        if seen[node] {
            return;
        }
        seen[node] = true;
        for next in &graph[node] {
            visit(*next, graph, seen, order);
        }
        order.push(node);
    }
    let mut graph = vec![Vec::new(); count];
    let mut reverse = vec![Vec::new(); count];
    for edge in dependencies {
        graph[edge.head.0].push(edge.body.0);
        reverse[edge.body.0].push(edge.head.0);
    }
    let mut seen = vec![false; count];
    let mut order = Vec::new();
    for node in 0..count {
        visit(node, &graph, &mut seen, &mut order);
    }
    seen.fill(false);
    let mut result = vec![0; count];
    let mut component = 0;
    for node in order.into_iter().rev() {
        if seen[node] {
            continue;
        }
        let mut members = Vec::new();
        visit(node, &reverse, &mut seen, &mut members);
        for member in members {
            result[member] = component;
        }
        component += 1;
    }
    result
}
fn path(from: RelationId, to: RelationId, dependencies: &[Dependency]) -> Option<Vec<&Dependency>> {
    fn visit<'a>(
        from: RelationId,
        to: RelationId,
        dependencies: &'a [Dependency],
        seen: &mut BTreeSet<RelationId>,
    ) -> Option<Vec<&'a Dependency>> {
        if from == to {
            return Some(Vec::new());
        }
        if !seen.insert(from) {
            return None;
        }
        for edge in dependencies.iter().filter(|edge| edge.head == from) {
            if let Some(mut rest) = visit(edge.body, to, dependencies, seen) {
                rest.insert(0, edge);
                return Some(rest);
            }
        }
        None
    }
    visit(from, to, dependencies, &mut BTreeSet::new())
}
fn render_expr(expression: &Expr) -> String {
    match expression {
        Expr::Variable(variable) => format!("?{}", variable.0),
        Expr::Value(Value::Text(text)) => format!("{text:?}"),
        Expr::Value(value) => format!("{value:?}"),
        Expr::List(values) => format!(
            "[{}]",
            values
                .iter()
                .map(render_expr)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Binary { op, left, right } => {
            format!("({} {op:?} {})", render_expr(left), render_expr(right))
        }
        Expr::Call {
            function,
            arguments,
        } => format!(
            "{function}({})",
            arguments
                .iter()
                .map(render_expr)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Query(_) => "query".into(),
        _ => format!("{expression:?}"),
    }
}
fn render_literal(literal: &Literal) -> String {
    match literal {
        Literal::Bind { variable, value } => format!("?{} = {}", variable.0, render_expr(value)),
        Literal::Member { variable, list } => format!("?{} in {}", variable.0, render_expr(list)),
        Literal::Positive(atom) => format!("positive relation {}", atom.relation.0),
        Literal::Negative(atom) => format!("not relation {}", atom.relation.0),
        Literal::Compare { op, left, right } => {
            format!("{} {op:?} {}", render_expr(left), render_expr(right))
        }
        Literal::Boolean(value) => render_expr(value),
        Literal::Aggregate { variable, op, .. } => format!("?{} = {op:?} where …", variable.0),
        Literal::ArgMin { variable, .. } => format!("?{} = first where …", variable.0),
    }
}
