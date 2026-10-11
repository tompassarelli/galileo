use galileo_core::state::{self, Run, State, Tuple};
use galileo_core::*;
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};

#[path = "support/north_offer.rs"]
mod north_offer;

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Admitted(Vec<Tuple>),
    Rejected(Vec<String>),
    Failed(String),
    Unknown,
}

fn failure(error: EvalError) -> Outcome {
    match error {
        EvalError::Unknown(Reason::OutOfBudget { .. }) => Outcome::Unknown,
        EvalError::Overflow { at } => Outcome::Failed(format!("overflow at {at}")),
        EvalError::Type { at, .. } => Outcome::Failed(format!("type at {at}")),
        other => Outcome::Failed(format!("{other:?}")),
    }
}

fn reference_error(error: EvalError) -> Outcome {
    match error {
        EvalError::Type { at, message } if at == "step" => {
            Outcome::Rejected(message.split('\n').map(str::to_string).collect())
        }
        other => failure(other),
    }
}

fn engine_outcome(result: Result<&state::StepRecord, EvalError>) -> Outcome {
    match result {
        Ok(record) => match &record.result {
            Ok(effects) => Outcome::Admitted(effects.clone()),
            Err(rejection) => Outcome::Rejected(rejection.explanation.clone()),
        },
        Err(error) => failure(error),
    }
}

struct Agreement<'a> {
    program: &'a Program,
    reference: State,
    run: Run,
    steps: usize,
    batches: usize,
    rejected: usize,
    failed: usize,
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
            failed: 0,
            unknown: 0,
            restores: 0,
        }
    }

    fn batch(&mut self, label: &str, events: &[Tuple], budget: Budget) -> Outcome {
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
            if !matches!(expected, Outcome::Unknown | Outcome::Failed(_)) {
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
                assert_eq!(prefix, self.run, "{boundary}: Unknown or error rollback");
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
            Outcome::Failed(_) => self.failed += 1,
            Outcome::Unknown => self.unknown += 1,
        }
        self.batches += 1;
        expected
    }

    fn restore(&mut self) {
        self.run = Run::restore(self.run.checkpoint());
        assert_eq!(self.run.state, self.reference);
        self.restores += 1;
    }

    fn report(&self, corpus: &str) {
        eprintln!("step_agreement corpus={corpus} occurrences={} batches={} rejected={} failed={} unknown={} restores={} revision={}", self.steps, self.batches, self.rejected, self.failed, self.unknown, self.restores, self.run.revision);
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
                let (event, annotation) = north_offer::translate(handler);
                if let Some(relation) = program
                    .relation_id(event)
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
                    values.extend(annotation);
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
            "offer-row",
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

const SEED: u64 = 0x0962_5eed;
const PROGRAMS: usize = 1_000;
const OCCURRENCES: usize = 20;
const RESTORES: usize = 1_000;
const OPS: [CompareOp; 6] = [
    CompareOp::Equal,
    CompareOp::NotEqual,
    CompareOp::Less,
    CompareOp::LessEqual,
    CompareOp::Greater,
    CompareOp::GreaterEqual,
];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn range(&mut self, low: usize, high: usize) -> usize {
        low + self.below(high - low + 1)
    }
    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

struct Generated {
    program: Program,
    events: Vec<RelationId>,
    guarded: RelationId,
    scan: Option<RelationId>,
    clash: Option<RelationId>,
}

fn constant(rng: &mut Rng) -> Expr {
    Expr::value(rng.below(4) as i64)
}

fn int_argument(rng: &mut Rng, near_max: usize) -> Value {
    if rng.chance(near_max) {
        Value::Int(i64::MAX - rng.below(3) as i64)
    } else {
        Value::Int(rng.below(4) as i64)
    }
}

fn bound_value(rng: &mut Rng, bound: &[String]) -> Expr {
    match rng.below(10) {
        0 | 1 => constant(rng),
        2 => Expr::binary(BinaryOp::Add, Expr::var(rng.pick(bound).clone()), Expr::value(1_i64)),
        _ => Expr::var(rng.pick(bound).clone()),
    }
}

fn generate(rng: &mut Rng, bulk: usize, clash: bool) -> Generated {
    let mut p = Program::builder();
    p.entity_type("Key");
    let keys: Vec<Value> = (0..2).map(|k| Value::entity("Key", format!("k{k}"))).collect();
    for k in 0..2 {
        p.entity(Entity::named("Key", format!("k{k}")));
    }
    p.state_entity_type("Item", [Slot::new("slot", Type::Int), Slot::new("tag", Type::Int)]);
    let item_slot = p.relation(
        Relation::new("Item.slot", RelationKind::State, [Slot::new("Item", Type::entity("Item")), Slot::new("slot", Type::Int)])
            .mode(Mode::ExactlyOne { key: 0, values: vec![1] }),
    );
    p.relation(
        Relation::new("Item.tag", RelationKind::State, [Slot::new("Item", Type::entity("Item")), Slot::new("tag", Type::Int)])
            .mode(Mode::AtMostOne { keys: vec![0], values: vec![1] }),
    );
    let pos = p.relation(
        Relation::new("pos", RelationKind::State, [Slot::new("key", Type::entity("Key")), Slot::new("at", Type::Int)])
            .mode(Mode::ExactlyOne { key: 0, values: vec![1] })
            .template("{key} is at {at}"),
    );
    for key in &keys {
        p.initially(Fact::new(pos, [key.clone(), Value::Int(0)]));
    }
    let held = p.relation(Relation::new("held", RelationKind::State, [Slot::new("item", Type::entity("Item"))]));
    let mut ints: Vec<(RelationId, usize, bool)> = Vec::new();
    for index in 0..rng.range(2, 4) {
        let arity = if index == 0 { 1 } else { rng.range(1, 2) };
        let moded = arity == 2 && rng.chance(60);
        let mut relation = Relation::new(format!("s{index}"), RelationKind::State, (0..arity).map(|slot| Slot::new(format!("v{slot}"), Type::Int)));
        if moded {
            relation = relation.mode(Mode::AtMostOne { keys: vec![0], values: vec![1] });
        }
        let id = p.relation(relation);
        let mut used = BTreeSet::new();
        for _ in 0..rng.range(0, 3) {
            let values: Vec<Value> = (0..arity).map(|_| Value::Int(rng.below(4) as i64)).collect();
            if !moded || used.insert(values[0].clone()) {
                p.initially(Fact::new(id, values));
            }
        }
        ints.push((id, arity, moded));
    }
    let stated = ints.len();
    for index in 0..rng.range(0, 3) {
        let arity = rng.range(0, 2);
        let mut relation = Relation::new(format!("d{index}"), RelationKind::Derived, (0..arity).map(|slot| Slot::new(format!("v{slot}"), Type::Int)));
        let moded = arity == 2 && rng.chance(25);
        if moded {
            relation = relation.mode(Mode::AtMostOne { keys: vec![0], values: vec![1] });
        }
        ints.push((p.relation(relation), arity, moded));
    }
    for rule in 0..(ints.len() - stated) * rng.range(1, 2) {
        let head = stated + rule % (ints.len() - stated);
        let mut body = Vec::new();
        let mut bound: Vec<String> = Vec::new();
        for _ in 0..rng.range(1, 2) {
            let (relation, arity, _) = *rng.pick(&ints);
            let arguments: Vec<Expr> = (0..arity)
                .map(|_| {
                    if rng.chance(20) {
                        constant(rng)
                    } else {
                        let variable = ["a", "b", "c"][rng.below(3)].to_string();
                        if !bound.contains(&variable) {
                            bound.push(variable.clone());
                        }
                        Expr::var(variable)
                    }
                })
                .collect();
            body.push(Atom::new(relation, arguments).positive());
        }
        if rng.chance(30) {
            let (relation, arity, _) = ints[rng.below(stated)];
            let arguments: Vec<Expr> = (0..arity)
                .map(|_| if bound.is_empty() || rng.chance(20) { constant(rng) } else { Expr::var(rng.pick(&bound).clone()) })
                .collect();
            body.push(Atom::new(relation, arguments).negative());
        }
        if !bound.is_empty() && rng.chance(30) {
            body.push(Literal::compare(*rng.pick(&OPS), Expr::var(rng.pick(&bound).clone()), constant(rng)));
        }
        let (relation, arity, _) = ints[head];
        let arguments: Vec<Expr> = (0..arity)
            .map(|_| if bound.is_empty() || rng.chance(15) { constant(rng) } else { Expr::var(rng.pick(&bound).clone()) })
            .collect();
        p.rule(Rule::new(format!("rule{rule}"), Atom::new(relation, arguments)).when(body));
    }
    let mut events = Vec::new();
    for index in 0..rng.range(2, 4) {
        let event = p.relation(Relation::new(format!("e{index}"), RelationKind::Event, [Slot::new("a", Type::Int), Slot::new("b", Type::Int)]));
        p.event(Event::new(event));
        let mut handler = Handler::new(Atom::new(event, [Expr::var("a"), Expr::var("b")]));
        if rng.chance(20) {
            handler = handler.may_skip();
        }
        for clause_index in 0..rng.range(1, 2) {
            let mut clause = HandlerClause::new();
            let mut bound = vec!["a".to_string(), "b".to_string()];
            let mut item = false;
            for _ in 0..rng.range(0, 2) {
                match rng.below(6) {
                    0 => {
                        let slot = if rng.chance(50) { "a" } else { "x" };
                        clause = clause.when([Atom::new(item_slot, [Expr::var("it"), Expr::var(slot)]).positive()]);
                        item = true;
                        if !bound.contains(&slot.to_string()) {
                            bound.push(slot.into());
                        }
                    }
                    1 => {
                        let (relation, arity, _) = *rng.pick(&ints);
                        let arguments: Vec<Expr> = (0..arity).map(|_| bound_value(rng, &bound[..2])).collect();
                        clause = clause.when([Atom::new(relation, arguments).negative()]);
                    }
                    2 => {
                        clause = clause.when([Literal::compare(*rng.pick(&OPS), Expr::var(rng.pick(&bound[..2]).clone()), constant(rng))]);
                    }
                    _ => {
                        let (relation, arity, _) = *rng.pick(&ints);
                        let arguments: Vec<Expr> = (0..arity)
                            .map(|_| {
                                if rng.chance(40) {
                                    Expr::var("y")
                                } else {
                                    Expr::var(rng.pick(&bound[..2]).clone())
                                }
                            })
                            .collect();
                        if arguments.contains(&Expr::var("y")) && !bound.contains(&"y".to_string()) {
                            bound.push("y".into());
                        }
                        clause = clause.when([Atom::new(relation, arguments).positive()]);
                    }
                }
            }
            if index == 0 && clause_index == 0 {
                clause = clause.action(Action::Include(Atom::new(ints[0].0, [Expr::var("a")])));
            }
            let writable: Vec<(RelationId, usize, bool)> = ints[..stated].to_vec();
            for _ in 0..rng.range(1, 3) {
                let action = match rng.below(10) {
                    0 | 1 => {
                        let moded: Vec<_> = writable.iter().filter(|(_, _, moded)| *moded).collect();
                        if moded.is_empty() || rng.chance(40) {
                            Action::Set(Atom::new(pos, [Expr::Value(rng.pick(&keys).clone()), bound_value(rng, &bound)]))
                        } else {
                            let (relation, arity, _) = **rng.pick(&moded);
                            Action::Set(Atom::new(relation, (0..arity).map(|_| bound_value(rng, &bound))))
                        }
                    }
                    2 | 3 => {
                        let (relation, arity, _) = *rng.pick(&writable);
                        Action::Include(Atom::new(relation, (0..arity).map(|_| bound_value(rng, &bound))))
                    }
                    4 => {
                        if rng.chance(25) {
                            Action::Withdraw(Atom::new(pos, [Expr::Value(rng.pick(&keys).clone()), bound_value(rng, &bound)]))
                        } else {
                            let (relation, arity, _) = *rng.pick(&writable);
                            Action::Withdraw(Atom::new(relation, (0..arity).map(|_| bound_value(rng, &bound))))
                        }
                    }
                    5 | 6 => {
                        let variable = format!("n{}", rng.below(2));
                        let mut fields = BTreeMap::from([("slot".to_string(), bound_value(rng, &bound))]);
                        if rng.chance(50) {
                            fields.insert("tag".to_string(), bound_value(rng, &bound));
                        }
                        if rng.chance(40) {
                            clause = clause.action(Action::Include(Atom::new(held, [Expr::var(variable.clone())])));
                        }
                        Action::Create { variable: Variable::new(variable), entity_type: "Item".into(), fields }
                    }
                    7 if item => {
                        if rng.chance(30) {
                            clause = clause.action(Action::Include(Atom::new(held, [Expr::var("it")])));
                        }
                        Action::Delete(Expr::var("it"))
                    }
                    8 if item => Action::Include(Atom::new(held, [Expr::var("it")])),
                    _ => {
                        let (relation, arity, _) = *rng.pick(&writable);
                        Action::Include(Atom::new(relation, (0..arity).map(|_| bound_value(rng, &bound))))
                    }
                };
                clause = clause.action(action);
            }
            handler = handler.clause(clause);
        }
        p.handler(handler);
        events.push(event);
    }
    let guarded = p.relation(Relation::new("guarded", RelationKind::Event, [Slot::new("x", Type::Int)]));
    p.event(Event::new(guarded));
    let sum = Expr::binary(BinaryOp::Add, Expr::var("x"), Expr::value(rng.range(1, 3) as i64));
    let mut clause = HandlerClause::new().when([Atom::new(ints[0].0, [Expr::var("x")]).positive()]);
    if rng.chance(30) {
        clause = clause.when([Literal::compare(CompareOp::Greater, sum.clone(), Expr::value(0_i64))]);
    }
    let (target, arity, moded) = *rng.pick(&ints[..stated]);
    let mut arguments = vec![sum];
    arguments.extend((1..arity).map(|_| Expr::var("x")));
    let write = Atom::new(target, arguments);
    clause = clause.action(if moded { Action::Set(write) } else { Action::Include(write) });
    p.handler(Handler::new(Atom::new(guarded, [Expr::var("x")])).may_skip().clause(clause));
    for index in 0..[0, 1, 1, 2, 2][rng.below(5)] {
        let (relation, arity, _) = *rng.pick(&ints);
        let variables: Vec<Variable> = (0..arity).map(|slot| Variable::new(format!("v{slot}"))).collect();
        let atom = Atom::new(relation, variables.iter().cloned().map(Expr::Variable));
        let condition = |rng: &mut Rng| -> Vec<Literal> {
            let Some(variable) = variables.first() else { return vec![] };
            let left = if rng.chance(10) { Expr::binary(BinaryOp::Add, Expr::Variable(variable.clone()), Expr::value(1_i64)) } else { Expr::Variable(variable.clone()) };
            vec![Literal::compare(*rng.pick(&OPS), left, constant(rng))]
        };
        let (form, claim) = match rng.below(4) {
            0 => ("sentence", StateClaim::Holds(Atom::new(pos, [Expr::Value(rng.pick(&keys).clone()), Expr::value(0_i64)]))),
            1 => ("some", StateClaim::Some { variables: variables.clone(), body: std::iter::once(atom.clone().positive()).chain(condition(rng)).collect() }),
            2 => ("no", StateClaim::No { variables: variables.clone(), body: std::iter::once(atom.clone().positive()).chain(condition(rng)).collect() }),
            _ => ("every", StateClaim::Every { variables: variables.clone(), domain: vec![atom.clone().positive()], body: condition(rng) }),
        };
        p.requirement(Requirement::invariant(format!("{form}{index}"), claim));
    }
    let scan = (bulk > 0).then(|| {
        let stock = p.relation(Relation::new("stock", RelationKind::State, [Slot::new("v", Type::Int)]));
        let wide = p.relation(Relation::new("wide", RelationKind::Derived, [Slot::new("v", Type::Int)]));
        for value in 0..bulk {
            p.initially(Fact::new(stock, [Value::Int(value as i64)]));
        }
        p.rule(Rule::new("wide", Atom::new(wide, [Expr::var("v")])).when([Atom::new(stock, [Expr::var("v")]).positive()]));
        let scan = p.relation(Relation::new("scan", RelationKind::Event, [Slot::new("x", Type::Int)]));
        p.event(Event::new(scan));
        let clause = HandlerClause::new()
            .when([Atom::new(wide, [Expr::var("y")]).positive(), Literal::compare(CompareOp::Less, Expr::var("y"), Expr::var("x"))])
            .action(Action::Include(Atom::new(ints[0].0, [Expr::var("x")])));
        p.handler(Handler::new(Atom::new(scan, [Expr::var("x")])).may_skip().clause(clause));
        scan
    });
    let clash = clash.then(|| {
        let mark = p.relation(
            Relation::new("mark", RelationKind::State, [Slot::new("key", Type::entity("Key")), Slot::new("at", Type::Int)])
                .mode(Mode::ExactlyOne { key: 0, values: vec![1] })
                .template("{key} is marked {at}"),
        );
        for key in &keys {
            p.initially(Fact::new(mark, [key.clone(), Value::Int(0)]));
        }
        let clash = p.relation(Relation::new("clash", RelationKind::Event, [Slot::new("a", Type::Int), Slot::new("b", Type::Int)]));
        p.event(Event::new(clash));
        let clause = HandlerClause::new()
            .action(Action::Include(Atom::new(pos, [Expr::Value(keys[0].clone()), Expr::var("a")])))
            .action(Action::Include(Atom::new(mark, [Expr::Value(keys[1].clone()), Expr::var("b")])));
        p.handler(Handler::new(Atom::new(clash, [Expr::var("a"), Expr::var("b")])).clause(clause));
        clash
    });
    Generated { program: p.build(), events, guarded, scan, clash }
}

fn continue_both(program: &Program, straight: &mut Run, restored: &mut Run, events: &[Tuple], budget: Budget, label: &str) -> usize {
    let mut cuts = 0;
    for at in 0..events.len() {
        let last = at + 1 == events.len();
        let (mut a, mut b) = if last { (None, None) } else { (Some(straight.clone()), Some(restored.clone())) };
        let a = a.as_mut().unwrap_or(straight);
        let b = b.as_mut().unwrap_or(restored);
        let expected = a.run_all(program, &events[..=at], budget).cloned();
        let actual = b.run_all(program, &events[..=at], budget).cloned();
        let boundary = format!("{label} prefix {}: restored against uninterrupted", at + 1);
        assert_eq!(actual, expected, "{boundary}: outcome, explanation or error");
        assert_eq!(b.last_work, a.last_work, "{boundary}: last_work");
        assert_eq!(*b, *a, "{boundary}: session");
        cuts += usize::from(matches!(expected, Err(EvalError::Unknown(Reason::OutOfBudget { .. }))));
    }
    cuts
}

fn two_failing_keys(program: &Program, state: &State, events: &[Tuple]) -> usize {
    let [(event, arguments)] = events else { return 0 };
    state::step_mode_failures(program, state, *event, arguments, Budget::default()).map_or(0, |failures| usize::from(failures.len() >= 2))
}

fn invariant_failures(program: &Program, state: &State, events: &[Tuple]) -> Option<(String, bool)> {
    let failed = |program: &Program| match state::step(program, state, events[0].0, &events[0].1, Budget::default()) {
        Err(EvalError::Type { at, message }) if at == "step" => message.strip_prefix("The invariant `").and_then(|rest| rest.split('`').next()).map(str::to_string),
        _ => None,
    };
    let first = failed(program)?;
    let mut rest = program.clone();
    rest.requirements.retain(|requirement| requirement.name != first);
    let second = failed(&rest).is_some();
    Some((first.trim_end_matches(char::is_numeric).to_string(), second))
}

#[test]
fn d62_generated_steps_agree_with_the_reference() {
    let pinned = std::env::var("GALILEO_SEED").ok().and_then(|seed| u64::from_str_radix(seed.trim_start_matches("0x"), 16).ok());
    let programs = if pinned.is_some() { 1 } else { PROGRAMS };
    let (mut accepted, mut skipped, mut occurrences, mut rejected, mut failed, mut guarded_empty, mut two_keys) = (0, 0, 0, 0, 0, 0, 0);
    let (mut forms, mut twice): (BTreeMap<String, usize>, usize) = (BTreeMap::new(), 0);
    let restores_wanted = if pinned.is_some() || std::env::var_os("GALILEO_FULL").is_some() { usize::MAX } else { RESTORES };
    let mut restores = (0, 0, 0, 0);
    let mut index = 0u64;
    while accepted < programs {
        let seed = pinned.unwrap_or_else(|| SEED ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        index += 1;
        let mut rng = Rng(seed);
        let mut plan = Rng(seed ^ 0x918b_918b_918b_918b);
        let cap = plan.chance(20).then(|| Budget::work(plan.range(1_000, 4_000) as u64));
        let bulk = cap.map_or(0, |cap| cap.work as usize * plan.range(20, 50) / 100);
        let generated = generate(&mut rng, bulk, plan.chance(50));
        let program = &generated.program;
        if check(program).is_err() {
            assert!(pinned.is_none(), "seed {seed:#x}: {:?}", check(program).err());
            skipped += 1;
            continue;
        }
        accepted += 1;
        let mut agreement = Agreement::new(program);
        let restore_at = (accepted <= restores_wanted).then(|| plan.below(OCCURRENCES));
        let mut restored: Option<Run> = None;
        let mut done = 0;
        while done < OCCURRENCES {
            let length = if rng.chance(80) { 1 } else { rng.range(2, 3) }.min(OCCURRENCES - done);
            let events: Vec<Tuple> = (0..length)
                .map(|_| {
                    if let Some(scan) = generated.scan.filter(|_| plan.chance(15)) {
                        return (scan, vec![Value::Int(plan.range(0, bulk + 1) as i64)]);
                    }
                    if let Some(clash) = generated.clash.filter(|_| plan.chance(15)) {
                        return (clash, vec![Value::Int(plan.below(3) as i64), Value::Int(plan.below(3) as i64)]);
                    }
                    if rng.chance(30) {
                        (generated.guarded, vec![int_argument(&mut rng, 40)])
                    } else {
                        (*rng.pick(&generated.events), vec![int_argument(&mut rng, 10), int_argument(&mut rng, 10)])
                    }
                })
                .collect();
            let label = format!("seed {seed:#x} occurrence {}", done + 1);
            if restored.is_none() && restore_at.is_some_and(|at| done >= at) {
                restored = Some(Run::restore(agreement.run.checkpoint()));
                restores.0 += 1;
            }
            if let (Some(restored), Some(cap)) = (restored.as_mut(), cap) {
                let first = restored.log.len() == agreement.run.log.len() && restored.build_work() == 0;
                restores.3 += two_failing_keys(program, &restored.state, &events);
                restores.2 += continue_both(program, &mut agreement.run, restored, &events, cap, &label);
                if first {
                    restores.1 += usize::from(restored.build_work() > cap.work);
                }
                done += length;
                continue;
            }
            let mut straight = restored.as_ref().map(|_| agreement.run.clone());
            if let (Some(restored), Some(straight)) = (restored.as_mut(), straight.as_mut()) {
                restores.3 += two_failing_keys(program, &restored.state, &events);
                continue_both(program, straight, restored, &events, Budget::default(), &label);
            }
            let before = (agreement.reference.clone(), agreement.run.revision);
            match agreement.batch(&label, &events, Budget::default()) {
                Outcome::Admitted(effects) => {
                    guarded_empty += usize::from(events.iter().all(|(event, _)| *event == generated.guarded) && effects.is_empty() && agreement.reference == before.0 && agreement.run.revision == before.1 + 1);
                }
                Outcome::Rejected(lines) => {
                    rejected += 1;
                    if let [(event, arguments)] = events.as_slice() {
                        two_keys += usize::from(state::step_mode_failures(program, &before.0, *event, arguments, Budget::default()).unwrap().len() >= 2);
                    }
                    if lines[0].starts_with("The invariant `") && events.len() == 1 {
                        let (form, second) = invariant_failures(program, &before.0, &events).expect("the reference rejects on an invariant");
                        *forms.entry(form).or_default() += 1;
                        twice += usize::from(second);
                    }
                }
                Outcome::Failed(_) => failed += 1,
                Outcome::Unknown => panic!("{label}: Unknown under the default budget"),
            }
            if let Some(straight) = straight {
                assert_eq!(straight, agreement.run, "{label}: straight copy");
            }
            done += length;
        }
        occurrences += agreement.steps;
    }
    eprintln!("step_agreement generated programs={accepted} skipped={skipped} occurrences={occurrences} rejections={rejected} evaluation_errors={failed} guarded_admitted_empty={guarded_empty} two_state_key_failures={two_keys} invariant_rejections={forms:?} failing_two={twice} mismatches=0");
    eprintln!("step_agreement restores={} build_over_budget={} budget_cuts={} two_failing_keys={} mismatches=0", restores.0, restores.1, restores.2, restores.3);
}
