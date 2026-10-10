use crate::check::{render_atom, render_expr, render_literal};
use crate::eval::{matches, value};
use crate::expr::Binding;
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

pub type Tuple = (RelationId, Vec<Value>);

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct State {
    pub facts: BTreeSet<Tuple>,
    pub live: BTreeSet<Entity>,
    pub next: u64,
}

impl State {
    pub fn initial(program: &Program) -> Self {
        let state_types: BTreeSet<&str> = program
            .entity_types
            .iter()
            .filter(|t| t.state)
            .map(|t| t.name.as_str())
            .collect();
        Self {
            facts: program
                .initially
                .iter()
                .map(|f| (f.relation, f.values.clone()))
                .collect(),
            live: program
                .entities
                .iter()
                .filter(|e| state_types.contains(e.entity_type.as_str()))
                .cloned()
                .collect(),
            next: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transition {
    pub state: State,
    pub effects: Vec<Tuple>,
    pub substitutions: usize,
}

pub fn step(
    program: &Program,
    state: &State,
    event: RelationId,
    arguments: &[Value],
    budget: Budget,
) -> Result<Transition, EvalError> {
    let facts: Vec<Fact> = state
        .facts
        .iter()
        .map(|(relation, values)| Fact::new(*relation, values.clone()))
        .collect();
    let model = evaluate(program, &facts, budget)?;

    let mut matched: BTreeMap<(String, Binding), &HandlerClause> = BTreeMap::new();
    for handler in program.handlers.iter().filter(|h| h.event.relation == event) {
        let Some(start) = bind_event(program, &model, &handler.event, arguments, budget)? else {
            continue;
        };
        for clause in &handler.clauses {
            let text = clause_text(program, clause);
            for binding in matches(program, &model, &clause.body, &start, budget)? {
                matched.insert((text.clone(), binding), clause);
            }
        }
    }

    let mut requests = Vec::new();
    for ((text, binding), clause) in &matched {
        for action in &clause.actions {
            if let Action::Create { variable, entity_type, fields } = action {
                let declared = program.entity_types.iter().find(|t| &t.name == entity_type);
                let mut values = BTreeMap::new();
                for (name, expression) in fields {
                    values.insert(name.clone(), value(program, &model, expression, binding, budget)?);
                }
                let ordered: Vec<Value> = match declared {
                    Some(t) => t.fields.iter().map(|f| values.get(&f.name).cloned().unwrap_or(Value::Null)).collect(),
                    None => values.values().cloned().collect(),
                };
                requests.push(((entity_type.clone(), ordered, text.clone(), binding.clone(), variable.clone()), values));
            }
        }
    }
    requests.sort_by(|a, b| a.0.cmp(&b.0));

    let mut created: BTreeMap<Entity, BTreeMap<String, Value>> = BTreeMap::new();
    let mut extended: BTreeMap<(String, Binding), Binding> = BTreeMap::new();
    for (offset, ((entity_type, _, text, binding, variable), values)) in requests.into_iter().enumerate() {
        let id = state.next + offset as u64;
        let entity = Entity::created(entity_type.clone(), format!("{entity_type}#{id}"), id);
        extended
            .entry((text, binding.clone()))
            .or_insert(binding)
            .insert(variable, Value::Entity(entity.clone()));
        created.insert(entity, values);
    }

    let mut withdrawn: BTreeSet<Tuple> = BTreeSet::new();
    let mut included: BTreeSet<Tuple> = BTreeSet::new();
    let mut deleted: BTreeSet<Entity> = BTreeSet::new();
    let mut effects: BTreeSet<(String, Vec<Value>, RelationId)> = BTreeSet::new();
    for (key, clause) in &matched {
        let binding = extended.get(key).unwrap_or(&key.1);
        for action in &clause.actions {
            match action {
                Action::Set(atom) => {
                    let tuple = ground(program, &model, atom, binding, budget)?;
                    let keys: &[usize] = match program.relations[atom.relation.0].modes.as_slice() {
                        [Mode::ExactlyOne { key, .. }] => std::slice::from_ref(key),
                        [Mode::AtMostOne { keys, .. }] => keys,
                        _ => &[],
                    };
                    if !keys.is_empty() {
                        withdrawn.extend(
                            state
                                .facts
                                .iter()
                                .filter(|(relation, values)| {
                                    *relation == atom.relation && keys.iter().all(|&k| values[k] == tuple[k])
                                })
                                .cloned(),
                        );
                    }
                    included.insert((atom.relation, tuple));
                }
                Action::Include(atom) => {
                    included.insert((atom.relation, ground(program, &model, atom, binding, budget)?));
                }
                Action::Withdraw(atom) => {
                    withdrawn.insert((atom.relation, ground(program, &model, atom, binding, budget)?));
                }
                Action::Delete(expression) => match value(program, &model, expression, binding, budget)? {
                    Value::Entity(entity) => {
                        deleted.insert(entity);
                    }
                    _ => {
                        return Err(EvalError::Type {
                            at: "handler action".into(),
                            message: "a deletion needs an entity".into(),
                        })
                    }
                },
                Action::Emit(atom) => {
                    let tuple = ground(program, &model, atom, binding, budget)?;
                    effects.insert((program.relations[atom.relation.0].name.clone(), tuple, atom.relation));
                }
                Action::Create { .. } => {}
            }
        }
    }

    let mut facts: BTreeSet<Tuple> = state
        .facts
        .iter()
        .filter(|fact| !withdrawn.contains(*fact) && !fact.1.iter().any(|v| mentions(v, &deleted)))
        .cloned()
        .collect();
    for (entity, values) in &created {
        for (field, value) in values {
            if let Some(relation) = program.field_relation(&entity.entity_type, field) {
                if value != &Value::Null
                    || !matches!(program.relations[relation.0].modes.as_slice(), [Mode::AtMostOne { .. }])
                {
                    facts.insert((relation, vec![Value::Entity(entity.clone()), value.clone()]));
                }
            }
        }
    }
    facts.extend(included);
    let mut live: BTreeSet<Entity> = state.live.difference(&deleted).cloned().collect();
    live.extend(created.keys().cloned());
    let next = state.next + created.len() as u64;
    Ok(Transition {
        state: State { facts, live, next },
        effects: effects.into_iter().map(|(_, values, relation)| (relation, values)).collect(),
        substitutions: matched.len(),
    })
}

fn bind_event(
    program: &Program,
    model: &Model,
    event: &Atom,
    arguments: &[Value],
    budget: Budget,
) -> Result<Option<Binding>, EvalError> {
    let mut binding = Binding::new();
    for (argument, given) in event.arguments.iter().zip(arguments) {
        match argument {
            Expr::Variable(variable) => match binding.get(variable) {
                Some(bound) if bound != given => return Ok(None),
                _ => {
                    binding.insert(variable.clone(), given.clone());
                }
            },
            other => {
                if value(program, model, other, &binding, budget)? != *given {
                    return Ok(None);
                }
            }
        }
    }
    Ok(Some(binding))
}

fn ground(program: &Program, model: &Model, atom: &Atom, binding: &Binding, budget: Budget) -> Result<Vec<Value>, EvalError> {
    atom.arguments
        .iter()
        .map(|argument| value(program, model, argument, binding, budget))
        .collect()
}

fn mentions(value: &Value, entities: &BTreeSet<Entity>) -> bool {
    match value {
        Value::Entity(entity) => entities.contains(entity),
        Value::List(items) => items.iter().any(|v| mentions(v, entities)),
        Value::Record(items) | Value::Attrs(items) => items.values().any(|v| mentions(v, entities)),
        _ => false,
    }
}

fn clause_text(program: &Program, clause: &HandlerClause) -> String {
    let mut lines: Vec<String> = clause.body.iter().map(|l| format!("when {}", render_literal(program, l))).collect();
    for action in &clause.actions {
        lines.push(match action {
            Action::Set(atom) | Action::Include(atom) | Action::Emit(atom) => format!("then {}", render_atom(program, atom)),
            Action::Withdraw(atom) => format!("then not {}", render_atom(program, atom)),
            Action::Create { variable, entity_type, fields } => {
                let values: Vec<String> = fields.iter().map(|(n, e)| format!("{n}: {}", render_expr(program, e))).collect();
                format!("then ?{} is a new {entity_type} {{{}}}", variable.0, values.join(", "))
            }
            Action::Delete(expression) => format!("then not {} is deleted", render_expr(program, expression)),
        });
    }
    lines.sort();
    lines.join("\n")
}
