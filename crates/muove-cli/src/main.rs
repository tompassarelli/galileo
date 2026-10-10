use muove_core::{check_program, claims, exit_code, why, why_not, Budget, Checked, Expr, Literal, Verdict};
use muove_syntax::check::{analyze, SourceFile};
use muove_syntax::record::RecordDecl;
use muove_syntax::schema::Schema;
mod diff;
mod schema;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const USAGE: &str = "usage: muove check [PATH...] [--budget WORK]\n       muove build [--check] [PATH...]\n       muove build --stdout FILE\n       muove why S [--all] [PATH...]\n       muove why not S [via RULE] [PATH...]\n       muove refs OPTION [PATH...]\n       muove query [PATH...] QUESTION --json\n       muove lsp\n       muove schema extract --host HOST [--flake DIR] [--out FILE|-]\n       muove schema explain PATH [--schema FILE]\n       muove fmt [--check] [PATH...]\n       muove help [syntax]\nPATH defaults to the project: the nearest ancestor holding .git, else the working directory";
const SYNTAX_CARD: &str = include_str!("../help/syntax.txt");
const NIX_LIB: &str = include_str!("../../../lib/firn/nix-lib.muove");

const LIBRARY: [(&str, &str); 4] = [
    ("firn/dispatch", include_str!("../../../lib/firn/dispatch.muove")),
    ("firn/nix-lib", NIX_LIB),
    ("firn/responsibilities", include_str!("../../../lib/firn/responsibilities.muove")),
    ("firn/tags", include_str!("../../../lib/firn/tags.muove")),
];

struct Module {
    name: String,
    file: String,
    text: String,
}

fn project_root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let root = cwd.ancestors().find(|a| a.join(".git").exists()).unwrap_or(&cwd);
    root.canonicalize().map_err(|e| format!("{}: {e}", root.display()))
}

fn modules(paths: &[String]) -> Result<Vec<Module>, String> {
    let root = project_root()?;
    let mut found = Vec::new();
    if paths.is_empty() {
        sources(&root, &mut found)?;
    }
    for path in paths {
        sources(Path::new(path), &mut found)?;
    }
    let mut out: Vec<Module> = Vec::new();
    for path in &found {
        let full = path.canonicalize().map_err(|e| format!("{}: {e}", path.display()))?;
        let relative = full
            .strip_prefix(&root)
            .map_err(|_| format!("{}: outside the project at {}", path.display(), root.display()))?;
        let name = relative.with_extension("").components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
        if LIBRARY.iter().any(|(library, _)| *library == name) {
            return Err(format!("{}: `{name}` is the name of a library module; rename the file", path.display()));
        }
        if out.iter().any(|m| m.name == name) {
            continue;
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        out.push(Module { name, file: path.display().to_string(), text });
    }
    let mut next = 0;
    while next < out.len() {
        let uses: Vec<String> = out[next]
            .text
            .lines()
            .filter_map(|line| line.strip_prefix("use "))
            .map(|used| used.split_whitespace().collect())
            .collect();
        next += 1;
        for used in uses {
            if out.iter().any(|m| m.name == used) {
                continue;
            }
            let file = root.join(format!("{used}.muove"));
            if let Some((_, text)) = LIBRARY.iter().find(|(library, _)| *library == used) {
                if file.exists() {
                    return Err(format!("{}: `{used}` is the name of a library module; rename the file", file.display()));
                }
                out.push(Module { name: used.clone(), file: format!("lib/{used}.muove"), text: text.to_string() });
            } else if let Ok(text) = std::fs::read_to_string(&file) {
                out.push(Module { name: used.clone(), file: file.display().to_string(), text });
            }
        }
    }
    Ok(out)
}

fn link(modules: &[Module]) -> Result<muove_syntax::lower::Lowered, String> {
    let sources: Vec<(&str, &str)> = modules.iter().map(|m| (m.name.as_str(), m.text.as_str())).collect();
    let render = |errors: &[muove_syntax::Error]| errors.iter().map(|e| located(modules, e)).collect::<Vec<_>>().join("\n");
    let lowered = muove_syntax::lower::lower_program(&sources).map_err(|e| render(&[e]))?;
    if !lowered.diagnostics.is_empty() {
        return Err(render(&lowered.diagnostics));
    }
    Ok(lowered)
}

fn located(modules: &[Module], error: &muove_syntax::Error) -> String {
    let Some((module, message)) = modules.iter().find_map(|m| Some((m, error.message.strip_prefix(&format!("{}: ", m.name))?))) else {
        return error.to_string();
    };
    let mut out = match error.line {
        0 => format!("{}: {message}", module.file),
        line => format!("{}:{line}:{}: {message}", module.file, error.column),
    };
    if let Some(text) = &error.text {
        out.push_str(&format!("\n    {text}"));
    }
    out
}

fn asking(modules: &[Module], body: &str) -> Module {
    let uses: String = modules.iter().map(|m| format!("use {}\n", m.name)).collect();
    Module { name: ASKED.into(), file: ASKED.into(), text: format!("{uses}\n{body}") }
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
        if command == "query" {
            return match rest {
                [paths @ .., question, flag] if flag == "--json" => match query(paths, question) {
                    Ok(json) => {
                        println!("{json}");
                        ExitCode::SUCCESS
                    }
                    Err(message) => {
                        eprintln!("{message}");
                        ExitCode::from(1)
                    }
                },
                _ => {
                    eprintln!("{USAGE}");
                    ExitCode::from(2)
                }
            };
        }
        if command == "why" {
            return match explain(rest) {
                Ok(code) => ExitCode::from(code),
                Err(message) => {
                    eprintln!("{message}");
                    ExitCode::from(2)
                }
            };
        }
        if command == "build" {
            let result = match rest {
                [flag, file] if flag == "--stdout" => stdout(file),
                [flag, paths @ ..] if flag == "--check" => check_built(paths),
                paths if paths.first().is_none_or(|p| !p.starts_with("--")) => build(paths).map(|()| true),
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
        [command, rest @ ..] if command == "check" => {
            let (names, budget) = match rest {
                [names @ .., flag, work] if flag == "--budget" => {
                    (names, work.parse().ok().map(Budget::work))
                }
                names => (names, Some(Budget::default())),
            };
            let program = modules(names).and_then(|m| link(&m)).map(|l| l.program);
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
    match check_program(&program, budget) {
        Ok(Checked { modes, claims: results }) => {
            for failure in &modes {
                println!("{}", muove_core::modes::report(&program, &program.initially, failure, budget));
            }
            for result in &results {
                println!("{}", claims::sentence(result));
                if let Some(instance) = claims::counterexample(&program, result) {
                    println!("  {instance}");
                }
            }
            ExitCode::from(if modes.is_empty() { exit_code(&results) as u8 } else { 1 })
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

const ASKED: &str = "muove-asked";
const ASKED_RULE: &str = "muove-asked-question";

fn explain(arguments: &[String]) -> Result<u8, String> {
    let (negated, rest) = match arguments {
        [not, rest @ ..] if not == "not" => (true, rest),
        rest => (false, rest),
    };
    let split = rest
        .iter()
        .position(|a| a == "." || a.contains('/') || a.ends_with(".muove"))
        .unwrap_or(rest.len());
    let (words, paths) = rest.split_at(split);
    let all = !negated && words.iter().any(|w| w == "--all");
    let mut words: Vec<&str> = words.iter().map(String::as_str).filter(|w| *w != "--all").collect();
    let via = match words.iter().rposition(|w| *w == "via") {
        Some(at) if negated && at + 2 == words.len() => {
            let rule = words[at + 1].to_string();
            words.truncate(at);
            Some(why_not::Via { rule, bindings: Vec::new() })
        }
        _ => None,
    };
    if words.is_empty() {
        return Err(USAGE.into());
    }
    let sentence = words.join(" ");
    let mut read = modules(paths)?;
    read.push(asking(&read, &format!("derived muove-asked\n\nrule {ASKED_RULE}\n  muove-asked\n  when\n    {sentence}\n")));
    let mut program = link(&read)?.program;
    let asked_rule = format!("{ASKED}/{ASKED_RULE}");
    let rule = program.rules.iter().position(|r| r.name == asked_rule).expect("the asked rule is lowered");
    let asked = program.rules.remove(rule);
    let [Literal::Positive(atom)] = asked.body.as_slice() else {
        return Err(format!("`{sentence}` does not read as one sentence of the program"));
    };
    let values = atom
        .arguments
        .iter()
        .map(|a| match a {
            Expr::Value(v) => Some(v.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| format!("`{sentence}` has a variable; ask about one fact"))?;
    let relation = atom.relation;
    let budget = Budget::default();
    let failed = |e: muove_core::EvalError| format!("{e:?}");
    let (text, code) = if negated {
        let verdict = why_not::why_not(&program, &[], relation, &values, via.as_ref(), budget).map_err(failed)?;
        let code = match verdict {
            Verdict::Fails(_) => 0,
            Verdict::Holds(()) => 1,
            Verdict::Unknown(_) => 2,
        };
        (why_not::render(&program, relation, &values, via.as_ref(), &verdict, budget), code)
    } else if all {
        let verdict = why::why_all(&program, &[], relation, &values, budget).map_err(failed)?;
        (why::render_all(&program, relation, &values, &verdict, budget), verdict_code(&verdict))
    } else {
        let verdict = why::why(&program, &[], relation, &values, budget).map_err(failed)?;
        (why::render(&program, relation, &values, &verdict, budget), verdict_code(&verdict))
    };
    println!("{text}");
    Ok(code)
}

fn query(paths: &[String], question: &str) -> Result<String, String> {
    let mut read = modules(paths)?;
    read.push(asking(&read, &format!("nix file\n  value\n    for ?asked in {question}\n      muove: true\n")));
    let lowered = link(&read)?;
    let Some((_, _, asked)) = lowered.for_queries.iter().find(|(file, _, _)| file == ASKED) else {
        return Err(format!("`{question}` is not a query, such as `all ?x where …` or `the ?x where …`"));
    };
    let model = muove_core::evaluate(&lowered.program, &[], Budget::default()).map_err(|e| format!("{e:?}"))?;
    let value = muove_core::answer(&lowered.program, &model, asked, &[], Budget::default()).map_err(|e| format!("{e:?}"))?;
    Ok(json(&value))
}

fn json(value: &muove_core::Value) -> String {
    use muove_core::Value;
    let text = |s: &str| {
        let mut out = String::from('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    };
    let fields = |map: &std::collections::BTreeMap<String, Value>| {
        format!("{{{}}}", map.iter().map(|(k, v)| format!("{}:{}", text(k), json(v))).collect::<Vec<_>>().join(","))
    };
    match value {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Text(s) | Value::Path(s) => text(s),
        Value::OptionPath(path) => text(&path.join(".")),
        Value::Entity(entity) => text(&entity.name),
        Value::List(items) => format!("[{}]", items.iter().map(json).collect::<Vec<_>>().join(",")),
        Value::Record(map) | Value::Attrs(map) => fields(map),
        Value::Nix { normalized } => text(normalized),
    }
}

fn verdict_code<H, F>(verdict: &Verdict<H, F>) -> u8 {
    match verdict {
        Verdict::Holds(_) => 0,
        Verdict::Fails(_) => 1,
        Verdict::Unknown(_) => 2,
    }
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
    if paths.is_empty() {
        sources(&project_root()?, &mut found)?;
    }
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
    if let Err(refused) = answered.get_or_init(|| answerable(paths)) {
        if refused.starts_with("Fails: ") {
            return Err(refused.clone());
        }
    }
    let mut out = Vec::new();
    for (path, file) in found.iter().zip(&files[1..]) {
        let answers = |line: usize, outer: &[(String, muove_core::Value)]| {
            let (read, lowered, model) = answered.get_or_init(|| answerable(paths)).as_ref().map_err(Clone::clone)?;
            let module = read.iter().find(|m| m.file == file.name).map_or(file.name.as_str(), |m| m.name.as_str());
            for_answer(lowered, model, module, line, outer)
        };
        let lower = muove_syntax::nix::Lower {
            text_concat: index.text_concat.get(&file.name).unwrap_or(&none),
            answers: &answers,
        };
        let lowered = match file.decls.as_slice() {
            [] => continue,
            [RecordDecl::Module(module)] => lower.lower_module(module),
            [RecordDecl::File(nix_file)] => {
                if let (Some(binding), Some("flake")) = (nix_file.lets.first(), path.file_stem().and_then(|s| s.to_str())) {
                    return Err(format!(
                        "{}:line {}: `flake.nix` takes no root `let`, because Nix reads a flake's top level without evaluating it; write `{}` in place or bind it inside `outputs`",
                        file.name, binding.line, binding.name
                    ));
                }
                lower.lower_file(nix_file)
            }
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

type Answerable = (Vec<Module>, muove_syntax::lower::Lowered, muove_core::Model);

fn answerable(paths: &[String]) -> Result<Answerable, String> {
    let read = modules(paths)?;
    let lowered = link(&read)?;
    let model = match muove_core::evaluate(&lowered.program, &[], Budget::default()) {
        Ok(model) => model,
        Err(muove_core::EvalError::Mode(failures)) => {
            let program = &lowered.program;
            let reports: Vec<String> =
                failures.iter().map(|f| muove_core::modes::report(program, &program.initially, f, Budget::default())).collect();
            return Err(reports.join("\n"));
        }
        Err(e) => return Err(format!("{e:?}")),
    };
    Ok((read, lowered, model))
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
    let variables = match &query.kind {
        muove_core::QueryKind::All(variables) => variables.as_slice(),
        muove_core::QueryKind::The(_) => &[],
    };
    rows.into_iter()
        .map(|row| match row {
            muove_core::Value::Record(mut fields) if variables.len() > 1 => variables
                .iter()
                .map(|v| fields.remove(&v.0).ok_or_else(|| format!("the query's rows carry no `{}`", v.0)))
                .collect(),
            value => Ok(vec![value]),
        })
        .collect()
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
