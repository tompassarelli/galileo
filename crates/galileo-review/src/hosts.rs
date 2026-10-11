use galileo_core::engine::evaluate;
use galileo_core::why::sentence;
use galileo_core::{Budget, EvalError, Model, Program, RelationId, RelationKind, Value};
use std::collections::{BTreeMap, BTreeSet};

const ENABLED: &str = "{module} is enabled on {host}";
const SETS: &str = "{host} gets {path} set to {value} by {module}";

fn name(value: &Value) -> String {
    match value {
        Value::Entity(entity) => entity.name.clone(),
        other => format!("{other:?}"),
    }
}

fn rows(program: &Program, model: &Model, template: &str, host: usize) -> BTreeMap<Vec<Value>, (String, String)> {
    let Some(index) = program.relations.iter().position(|r| r.template == template && r.kind == RelationKind::Derived) else {
        return BTreeMap::new();
    };
    let id = RelationId(index);
    model.tuples(id).map(|values| (values.clone(), (name(&values[host]), sentence(program, id, values)))).collect()
}

pub fn host_consequences(before: &Program, after: &Program, budget: Budget) -> Result<Vec<(String, String)>, EvalError> {
    let mut out = BTreeSet::new();
    let (old_model, new_model) = (evaluate(before, &[], budget)?, evaluate(after, &[], budget)?);
    let (old, new) = (rows(before, &old_model, ENABLED, 1), rows(after, &new_model, ENABLED, 1));
    for (values, (host, _)) in &old {
        if !new.contains_key(values) {
            out.insert((host.clone(), format!("turns off {}", name(&values[0]))));
        }
    }
    for (values, (host, _)) in &new {
        if !old.contains_key(values) {
            out.insert((host.clone(), format!("turns on {}", name(&values[0]))));
        }
    }
    let (old, new) = (rows(before, &old_model, SETS, 0), rows(after, &new_model, SETS, 0));
    for (values, (host, text)) in &old {
        if !new.contains_key(values) {
            out.insert((host.clone(), format!("no longer: {text}")));
        }
    }
    for (values, (host, text)) in &new {
        if !old.contains_key(values) {
            out.insert((host.clone(), format!("now: {text}")));
        }
    }
    Ok(out.into_iter().collect())
}
