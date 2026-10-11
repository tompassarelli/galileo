use galileo_core::state::{self, RejectionKind, Run, State, Tuple};
use galileo_core::*;
use serde_json::Value as Json;
use std::collections::BTreeSet;

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Admitted(Vec<Tuple>),
    Rejected(RejectionKind),
    Unknown,
}

fn reference_error(error: EvalError) -> Outcome {
    match error {
        EvalError::Unknown(Reason::OutOfBudget { .. }) => Outcome::Unknown,
        EvalError::Type { at, message } if at == "step" => {
            let kind = if message.contains("has no matching clause.") {
                RejectionKind::NoMatch
            } else if message.contains("allows at most one request per step:") {
                RejectionKind::CompetingEffects
            } else {
                panic!("unrecognized reference rejection: {message}");
            };
            Outcome::Rejected(kind)
        }
        other => panic!("unexpected reference error: {other:?}"),
    }
}

fn engine_outcome(result: Result<&state::StepRecord, EvalError>) -> Outcome {
    match result {
        Ok(record) => match &record.result {
            Ok(effects) => Outcome::Admitted(effects.clone()),
            Err(rejection) => Outcome::Rejected(rejection.kind),
        },
        Err(EvalError::Unknown(Reason::OutOfBudget { .. })) => Outcome::Unknown,
        Err(other) => panic!("unexpected engine error: {other:?}"),
    }
}

struct Agreement<'a> {
    program: &'a Program,
    reference: State,
    run: Run,
    steps: usize,
    batches: usize,
    rejected: usize,
    unknown: usize,
    restores: usize,
}

impl<'a> Agreement<'a> {
    fn new(program: &'a Program) -> Self {
        Self {
            program,
            reference: State::initial(program),
            run: Run::new(program),
            steps: 0,
            batches: 0,
            rejected: 0,
            unknown: 0,
            restores: 0,
        }
    }

    fn batch(&mut self, label: &str, events: &[Tuple], budget: Budget) {
        assert!(!events.is_empty(), "{label}: empty corpus transaction");
        let before = self.reference.clone();
        let revision = self.run.revision;
        let mut state = before.clone();
        let mut effects = BTreeSet::new();
        let mut expected = Outcome::Admitted(vec![]);
        for (at, (event, arguments)) in events.iter().enumerate() {
            if matches!(expected, Outcome::Admitted(_)) {
                match state::step(self.program, &state, *event, arguments, budget) {
                    Ok(mut transition) => {
                        let requested = transition.change.requested.len();
                        transition
                            .state
                            .pending
                            .truncate(transition.state.pending.len() - requested);
                        for request in transition.change.requested {
                            let key = (
                                self.program.relations[request.0 .0].name.clone(),
                                request.1.clone(),
                                request.0,
                            );
                            if !effects.contains(&key) {
                                transition.state.pending.push(request);
                            }
                        }
                        for (relation, values) in transition.effects {
                            effects.insert((
                                self.program.relations[relation.0].name.clone(),
                                values,
                                relation,
                            ));
                        }
                        state = transition.state;
                        expected = Outcome::Admitted(
                            effects
                                .iter()
                                .map(|(_, values, relation)| (*relation, values.clone()))
                                .collect(),
                        );
                    }
                    Err(error) => expected = reference_error(error),
                }
            }
            let mut prefix = self.run.clone();
            let actual = engine_outcome(prefix.run_all(self.program, &events[..=at], budget));
            let boundary = format!(
                "{label} occurrence={} event={}",
                at + 1,
                self.program.relations[event.0].name
            );
            assert_eq!(
                actual, expected,
                "{boundary}: admission/rejection and ordered effects"
            );
            let admitted = matches!(expected, Outcome::Admitted(_));
            let expected_state = if admitted { &state } else { &before };
            assert_eq!(
                prefix.revision,
                revision + u64::from(admitted),
                "{boundary}: revision"
            );
            assert_eq!(
                prefix.state.hash(self.program),
                expected_state.hash(self.program),
                "{boundary}: state hash"
            );
            assert_eq!(&prefix.state, expected_state, "{boundary}: state");
            if !matches!(expected, Outcome::Unknown) {
                let record = prefix.log.last().unwrap();
                assert_eq!(
                    record.hash,
                    expected_state.hash(self.program),
                    "{boundary}: logged hash"
                );
                assert_eq!(
                    record.revision, prefix.revision,
                    "{boundary}: logged revision"
                );
            } else {
                assert_eq!(prefix, self.run, "{boundary}: Unknown rollback");
            }
            self.steps += 1;
        }
        assert_eq!(
            engine_outcome(self.run.run_all(self.program, events, budget)),
            expected,
            "{label}: transaction boundary"
        );
        match expected {
            Outcome::Admitted(_) => self.reference = state,
            Outcome::Rejected(_) => self.rejected += 1,
            Outcome::Unknown => self.unknown += 1,
        }
        self.batches += 1;
    }

    fn restore(&mut self) {
        self.run = Run::restore(self.run.checkpoint());
        assert_eq!(self.run.state, self.reference);
        self.restores += 1;
    }

    fn report(&self, corpus: &str) {
        eprintln!("step_agreement corpus={corpus} occurrences={} batches={} rejected={} unknown={} restores={} revision={}", self.steps, self.batches, self.rejected, self.unknown, self.restores, self.run.revision);
    }
}

fn lower(source: &str, name: &str) -> Program {
    let lowered = galileo_syntax::lower::lower(source, name).unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    lowered.program
}

fn menu_event(program: &Program, name: &str, values: Vec<Value>) -> Tuple {
    let mut arguments = vec![Value::entity("North", "north-main")];
    arguments.extend(values);
    (program.relation_id(name).unwrap(), arguments)
}

fn north_corpus(program: &Program, name: &str, input: &str) -> usize {
    let mut agreement = Agreement::new(program);
    let mut pending = Vec::new();
    let mut checkpoint = None;
    let mut straight = None;
    let mut projected = [0usize; 3];
    for line in input.lines() {
        let record: Json = serde_json::from_str(line).unwrap();
        let label = format!(
            "{name} session={} transaction={}",
            record["session"], record["transaction"]
        );
        match record["kind"].as_str().unwrap() {
            "transaction" if record["boundary"] == "begin" => pending.clear(),
            "occurrence" => {
                let handler = record["handler"].as_str().unwrap();
                if let Some(relation) = program
                    .relation_id(handler)
                    .filter(|relation| program.relations[relation.0].kind == RelationKind::Event)
                {
                    let mut values = vec![Value::entity("North", "north-main")];
                    for argument in record["arguments"].as_array().unwrap() {
                        values.push(if let Some(text) = argument["text"].as_str() {
                            Value::text(text)
                        } else if let Some(number) = argument["number"].as_f64() {
                            assert_eq!(number.fract(), 0.0);
                            assert!(number.abs() <= (1u64 << 53) as f64);
                            Value::Int(number as i64)
                        } else {
                            Value::Bool(argument["boolean"].as_bool().unwrap())
                        });
                    }
                    pending.push((relation, values));
                    projected[0] += 1;
                } else if matches!(handler, "count-menu" | "filter-menu") {
                    projected[1] += 1;
                } else {
                    projected[2] += 1;
                }
            }
            "effects" if !pending.is_empty() => {
                agreement.batch(&label, &pending, Budget::default());
                if let Some(straight) = &mut straight {
                    let straight: &mut Run = straight;
                    straight
                        .run_all(program, &pending, Budget::default())
                        .unwrap();
                    assert_eq!(*straight, agreement.run, "{label}: restored continuation");
                }
            }
            "continuation" if record["operation"] == "checkpoint" => {
                checkpoint = Some((agreement.run.checkpoint(), agreement.reference.clone()));
            }
            "continuation" if record["operation"] == "restore" => {
                let (saved, reference) = checkpoint.take().expect("recorded checkpoint");
                straight = Some(agreement.run.clone());
                agreement.run = Run::restore(saved);
                agreement.reference = reference;
                agreement.restores += 1;
                assert_eq!(
                    agreement.run.state, agreement.reference,
                    "{label}: checkpoint state"
                );
            }
            _ => (),
        }
    }
    agreement.report(name);
    eprintln!(
        "step_agreement projection={name} retained={} derived_maintenance={} dropped={}",
        projected[0], projected[1], projected[2]
    );
    agreement.steps
}

fn pending_corpus() -> usize {
    let mut p = Program::builder();
    p.entity_type("Turn");
    for turn in ["first", "other"] {
        p.entity(Entity::named("Turn", turn));
    }
    p.entity_type("TurnStatus");
    for status in ["ok", "failed"] {
        p.entity(Entity::named("TurnStatus", status));
    }
    let send = p.relation(Relation::new(
        "send",
        RelationKind::Effect,
        [
            Slot::new("prompt", Type::Text),
            Slot::new("turn", Type::entity("Turn")),
        ],
    ));
    let request = p.relation(Relation::new(
        "request",
        RelationKind::Event,
        [
            Slot::new("turn", Type::entity("Turn")),
            Slot::new("prompt", Type::Text),
        ],
    ));
    let finished = p.relation(Relation::new(
        "finished",
        RelationKind::Event,
        [
            Slot::new("turn", Type::entity("Turn")),
            Slot::new("status", Type::entity("TurnStatus")),
        ],
    ));
    let burst = p.relation(Relation::new(
        "burst",
        RelationKind::Event,
        [Slot::new("turn", Type::entity("Turn"))],
    ));
    for event in [request, finished, burst] {
        p.event(Event::new(event));
    }
    p.effect(
        Effect::new(send)
            .at_most_one_per_step()
            .settled_by(finished),
    );
    p.handler(
        Handler::new(Atom::new(request, [Expr::var("turn"), Expr::var("prompt")])).clause(
            HandlerClause::new().action(Action::Emit(Atom::new(
                send,
                [Expr::var("prompt"), Expr::var("turn")],
            ))),
        ),
    );
    p.handler(
        Handler::new(Atom::new(
            finished,
            [
                Expr::var("turn"),
                Expr::value(Value::entity("TurnStatus", "ok")),
            ],
        ))
        .clause(HandlerClause::new()),
    );
    p.handler(
        Handler::new(Atom::new(burst, [Expr::var("turn")])).clause(
            HandlerClause::new()
                .action(Action::Emit(Atom::new(
                    send,
                    [Expr::value("a"), Expr::var("turn")],
                )))
                .action(Action::Emit(Atom::new(
                    send,
                    [Expr::value("b"), Expr::var("turn")],
                ))),
        ),
    );
    let p = p.build();
    check(&p).unwrap();
    let turn = Value::entity("Turn", "first");
    let oldest = (request, vec![turn.clone(), Value::text("z-oldest")]);
    let newest = (request, vec![turn.clone(), Value::text("a-newest")]);
    let receipt = (
        finished,
        vec![turn.clone(), Value::entity("TurnStatus", "ok")],
    );
    let mut agreement = Agreement::new(&p);
    for event in [&oldest, &newest] {
        agreement.batch("pending request", &[event.clone()], Budget::default());
    }
    let saved = agreement.run.checkpoint();
    let reference = agreement.reference.clone();
    agreement.batch(
        "pending settle oldest",
        &[receipt.clone()],
        Budget::default(),
    );
    agreement.restore();
    for event in [
        (
            finished,
            vec![
                Value::entity("Turn", "other"),
                Value::entity("TurnStatus", "ok"),
            ],
        ),
        (
            finished,
            vec![turn.clone(), Value::entity("TurnStatus", "failed")],
        ),
        (burst, vec![turn.clone()]),
    ] {
        agreement.batch("pending receipt or rejection", &[event], Budget::default());
    }
    agreement.batch(
        "pending atomic rejected",
        &[receipt.clone(), (burst, vec![turn.clone()])],
        Budget::default(),
    );
    agreement.run = Run::restore(saved.clone());
    agreement.reference = reference.clone();
    agreement.batch(
        "pending repeated request",
        &[oldest.clone()],
        Budget::default(),
    );
    agreement.batch(
        "pending repeated receipt",
        &[receipt.clone()],
        Budget::default(),
    );
    agreement.run = Run::restore(saved);
    agreement.reference = reference;
    agreement.batch(
        "pending duplicate batch",
        &[oldest.clone(), receipt.clone()],
        Budget::default(),
    );
    let mut atomic = Agreement::new(&p);
    atomic.batch(
        "pending atomic settlement",
        &[oldest, receipt],
        Budget::default(),
    );
    atomic.batch(
        "pending batched duplicate",
        &[newest.clone(), newest],
        Budget::default(),
    );
    agreement.report("pending receipts");
    atomic.report("pending atomic");
    agreement.steps + atomic.steps
}

#[test]
fn d16_d58_recorded_steps_agree_with_independent_reference() {
    let menu = lower(
        include_str!("../../../examples/north-menu.gal"),
        "north-menu.gal",
    );
    let history = north_corpus(
        &menu,
        "north history",
        include_str!("../../../tests/external/north-menu/history.jsonl"),
    );
    let restart = north_corpus(
        &menu,
        "north restart",
        include_str!("../../../tests/external/north-menu/restart.jsonl"),
    );
    let pending = pending_corpus();
    let mut scale = Agreement::new(&menu);
    let mut setup = vec![
        menu_event(&menu, "clear-menu-rows", vec![]),
        menu_event(
            &menu,
            "open-settings-menu",
            vec![Value::text("config"), Value::text("Context Switchboard")],
        ),
    ];
    let rows: Vec<usize> = if std::env::var_os("GALILEO_FULL").is_some() { (0..30).collect() } else { vec![0, 1, 2, 20] };
    setup.extend(rows.iter().map(|i| {
        menu_event(
            &menu,
            "offer-settings-row",
            vec![
                Value::text(format!("row-{i}")),
                Value::text(format!("Row {i}")),
                Value::text("Settings entry"),
                Value::text("on"),
            ],
        )
    }));
    setup.push(menu_event(&menu, "query-menu", vec![Value::text("")]));
    scale.batch(&format!("menu_scale {} rows setup", rows.len()), &setup, Budget::default());
    for (name, values) in [
        ("move-menu", vec![Value::Int(1)]),
        ("query-menu", vec![Value::text("Row 2")]),
        ("move-menu", vec![Value::Int(-1)]),
        ("query-menu", vec![Value::text("missing")]),
        ("move-menu", vec![Value::Int(1)]),
        ("query-menu", vec![Value::text("")]),
        ("close-menu", vec![]),
    ] {
        scale.batch(
            "menu_scale queries moves closes",
            &[menu_event(&menu, name, values)],
            Budget::default(),
        );
    }
    scale.batch(
        "menu_scale Unknown rollback",
        &[menu_event(&menu, "query-menu", vec![Value::text("x")])],
        Budget::work(0),
    );
    scale.restore();
    scale.batch(
        "menu_scale restore continuation",
        &[menu_event(&menu, "clear-menu-rows", vec![])],
        Budget::default(),
    );
    scale.report(&format!("menu_scale {} rows", rows.len()));
    eprintln!(
        "step_agreement corpora=4 occurrences={} mismatches=0",
        history + restart + pending + scale.steps
    );
}
