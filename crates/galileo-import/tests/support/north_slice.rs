use galileo_core::{RelationKind, Value as RuntimeValue};
use galileo_import::clause_checkpoint;
use galileo_runtime::{Program, Session, Tuple};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub struct Replay {
    pub recording: String,
    pub compared: usize,
    pub skipped: Vec<(String, String)>,
    pub mismatches: Vec<String>,
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

pub fn replay(program: &Program, recording: &str) -> Replay {
    let path = root().join(recording);
    let input = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let records: Vec<Value> = input.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let events: BTreeSet<&str> = program
        .relations
        .iter()
        .filter(|relation| relation.kind == RelationKind::Event)
        .map(|relation| relation.name.as_str())
        .collect();
    let north = program
        .entities
        .iter()
        .find(|entity| entity.name == "north-main")
        .unwrap_or_else(|| panic!("{recording}: the program declares no north-main"));
    let mut out = Replay { recording: recording.into(), compared: 0, skipped: Vec::new(), mismatches: Vec::new() };
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
                    !events.contains(occurrence["handler"].as_str().unwrap())
                        && (evaluation["rule_applied"] == true || admission["result"] != "admitted")
                });
                let recorded_mailbox = record["mailbox_at_boundary"].as_array().unwrap().clone();
                if let Some((occurrence, _, _)) = blocking {
                    out.skipped.push((boundary, occurrence["handler"].as_str().unwrap().to_owned()));
                    carried = recorded_mailbox;
                    continue;
                }
                let mut session = match &term {
                    None => Session::new(program),
                    Some(term) => match clause_checkpoint::convert(program, term) {
                        Ok(converted) => Session::converted(program, converted.state),
                        Err(error) => {
                            out.mismatches.push(format!("{boundary} conversion: {error}"));
                            carried = recorded_mailbox;
                            continue;
                        }
                    },
                };
                let mut pending = Vec::new();
                let mut kept = Vec::new();
                for (occurrence, admission, evaluation) in &occurrences {
                    let handler = occurrence["handler"].as_str().unwrap();
                    if !events.contains(handler) {
                        continue;
                    }
                    let mut arguments = vec![RuntimeValue::Entity(north.clone())];
                    for recorded in occurrence["arguments"].as_array().unwrap() {
                        match argument(recorded, &boundary) {
                            Ok(value) => arguments.push(value),
                            Err(error) => out.mismatches.push(error),
                        }
                    }
                    pending.push((program.relation_id(handler).unwrap(), arguments));
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
                out.compared += 1;
                carried = recorded_mailbox;
            }
            _ => (),
        }
    }
    out
}
