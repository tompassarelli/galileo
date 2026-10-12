use galileo_syntax::read_program;

fn main() {
    let source = "type Hero\nprivate type Room\ngiven {hero: Hero} is in {room: Room}\n";
    let (module, error) = read_program(&[("level", source)]).unwrap_err();
    assert_eq!(module, "level");
    assert_eq!(error.line, 3);
    assert!(error.message.contains("Room"));
    read_program(&[("level", &source.replace("given ", "private given "))]).unwrap();
    read_program(&[("level", &source.replace("private type Room", "type Room"))]).unwrap();

    let source = "private type Room\nstate entity Hero\n  room: Room\n";
    let (_, error) = read_program(&[("level", source)]).unwrap_err();
    assert_eq!(error.line, 3);
    assert!(error.message.contains("room"));
    assert!(error.message.contains("Room"));
    read_program(&[("level", &source.replace("state entity", "private state entity"))]).unwrap();
    read_program(&[("level", &source.replace("private type Room", "type Room"))]).unwrap();

    let source = "private type Room\ndefine room(value: List Room): Text\n  \"room\"\n";
    let (_, error) = read_program(&[("level", source)]).unwrap_err();
    assert_eq!(error.line, 2);
    assert!(error.message.contains("Room"));
    read_program(&[("level", &source.replace("define ", "private define "))]).unwrap();

    let source = "private type Room\ndefine room(): maybe Room\n  null\n";
    let (_, error) = read_program(&[("level", source)]).unwrap_err();
    assert_eq!(error.line, 2);
    assert!(error.message.contains("Room"));
    read_program(&[("level", &source.replace("define ", "private define "))]).unwrap();
    println!("2.10d: 10 public-interface outcomes passed");
}

