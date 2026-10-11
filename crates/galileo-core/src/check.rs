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

impl DependencyKind {
    fn phrase(self) -> &'static str {
        match self {
            Self::Positive => "",
            Self::Negation => " through not",
            Self::Aggregate => " through an aggregate",
            Self::Query => " through a query",
        }
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
        slot: String,
        literal: String,
        source: String,
    },
    InvalidRelation(RelationId),
    SortKey {
        key: String,
        variables: Vec<Variable>,
    },
    Recursion {
        cycle: Vec<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticError {
    pub owner: String,
    pub source: Option<SourceSpan>,
    pub kind: StaticErrorKind,
    pub offers: Vec<String>,
}

impl fmt::Display for StaticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(source) = &self.source {
            write!(f, "{}:{}:{}: ", source.file, source.line, source.column)?;
        }
        write!(f, "{}: ", self.owner)?;
        match &self.kind {
            StaticErrorKind::Grounding { literal, variables } => {
                let names: Vec<String> = variables
                    .iter()
                    .map(|v| format!("?{}", if v.is_anonymous() { "_" } else { &v.0 }))
                    .collect();
                write!(f, "nothing grounds {}, which {literal} needs", names.join(", "))
            }
            StaticErrorKind::Stratification { cycle } => {
                let steps: Vec<String> = cycle
                    .iter()
                    .map(|(head, kind, body)| format!("{head} reads {body}{}", kind.phrase()))
                    .collect();
                write!(f, "stratification cycle: {}", steps.join(", "))
            }
            StaticErrorKind::ValueInvention {
                slot,
                literal,
                source,
            } => write!(
                f,
                "value invention in recursion: head slot {slot} gets {literal}, a new value built from {source}"
            ),
            StaticErrorKind::InvalidRelation(id) => write!(f, "unknown relation id {}", id.0),
            StaticErrorKind::Recursion { cycle } => {
                let steps: Vec<String> =
                    cycle.iter().zip(cycle.iter().cycle().skip(1)).map(|(caller, callee)| format!("{caller} calls {callee}")).collect();
                write!(f, "recursive definition: {}; a definition may not call itself, directly or through other definitions", steps.join(", "))
            }
            StaticErrorKind::SortKey { key, variables } => {
                let names: Vec<String> = variables
                    .iter()
                    .map(|v| format!("?{}", if v.is_anonymous() { "_" } else { &v.0 }))
                    .collect();
                write!(
                    f,
                    "sort key {key} reads {}, which only the query's condition binds; a sort key reads only the query's variables and outside ones",
                    names.join(", ")
                )
            }
        }?;
        for offer in &self.offers {
            write!(f, "; add `{offer}`")?;
        }
        Ok(())
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
    let mut dependencies = Vec::new();
    for (index, rule) in program.rules.iter().enumerate() {
        let mut collector = Collector {
            program,
            rule: index,
            head: rule.head.relation,
            dependencies: &mut dependencies,
            visiting: BTreeSet::new(),
            calls: None,
        };
        collector.body(&rule.body, DependencyKind::Positive);
        for argument in &rule.head.arguments {
            collector.expression(argument);
        }
        let mut ids = vec![rule.head.relation];
        ids.extend(
            dependencies
                .iter()
                .filter(|edge| edge.rule == index)
                .map(|edge| edge.body),
        );
        if let Some(id) = ids.into_iter().find(|id| program.relation(*id).is_none()) {
            errors.push(StaticError {
                offers: Vec::new(),
                owner: rule.name.clone(),
                source: rule.source.clone(),
                kind: StaticErrorKind::InvalidRelation(id),
            });
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut schedules = Vec::new();
    for rule in &program.rules {
        let top = level_variables(&rule.body, rule.head.arguments.iter());
        let result =
            schedule(program, &rule.body, &rule.body_sources, &top, &Variables::new()).and_then(|(order, grounded)| {
                for argument in &rule.head.arguments {
                    expr_ready(program, argument, &grounded, &top).map_err(
                        |(literal, missing)| {
                            (
                                literal.or_else(|| Some(Blame::Literal(render_atom(program, &rule.head)))),
                                missing,
                            )
                        },
                    )?;
                }
                Ok(order)
            });
        match result {
            Ok(order) => schedules.push(order),
            Err((blame, variables)) => {
                let mut source = match &blame {
                    Some(Blame::At(source, _)) => Some(source.clone()),
                    _ => rule.head.source.clone().or_else(|| rule.source.clone()),
                };
                let offers = program.relation(rule.head.relation).into_iter()
                    .flat_map(|relation| relation.slots.iter().zip(&rule.head.arguments))
                    .filter_map(|(slot, argument)| match (&slot.ty, argument) {
                        (Type::Entity(ty), Expr::Variable(variable)) if variables.contains(variable) => {
                            Some(format!("and ?{} is a {}", variable.0, ty.rsplit('/').next().unwrap_or(ty)))
                        }
                        _ => None,
                    }).collect();
                let kind = stuck_kind(blame, variables, || render_atom(program, &rule.head), &mut source);
                errors.push(StaticError {
                    offers,
                    owner: rule.name.clone(),
                    source,
                    kind,
                });
                schedules.push(Vec::new());
            }
        }
    }
    for handler in &program.handlers {
        let grounded = bare_variables(&handler.event);
        for clause in &handler.clauses {
            let extras = handler.event.arguments.iter().chain(clause.actions.iter().flat_map(|action| {
                match action {
                    Action::Set(atom) | Action::Include(atom) | Action::Withdraw(atom) | Action::Emit(atom) => {
                        atom.arguments.iter().collect::<Vec<_>>()
                    }
                    Action::Create { fields, .. } => fields.iter().map(|(_, value)| value).collect(),
                    Action::Delete(value) => vec![value],
                }
            }));
            let mut top = level_variables(&clause.body, extras);
            for action in &clause.actions {
                if let Action::Create { variable, .. } = action {
                    top.insert(variable.clone());
                }
            }
            if let Err((blame, variables)) = schedule(program, &clause.body, &clause.body_sources, &top, &grounded) {
                let mut source = match &blame {
                    Some(Blame::At(source, _)) => Some(source.clone()),
                    _ => clause.source.clone(),
                };
                let kind = stuck_kind(blame, variables, || render_atom(program, &handler.event), &mut source);
                errors.push(StaticError {
                    offers: Vec::new(),
                    owner: format!("handler {}", name(program, handler.event.relation)),
                    source,
                    kind,
                });
            }
        }
    }
    for query in &program.queries {
        if let Err((blame, variables)) =
            query_ready(program, &query.query, &Variables::new(), &Variables::new())
        {
            let mut source = query.query.source.clone();
            let kind = stuck_kind(blame, variables, || render_query(program, &query.query), &mut source);
            errors.push(StaticError { offers: Vec::new(), owner: format!("query {}", query.name), source, kind });
        }
    }

    let owners = program
        .definitions
        .iter()
        .map(|d| (format!("definition {}", d.name), &d.parameters, vec![&d.body], &d.source))
        .chain(program.views.iter().map(|v| {
            (
                format!("view {}", v.name),
                &v.parameters,
                v.fields.iter().map(|(_, value)| value).collect(),
                &v.source,
            )
        }));
    for (owner, parameters, bodies, source) in owners {
        let grounded = lambda_parameters(parameters);
        for body in bodies {
            if let Err((blame, variables)) = expr_ready(program, body, &grounded, &grounded) {
                let mut source = source.clone();
                let kind = stuck_kind(blame, variables, || render_expr(program, body), &mut source);
                errors.push(StaticError { offers: Vec::new(), owner: owner.clone(), source, kind });
            }
        }
    }

    errors.extend(recursion(program));

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
                offers: Vec::new(),
                owner: rule.name.clone(),
                source: rule.source.clone(),
                kind: StaticErrorKind::Stratification { cycle },
            });
        }
    }
    let strata = strata(program.relations.len(), &components, &dependencies);

    for (index, rule) in program.rules.iter().enumerate() {
        let recursive = dependencies.iter().any(|edge| {
            edge.rule == index
                && edge.kind == DependencyKind::Positive
                && components[edge.head.0] == components[edge.body.0]
        });
        if recursive {
            errors.extend(value_invention(program, rule, &components));
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

fn recursion(program: &Program) -> Vec<StaticError> {
    let calls: Vec<Vec<usize>> = program
        .definitions
        .iter()
        .map(|definition| {
            let mut dependencies = Vec::new();
            let mut collector = Collector {
                program,
                rule: 0,
                head: RelationId(0),
                dependencies: &mut dependencies,
                visiting: BTreeSet::new(),
                calls: Some(BTreeSet::new()),
            };
            collector.expression(&definition.body);
            let called = collector.calls.unwrap_or_default();
            program.definitions.iter().enumerate().filter(|(_, d)| called.contains(&d.name)).map(|(i, _)| i).collect()
        })
        .collect();
    let mut errors = Vec::new();
    for (start, definition) in program.definitions.iter().enumerate() {
        let mut previous: Vec<Option<usize>> = vec![None; calls.len()];
        let mut queue = std::collections::VecDeque::from([start]);
        let mut seen = vec![false; calls.len()];
        let mut back = None;
        while let Some(at) = queue.pop_front() {
            if calls[at].contains(&start) {
                back = Some(at);
                break;
            }
            for &next in &calls[at] {
                if !seen[next] {
                    seen[next] = true;
                    previous[next] = Some(at);
                    queue.push_back(next);
                }
            }
        }
        let Some(mut at) = back else { continue };
        let mut cycle = vec![program.definitions[at].name.clone()];
        while at != start {
            at = previous[at].unwrap_or(start);
            cycle.push(program.definitions[at].name.clone());
        }
        cycle.reverse();
        errors.push(StaticError {
            offers: Vec::new(),
            owner: format!("definition {}", definition.name),
            source: definition.source.clone(),
            kind: StaticErrorKind::Recursion { cycle },
        });
    }
    errors
}

fn name(program: &Program, relation: RelationId) -> String {
    program.relations[relation.0].name.clone()
}

pub(crate) fn field_relations<'a>(program: &'a Program, field: &'a str) -> impl Iterator<Item = RelationId> + 'a {
    program.relations.iter().enumerate().filter_map(move |(index, relation)| {
        relation.name.rsplit_once('.').filter(|(_, name)| *name == field).map(|_| RelationId(index))
    })
}

pub(crate) fn strata(count: usize, components: &[usize], dependencies: &[Dependency]) -> Vec<usize> {
    let mut strata = vec![0; count];
    for _ in 0..count {
        let mut changed = false;
        for dependency in dependencies {
            let (head, body) = (dependency.head.0, dependency.body.0);
            if components[head] != components[body] && strata[head] <= strata[body] {
                let next = strata[body] + 1;
                for (relation, stratum) in strata.iter_mut().enumerate() {
                    if components[relation] == components[head] {
                        *stratum = next;
                    }
                }
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    strata
}

fn value_invention(program: &Program, rule: &Rule, components: &[usize]) -> Vec<StaticError> {
    let component = components[rule.head.relation.0];
    let in_component = |relation: RelationId| components[relation.0] == component;
    let reads_component = |collect: &dyn Fn(&mut Collector)| {
        let mut dependencies = Vec::new();
        collect(&mut Collector {
            program,
            rule: 0,
            head: rule.head.relation,
            dependencies: &mut dependencies,
            visiting: BTreeSet::new(),
            calls: None,
        });
        dependencies.iter().any(|edge| in_component(edge.body))
    };
    let top = level_variables(&rule.body, rule.head.arguments.iter());
    let mut relational = Variables::new();
    let mut lower = Variables::new();
    for literal in &rule.body {
        if let Literal::Positive(atom) = literal {
            for variable in bare_variables(atom) {
                relational.insert(variable.clone());
                if !in_component(atom.relation) {
                    lower.insert(variable);
                }
            }
        }
    }
    loop {
        let before = (relational.len(), lower.len());
        for literal in &rule.body {
            if let Literal::Bind {
                variable,
                value: Expr::Variable(alias),
            } = literal
            {
                if relational.contains(alias) {
                    relational.insert(variable.clone());
                }
            }
            if let Some(variable) = bound_variable(literal) {
                let inputs: Variables = literal_inputs(literal)
                    .intersection(&top)
                    .cloned()
                    .collect();
                if inputs.is_subset(&lower)
                    && !reads_component(&|c| {
                        c.body(std::slice::from_ref(literal), DependencyKind::Positive)
                    })
                {
                    lower.insert(variable.clone());
                }
            }
        }
        if (relational.len(), lower.len()) == before {
            break;
        }
    }
    let binding = |variable: &Variable| {
        rule.body
            .iter()
            .find(|literal| bound_variable(literal) == Some(variable))
    };
    fn trace(
        variable: &Variable,
        seen: &mut Variables,
        sentence: &dyn Fn(&Variable) -> Option<String>,
        inputs: &dyn Fn(&Variable) -> Variables,
    ) -> Option<String> {
        if !seen.insert(variable.clone()) {
            return None;
        }
        sentence(variable).or_else(|| {
            inputs(variable)
                .iter()
                .find_map(|input| trace(input, seen, sentence, inputs))
        })
    }
    let recursive_source = |variable: &Variable| {
        let sentence = |variable: &Variable| {
            rule.body.iter().find_map(|literal| match literal {
                Literal::Positive(atom)
                    if in_component(atom.relation) && bare_variables(atom).contains(variable) =>
                {
                    Some(format!(
                        "?{} of the recursive sentence {}",
                        variable.0,
                        render_atom(program, atom)
                    ))
                }
                _ => None,
            })
        };
        let inputs = |variable: &Variable| {
            binding(variable)
                .map(|literal| &(&literal_inputs(literal) & &top) - &lower)
                .unwrap_or_default()
        };
        match trace(variable, &mut Variables::new(), &sentence, &inputs) {
            Some(source) if sentence(variable).is_none() => {
                format!("{source} through ?{}", variable.0)
            }
            Some(source) => source,
            None => format!("?{}", variable.0),
        }
    };
    let source_of = |inputs: Variables| {
        inputs
            .difference(&lower)
            .next()
            .map(|variable| recursive_source(variable))
            .unwrap_or_else(|| "a recursive relation".to_string())
    };
    let mut errors = Vec::new();
    let relation = &program.relations[rule.head.relation.0];
    for (slot, expression) in rule.head.arguments.iter().enumerate() {
        let invented = match expression {
            Expr::Value(_) => None,
            Expr::Variable(variable)
                if relational.contains(variable) || lower.contains(variable) =>
            {
                None
            }
            Expr::Variable(variable) => {
                let mut literal = binding(variable);
                let mut seen = Variables::new();
                while let Some(Literal::Bind {
                    value: Expr::Variable(alias),
                    ..
                }) = literal
                {
                    if relational.contains(alias)
                        || lower.contains(alias)
                        || !seen.insert(alias.clone())
                    {
                        break;
                    }
                    literal = binding(alias);
                }
                literal.map(|literal| {
                    (
                        render_literal(program, literal),
                        source_of(
                            literal_inputs(literal)
                                .intersection(&top)
                                .cloned()
                                .collect(),
                        ),
                    )
                })
            }
            _ => {
                let inputs: Variables = mentions(expression).intersection(&top).cloned().collect();
                (!inputs.is_subset(&lower) || reads_component(&|c| c.expression(expression)))
                    .then(|| (render_expr(program, expression), source_of(inputs)))
            }
        };
        if let Some((literal, source)) = invented {
            let slot = relation
                .slots
                .get(slot)
                .map(|slot| slot.name.clone())
                .unwrap_or_else(|| slot.to_string());
            errors.push(StaticError {
                offers: Vec::new(),
                owner: rule.name.clone(),
                source: rule.head.source.clone().or_else(|| rule.source.clone()),
                kind: StaticErrorKind::ValueInvention {
                    slot,
                    literal,
                    source,
                },
            });
        }
    }
    errors
}

pub fn expression_reads(program: &Program, expression: &Expr) -> BTreeSet<RelationId> {
    let mut dependencies = Vec::new();
    Collector {
        program,
        rule: 0,
        head: RelationId(0),
        dependencies: &mut dependencies,
        visiting: BTreeSet::new(),
        calls: None,
    }.expression(expression);
    dependencies.into_iter().map(|dependency| dependency.body).collect()
}

struct Collector<'a> {
    program: &'a Program,
    rule: usize,
    head: RelationId,
    dependencies: &'a mut Vec<Dependency>,
    visiting: BTreeSet<String>,
    calls: Option<BTreeSet<String>>,
}
impl Collector<'_> {
    fn body(&mut self, body: &[Literal], context: DependencyKind) {
        for literal in body {
            match literal {
                Literal::Positive(atom) | Literal::Negative(atom) => {
                    let kind = match (context, literal) {
                        (DependencyKind::Positive, Literal::Negative(_)) => {
                            DependencyKind::Negation
                        }
                        _ => context,
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
                Literal::Aggregate { op, body, .. } => {
                    self.body(body, DependencyKind::Aggregate);
                    if let Some(value) = op.value() {
                        self.expression(value);
                    }
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
    fn query(&mut self, query: &Query) {
        self.body(&query.body, DependencyKind::Query);
        for expression in crate::expr::query_extras(query) { self.expression(expression); }
    }
    fn expression(&mut self, expression: &Expr) {
        match expression {
            Expr::Query(query) => self.query(query),
            Expr::Field { value, field } => {
                self.expression(value);
                for relation in field_relations(self.program, field) {
                    self.dependencies.push(Dependency {
                        head: self.head,
                        body: relation,
                        kind: DependencyKind::Positive,
                        rule: self.rule,
                    });
                }
            }
            Expr::Call {
                function,
                arguments,
            } => {
                for argument in arguments {
                    self.expression(argument);
                }
                if let Some(calls) = &mut self.calls {
                    calls.insert(function.clone());
                }
                if self.visiting.insert(function.clone()) {
                    let program = self.program;
                    if let Some(definition) =
                        program.definitions.iter().find(|d| d.name == *function).filter(|_| self.calls.is_none())
                    {
                        self.expression(&definition.body);
                    }
                    if let Some(query) = program.queries.iter().find(|q| q.name == *function) {
                        self.query(&query.query);
                    }
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

fn children(expression: &Expr) -> Vec<&Expr> {
    match expression {
        Expr::List(values)
        | Expr::Call {
            arguments: values, ..
        } => values.iter().collect(),
        Expr::Record(values) | Expr::Attrs(values) => values.values().collect(),
        Expr::Field { value, .. }
        | Expr::Unary { value, .. }
        | Expr::Lambda { body: value, .. } => {
            vec![value]
        }
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

fn lambda_parameters(parameters: &[Slot]) -> Variables {
    parameters.iter().map(|p| Variable::new(&p.name)).collect()
}

fn bare_variables(atom: &Atom) -> Variables {
    atom.arguments
        .iter()
        .filter_map(|argument| match argument {
            Expr::Variable(variable) => Some(variable.clone()),
            _ => None,
        })
        .collect()
}

fn bound_variable(literal: &Literal) -> Option<&Variable> {
    match literal {
        Literal::Bind { variable, .. }
        | Literal::Member { variable, .. }
        | Literal::Aggregate { variable, .. }
        | Literal::ArgMin { variable, .. } => Some(variable),
        _ => None,
    }
}

fn top_mentions(expression: &Expr) -> Variables {
    match expression {
        Expr::Variable(variable) => BTreeSet::from([variable.clone()]),
        Expr::Query(_) => Variables::new(),
        Expr::Lambda { parameters, body } => &top_mentions(body) - &lambda_parameters(parameters),
        _ => children(expression)
            .into_iter()
            .flat_map(top_mentions)
            .collect(),
    }
}

fn mentions(expression: &Expr) -> Variables {
    match expression {
        Expr::Variable(variable) => BTreeSet::from([variable.clone()]),
        Expr::Query(query) => query_mentions(query),
        Expr::Lambda { parameters, body } => &mentions(body) - &lambda_parameters(parameters),
        _ => children(expression)
            .into_iter()
            .flat_map(mentions)
            .collect(),
    }
}

fn query_mentions(query: &Query) -> Variables {
    let mut variables = declared(query);
    variables.extend(query.body.iter().flat_map(literal_mentions));
    variables.extend(query.head.iter().chain(query.sorted_by.iter()).flat_map(|expression| mentions(expression)));
    variables
}

fn literal_mentions(literal: &Literal) -> Variables {
    let mut variables = literal_inputs(literal);
    variables.extend(bound_variable(literal).cloned());
    variables
}

fn literal_inputs(literal: &Literal) -> Variables {
    match literal {
        Literal::Positive(atom) | Literal::Negative(atom) => {
            atom.arguments.iter().flat_map(mentions).collect()
        }
        Literal::Compare { left, right, .. } => &mentions(left) | &mentions(right),
        Literal::Boolean(value)
        | Literal::Bind { value, .. }
        | Literal::Member { list: value, .. } => mentions(value),
        Literal::Aggregate { op, body, .. } => {
            let mut variables: Variables = op.value().into_iter().flat_map(mentions).collect();
            variables.extend(body.iter().flat_map(literal_mentions));
            variables
        }
        Literal::ArgMin {
            candidate,
            key,
            body,
            ..
        } => {
            let mut variables = &mentions(candidate) | &mentions(key);
            variables.extend(body.iter().flat_map(literal_mentions));
            variables
        }
    }
}

fn level_variables<'a>(body: &[Literal], extras: impl IntoIterator<Item = &'a Expr>) -> Variables {
    let mut variables: Variables = extras.into_iter().flat_map(top_mentions).collect();
    for literal in body {
        match literal {
            Literal::Positive(atom) | Literal::Negative(atom) => {
                variables.extend(atom.arguments.iter().flat_map(top_mentions))
            }
            Literal::Compare { left, right, .. } => {
                variables.extend(top_mentions(left));
                variables.extend(top_mentions(right));
            }
            Literal::Boolean(value) => variables.extend(top_mentions(value)),
            Literal::Bind { value, .. } | Literal::Member { list: value, .. } => {
                variables.extend(top_mentions(value))
            }
            Literal::Aggregate { .. } | Literal::ArgMin { .. } => {}
        }
        variables.extend(bound_variable(literal).cloned());
    }
    variables
}

fn declared(query: &Query) -> Variables {
    match &query.kind {
        QueryKind::All(variables) => variables.iter().cloned().collect(),
        QueryKind::The(variable) => BTreeSet::from([variable.clone()]),
    }
}

enum Blame {
    At(SourceSpan, Box<Blame>),
    Literal(String),
    SortKey(String, Option<SourceSpan>),
}

type Stuck = (Option<Blame>, Variables);

fn stuck_kind(
    blame: Option<Blame>,
    variables: Variables,
    owner: impl FnOnce() -> String,
    source: &mut Option<SourceSpan>,
) -> StaticErrorKind {
    let variables: Vec<Variable> = variables.into_iter().collect();
    match blame {
        Some(Blame::At(at, inner)) => {
            *source = Some(at);
            stuck_kind(Some(*inner), variables.into_iter().collect(), owner, source)
        },
        Some(Blame::SortKey(key, at)) => {
            *source = at.or_else(|| source.take());
            StaticErrorKind::SortKey { key, variables }
        },
        Some(Blame::Literal(literal)) => StaticErrorKind::Grounding { literal, variables },
        None => StaticErrorKind::Grounding {
            literal: owner(),
            variables,
        },
    }
}

pub(crate) fn nested_order(
    program: &Program,
    body: &[Literal],
    extras: &[&Expr],
    bound: &Variables,
) -> Vec<usize> {
    let inside = bound | &level_variables(body, extras.iter().copied());
    schedule(program, body, &[], &inside, bound)
        .map(|(order, _)| order)
        .unwrap_or_else(|_| (0..body.len()).collect())
}


fn schedule(
    program: &Program,
    body: &[Literal],
    sources: &[SourceSpan],
    outer: &Variables,
    initial: &Variables,
) -> Result<(Vec<usize>, Variables), Stuck> {
    let mut grounded = initial.clone();
    let mut order = Vec::new();
    let mut remaining: Vec<usize> = (0..body.len()).collect();
    while !remaining.is_empty() {
        let Some(position) = remaining
            .iter()
            .position(|index| literal_ready(program, &body[*index], &grounded, outer).is_ok())
        else {
            let literal = &body[remaining[0]];
            let (inner, missing) = literal_ready(program, literal, &grounded, outer).unwrap_err();
            let blame = inner.unwrap_or_else(|| Blame::Literal(render_literal(program, literal)));
            let blame = match sources.get(remaining[0]) {
                Some(source) => Blame::At(source.clone(), Box::new(blame)),
                None => blame,
            };
            return Err((Some(blame), missing));
        };
        let index = remaining.remove(position);
        match &body[index] {
            Literal::Positive(atom) => grounded.extend(bare_variables(atom)),
            literal => grounded.extend(bound_variable(literal).cloned()),
        }
        order.push(index);
    }
    Ok((order, grounded))
}

fn literal_ready(
    program: &Program,
    literal: &Literal,
    grounded: &Variables,
    outer: &Variables,
) -> Result<(), Stuck> {
    match literal {
        Literal::Positive(atom) => atom
            .arguments
            .iter()
            .filter(|argument| !matches!(argument, Expr::Variable(_)))
            .try_for_each(|argument| expr_ready(program, argument, grounded, outer)),
        Literal::Negative(atom) => atom
            .arguments
            .iter()
            .filter(|argument| !matches!(argument, Expr::Variable(v) if v.is_anonymous()))
            .try_for_each(|argument| expr_ready(program, argument, grounded, outer)),
        Literal::Compare { left, right, .. } => {
            expr_ready(program, left, grounded, outer)?;
            expr_ready(program, right, grounded, outer)
        }
        Literal::Boolean(value)
        | Literal::Bind { value, .. }
        | Literal::Member { list: value, .. } => expr_ready(program, value, grounded, outer),
        Literal::Aggregate { op, body, .. } => {
            let extras: Vec<&Expr> = op.value().into_iter().collect();
            nested_ready(program, body, &Variables::new(), &extras, grounded, outer)
        }
        Literal::ArgMin {
            candidate,
            key,
            body,
            ..
        } => nested_ready(
            program,
            body,
            &Variables::new(),
            &[candidate, key],
            grounded,
            outer,
        ),
    }
}

fn expr_ready(
    program: &Program,
    expression: &Expr,
    grounded: &Variables,
    outer: &Variables,
) -> Result<(), Stuck> {
    let missing = &top_mentions(expression) - grounded;
    if !missing.is_empty() {
        return Err((None, missing));
    }
    nested_queries_ready(program, expression, grounded, outer)
}

fn nested_queries_ready(
    program: &Program,
    expression: &Expr,
    grounded: &Variables,
    outer: &Variables,
) -> Result<(), Stuck> {
    match expression {
        Expr::Query(query) => query_ready(program, query, grounded, outer),
        Expr::Lambda { parameters, body } => {
            let parameters = lambda_parameters(parameters);
            nested_queries_ready(
                program,
                body,
                &(grounded | &parameters),
                &(outer | &parameters),
            )
        }
        _ => children(expression)
            .into_iter()
            .try_for_each(|child| nested_queries_ready(program, child, grounded, outer)),
    }
}

fn query_ready(
    program: &Program,
    query: &Query,
    grounded: &Variables,
    outer: &Variables,
) -> Result<(), Stuck> {
    let sort = crate::expr::query_extras(query);
    let required = declared(query);
    nested_ready(program, &query.body, &required, &sort, grounded, outer)?;
    match &query.sorted_by {
        Some(key) => {
            let condition_only = &(&top_mentions(key) - &required) - outer;
            if condition_only.is_empty() {
                Ok(())
            } else {
                Err((
                    Some(Blame::SortKey(render_expr(program, key), query.source.clone())),
                    condition_only,
                ))
            }
        }
        None => Ok(()),
    }
}

fn nested_ready(
    program: &Program,
    body: &[Literal],
    required: &Variables,
    extras: &[&Expr],
    grounded: &Variables,
    outer: &Variables,
) -> Result<(), Stuck> {
    let mut used: Variables = body.iter().flat_map(literal_mentions).collect();
    used.extend(extras.iter().flat_map(|e| mentions(e)));
    used.extend(required.iter().cloned());
    let missing = &(&used & outer) - grounded;
    if !missing.is_empty() {
        return Err((None, missing));
    }
    let mut inside = outer | &level_variables(body, extras.iter().copied());
    inside.extend(required.iter().cloned());
    let (_, local) = schedule(program, body, &[], &inside, grounded)?;
    let missing = required - &local;
    if !missing.is_empty() {
        return Err((None, missing));
    }
    extras
        .iter()
        .try_for_each(|expression| expr_ready(program, expression, &local, &inside))
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

fn join<T>(
    items: impl IntoIterator<Item = T>,
    render: impl Fn(T) -> String,
    separator: &str,
) -> String {
    items
        .into_iter()
        .map(render)
        .collect::<Vec<_>>()
        .join(separator)
}

pub(crate) fn render_value(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Int(value) => value.to_string(),
        Value::Text(text) => format!("{text:?}"),
        Value::Path(path) => path.clone(),
        Value::OptionPath(path) => path.join("."),
        Value::Entity(entity) => entity.name.clone(),
        Value::List(values) => format!("[{}]", join(values, render_value, ", ")),
        Value::Record(fields) | Value::Attrs(fields) => format!(
            "{{{}}}",
            join(fields, |(k, v)| format!("{k}: {}", render_value(v)), ", ")
        ),
        Value::Nix { normalized } => normalized.clone(),
    }
}

fn binary_symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::Concat => "++",
        BinaryOp::Merge => "//",
        BinaryOp::And => "and",
        BinaryOp::Or => "or",
        BinaryOp::Compare(op) => compare_symbol(op),
    }
}

pub(crate) fn compare_symbol(op: CompareOp) -> &'static str {
    match op {
        CompareOp::Equal => "==",
        CompareOp::NotEqual => "!=",
        CompareOp::Less => "<",
        CompareOp::LessEqual => "<=",
        CompareOp::Greater => ">",
        CompareOp::GreaterEqual => ">=",
        CompareOp::In => "in",
    }
}

pub(crate) fn render_expr(program: &Program, expression: &Expr) -> String {
    let operand = |expression: &Expr| match expression {
        Expr::Binary { .. } => format!("({})", render_expr(program, expression)),
        _ => render_expr(program, expression),
    };
    let list = |values: &[Expr]| join(values, |v| render_expr(program, v), ", ");
    match expression {
        Expr::Value(value) => render_value(value),
        Expr::Variable(variable) if variable.is_anonymous() => "?_".into(),
        Expr::Variable(variable) => format!("?{}", variable.0),
        Expr::List(values) => format!("[{}]", list(values)),
        Expr::Record(fields) | Expr::Attrs(fields) => format!(
            "{{{}}}",
            join(
                fields,
                |(k, v)| format!("{k}: {}", render_expr(program, v)),
                ", "
            )
        ),
        Expr::Field { value, field } => format!("{}.{field}", operand(value)),
        Expr::Unary {
            op: UnaryOp::Not,
            value,
        } => format!("not {}", operand(value)),
        Expr::Unary {
            op: UnaryOp::Negate,
            value,
        } => format!("-{}", operand(value)),
        Expr::Binary { op, left, right } => {
            format!(
                "{} {} {}",
                operand(left),
                binary_symbol(*op),
                operand(right)
            )
        }
        Expr::Call {
            function,
            arguments,
        } => format!("{function}({})", list(arguments)),
        Expr::If {
            condition,
            then_value,
            else_value,
        } => format!(
            "if {} then {} else {}",
            render_expr(program, condition),
            render_expr(program, then_value),
            render_expr(program, else_value)
        ),
        Expr::Lambda { parameters, body } => format!(
            "({}) => {}",
            join(parameters, |p| p.name.clone(), ", "),
            render_expr(program, body)
        ),
        Expr::Apply {
            function,
            arguments,
        } => format!("{}({})", operand(function), list(arguments)),
        Expr::Query(query) => render_query(program, query),
    }
}

fn render_query(program: &Program, query: &Query) -> String {
    let (word, variables) = match &query.kind {
        QueryKind::All(variables) => ("all", variables.as_slice()),
        QueryKind::The(variable) => ("the", std::slice::from_ref(variable)),
    };
    let mut text = format!(
        "{word} {} where {}",
        query.head.as_ref().map_or_else(|| join(variables, |v| format!("?{}", v.0), ", "), |head| render_expr(program, head)),
        render_body(program, &query.body)
    );
    if let Some(sort) = &query.sorted_by {
        text.push_str(&format!(", sorted by {}", render_expr(program, sort)));
    }
    text
}

fn render_body(program: &Program, body: &[Literal]) -> String {
    join(body, |literal| render_literal(program, literal), ", ")
}

pub(crate) fn render_atom(program: &Program, atom: &Atom) -> String {
    let arguments: Vec<String> = atom
        .arguments
        .iter()
        .map(|a| render_expr(program, a))
        .collect();
    let Some(relation) = program.relation(atom.relation) else {
        return format!(
            "unknown relation id {}({})",
            atom.relation.0,
            arguments.join(", ")
        );
    };
    if relation.template.is_empty() {
        return format!("{}({})", relation.name, arguments.join(", "));
    }
    let mut text = relation.template.clone();
    for (slot, argument) in relation.slots.iter().zip(&arguments) {
        text = text.replace(&format!("{{{}}}", slot.name), argument);
    }
    text
}

pub(crate) fn render_literal(program: &Program, literal: &Literal) -> String {
    match literal {
        Literal::Positive(atom) => render_atom(program, atom),
        Literal::Negative(atom) => format!("not {}", render_atom(program, atom)),
        Literal::Compare { op, left, right } => format!(
            "{} {} {}",
            render_expr(program, left),
            compare_symbol(*op),
            render_expr(program, right)
        ),
        Literal::Boolean(value) => render_expr(program, value),
        Literal::Bind { variable, value } => {
            format!("?{} = {}", variable.0, render_expr(program, value))
        }
        Literal::Member { variable, list } => {
            format!("?{} in {}", variable.0, render_expr(program, list))
        }
        Literal::Aggregate { variable, op, body } => {
            let name = match op {
                AggregateOp::Count => "count",
                AggregateOp::Sum(_) => "sum",
                AggregateOp::Min(_) => "min",
                AggregateOp::Max(_) => "max",
            };
            let value = op
                .value()
                .map(|value| format!(" {}", render_expr(program, value)))
                .unwrap_or_default();
            format!(
                "?{} = {name}{value} where {}",
                variable.0,
                render_body(program, body)
            )
        }
        Literal::ArgMin {
            direction,
            variable,
            candidate,
            key,
            body,
        } => format!(
            "?{} = {} {} by {} where {}",
            variable.0,
            match direction {
                ArgMinDirection::Min => "min",
                ArgMinDirection::Max => "max",
            },
            render_expr(program, candidate),
            render_expr(program, key),
            render_body(program, body)
        ),
    }
}
