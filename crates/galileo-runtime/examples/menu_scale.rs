use galileo_core::{engine, Budget, Entity, Fact, RelationKind};
use galileo_runtime::{EvalError, Program, Projection, Session, Tuple, Value, View};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Instant;

pub struct MenuProbe {
    pub kind: String,
    pub title: String,
    pub query: String,
    pub help: String,
    pub empty: String,
    pub selection: i64,
    pub rows: Vec<MenuRowProbe>,
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

fn field<'a>(fields: &'a BTreeMap<String, Value>, name: &str) -> Result<&'a Value, EvalError> {
    fields.get(name).ok_or_else(|| invalid(name))
}

fn text(fields: &BTreeMap<String, Value>, name: &str) -> Result<String, EvalError> {
    match field(fields, name)? {
        Value::Text(value) => Ok(value.clone()),
        _ => Err(invalid(name)),
    }
}

fn integer(fields: &BTreeMap<String, Value>, name: &str) -> Result<i64, EvalError> {
    match field(fields, name)? {
        Value::Int(value) => Ok(*value),
        _ => Err(invalid(name)),
    }
}

impl MenuRowProbe {
    fn decode(value: &Value) -> Result<Self, EvalError> {
        let fields = record(value)?;
        let row = match field(fields, "row")? {
            Value::Entity(row) if row.entity_type == "north-menu/MenuRow" => row.clone(),
            _ => return Err(invalid("row")),
        };
        Ok(Self {
            row,
            key: text(fields, "key")?,
            label: text(fields, "label")?,
            description: text(fields, "description")?,
            annotation: text(fields, "annotation")?,
            position: integer(fields, "position")?,
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

    fn decode(value: &Value) -> Result<Self, EvalError> {
        let fields = record(value)?;
        let rows = match field(fields, "rows")? {
            Value::List(rows) => rows
                .iter()
                .map(MenuRowProbe::decode)
                .collect::<Result<_, _>>()?,
            _ => return Err(invalid("rows")),
        };
        Ok(Self {
            kind: text(fields, "kind")?,
            title: text(fields, "title")?,
            query: text(fields, "query")?,
            help: text(fields, "help")?,
            empty: text(fields, "empty")?,
            selection: integer(fields, "selection")?,
            rows,
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
            "offer-settings-row",
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

fn percentile(samples: &mut [u128], percent: usize) -> u128 {
    samples.sort_unstable();
    samples[(samples.len() * percent).div_ceil(100).saturating_sub(1)]
}

fn run() -> Result<(), String> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let diagnostic = arguments.first().is_some_and(|arg| arg == "--eval");
    let usage = "usage: menu_scale ROWS STEPS | menu_scale --eval ROWS";
    if arguments.len() != 2 {
        return Err(usage.into());
    }
    let rows: usize = arguments[usize::from(diagnostic)]
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
        let mut current = (*program).clone();
        current.entities.retain(|entity| {
            !program
                .entity_types
                .iter()
                .any(|ty| ty.state && ty.name == entity.entity_type)
        });
        current
            .entities
            .extend(session.state().live.iter().cloned());
        let facts: Vec<_> = session
            .state()
            .facts
            .iter()
            .map(|(relation, values)| Fact::new(*relation, values.clone()))
            .collect();
        let model = engine::evaluate(&current, &facts, Budget::work(u64::MAX))
            .map_err(|error| format!("{error:?}"))?;
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
    let steps: usize = arguments[1].parse().map_err(|_| usage)?;
    if steps == 0 {
        return Err("STEPS must be positive".into());
    }
    let mut session = Session::new(&program);
    let started = Instant::now();
    session
        .run_all(&events)
        .map_err(|error| format!("{error:?}"))?;
    println!("setup_ms={:.3}", started.elapsed().as_secs_f64() * 1000.0);
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
