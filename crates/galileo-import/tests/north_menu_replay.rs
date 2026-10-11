use galileo_import::clause_checkpoint;
use galileo_import::clause_term::{self, Identity, Referent};
use galileo_core::{engine, Fact, RelationKind, Value as RuntimeValue};
use galileo_runtime::{Program, Session, Tuple};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[path = "support/north_offer.rs"]
mod north_offer;

fn identity(referent: &Referent, term: &clause_term::Term, creations: &[String]) -> Value {
    match referent.identity {
        Identity::Declared(_) => json!(term
            .singletons
            .iter()
            .find(|(_, singleton)| *singleton == referent)
            .expect("declared singleton")
            .0),
        Identity::Created(bytes) => {
            let reference = format!("{}:Created({bytes:?})", referent.domain);
            json!(creations
                .iter()
                .position(|created| *created == reference)
                .expect("recorded row creation"))
        }
    }
}

fn projection(record: &Value) -> Vec<u8> {
    let hex = record["projection_hex"].as_str().unwrap();
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect()
}

fn table(term: &clause_term::Term, name: &str, creations: &[String]) -> Vec<Value> {
    term.tables
        .get(name)
        .expect(name)
        .iter()
        .map(|(subject, value)| {
            let value = match value {
                clause_term::Value::Number(number) => json!(number),
                clause_term::Value::Bool(value) => json!(value),
                clause_term::Value::Text(text) => json!(text),
                clause_term::Value::Referent(referent) => identity(referent, term, creations),
            };
            json!([identity(subject, term, creations), value])
        })
        .collect()
}

fn actual_value(value: &RuntimeValue) -> Value {
    match value {
        RuntimeValue::Text(value) => json!(value),
        RuntimeValue::Int(value) => json!(value),
        RuntimeValue::Bool(value) => json!(value),
        RuntimeValue::Entity(entity) => match entity.creation_order {
            Some(order) => json!(order),
            None => json!(entity.name),
        },
        value => panic!("unsupported menu runtime value {value:?}"),
    }
}

fn equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => left.as_f64() == right.as_f64(),
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| equal(left, right))
        }
        _ => left == right,
    }
}

fn effects(program: &Program, tuples: &[Tuple]) -> Value {
    json!(tuples
        .iter()
        .map(|(relation, values)| {
            assert_eq!(program.relations[relation.0].name, "menu-host-effect");
            json!({"action": actual_value(&values[1]), "payload": actual_value(&values[2])})
        })
        .collect::<Vec<_>>())
}

#[test]
fn north_menu_clause_reference_projected_transactions_and_restore() {
    let source = include_str!("../../../examples/north-menu.gal");
    let lowered = galileo_syntax::lower::lower(source, "north-menu.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let program = Program::from(lowered.program);
    let available: BTreeMap<_, _> = program
        .relations
        .iter()
        .enumerate()
        .filter(|(_, relation)| relation.kind == RelationKind::Event)
        .map(|(at, relation)| (relation.name.as_str(), galileo_core::RelationId(at)))
        .collect();
    assert_eq!(available.len(), 9);
    assert!(equal(&json!(5), &json!(5.0)));
    let mut mismatches = Vec::new();
    let mut census = [0usize; 7];
    let mut replay = [0usize; 6];
    let mut kept: BTreeMap<String, usize> = BTreeMap::new();
    let mut kept_skips: BTreeMap<String, usize> = BTreeMap::new();
    let mut derived_maintenance: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut dropped: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut handlers = BTreeSet::new();
    let mut evaluated = BTreeSet::new();
    let mut selected = BTreeSet::new();
    let mut slice_evaluated = BTreeSet::new();
    let mut slice_selected = BTreeSet::new();
    let mut slice_evaluations = 0;
    for (sequence, input) in [
        (
            "history",
            include_str!("../../../tests/external/north-menu/history.jsonl"),
        ),
        (
            "restart",
            include_str!("../../../tests/external/north-menu/restart.jsonl"),
        ),
    ] {
        let records: Vec<Value> = input
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let mut session = Session::new(&program);
        replay[0] += 1;
        let mut straight = None;
        let mut creations = Vec::new();
        let mut pending = Vec::new();
        let mut observations = Vec::new();
        let mut dropped_applied = Vec::new();
        for record in &records {
            let boundary = format!(
                "{sequence} session={} transaction={}",
                record["session"], record["transaction"]
            );
            match record["kind"].as_str().unwrap() {
                "session" => census[0] += 1,
                "transaction" if record["boundary"] == "begin" => {
                    census[1] += 1;
                    pending.clear();
                    observations.clear();
                    dropped_applied.clear();
                }
                "occurrence" => {
                    census[2] += 1;
                    let handler = record["handler"].as_str().unwrap();
                    handlers.insert(handler.to_owned());
                    let observation = |kind: &str| {
                        records
                            .iter()
                            .find(|other| {
                                other["kind"] == kind
                                    && other["session"] == record["session"]
                                    && other["transaction"] == record["transaction"]
                                    && other["ordinal"] == record["ordinal"]
                            })
                            .unwrap()
                    };
                    let evaluation = observation("evaluation");
                    census[3] += usize::from(evaluation["rule_applied"] == false);
                    census[4] += usize::from(observation("admission")["result"] != "admitted");
                    for clause in evaluation["clauses"].as_array().unwrap() {
                        census[6] += 1;
                        evaluated.insert(clause["rule"].as_u64().unwrap());
                        if clause["selected"] == true {
                            selected.insert(clause["rule"].as_u64().unwrap());
                        }
                    }
                    let (event, annotation) = north_offer::translate(handler);
                    let relation = available.get(event);
                    if relation.is_some() || matches!(handler, "count-menu" | "filter-menu") {
                        *kept.entry(event.to_owned()).or_default() += 1;
                        *kept_skips.entry(event.to_owned()).or_default() +=
                            usize::from(evaluation["rule_applied"] == false);
                        let mut arguments = vec![RuntimeValue::entity("North", "north-main")];
                        for argument in record["arguments"].as_array().unwrap() {
                            arguments.push(if let Some(text) = argument["text"].as_str() {
                                RuntimeValue::Text(text.into())
                            } else if let Some(number) = argument["number"].as_f64() {
                                assert_eq!(number.fract(), 0.0);
                                assert!(number.abs() <= (1u64 << 53) as f64);
                                RuntimeValue::Int(number as i64)
                            } else {
                                RuntimeValue::Bool(argument["boolean"].as_bool().unwrap())
                            });
                        }
                        arguments.extend(annotation);
                        if let Some(relation) = relation {
                            pending.push((*relation, arguments));
                        } else {
                            let count = derived_maintenance.entry(handler.to_owned()).or_default();
                            count.0 += 1;
                            count.1 += usize::from(evaluation["rule_applied"] == false);
                        }
                        observations.push((handler, observation("admission"), evaluation));
                        for clause in evaluation["clauses"].as_array().unwrap() {
                            slice_evaluations += 1;
                            slice_evaluated.insert(clause["rule"].as_u64().unwrap());
                            if clause["selected"] == true {
                                slice_selected.insert(clause["rule"].as_u64().unwrap());
                            }
                        }
                        if event == "offer-row" {
                            for effect in evaluation["ordered_state_effects"].as_array().unwrap() {
                                let subject = &effect["subject"];
                                if subject["identity"]
                                    .as_str()
                                    .unwrap()
                                    .starts_with("Created(")
                                {
                                    let id = format!(
                                        "{}:{}",
                                        subject["domain"],
                                        subject["identity"].as_str().unwrap()
                                    );
                                    if !creations.contains(&id) {
                                        creations.push(id);
                                    }
                                }
                            }
                        }
                    } else {
                        let count = dropped.entry(handler.to_owned()).or_default();
                        count.0 += 1;
                        count.1 += usize::from(evaluation["rule_applied"] == true);
                        if evaluation["rule_applied"] == true {
                            dropped_applied.push(handler);
                        }
                    }
                }
                "effects" => {
                    let result = if pending.is_empty() {
                        None
                    } else {
                        replay[1] += 1;
                        replay[2] += pending.len();
                        Some(session.run_all(&pending))
                    };
                    for (handler, admission, _) in &observations {
                        let actual = match &result {
                            Some(Ok(_)) => json!("admitted"),
                            Some(Err(error)) => json!(format!("{error:?}")),
                            None => unreachable!(),
                        };
                        if !equal(&admission["result"], &actual) {
                            mismatches.push(format!("DECISION 8.7: {boundary} handler={handler} admission; Clause={} Galileo={actual}", admission["result"]));
                        }
                    }
                    let expected = json!(observations
                        .iter()
                        .flat_map(|(_, _, evaluation)| evaluation["ordered_host_effects"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .cloned())
                        .collect::<Vec<_>>());
                    let actual = match &result {
                        Some(Ok(step)) => effects(&program, &step.effects),
                        _ => json!([]),
                    };
                    if !equal(&expected, &actual) {
                        mismatches.push(format!("DECISION 8.7: {boundary} ordered effects; Clause={expected} Galileo={actual}"));
                    }
                    let recorded = record;
                    assert!(
                        equal(&expected, &recorded["ordered"]),
                        "dropped host effects at {boundary}"
                    );
                    let mailbox = effects(&program, session.pending());
                    if !equal(&recorded["mailbox_at_boundary"], &mailbox) {
                        mismatches.push(format!(
                            "DECISION 8.7: {boundary} mailbox; Clause={} Galileo={mailbox}",
                            recorded["mailbox_at_boundary"]
                        ));
                    }
                    if let Some(straight) = &mut straight {
                        let straight: &mut Session<'_> = straight;
                        if !pending.is_empty() {
                            let continued = straight.run_all(&pending);
                            assert_eq!(
                                result.as_ref().map(|result| result.as_ref().map(|step| &step.effects)),
                                Some(continued.as_ref().map(|step| &step.effects)),
                                "restore effects at {boundary}"
                            );
                        }
                        assert_eq!(
                            session.state(),
                            straight.state(),
                            "restore continuation at {boundary}"
                        );
                        replay[5] += 1;
                    }
                }
                "projection" => {
                    replay[3] += 1;
                    let bytes = projection(record);
                    let term = clause_term::decode(&bytes).unwrap();
                    let mut current = (*program).clone();
                    current.entities.retain(|entity| {
                        !program
                            .entity_types
                            .iter()
                            .any(|ty| ty.state && ty.name == entity.entity_type)
                    });
                    current.entities.extend(session.state().live.iter().cloned());
                    let facts: Vec<_> = session
                        .state()
                        .facts
                        .iter()
                        .map(|(relation, values)| Fact::new(*relation, values.clone()))
                        .collect();
                    let model = engine::evaluate(&current, &facts, Default::default()).unwrap();
                    for (at, relation) in
                        program
                            .relations
                            .iter()
                            .enumerate()
                    {
                        if !matches!(relation.kind, RelationKind::State | RelationKind::Derived)
                            || relation.name.starts_with("is-a-")
                        {
                            continue;
                        }
                        let reference_name = match relation.name.as_str() {
                            "MenuRow.key" => "menu-row-key",
                            "MenuRow.label" => "menu-row-label",
                            "MenuRow.description" => "menu-row-description",
                            "MenuRow.annotation" => "menu-row-annotation",
                            "MenuRow.order" => "menu-row-order",
                            name => name,
                        };
                        let mut expected = table(&term, reference_name, &creations);
                        let mut actual: Vec<_> = model
                            .tuples(galileo_core::RelationId(at))
                            .map(|values| {
                                json!(values.iter().map(actual_value).collect::<Vec<_>>())
                            })
                            .collect();
                        expected.sort_by_key(Value::to_string);
                        actual.sort_by_key(Value::to_string);
                        if !equal(&json!(expected), &json!(actual)) {
                            mismatches.push(format!("DECISION 8.7: {boundary} projection {} dropped_rule_applied={dropped_applied:?}; Clause={} Galileo={}", relation.name, json!(expected), json!(actual)));
                        }
                    }
                }
                "continuation" if record["operation"] == "restore" => {
                    census[5] += 1;
                    let term = clause_term::decode(&projection(record)).unwrap();
                    let singletons: Vec<_> = term
                        .singletons
                        .iter()
                        .filter(|(_, referent)| matches!(referent.identity, Identity::Declared(_)))
                        .map(|(name, _)| name.as_str())
                        .collect();
                    eprintln!("Restore term: tables={} declared_singletons={singletons:?}", term.tables.len());
                    assert_eq!(term.tables.len(), 142);
                    assert_eq!(singletons, ["initial-context", "north-main", "view-chat", "view-goals"]);
                    assert_eq!(term.singletons.len(), 4);
                    let converted = clause_checkpoint::convert(&program, &projection(record)).unwrap();
                    let entities: Vec<_> = converted.entities.iter().map(|(_, entity)| entity.name.as_str()).collect();
                    eprintln!("Converted Clause term: entities={entities:?} next={} dropped_tables={}", converted.state.next, converted.dropped.len());
                    assert_eq!(&converted.state, session.state());
                    assert_eq!(entities, ["MenuRow#0"]);
                    assert_eq!((converted.state.next, converted.dropped.len()), (1, 126));
                    let mut seeded = converted.entities.clone();
                    seeded.sort_by_key(|(_, entity)| entity.creation_order);
                    creations = seeded
                        .iter()
                        .map(|(referent, _)| match referent.identity {
                            Identity::Created(bytes) => format!("{}:Created({bytes:?})", referent.domain),
                            Identity::Declared(_) => unreachable!("converted entities are created"),
                        })
                        .collect();
                    let restored = Session::converted(&program, converted.state.clone());
                    assert_eq!(restored.revision(), 1);
                    assert_eq!(restored.as_of(1).unwrap().state(), &converted.state);
                    let before = restored.as_of(0).err().expect("no revision 0 after conversion");
                    assert!(format!("{before:?}").contains("revision 1"), "{before:?}");
                    let next = restored.state().next;
                    assert!(restored.state().live.iter().all(|entity| entity.creation_order.is_some_and(|id| id < next)));
                    straight = Some(std::mem::replace(&mut session, restored));
                    replay[0] += 1;
                    replay[4] += 1;
                }
                _ => (),
            }
        }
    }
    for handler in available.keys() {
        eprintln!(
            "Kept menu handler {handler}: occurrences={} skips={}",
            kept.get(*handler).unwrap_or(&0),
            kept_skips.get(*handler).unwrap_or(&0)
        );
    }
    for (handler, (count, skips)) in &derived_maintenance {
        eprintln!(
            "Derived maintenance handler {handler}: occurrences={count} skips={skips}"
        );
    }
    for (handler, (count, applied)) in &dropped {
        eprintln!(
            "Dropped scope handler {handler}: occurrences={count} rule_applied_true={applied}"
        );
    }
    eprintln!(
        "Menu program handlers: named={} clauses={}",
        program.handlers.len(),
        program.handlers.iter().map(|handler| handler.clauses.len()).sum::<usize>()
    );
    eprintln!("North Clause reference census: sessions={} transactions={} occurrences={} handlers={} evaluated_clauses={} selected_clauses={} evaluation_instances={} restores={} skips={} rejections={}", census[0], census[1], census[2], handlers.len(), evaluated.len(), selected.len(), census[6], census[5], census[3], census[4]);
    eprintln!("Projected Galileo coverage: sessions={} stepped_transactions={} retained_occurrences={} replayed_occurrences={} derived_maintenance_occurrences={} dropped_occurrences={} compared_boundaries={} restores={} continuation_boundaries={} imported_events={} observed_events={} imported_handler_evaluated_clauses={} imported_handler_selected_clauses={} imported_handler_evaluation_instances={} mismatches={}", replay[0], replay[1], kept.values().sum::<usize>(), replay[2], derived_maintenance.values().map(|(count, _)| count).sum::<usize>(), dropped.values().map(|(count, _)| count).sum::<usize>(), replay[3], replay[4], replay[5], available.len(), kept.len(), slice_evaluated.len(), slice_selected.len(), slice_evaluations, mismatches.len());
    for mismatch in &mismatches {
        eprintln!("{mismatch}");
    }
    assert!(
        mismatches.is_empty(),
        "projected external-reference replay: {} mismatches; first: {}",
        mismatches.len(),
        mismatches.first().map(String::as_str).unwrap_or("none")
    );
}
