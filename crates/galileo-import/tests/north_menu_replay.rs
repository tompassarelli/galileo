use galileo_core::{RelationKind, Value as RuntimeValue};
use galileo_runtime::{Program, Session, Tuple};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

fn take<'a>(input: &mut &'a [u8], count: usize) -> &'a [u8] {
    let (head, tail) = input.split_at(count);
    *input = tail;
    head
}

fn count(input: &mut &[u8]) -> usize {
    u16::from_le_bytes(take(input, 2).try_into().unwrap()) as usize
}

fn referent(input: &mut &[u8]) -> String {
    let domain = u32::from_le_bytes(take(input, 4).try_into().unwrap());
    let identity = match take(input, 1)[0] {
        0 => format!(
            "Declared({})",
            u32::from_le_bytes(take(input, 4).try_into().unwrap())
        ),
        1 => format!("Created({:?})", take(input, 32)),
        tag => panic!("unsupported reference identity {tag}"),
    };
    format!("{domain}:{identity}")
}

fn reference_value(input: &mut &[u8], creations: &[String]) -> Value {
    match take(input, 1)[0] {
        0 => json!(f64::from_le_bytes(take(input, 8).try_into().unwrap())),
        1 => json!(take(input, 1)[0] == 1),
        4 => {
            let length = count(input);
            json!(std::str::from_utf8(take(input, length)).unwrap())
        }
        5 => identity(&referent(input), creations),
        tag => panic!("unsupported menu reference value {tag}"),
    }
}

fn identity(reference: &str, creations: &[String]) -> Value {
    if reference.contains("Declared(") {
        json!("north-main")
    } else {
        json!(creations
            .iter()
            .position(|created| created == reference)
            .expect("recorded row creation"))
    }
}

fn table(bytes: &[u8], name: &str, creations: &[String]) -> Vec<Value> {
    let kind = b"clause/js-field-v1";
    let mut marker = vec![0];
    marker.extend((kind.len() as u32).to_be_bytes());
    marker.extend(kind);
    marker.extend((name.len() as u32).to_be_bytes());
    marker.extend(name.as_bytes());
    marker.extend([0, 0]);
    let mut result = None;
    for at in 0..bytes.len().saturating_sub(marker.len()) {
        if !bytes[at..].starts_with(&marker) {
            continue;
        }
        let mut input = &bytes[at + marker.len()..];
        let length = u32::from_be_bytes(take(&mut input, 4).try_into().unwrap()) as usize;
        let kind = take(&mut input, length);
        let length = u32::from_be_bytes(take(&mut input, 4).try_into().unwrap()) as usize;
        let mut payload = take(&mut input, length);
        assert_eq!(take(&mut input, 1), &[0]);
        if kind != b"clause/process-projected-relation-table-v1" {
            continue;
        }
        assert!(result.is_none(), "duplicate reference table {name}");
        assert_eq!(take(&mut payload, 1), &[6]);
        take(&mut payload, 5);
        if take(&mut payload, 1)[0] == 1 {
            take(&mut payload, 4);
        }
        take(&mut payload, 1);
        let mut rows = Vec::new();
        for _ in 0..count(&mut payload) {
            let subject = identity(&referent(&mut payload), creations);
            for _ in 0..count(&mut payload) {
                rows.push(json!([subject, reference_value(&mut payload, creations)]));
            }
        }
        assert!(payload.is_empty(), "trailing reference table {name}");
        result = Some(rows);
    }
    result.expect(name)
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
    let source = std::fs::read_to_string(
        "/home/tom/code/north/pins/eaee1838d1968b7caba65195866e5d52fd30b2e1/clause/north.clause",
    )
    .unwrap();
    let imported = galileo_import::clause_menu::import(&source).unwrap();
    let lowered = galileo_syntax::lower::lower(&imported.source, "north-menu.gal").unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let program = Program::from(lowered.program);
    let available: BTreeMap<_, _> = program
        .relations
        .iter()
        .enumerate()
        .filter(|(_, relation)| relation.kind == RelationKind::Event)
        .map(|(at, relation)| (relation.name.as_str(), galileo_core::RelationId(at)))
        .collect();
    assert_eq!(available.len(), 13);
    assert!(equal(&json!(5), &json!(5.0)));
    let mut mismatches = Vec::new();
    let mut census = [0usize; 7];
    let mut replay = [0usize; 6];
    let mut kept: BTreeMap<String, usize> = BTreeMap::new();
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
        let mut checkpoint = None;
        let mut creations = Vec::new();
        let mut pending = Vec::new();
        let mut observations = Vec::new();
        let mut dropped_applied = Vec::new();
        let mut last_projection = None;
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
                    if let Some(relation) = available.get(handler) {
                        *kept.entry(handler.to_owned()).or_default() += 1;
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
                        pending.push((*relation, arguments));
                        observations.push((handler, observation("admission"), evaluation));
                        for clause in evaluation["clauses"].as_array().unwrap() {
                            slice_evaluations += 1;
                            slice_evaluated.insert(clause["rule"].as_u64().unwrap());
                            if clause["selected"] == true {
                                slice_selected.insert(clause["rule"].as_u64().unwrap());
                            }
                        }
                        if matches!(
                            handler,
                            "offer-history-row" | "offer-settings-row" | "offer-work-row"
                        ) {
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
                            assert_eq!(result, Some(continued), "restore effects at {boundary}");
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
                    let hex = record["projection_hex"].as_str().unwrap();
                    let bytes: Vec<_> = (0..hex.len())
                        .step_by(2)
                        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
                        .collect();
                    for (at, relation) in
                        program
                            .relations
                            .iter()
                            .enumerate()
                    {
                        if relation.kind != RelationKind::State || relation.name.starts_with("is-a-") {
                            continue;
                        }
                        let mut expected = table(&bytes, &relation.name, &creations);
                        let mut actual: Vec<_> = session
                            .state()
                            .facts
                            .iter()
                            .filter(|(id, _)| id.0 == at)
                            .map(|(_, values)| {
                                json!(values.iter().map(actual_value).collect::<Vec<_>>())
                            })
                            .collect();
                        expected.sort_by_key(Value::to_string);
                        actual.sort_by_key(Value::to_string);
                        if !equal(&json!(expected), &json!(actual)) {
                            mismatches.push(format!("DECISION 8.7: {boundary} projection {} dropped_rule_applied={dropped_applied:?}; Clause={} Galileo={}", relation.name, json!(expected), json!(actual)));
                        }
                    }
                    last_projection = Some(session.state().clone());
                }
                "continuation" if record["operation"] == "checkpoint" => {
                    checkpoint = Some(session.checkpoint())
                }
                "continuation" if record["operation"] == "restore" => {
                    census[5] += 1;
                    let restored =
                        Session::restore(&program, checkpoint.take().expect("recorded checkpoint"))
                            .unwrap();
                    assert_eq!(restored.state(), last_projection.as_ref().unwrap());
                    assert_eq!(
                        restored.as_of(0).unwrap().state(),
                        Session::new(&program).state()
                    );
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
            "Kept menu handler {handler}: occurrences={}",
            kept.get(*handler).unwrap_or(&0)
        );
    }
    for (handler, (count, applied)) in &dropped {
        eprintln!(
            "Dropped scope handler {handler}: occurrences={count} rule_applied_true={applied}"
        );
    }
    eprintln!("North Clause reference census: sessions={} transactions={} occurrences={} handlers={} evaluated_clauses={} selected_clauses={} evaluation_instances={} restores={} skips={} rejections={}", census[0], census[1], census[2], handlers.len(), evaluated.len(), selected.len(), census[6], census[5], census[3], census[4]);
    eprintln!("Projected Galileo coverage: sessions={} stepped_transactions={} retained_occurrences={} dropped_occurrences={} compared_boundaries={} restores={} continuation_boundaries={} imported_events={} observed_events={} imported_handler_evaluated_clauses={} imported_handler_selected_clauses={} imported_handler_evaluation_instances={} mismatches={}", replay[0], replay[1], replay[2], dropped.values().map(|(count, _)| count).sum::<usize>(), replay[3], replay[4], replay[5], available.len(), kept.len(), slice_evaluated.len(), slice_selected.len(), slice_evaluations, mismatches.len());
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
