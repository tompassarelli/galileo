use std::path::Path;
use std::process::Command;

#[test]
fn spec_9_7_generated_menu_compiles_and_moves_through_runtime_session() {
    let source = format!(
        "{}\n{}",
        include_str!("../../galileo-syntax/tests/menu.gal"),
        r#"
North-App is a North

initially
  North-App has menu kind "files"
  North-App has menu query ""
  North-App has menu selection 0

type MenuNotice = "changed" | "cleared"
effect menu-notice: notify {north: North} with {notice: MenuNotice}
event add-row: {north: North} adds row {key: Text} labelled {label: Text} ordered {order: Int}
on ?north adds row ?key labelled ?label ordered ?order
  then ?row is a new MenuRow {key: ?key, label: ?label, description: "", annotation: "", order: ?order}
  and ?north lists ?row

on ?north moves menu selection by ?delta
  then notify ?north with "changed"
  may skip
"#
    );
    let lowered = galileo_syntax::lower::lower_program(&[("menu", &source)]).unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let read = galileo_syntax::read_program(&[("menu", &source)]).unwrap();
    let generated = galileo_bindgen::rust(&lowered.program, &[&read["menu"].vocabulary]).unwrap();
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/generated-menu");
    std::fs::create_dir_all(&directory).unwrap();
    let runner = format!(
        "{generated}\n{}",
        format!(
            r#"
fn main() {{
    let lowered = galileo_syntax::lower::lower_program(&[("menu", {source:?})]).unwrap();
    assert!(lowered.diagnostics.is_empty());
    let program = galileo_runtime::Program::from(lowered.program);
    let mut session = galileo_runtime::Session::new(&program);
    let north = North::named("North-App");
    for (key, label, order) in [("logs", "Logs", 2), ("files", "Files", 1)] {{
        session.run(Event::AddRow {{ north: north.clone(), key: key.into(), label: label.into(), order }}).unwrap();
    }}
    let initial = session.view::<Menu>((north.clone(),)).unwrap();
    assert_eq!(initial.kind, "files");
    assert_eq!(initial.query, "");
    assert_eq!(initial.selection, 0);
    assert_eq!(initial.rows.len(), 2);
    assert_eq!(initial.rows[0].label(session.state(), &program).unwrap(), "Files");
    assert_eq!(initial.rows[1].label(session.state(), &program).unwrap(), "Logs");
    for (delta, selection, revision) in [(1, 1, 3), (1, 0, 4), (-1, 1, 5)] {{
        let step = session.run(MoveMenu {{ north: north.clone(), delta }}).unwrap();
        assert_eq!(step.revision, revision);
        assert_eq!(step.effects.len(), 1);
        assert_eq!(Effect::from_tuple(step.effects[0].clone(), &program).unwrap(), Effect::MenuNotice {{ north: north.clone(), notice: MenuNotice::Changed }});
        let menu = session.view::<Menu>((north.clone(),)).unwrap();
        assert_eq!(menu.selection, selection);
        assert_eq!(menu.rows, initial.rows);
    }}
    session.run(Event::ClearMenuRows {{ north: north.clone() }}).unwrap();
    assert!(session.view::<Menu>((north.clone(),)).unwrap().rows.is_empty());
    session.run(Event::MoveMenu {{ north: north.clone(), delta: 1 }}).unwrap();
    assert_eq!(session.view::<Menu>((north,)).unwrap().selection, 1);
    assert!(Menu::from_value(Value::Text("bad projection".into())).is_err());
    println!("generated_menu: rows=2 moves=4 revisions=7 wrap_checks=2 typed_effects=3 malformed_projection=rejected");
}}
"#
        )
    );
    let file = directory.join("main.rs");
    std::fs::write(&file, runner).unwrap();
    let dependencies = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let executable = directory.join("generated-menu");
    let mut compiler = Command::new("rustc");
    compiler
        .arg("--edition=2021")
        .arg(&file)
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("-o")
        .arg(&executable);
    for name in ["galileo_core", "galileo_runtime", "galileo_syntax"] {
        let library = std::fs::read_dir(&dependencies)
            .unwrap()
            .map(Result::unwrap)
            .filter(|entry| {
                let file = entry.file_name();
                let file = file.to_string_lossy();
                file.starts_with(&format!("lib{name}-")) && file.ends_with(".rlib")
            })
            .max_by_key(|entry| entry.metadata().unwrap().modified().unwrap())
            .unwrap();
        compiler
            .arg("--extern")
            .arg(format!("{name}={}", library.path().display()));
    }
    let compiled = compiler.output().unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let run = Command::new(executable).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    print!("{}", String::from_utf8_lossy(&run.stdout));
}
