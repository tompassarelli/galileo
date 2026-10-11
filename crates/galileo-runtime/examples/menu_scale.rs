use galileo_core::{engine, Budget, Entity, Fact, RelationKind};
use galileo_runtime::{Error, EvalError, Fields, Program, Projection, Session, Tuple, Value, View};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

pub struct MenuProbe {
    pub kind: String,
    pub title: String,
    pub query: String,
    pub help: String,
    pub empty: String,
    pub selection: i64,
    pub rows: Arc<[MenuRowProbe]>,
}

pub struct MenuRowProbe {
    pub row: Entity,
    pub key: String,
    pub label: String,
    pub description: String,
    pub annotation: String,
    pub position: i64,
}

fn invalid(field: &str) -> EvalError {
    EvalError::Type {
        at: field.into(),
        message: "invalid menu projection".into(),
    }
}

fn record(value: &Value) -> Result<&BTreeMap<String, Value>, EvalError> {
    match value {
        Value::Record(fields) => Ok(fields),
        _ => Err(invalid("record")),
    }
}

fn field<'a>(value: Option<&'a Value>, name: &str) -> Result<&'a Value, EvalError> {
    value.ok_or_else(|| invalid(name))
}

fn text(value: Option<&Value>, name: &str) -> Result<String, EvalError> {
    match field(value, name)? {
        Value::Text(value) => Ok(value.clone()),
        _ => Err(invalid(name)),
    }
}

fn integer(value: Option<&Value>, name: &str) -> Result<i64, EvalError> {
    match field(value, name)? {
        Value::Int(value) => Ok(*value),
        _ => Err(invalid(name)),
    }
}

impl MenuRowProbe {
    fn decode(value: &Value) -> Result<Self, EvalError> {
        let fields = record(value)?;
        let mut row = None;
        let mut key = None;
        let mut label = None;
        let mut description = None;
        let mut annotation = None;
        let mut position = None;
        for (name, value) in fields {
            match name.as_str() {
                "row" => row = Some(value),
                "key" => key = Some(value),
                "label" => label = Some(value),
                "description" => description = Some(value),
                "annotation" => annotation = Some(value),
                "position" => position = Some(value),
                _ => {}
            }
        }
        let row = match field(row, "row")? {
            Value::Entity(row) if row.entity_type == "north-menu/MenuRow" => row.clone(),
            _ => return Err(invalid("row")),
        };
        Ok(Self {
            row,
            key: text(key, "key")?,
            label: text(label, "label")?,
            description: text(description, "description")?,
            annotation: text(annotation, "annotation")?,
            position: integer(position, "position")?,
        })
    }
}

impl Projection for MenuProbe {
    type Arguments = Entity;

    fn view(program: &Program) -> &View {
        program
            .views
            .iter()
            .find(|view| view.name == "north-menu/Menu")
            .unwrap()
    }

    fn arguments(north: Entity) -> Vec<Value> {
        vec![Value::Entity(north)]
    }

    fn decode(fields: &mut Fields<'_>) -> Result<Self, EvalError> {
        Ok(Self {
            kind: fields.field("kind", |value| text(Some(value), "kind"))?,
            title: fields.field("title", |value| text(Some(value), "title"))?,
            query: fields.field("query", |value| text(Some(value), "query"))?,
            help: fields.field("help", |value| text(Some(value), "help"))?,
            empty: fields.field("empty", |value| text(Some(value), "empty"))?,
            selection: fields.field("selection", |value| integer(Some(value), "selection"))?,
            rows: fields.field("rows", |value| match value {
                Value::List(rows) => rows
                    .iter()
                    .map(MenuRowProbe::decode)
                    .collect::<Result<Vec<_>, _>>()
                    .map(Arc::from),
                _ => Err(invalid("rows")),
            })?,
        })
    }
}

fn event(program: &Program, north: &Entity, name: &str, values: Vec<Value>) -> Tuple {
    let relation = program.relation_id(&format!("north-menu/{name}")).unwrap();
    let mut arguments = vec![Value::Entity(north.clone())];
    arguments.extend(values);
    (relation, arguments)
}

fn setup(program: &Program, north: &Entity, rows: usize) -> Vec<Tuple> {
    let mut events = vec![
        event(program, north, "clear-menu-rows", vec![]),
        event(
            program,
            north,
            "open-settings-menu",
            vec![Value::text("config"), Value::text("Context Switchboard")],
        ),
    ];
    events.extend((0..rows).map(|i| {
        event(
            program,
            north,
            "offer-row",
            vec![
                Value::text(format!("row-{i}")),
                Value::text(format!("Row {i}")),
                Value::text("Settings entry"),
                Value::text("on"),
            ],
        )
    }));
    events.push(event(program, north, "query-menu", vec![Value::text("")]));
    events
}

fn evaluate(program: &Program, session: &Session<'_>) -> Result<galileo_core::Model, String> {
    let mut current = (**program).clone();
    current.entities.retain(|entity| {
        !program.entity_types.iter().any(|ty| ty.state && ty.name == entity.entity_type)
    });
    current.entities.extend(session.state().live.iter().cloned());
    let facts: Vec<_> = session.state().facts.iter()
        .map(|(relation, values)| Fact::new(*relation, values.clone())).collect();
    engine::evaluate(&current, &facts, Budget::work(u64::MAX))
        .map_err(|error| format!("{error:?}"))
}

fn percentile(samples: &mut [u128], percent: usize) -> u128 {
    samples.sort_unstable();
    samples[(samples.len() * percent).div_ceil(100).saturating_sub(1)]
}

fn run() -> Result<(), String> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let diagnostic = arguments.first().is_some_and(|arg| arg == "--eval");
    let step_diagnostic = arguments.first().is_some_and(|arg| arg == "--steps");
    let usage = "usage: menu_scale ROWS STEPS | menu_scale --eval ROWS | menu_scale --steps ROWS";
    if arguments.len() != 2 {
        return Err(usage.into());
    }
    let rows: usize = arguments[usize::from(diagnostic || step_diagnostic)]
        .parse()
        .map_err(|_| usage)?;
    let lowered = galileo_syntax::lower::lower_program(&[(
        "north-menu",
        include_str!("../../../examples/north-menu.gal"),
    )])
    .map_err(|error| error.to_string())?;
    if !lowered.diagnostics.is_empty() {
        return Err(format!("{:?}", lowered.diagnostics));
    }
    let program = Program::from(lowered.program);
    let north = program
        .entities
        .iter()
        .find(|entity| entity.name == "north-main")
        .unwrap()
        .clone();
    let events = setup(&program, &north, rows);
    if diagnostic {
        let mut copy = (*program).clone();
        copy.rules.clear();
        for relation in &mut copy.relations {
            if relation.kind == RelationKind::Derived {
                relation.modes.clear();
            }
        }
        let copy = Program::from(copy);
        let mut session = Session::new(&copy);
        session
            .run_all(&events)
            .map_err(|error| format!("{error:?}"))?;
        let model = evaluate(&program, &session)?;
        println!("eval_work={}", black_box(model).work);
        let mut samples = Vec::with_capacity(6);
        for i in 0..6 {
            let query = event(
                &copy,
                &north,
                "query-menu",
                vec![Value::text(if i % 2 == 0 { "a" } else { "" })],
            );
            let started = Instant::now();
            black_box(session.run(query).map_err(|error| format!("{error:?}"))?);
            samples.push(started.elapsed().as_nanos());
        }
        println!(
            "overhead_step_ms={:.3}",
            percentile(&mut samples, 50) as f64 / 1_000_000.0
        );
        return Ok(());
    }
    let steps: usize = if step_diagnostic { 1 } else { arguments[1].parse().map_err(|_| usage)? };
    if steps == 0 {
        return Err("STEPS must be positive".into());
    }
    let mut session = Session::new(&program);
    let started = Instant::now();
    session
        .run_all(&events)
        .map_err(|error| format!("{error:?}"))?;
    println!("setup_ms={:.3}", started.elapsed().as_secs_f64() * 1000.0);
    if step_diagnostic {
        let before = evaluate(&program, &session)?;
        let occurrences = [
            ("offer", event(&program, &north, "offer-settings-row", vec![
                Value::text(format!("row-{rows}")), Value::text(format!("Row {rows}")),
                Value::text("Settings entry"), Value::text("on"),
            ])),
            ("move", event(&program, &north, "move-menu", vec![Value::Int(1)])),
            ("query", event(&program, &north, "query-menu", vec![Value::text("row-1")])),
            ("accept", event(&program, &north, "accept-menu", vec![])),
        ];
        for (name, occurrence) in occurrences {
            let mut changed = session.clone();
            let started = Instant::now();
            black_box(changed.run(occurrence.clone()).map_err(|error| format!("{error:?}"))?);
            let elapsed = started.elapsed().as_nanos();
            let work = changed.last_work();
            if name == "query" {
                let started = Instant::now();
                black_box(changed.view::<MenuProbe>(north.clone()).map_err(|error| format!("{error:?}"))?);
                println!("query_view_ns={}", started.elapsed().as_nanos());
            }
            let after = evaluate(&program, &changed)?;
            let mut tuples_changed = session.state().facts.symmetric_difference(&changed.state().facts).count();
            for (index, relation) in program.relations.iter().enumerate() {
                if relation.kind == RelationKind::Derived {
                    let id = galileo_core::RelationId(index);
                    let old: std::collections::BTreeSet<_> = before.tuples(id).collect();
                    let new: std::collections::BTreeSet<_> = after.tuples(id).collect();
                    tuples_changed += old.symmetric_difference(&new).count();
                }
            }
            println!("{name}_work={work}");
            println!("{name}_changed={tuples_changed}");
            println!("{name}_full={}", after.work);
            println!("{name}_ns={elapsed}");
            for (suffix, budget) in [("at_work", work), ("below_work", work.saturating_sub(1))] {
                let mut bounded = session.clone();
                let outcome = match bounded.run_with_budget(occurrence.clone(), Budget::work(budget)) {
                    Ok(_) => "admitted",
                    Err(Error::Evaluation(EvalError::Unknown(_))) => "unknown",
                    Err(error) => return Err(format!("{name}_{suffix}: {error:?}")),
                };
                println!("{name}_{suffix}={outcome}");
                if suffix == "below_work" {
                    println!("{name}_{suffix}_last_work={}", bounded.last_work());
                    assert_eq!(bounded.last_work(), budget);
                }
            }
        }
        return Ok(());
    }
    let mut moves = Vec::with_capacity(steps);
    let mut views = Vec::with_capacity(steps);
    let mut totals = Vec::with_capacity(steps);
    for _ in 0..steps {
        let movement = event(&program, &north, "move-menu", vec![Value::Int(1)]);
        let started = Instant::now();
        black_box(
            session
                .run(movement)
                .map_err(|error| format!("{error:?}"))?,
        );
        let moved = started.elapsed().as_nanos();
        let viewed = Instant::now();
        black_box(
            session
                .view::<MenuProbe>(north.clone())
                .map_err(|error| format!("{error:?}"))?,
        );
        views.push(viewed.elapsed().as_nanos());
        moves.push(moved);
        totals.push(started.elapsed().as_nanos());
    }
    println!("move_median_ns={}", percentile(&mut moves, 50));
    println!("view_median_ns={}", percentile(&mut views, 50));
    println!("step_median_ns={}", percentile(&mut totals, 50));
    println!("step_p95_ns={}", percentile(&mut totals, 95));
    let movement = event(&program, &north, "move-menu", vec![Value::Int(1)]);
    println!(
        "small_budget_move={}",
        session
            .run_with_budget(movement, Budget::work(1_000))
            .is_ok()
    );
    let query = event(&program, &north, "query-menu", vec![Value::text("x")]);
    println!(
        "small_budget_query={}",
        session.run_with_budget(query, Budget::work(1_000)).is_ok()
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
