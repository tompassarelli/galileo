use std::path::{Path, PathBuf};
use std::process::Command;

use muove_syntax::check::distance;
use muove_syntax::schema::Schema;

const EXPRESSION: &str = include_str!("schema.nix");

pub enum Error {
    Usage,
    Failed(String),
}

pub fn extract(arguments: &[String]) -> Result<(), Error> {
    let (mut host, mut flake, mut out) = (None, String::from("."), None);
    let mut rest = arguments.iter();
    while let Some(flag) = rest.next() {
        let value = rest.next().ok_or(Error::Usage)?.clone();
        match flag.as_str() {
            "--host" => host = Some(value),
            "--flake" => flake = value,
            "--out" => out = Some(value),
            _ => return Err(Error::Usage),
        }
    }
    let host = host.ok_or(Error::Usage)?;
    let root = std::fs::canonicalize(&flake).map_err(|e| Error::Failed(format!("{flake}: {e}")))?;
    let out = out.map(PathBuf::from).unwrap_or_else(|| root.join(".beagle-cache/schema.json"));
    let scratch = std::env::temp_dir().join(format!("muove-schema-{}.nix", std::process::id()));
    std::fs::write(&scratch, EXPRESSION).map_err(|e| Error::Failed(format!("{}: {e}", scratch.display())))?;
    eprintln!("muove schema extract: evaluating nixosConfigurations.{host}.options ...");
    let result = Command::new("nix")
        .args(["eval", "--impure", "--json", "--expr"])
        .arg(format!(
            "import {} {{ flake = {}; host = {}; }}",
            nix_string(&scratch.display().to_string()),
            nix_string(&root.display().to_string()),
            nix_string(&host)
        ))
        .output();
    let _ = std::fs::remove_file(&scratch);
    let result = result.map_err(|e| Error::Failed(format!("nix: {e}")))?;
    if !result.status.success() {
        return Err(Error::Failed(format!("muove schema extract: nix eval failed\n{}", String::from_utf8_lossy(&result.stderr))));
    }
    if out == Path::new("-") {
        use std::io::Write;
        return std::io::stdout().write_all(&result.stdout).map_err(|e| Error::Failed(e.to_string()));
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::Failed(format!("{}: {e}", parent.display())))?;
    }
    let staged = out.with_extension("json.tmp");
    std::fs::write(&staged, &result.stdout)
        .and_then(|()| std::fs::rename(&staged, &out))
        .map_err(|e| Error::Failed(format!("{}: {e}", out.display())))?;
    eprintln!("muove schema extract: wrote {}", out.display());
    Ok(())
}

fn nix_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\"").replace('$', "\\$"))
}

pub fn explain(arguments: &[String]) -> Result<(), Error> {
    let (mut path, mut schema) = (None, String::from(".beagle-cache/schema.json"));
    let mut rest = arguments.iter();
    while let Some(argument) = rest.next() {
        match argument.as_str() {
            "--schema" => schema = rest.next().ok_or(Error::Usage)?.clone(),
            _ if path.is_none() && !argument.starts_with("--") => path = Some(argument.clone()),
            _ => return Err(Error::Usage),
        }
    }
    let path = path.ok_or(Error::Usage)?;
    let text = std::fs::read_to_string(&schema).map_err(|e| {
        Error::Failed(format!("{schema}: {e}\nmuove schema explain: run `muove schema extract --host HOST` first"))
    })?;
    let schema = Schema::parse(&schema, &text).map_err(Error::Failed)?;
    let Some(option) = schema.get(&path) else {
        let mut ranked: Vec<(usize, &str)> =
            schema.options.values().map(|o| (distance(&path, &o.path), o.path.as_str())).collect();
        ranked.sort();
        let near: Vec<&str> = ranked.iter().take(5).filter(|(d, _)| *d <= path.len() / 3 + 1).map(|(_, p)| *p).collect();
        let mut message = format!("no option `{path}` in {}", schema.source);
        if !near.is_empty() {
            message.push_str("\ndid you mean:");
            for p in near {
                message.push_str(&format!("\n  {p}"));
            }
        }
        return Err(Error::Failed(message));
    };
    println!("path:  {}", option.path);
    println!("type:  {}", option.ty);
    if !option.values.is_empty() {
        let quoted: Vec<String> = option.values.iter().map(|v| format!("\"{v}\"")).collect();
        println!("enum:  {}", quoted.join(", "));
    }
    if !option.declarations.is_empty() {
        println!("declared in:");
        for declaration in &option.declarations {
            println!("  {declaration}");
        }
    }
    println!("from:  {}", schema.source);
    Ok(())
}
