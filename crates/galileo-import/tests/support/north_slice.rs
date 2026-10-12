use galileo_core::{Entity, Fact, RelationKind, Type, Value as RuntimeValue};
use galileo_import::clause_checkpoint;
use galileo_import::clause_term::{self, Identity, Referent};
use galileo_runtime::{Program, Session, Tuple};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub struct Replay {
    pub recording: String,
    pub compared: usize,
    pub skipped: Vec<(String, String)>,
    pub mismatches: Vec<String>,
    pub coverage: BTreeMap<String, Coverage>,
}

#[derive(Default)]
pub struct Coverage {
    pub occurrences: usize,
    pub matched: usize,
    pub skipped: Vec<String>,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn program(module: &str) -> Program {
    let read = |name: &str| {
        let path = root().join(format!("{name}.gal"));
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    };
    let mut modules = vec![(String::new(), read(module))];
    let mut next = 0;
    while next < modules.len() {
        let uses: Vec<String> = modules[next]
            .1
            .lines()
            .filter_map(|line| line.strip_prefix("use "))
            .map(|used| used.split_whitespace().collect())
            .collect();
        next += 1;
        for used in uses {
            if used != module && !modules.iter().any(|(name, _)| *name == used) {
                let text = read(&used);
                modules.push((used, text));
            }
        }
    }
    let sources: Vec<(&str, &str)> = modules.iter().map(|(name, text)| (name.as_str(), text.as_str())).collect();
    let lowered = galileo_syntax::lower::lower_program(&sources).unwrap_or_else(|e| panic!("{module}: {e:?}"));
    assert!(lowered.diagnostics.is_empty(), "{module}: {:?}", lowered.diagnostics);
    Program::from(lowered.program)
}

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len()).step_by(2).map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap()).collect()
}

fn rendered(value: &RuntimeValue) -> Value {
    match value {
        RuntimeValue::Text(value) => json!(value),
        RuntimeValue::Int(value) => json!(value),
        RuntimeValue::Bool(value) => json!(value),
        value => panic!("host effect value {value:?} has no recorded form"),
    }
}

fn effects(program: &Program, tuples: &[Tuple]) -> Vec<Value> {
    tuples
        .iter()
        .map(|(relation, values)| match values.as_slice() {
            [_, action, payload] => json!({"action": rendered(action), "payload": rendered(payload)}),
            _ => panic!("host effect {} has {} slots", program.relations[relation.0].name, values.len()),
        })
        .collect()
}

fn argument(argument: &Value, boundary: &str) -> Result<RuntimeValue, String> {
    if let Some(text) = argument["text"].as_str() {
        Ok(RuntimeValue::Text(text.into()))
    } else if let Some(number) = argument["number"].as_f64() {
        if number.fract() != 0.0 || number.abs() > (1u64 << 53) as f64 {
            return Err(format!("{boundary}: argument {number} is not an integer of magnitude at most 2^53"));
        }
        Ok(RuntimeValue::Int(number as i64))
    } else if let Some(value) = argument["boolean"].as_bool() {
        Ok(RuntimeValue::Bool(value))
    } else {
        Err(format!("{boundary}: argument {argument} has no recorded form"))
    }
}

fn recorded_referent(value: &Value) -> Option<Referent> {
    let domain = value["domain"].as_u64()? as u32;
    let identity = value["identity"].as_str()?;
    let identity = if let Some(id) = identity.strip_prefix("Declared(").and_then(|rest| rest.strip_suffix(')')) {
        Identity::Declared(id.parse().ok()?)
    } else {
        let bytes = identity.strip_prefix("Created([")?.strip_suffix("])")?;
        let bytes: Vec<u8> = bytes.split(", ").map(|byte| byte.parse().ok()).collect::<Option<_>>()?;
        Identity::Created(bytes.try_into().ok()?)
    };
    Some(Referent { domain, identity })
}

fn projected(
    program: &Program,
    term: &clause_term::Term,
    created: &BTreeMap<Referent, Entity>,
    name: &str,
    value: clause_term::Value,
    ty: &Type,
) -> Result<RuntimeValue, String> {
    match (ty, value) {
        (Type::Text, clause_term::Value::Text(text)) => Ok(RuntimeValue::Text(text)),
        (Type::Bool, clause_term::Value::Bool(value)) => Ok(RuntimeValue::Bool(value)),
        (Type::Int, clause_term::Value::Number(number)) if number.fract() == 0.0 && number.abs() <= (1u64 << 53) as f64 => {
            Ok(RuntimeValue::Int(number as i64))
        }
        (Type::Entity(ty), clause_term::Value::Referent(referent)) => match referent.identity {
            Identity::Declared(_) => clause_checkpoint::declared(program, term, referent, ty)
                .map(RuntimeValue::Entity)
                .map_err(|error| format!("{name}: {error}")),
            Identity::Created(_) => created
                .get(&referent)
                .map(|entity| RuntimeValue::Entity(entity.clone()))
                .ok_or_else(|| format!("{name} holds created referent {referent:?}, which no Galileo entity corresponds to")),
        },
        (ty, value) => Err(format!("{name} holds {value:?} in a {ty:?} slot")),
    }
}

fn compare_projections(
    program: &Program,
    session: &Session,
    seeded: &[(Referent, Entity)],
    created_order: &[Referent],
    term: &[u8],
) -> Vec<String> {
    let term = match clause_term::decode(term) {
        Ok(term) => term,
        Err(error) => return vec![format!("next term: {error}")],
    };
    let mut mismatches = Vec::new();
    let mut created: BTreeMap<Referent, Entity> = seeded.iter().cloned().collect();
    let seeded_entities: BTreeSet<&Entity> = seeded.iter().map(|(_, entity)| entity).collect();
    let mut fresh: Vec<&Entity> = session
        .state()
        .live
        .iter()
        .filter(|entity| entity.creation_order.is_some() && !seeded_entities.contains(entity))
        .collect();
    fresh.sort_by_key(|entity| entity.creation_order);
    let fresh_referents: Vec<Referent> = created_order.iter().filter(|referent| !created.contains_key(referent)).copied().collect();
    if fresh.len() != fresh_referents.len() {
        mismatches.push(format!("created entities; Clause={} Galileo={}", fresh_referents.len(), fresh.len()));
    }
    for (referent, entity) in fresh_referents.into_iter().zip(fresh) {
        created.insert(referent, entity.clone());
    }
    let memberships: BTreeSet<String> = program.entity_types.iter().map(|ty| clause_checkpoint::membership(&ty.name)).collect();
    let mut model = None;
    for (at, relation) in program.relations.iter().enumerate() {
        let name = &relation.name;
        if !matches!(relation.kind, RelationKind::State | RelationKind::Derived) || memberships.contains(name) {
            continue;
        }
        if relation.slots.len() != 2 {
            mismatches.push(format!("{name} has {} slots; only binary relations project", relation.slots.len()));
            continue;
        }
        let table = clause_checkpoint::table_name(&term, name);
        let Some(rows) = term.tables.get(&table) else {
            mismatches.push(format!("{name} has no table {table} in the Clause term"));
            continue;
        };
        let mut expected = BTreeSet::new();
        for (subject, value) in rows {
            let values = [clause_term::Value::Referent(*subject), value.clone()]
                .into_iter()
                .zip(&relation.slots)
                .map(|(value, slot)| projected(program, &term, &created, name, value, &slot.ty))
                .collect::<Result<Vec<_>, _>>();
            match values {
                Ok(values) => {
                    expected.insert(values);
                }
                Err(error) => mismatches.push(error),
            }
        }
        let id = galileo_core::RelationId(at);
        let actual: BTreeSet<Vec<RuntimeValue>> = if relation.kind == RelationKind::State {
            session.state().facts.iter().filter(|(other, _)| *other == id).map(|(_, values)| values.clone()).collect()
        } else {
            let model = model.get_or_insert_with(|| {
                let state = session.state();
                let mut current = (**program).clone();
                current.entities.retain(|entity| !program.entity_types.iter().any(|ty| ty.state && ty.name == entity.entity_type));
                current.entities.extend(state.live.iter().cloned());
                let facts: Vec<Fact> = state.facts.iter().map(|(relation, values)| Fact::new(*relation, values.clone())).collect();
                galileo_core::evaluate(&current, &facts, Default::default())
            });
            match model {
                Ok(model) => model.tuples(id).cloned().collect(),
                Err(error) => {
                    mismatches.push(format!("{name}: {error:?}"));
                    continue;
                }
            }
        };
        if expected != actual {
            let missing: Vec<_> = expected.difference(&actual).collect();
            let extra: Vec<_> = actual.difference(&expected).collect();
            mismatches.push(format!("{name} (table {table}); only Clause={missing:?} only Galileo={extra:?}"));
        }
    }
    mismatches
}

pub fn replay(program: &Program, recording: &str) -> Replay {
    let path = root().join(recording);
    let input = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let records: Vec<Value> = input.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let events: BTreeMap<&str, &str> = program
        .relations
        .iter()
        .filter(|relation| relation.kind == RelationKind::Event)
        .map(|relation| (relation.name.rsplit('/').next().unwrap(), relation.name.as_str()))
        .collect();
    let north = program
        .entities
        .iter()
        .find(|entity| entity.name == "north-main")
        .unwrap_or_else(|| panic!("{recording}: the program declares no north-main"));
    let coverage = events.keys().map(|event| (event.to_string(), Coverage::default())).collect();
    let mut out = Replay { recording: recording.into(), compared: 0, skipped: Vec::new(), mismatches: Vec::new(), coverage };
    let mut term: Option<Vec<u8>> = None;
    let mut carried: Vec<Value> = Vec::new();
    for record in &records {
        let boundary = format!("{recording} session={} transaction={}", record["session"], record["transaction"]);
        match record["kind"].as_str().unwrap() {
            "session" => {
                term = None;
                carried.clear();
            }
            "projection" => term = Some(bytes(record["projection_hex"].as_str().unwrap())),
            "effects" => {
                let observed = |kind: &str, ordinal: &Value| {
                    records
                        .iter()
                        .find(|other| {
                            other["kind"] == kind
                                && other["session"] == record["session"]
                                && other["transaction"] == record["transaction"]
                                && other["ordinal"] == *ordinal
                        })
                        .unwrap_or_else(|| panic!("{boundary}: no {kind} for occurrence {ordinal}"))
                };
                let occurrences: Vec<_> = records
                    .iter()
                    .filter(|other| {
                        other["kind"] == "occurrence"
                            && other["session"] == record["session"]
                            && other["transaction"] == record["transaction"]
                    })
                    .map(|occurrence| {
                        let ordinal = &occurrence["ordinal"];
                        (occurrence, observed("admission", ordinal), observed("evaluation", ordinal))
                    })
                    .collect();
                let blocking = occurrences.iter().find(|(occurrence, admission, evaluation)| {
                    !events.contains_key(occurrence["handler"].as_str().unwrap())
                        && (evaluation["rule_applied"] == true || admission["result"] != "admitted")
                });
                let recorded_mailbox = record["mailbox_at_boundary"].as_array().unwrap().clone();
                let handlers: BTreeSet<&str> = occurrences.iter().map(|(occurrence, _, _)| occurrence["handler"].as_str().unwrap()).collect();
                for (occurrence, _, _) in &occurrences {
                    if let Some(coverage) = out.coverage.get_mut(occurrence["handler"].as_str().unwrap()) {
                        coverage.occurrences += 1;
                    }
                }
                if let Some((occurrence, _, _)) = blocking {
                    let blocker = occurrence["handler"].as_str().unwrap().to_owned();
                    for handler in &handlers {
                        if let Some(coverage) = out.coverage.get_mut(*handler) {
                            coverage.skipped.push(blocker.clone());
                        }
                    }
                    out.skipped.push((boundary, blocker));
                    carried = recorded_mailbox;
                    continue;
                }
                let mut seeded = Vec::new();
                let mut session = match &term {
                    None => Session::new(program),
                    Some(term) => match clause_checkpoint::convert(program, term) {
                        Ok(converted) => {
                            seeded = converted.entities;
                            Session::converted(program, converted.state)
                        }
                        Err(error) => {
                            out.mismatches.push(format!("{boundary} conversion: {error}"));
                            carried = recorded_mailbox;
                            continue;
                        }
                    },
                };
                let before = out.mismatches.len();
                let mut pending = Vec::new();
                let mut kept = Vec::new();
                for (occurrence, admission, evaluation) in &occurrences {
                    let handler = occurrence["handler"].as_str().unwrap();
                    let Some(relation) = events.get(handler) else {
                        continue;
                    };
                    let mut arguments = vec![RuntimeValue::Entity(north.clone())];
                    for recorded in occurrence["arguments"].as_array().unwrap() {
                        match argument(recorded, &boundary) {
                            Ok(value) => arguments.push(value),
                            Err(error) => out.mismatches.push(error),
                        }
                    }
                    pending.push((program.relation_id(relation).unwrap(), arguments));
                    kept.push((handler, *admission, *evaluation));
                }
                let result = (!pending.is_empty()).then(|| session.run_all(&pending));
                for (handler, admission, _) in &kept {
                    let actual = match &result {
                        Some(Ok(_)) => json!("admitted"),
                        Some(Err(error)) => json!(format!("{error:?}")),
                        None => unreachable!(),
                    };
                    if admission["result"] != actual {
                        out.mismatches.push(format!("{boundary} handler={handler} admission; Clause={} Galileo={actual}", admission["result"]));
                    }
                }
                let expected: Vec<Value> = kept
                    .iter()
                    .flat_map(|(_, _, evaluation)| evaluation["ordered_host_effects"].as_array().unwrap().iter().cloned())
                    .collect();
                let actual = match &result {
                    Some(Ok(step)) => effects(program, &step.effects),
                    _ => Vec::new(),
                };
                if expected != actual {
                    out.mismatches.push(format!("{boundary} ordered effects; Clause={} Galileo={}", json!(expected), json!(actual)));
                }
                let mut expected = recorded_mailbox.clone();
                for request in &carried {
                    if let Some(at) = expected.iter().position(|other| other == request) {
                        expected.remove(at);
                    }
                }
                let actual = effects(program, session.pending());
                if expected != actual {
                    out.mismatches.push(format!("{boundary} mailbox; Clause={} Galileo={}", json!(expected), json!(actual)));
                }
                let mut created_order = Vec::new();
                for (_, _, evaluation) in &kept {
                    for effect in evaluation["ordered_state_effects"].as_array().unwrap() {
                        for referent in [&effect["subject"], &effect["value"]].into_iter().filter_map(recorded_referent) {
                            if matches!(referent.identity, Identity::Created(_)) && !created_order.contains(&referent) {
                                created_order.push(referent);
                            }
                        }
                    }
                }
                let next = records.iter().find(|other| {
                    other["kind"] == "projection" && other["session"] == record["session"] && other["transaction"] == record["transaction"]
                });
                match next {
                    Some(next) => {
                        let next = bytes(next["projection_hex"].as_str().unwrap());
                        for mismatch in compare_projections(program, &session, &seeded, &created_order, &next) {
                            out.mismatches.push(format!("{boundary} projection {mismatch}"));
                        }
                    }
                    None => out.mismatches.push(format!("{boundary}: no projection follows")),
                }
                if out.mismatches.len() == before {
                    for handler in &handlers {
                        if let Some(coverage) = out.coverage.get_mut(*handler) {
                            coverage.matched += 1;
                        }
                    }
                }
                out.compared += 1;
                carried = recorded_mailbox;
            }
            _ => (),
        }
    }
    out
}
