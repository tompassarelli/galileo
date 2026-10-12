use galileo_core::Entity;
use galileo_runtime::{Program, Session, Tuple, Value};

fn event(program: &Program, north: &Entity, name: &str, values: Vec<Value>) -> Tuple {
    let relation = program.relation_id(&format!("north-menu/{name}")).unwrap();
    let mut arguments = vec![Value::Entity(north.clone())];
    arguments.extend(values);
    (relation, arguments)
}

fn offer(program: &Program, north: &Entity, i: usize) -> Tuple {
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
}

fn step_work(program: &Program, north: &Entity, rows: usize) -> (u64, u64, usize) {
    let mut events = vec![
        event(program, north, "clear-menu-rows", vec![]),
        event(
            program,
            north,
            "open-settings-menu",
            vec![Value::text("config"), Value::text("Context Switchboard")],
        ),
    ];
    events.extend((0..rows).map(|i| offer(program, north, i)));
    events.push(event(program, north, "query-menu", vec![Value::text("")]));
    let mut session = Session::new(program);
    session.run_all(&events).unwrap();
    let mut offered = session.clone();
    offered.run(offer(program, north, rows)).unwrap();
    session
        .run(event(program, north, "move-menu", vec![Value::Int(1)]))
        .unwrap();
    (offered.last_work(), session.last_work(), offered.last_untracked_rows())
}

#[test]
fn a_step_costs_the_same_work_at_200_and_2000_menu_rows() {
    let lowered = galileo_syntax::lower::lower_program(&[(
        "north-menu",
        include_str!("../../../examples/north-menu.gal"),
    )])
    .unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let program = Program::from(lowered.program);
    let north = program
        .entities
        .iter()
        .find(|entity| entity.name == "north-main")
        .unwrap()
        .clone();
    let (small_offer, small_move, small_untracked) = step_work(&program, &north, 200);
    let (large_offer, large_move, large_untracked) = step_work(&program, &north, 2_000);
    eprintln!("offer_work 200={small_offer} 2000={large_offer}; move_work 200={small_move} 2000={large_move}; offer_untracked 200={small_untracked} 2000={large_untracked}");
    assert!(small_offer <= 500 && large_offer <= 500);
    assert!(small_offer.abs_diff(large_offer) * 10 <= small_offer.min(large_offer));
    assert!(small_move <= 100 && large_move <= 100);
}
