use clause_package::{Term, decode_canonical_term_bytes};
use clause_runtime::{ExecutableValueV1, projected_text_value_v1};
use clause_workbench::{ResidentSourceWorkbenchV1, render_authoring_card_v1};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    error::Error,
    fs,
    io::{self, Write},
    path::Path,
};

#[derive(Deserialize)]
struct Input {
    id: String,
    text: String,
    query: String,
}

fn field<'a>(mut term: &'a Term, name: &[u8]) -> Result<&'a Term, Box<dyn Error>> {
    while let Some(triple) = term.as_triple() {
        let [key, value, tail] = triple.slots();
        if key
            .as_atom()
            .is_some_and(|atom| atom.canonical_payload() == name)
        {
            return Ok(value);
        }
        term = tail;
    }
    Err(format!("projection lacks {}", String::from_utf8_lossy(name)).into())
}

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "authoring-card")
    {
        print!("{}", render_authoring_card_v1());
        return Ok(());
    }
    let directory = Path::new(arguments.first().ok_or("expected fixture directory")?);
    let source = fs::read(directory.join("source.clause"))?;
    let mut workbench = ResidentSourceWorkbenchV1::open_continuous(&source)?;
    if arguments
        .get(1)
        .is_some_and(|argument| argument == "check-source")
    {
        println!("reading, elaboration, lowering and session opening: accepted");
        return Ok(());
    }
    let inputs: Vec<Input> = serde_json::from_slice(&fs::read(directory.join("inputs.json"))?)?;
    let mut output = io::BufWriter::new(io::stdout().lock());
    for input in inputs {
        let occurrence = workbench.handler_occurrence(
            b"record",
            &[
                ExecutableValueV1::text(&input.text)?,
                ExecutableValueV1::text(&input.query)?,
            ],
        )?;
        workbench.run_occurrences_to_candidate(&[occurrence])?;
        let admission = workbench.admit()?;
        let term = decode_canonical_term_bytes(&admission.projection.exact_term_bytes())?;
        let document = field(&term, b"document")?;
        for (function, relation, binary) in [
            ("lowercase", "lowered", false),
            ("contains-text", "matched", true),
            ("trim", "cleaned", false),
            ("first-word", "first", false),
            ("remaining-words", "remaining", false),
            ("starts-with", "prefixed", true),
        ] {
            let value = field(document, relation.as_bytes())?;
            let result = if let Some(text) = projected_text_value_v1(value)? {
                Value::String(text.to_owned())
            } else {
                match value.as_atom().map(|atom| atom.canonical_payload()) {
                    Some([0]) => Value::Bool(false),
                    Some([1]) => Value::Bool(true),
                    _ => return Err("expected projected Text or Boolean".into()),
                }
            };
            let arguments = if binary {
                json!([input.text, input.query])
            } else {
                json!([input.text])
            };
            serde_json::to_writer(
                &mut output,
                &json!({
                    "id": format!("{}:{function}", input.id),
                    "function": function,
                    "arguments": arguments,
                    "output": result,
                }),
            )?;
            writeln!(output)?;
        }
    }
    output.flush()?;
    Ok(())
}
