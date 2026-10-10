use muove_core::{check_claims, claims, examples, exit_code, Budget, Program};
use muove_syntax::check::{analyze, SourceFile};
use muove_syntax::record::RecordDecl;
use muove_syntax::schema::Schema;
mod diff;
mod schema;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const USAGE: &str = "usage: muove check FIXTURE|PATH... [--budget WORK]\n       muove build [--check] PATH...\n       muove build --stdout FILE\n       muove refs OPTION [PATH...]\n       muove lsp\n       muove schema extract --host HOST [--flake DIR] [--out FILE|-]\n       muove schema explain PATH [--schema FILE]\n       muove fmt [--check] [PATH...]\n       muove help [syntax]\nfixtures: gate, gate-never, chain, dungeon-static, tags, r13";
const SYNTAX_CARD: &str = include_str!("../help/syntax.txt");
const NIX_LIB: &str = include_str!("../../../lib/firn/nix-lib.muove");

fn fixture(name: &str) -> Option<Program> {
    Some(match name {
        "gate" => examples::gate(),
        "gate-never" => examples::gate_never(),
        "chain" => examples::chain(101),
        "dungeon-static" => examples::dungeon_static(),
        "tags" => examples::tags(),
        "r13" => examples::r13(),
        _ => return None,
    })
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let [command, sub, rest @ ..] = arguments.as_slice() {
        if command == "schema" && (sub == "extract" || sub == "explain") {
            let result = if sub == "extract" { schema::extract(rest) } else { schema::explain(rest) };
            return match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(schema::Error::Usage) => {
                    eprintln!("{USAGE}");
                    ExitCode::from(2)
                }
                Err(schema::Error::Failed(message)) => {
                    eprintln!("{message}");
                    ExitCode::from(1)
                }
            };
        }
    }
    match arguments.as_slice() {
        [command] if command == "help" => {
            println!("{USAGE}\ntopics: syntax");
            return ExitCode::SUCCESS;
        }
        [command, topic] if command == "help" => {
            if topic == "syntax" {
                print!("{SYNTAX_CARD}");
                return ExitCode::SUCCESS;
            }
            eprintln!("no help topic `{topic}`; topics: syntax");
            return ExitCode::from(2);
        }
        [command] if command == "lsp" => {
            return match muove_lsp::run(&[("lib/firn/nix-lib.muove", NIX_LIB)]) {
                Ok(()) => ExitCode::SUCCESS,
                Err(message) => {
                    eprintln!("{message}");
                    ExitCode::from(1)
                }
            };
        }
        [command, name, paths @ ..] if command == "refs" => {
            let default = [".".to_string()];
            return match refs(name, if paths.is_empty() { &default } else { paths }) {
                Ok(_) => ExitCode::SUCCESS,
                Err(message) => {
                    eprintln!("{message}");
                    ExitCode::from(1)
                }
            };
        }
        _ => {}
    }
    if let [command, rest @ ..] = arguments.as_slice() {
        if command == "fmt" {
            return fmt(rest);
        }
        if command == "build" {
            let result = match rest {
                [flag, file] if flag == "--stdout" => stdout(file),
                [flag, paths @ ..] if flag == "--check" && !paths.is_empty() => check_built(paths),
                paths if !paths.is_empty() && !paths[0].starts_with("--") => build(paths).map(|()| true),
                _ => {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                }
            };
            return match result {
                Ok(true) => ExitCode::SUCCESS,
                Ok(false) => ExitCode::from(1),
                Err(message) => {
                    eprintln!("{message}");
                    ExitCode::from(1)
                }
            };
        }
    }
    let (program, budget) = match arguments.as_slice() {
        [command, rest @ ..] if command == "check" && !rest.is_empty() => {
            let (names, budget) = match rest {
                [names @ .., flag, work] if flag == "--budget" => {
                    (names, work.parse().ok().map(Budget::work))
                }
                names => (names, Some(Budget::default())),
            };
            let program = match names {
                [name] if fixture(name).is_some() => fixture(name).ok_or_else(String::new),
                [] => Err(String::new()),
                paths => source_program(paths),
            };
            match (program, budget) {
                (Ok(program), Some(budget)) => (program, budget),
                (Err(message), _) if !message.is_empty() => {
                    eprintln!("{message}");
                    return ExitCode::from(2);
                }
                _ => {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                }
            }
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match check_claims(&program, budget) {
        Ok(results) => {
            for result in &results {
                println!("{}", claims::sentence(result));
                if let Some(instance) = claims::counterexample(&program, result) {
                    println!("  {instance}");
                }
            }
            ExitCode::from(exit_code(&results) as u8)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn source_program(paths: &[String]) -> Result<Program, String> {
    let mut found = Vec::new();
    for path in paths {
        sources(Path::new(path), &mut found)?;
    }
    let mut texts = Vec::new();
    for path in &found {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        texts.push((path.display().to_string(), text));
    }
    let files: Vec<(&str, &str)> = texts.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let lowered = muove_syntax::lower::lower_files(&files).map_err(|e| e.to_string())?;
    if !lowered.diagnostics.is_empty() {
        return Err(lowered.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"));
    }
    Ok(lowered.program)
}

fn sources(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<_, _>>()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        entries.sort();
        for entry in entries {
            let hidden = entry.file_name().is_some_and(|name| {
                name.to_string_lossy().starts_with('.') || name == "target"
            });
            if entry.is_dir() && !hidden || entry.extension().is_some_and(|ext| ext == "muove") {
                sources(&entry, out)?;
            }
        }
    } else {
        out.push(path.to_path_buf());
    }
    Ok(())
}

fn load(paths: &[String]) -> Result<(Vec<PathBuf>, Vec<SourceFile>, Schema), String> {
    let mut found = Vec::new();
    for path in paths {
        sources(Path::new(path), &mut found)?;
    }
    let mut files = vec![SourceFile::parse("lib/firn/nix-lib.muove", NIX_LIB).map_err(|e| e.to_string())?];
    for path in &found {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        files.push(SourceFile::parse(&path.display().to_string(), &text).map_err(|e| e.to_string())?);
    }
    let mut schema = Schema::default();
    for file in &files {
        for foreign in &file.foreign {
            let base = Path::new(&file.name).parent().unwrap_or(Path::new(""));
            let path = base.join(&foreign.path);
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("{}:{}: foreign source {}: {e}", file.name, foreign.line, path.display()))?;
            let loaded = Schema::parse(&path.display().to_string(), &text)?;
            schema.source = loaded.source;
            schema.options.extend(loaded.options);
        }
    }
    Ok((found, files, schema))
}

fn refs(name: &str, paths: &[String]) -> Result<bool, String> {
    let (_, files, schema) = load(paths)?;
    let (_, index) = analyze(&files, &schema);
    let path: Vec<String> = name.split('.').map(String::from).collect();
    let Some(declared) = index.options.get(&path) else {
        return Err(format!("`{name}` isn't a declared option"));
    };
    println!("{}:{}: option {name}", declared.file, declared.line);
    for typed in index.uses(&path) {
        let text = &files.iter().find(|f| f.name == typed.file).expect("indexed file").text;
        let line_start = text[..typed.start].rfind('\n').map_or(0, |i| i + 1);
        let line = text[..typed.start].matches('\n').count() + 1;
        let column = text[line_start..typed.start].chars().count() + 1;
        println!("{}:{line}:{column}: {}", typed.file, &text[typed.start..typed.end]);
    }
    Ok(true)
}

fn emit(paths: &[String]) -> Result<Vec<(PathBuf, String)>, String> {
    let (found, files, schema) = load(paths)?;
    let (diagnostics, index) = analyze(&files, &schema);
    if !diagnostics.is_empty() {
        return Err(diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"));
    }
    let none = Default::default();
    let answered = std::cell::OnceCell::new();
    let mut out = Vec::new();
    for (path, file) in found.iter().zip(&files[1..]) {
        let answers = |line: usize, outer: &[(String, muove_core::Value)]| {
            let (lowered, model) = answered.get_or_init(|| answerable(&files)).as_ref().map_err(Clone::clone)?;
            for_answer(lowered, model, &file.name, line, outer)
        };
        let lower = muove_syntax::nix::Lower {
            text_concat: index.text_concat.get(&file.name).unwrap_or(&none),
            answers: &answers,
        };
        let lowered = match file.decls.as_slice() {
            [] => continue,
            [RecordDecl::Module(module)] => lower.lower_module(module),
            [RecordDecl::File(nix_file)] => lower.lower_file(nix_file),
            _ => return Err(format!("{}: one `nix module` or `nix file` per source file", file.name)),
        }
        .map_err(|e| format!("{}:{e}", file.name))?;
        let target = path.with_extension("nix");
        let text = muove_nix::print(&lowered);
        parse_gate(&target, &text)?;
        out.push((target, text));
    }
    Ok(out)
}

fn answerable(files: &[SourceFile]) -> Result<(muove_syntax::lower::Lowered, muove_core::Model), String> {
    let texts: Vec<(&str, &str)> = files.iter().map(|f| (f.name.as_str(), f.text.as_str())).collect();
    let lowered = muove_syntax::lower::lower_files(&texts).map_err(|e| e.to_string())?;
    if !lowered.diagnostics.is_empty() {
        return Err(lowered.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"));
    }
    let model = muove_core::evaluate(&lowered.program, &[], Budget::default()).map_err(|e| format!("{e:?}"))?;
    Ok((lowered, model))
}

fn for_answer(
    lowered: &muove_syntax::lower::Lowered,
    model: &muove_core::Model,
    file: &str,
    line: usize,
    outer: &[(String, muove_core::Value)],
) -> Result<Vec<Vec<muove_core::Value>>, String> {
    let Some((_, _, query)) = lowered.for_queries.iter().find(|(f, l, _)| f == file && *l == line) else {
        return Err("a `for` block reads a query, such as `all ?m where …`".into());
    };
    let outer: Vec<_> = outer.iter().map(|(name, value)| (muove_core::Variable::new(name.as_str()), value.clone())).collect();
    let muove_core::Value::List(rows) = muove_core::answer(&lowered.program, model, query, &outer, Budget::default())
        .map_err(|e| format!("{e:?}"))?
    else {
        return Err("a `for` block reads an `all` query".into());
    };
    let width = match &query.kind {
        muove_core::QueryKind::All(variables) => variables.len(),
        muove_core::QueryKind::The(_) => 1,
    };
    Ok(rows
        .into_iter()
        .map(|row| match row {
            muove_core::Value::List(values) if width > 1 => values,
            value => vec![value],
        })
        .collect())
}

fn parse_gate(target: &Path, text: &str) -> Result<(), String> {
    let mut child = Command::new("nix-instantiate")
        .args(["--parse", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("nix-instantiate: {e}"))?;
    child.stdin.take().expect("piped stdin").write_all(text.as_bytes()).map_err(|e| format!("nix-instantiate: {e}"))?;
    let out = child.wait_with_output().map_err(|e| format!("nix-instantiate: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("{}: emitted Nix fails `nix-instantiate --parse`, not written\n{}", target.display(), String::from_utf8_lossy(&out.stderr)))
    }
}

fn build(paths: &[String]) -> Result<(), String> {
    for (target, text) in emit(paths)? {
        write_if_changed(&target, text.as_bytes())?;
    }
    Ok(())
}

fn check_built(paths: &[String]) -> Result<bool, String> {
    let mut clean = true;
    for (target, text) in emit(paths)? {
        let committed = std::fs::read_to_string(&target).unwrap_or_default();
        if committed != text {
            clean = false;
            print!("{}", diff::unified(&target.display().to_string(), &committed, &text));
        }
    }
    Ok(clean)
}

fn stdout(file: &str) -> Result<bool, String> {
    let emitted = emit(&[file.to_string()])?;
    let [(_, text)] = emitted.as_slice() else {
        return Err(format!("{file}: no `nix module` or `nix file` to emit"));
    };
    std::io::stdout().write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(true)
}

fn write_if_changed(target: &Path, bytes: &[u8]) -> Result<(), String> {
    if std::fs::read(target).is_ok_and(|old| old == bytes) {
        return Ok(());
    }
    let staged = target.with_extension("nix.tmp");
    std::fs::write(&staged, bytes)
        .and_then(|()| std::fs::rename(&staged, target))
        .map_err(|e| format!("{}: {e}", target.display()))
}

fn fmt(arguments: &[String]) -> ExitCode {
    let check = arguments.iter().any(|a| a == "--check");
    let mut paths: Vec<&str> = arguments.iter().filter(|a| *a != "--check").map(String::as_str).collect();
    if paths.is_empty() {
        paths.push(".");
    }
    let mut found = Vec::new();
    let mut changed = false;
    let result = paths
        .iter()
        .try_for_each(|path| sources(Path::new(path), &mut found))
        .and_then(|()| {
            found.iter().try_for_each(|path| {
                let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
                let formatted = muove_syntax::fmt::format(&text).map_err(|e| format!("{}:{e}", path.display()))?;
                if formatted == text {
                    return Ok(());
                }
                changed = true;
                println!("{}", path.display());
                if check {
                    return Ok(());
                }
                write_if_changed(path, formatted.as_bytes())
            })
        });
    match result {
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
        Ok(()) if check && changed => ExitCode::from(1),
        Ok(()) => ExitCode::SUCCESS,
    }
}
