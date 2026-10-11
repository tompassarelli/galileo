use galileo_core::{check_program, claims, exit_code, why, why_not, Budget, Checked, Expr, Literal, Verdict};
use galileo_syntax::check::{analyze, SourceFile};
use galileo_syntax::record::RecordDecl;
use galileo_syntax::schema::Schema;
mod diff;
mod schema;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const USAGE: &str = "usage: galileo check [PATH...] [--events FILE [--as-of R]] [--budget WORK]\n       galileo build [--check] [PATH...]\n       galileo build --stdout FILE\n       galileo import nix FILE\n       galileo import clause FILE\n       galileo why S [--all] [PATH...] [--events FILE [--as-of R]] [--budget WORK]\n       galileo why not S [--via RULE] [PATH...]\n       galileo refs OPTION [PATH...]\n       galileo query [PATH...] QUESTION [--events FILE [--as-of R] [--since R]] --json\n       galileo compile MODULE -o IMAGE\n       galileo run [PATH...] --events FILE\n       galileo bindgen rust [PATH...] [-o FILE]\n       galileo lsp\n       galileo schema extract --host HOST [--flake DIR] [--out FILE|-]\n       galileo schema explain PATH [--schema FILE]\n       galileo fmt [--check] [PATH...]\n       galileo help [syntax]\nPATH defaults to the project: the nearest ancestor holding .git, else the working directory";
const SYNTAX_CARD: &str = include_str!("../help/syntax.txt");

const LIBRARY: [(&str, &str); 3] = [
    ("firn/dispatch", include_str!("../../../lib/firn/dispatch.gal")),
    ("firn/responsibilities", include_str!("../../../lib/firn/responsibilities.gal")),
    ("firn/tags", include_str!("../../../lib/firn/tags.gal")),
];

#[derive(Clone)]
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
            let file = root.join(format!("{used}.gal"));
            if let Some((_, text)) = LIBRARY.iter().find(|(library, _)| *library == used) {
                if file.exists() {
                    return Err(format!("{}: `{used}` is the name of a library module; rename the file", file.display()));
                }
                out.push(Module { name: used.clone(), file: format!("lib/{used}.gal"), text: text.to_string() });
            } else if let Ok(text) = std::fs::read_to_string(&file) {
                out.push(Module { name: used.clone(), file: file.display().to_string(), text });
            }
        }
    }
    Ok(out)
}

fn link(modules: &[Module]) -> Result<galileo_syntax::lower::Lowered, String> {
    let sources: Vec<(&str, &str)> = modules.iter().map(|m| (m.name.as_str(), m.text.as_str())).collect();
    let render = |errors: &[galileo_syntax::Error]| errors.iter().map(|e| located(modules, e)).collect::<Vec<_>>().join("\n");
    let lowered = galileo_syntax::lower::lower_program(&sources).map_err(|e| render(&[e]))?;
    for warning in &lowered.warnings {
        let mut warning = warning.clone();
        if let Some((module, message)) = warning.message.split_once(": ") {
            warning.message = format!("{module}: warning: {message}");
        }
        eprintln!("{}", located(modules, &warning));
    }
    if !lowered.diagnostics.is_empty() {
        return Err(render(&lowered.diagnostics));
    }
    Ok(lowered)
}

fn located(modules: &[Module], error: &galileo_syntax::Error) -> String {
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
    if let [command, target, rest @ ..] = arguments.as_slice() {
        if command == "bindgen" && target == "rust" {
            let (paths, output) = match rest {
                [paths @ .., flag, output] if flag == "-o" => (paths, Some(output)),
                paths if !paths.iter().any(|path| path.starts_with('-')) => (paths, None),
                _ => { eprintln!("{USAGE}"); return ExitCode::from(2); }
            };
            let result = modules(paths).and_then(|modules| {
                let lowered = link(&modules)?;
                let sources = modules.iter().map(|module| (module.name.as_str(), module.text.as_str())).collect::<Vec<_>>();
                let read = galileo_syntax::read_program(&sources).map_err(|(module, error)| format!("{module}: {error}"))?;
                let vocabularies = read.values().map(|sentences| &sentences.vocabulary).collect::<Vec<_>>();
                let generated = galileo_bindgen::rust(&lowered.program, &vocabularies)?;
                if let Some(output) = output {
                    std::fs::write(output, generated).map_err(|error| format!("{output}: {error}"))?;
                } else {
                    print!("{generated}");
                }
                Ok(())
            });
            return match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(message) => { eprintln!("{message}"); ExitCode::from(2) }
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
            return match galileo_lsp::run() {
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
        if command == "import" {
            let [kind, file] = rest else {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            };
            if !matches!(kind.as_str(), "nix" | "clause") || file.starts_with("--") {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
            if kind == "clause" {
                return match std::fs::read_to_string(file)
                    .map_err(|error| format!("{file}: {error}"))
                    .and_then(|source| galileo_import::clause_menu::import(&source))
                {
                    Ok(import) => {
                        print!("{}", import.source);
                        eprintln!(
                            "menu import: clauses={}, maintenance={}, unpaired includes={}",
                            import.clauses,
                            import.maintenance,
                            import.unpaired.len()
                        );
                        for clause in import.unpaired {
                            eprintln!("DECISION 8.6: unpaired include in {clause}");
                        }
                        ExitCode::SUCCESS
                    }
                    Err(message) => {
                        eprintln!("{}", message.replace('\n', " "));
                        ExitCode::from(1)
                    }
                };
            }
            return match std::fs::read_to_string(file)
                .map_err(|error| format!("{file}: {error}"))
                .and_then(|source| galileo_import::nix::parse(&source))
                .and_then(|root| galileo_import::nix::lift(&root))
            {
                Ok(record) => {
                    let module = galileo_syntax::record::print(&record);
                    let path = std::path::Path::new(file);
                    let tags = path.with_file_name("tags.clause");
                    let name = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str());
                    match (std::fs::read_to_string(&tags), name) {
                        (Ok(clause), Some(name)) => match galileo_import::bnix_tags::clause_module(name, &clause, &module) {
                            Ok(text) => print!("{text}"),
                            Err(message) => {
                                eprintln!("{}: {message}", tags.display());
                                return ExitCode::from(1);
                            }
                        },
                        _ => print!("{module}"),
                    }
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    eprintln!("{}", message.replace('\n', " "));
                    ExitCode::from(1)
                }
            };
        }
        if command == "compile" {
            return match compile(rest) {
                Ok(()) => ExitCode::SUCCESS,
                Err(message) => {
                    eprintln!("{message}");
                    ExitCode::from(2)
                }
            };
        }
        if command == "run" {
            return match run(rest) {
                Ok(code) => ExitCode::from(code),
                Err(message) => {
                    eprintln!("{message}");
                    ExitCode::from(2)
                }
            };
        }
        if command == "fmt" {
            return fmt(rest);
        }
        if command == "query" {
            let mut arguments = rest.to_vec();
            let (events, as_of) = match event_argument(&mut arguments) {
                Ok(events) => events,
                Err(message) => { eprintln!("{message}"); return ExitCode::from(2); }
            };
            let since = if let Some(index) = arguments.iter().position(|a| a == "--since") {
                let Some(revision) = arguments.get(index + 1).and_then(|r| r.parse::<u64>().ok()) else {
                    eprintln!("{USAGE}"); return ExitCode::from(2);
                };
                arguments.drain(index..=index + 1);
                if arguments.iter().any(|a| a == "--since") { eprintln!("{USAGE}"); return ExitCode::from(2); }
                if events.is_none() { eprintln!("--since requires --events; last revision is 0"); return ExitCode::from(2); }
                Some(revision)
            } else { None };
            return match arguments.as_slice() {
                [paths @ .., question, flag] if flag == "--json" => match query(paths, question, events.as_deref(), as_of, since) {
                    Ok(json) => { println!("{json}"); ExitCode::SUCCESS }
                    Err((code, message)) => { eprintln!("{message}"); ExitCode::from(code) }
                },
                _ => { eprintln!("{USAGE}"); ExitCode::from(2) }
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
    let (program, budget, original) = match arguments.as_slice() {
        [command, rest @ ..] if command == "check" => {
            let mut arguments = rest.to_vec();
            let (events, as_of) = match event_argument(&mut arguments) {
                Ok(events) => events,
                Err(message) => { eprintln!("{message}"); return ExitCode::from(2); }
            };
            let (names, budget) = match arguments.as_slice() {
                [names @ .., flag, work] if flag == "--budget" => {
                    (names, work.parse().ok().map(Budget::work))
                }
                names => (names, Some(Budget::default())),
            };
            let program = modules(names).and_then(|m| {
                let mut program = link(&m)?.program;
                let (_, files, schema) = load(names)?;
                let (diagnostics, _) = analyze(&files, &schema);
                if !diagnostics.is_empty() {
                    return Err(diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"));
                }
                let original = events.as_ref().map(|_| program.clone());
                if let Some(file) = &events { apply_events(&mut program, &m, file, budget.ok_or(USAGE)?, as_of)?; }
                Ok((program, original))
            });
            match (program, budget) {
                (Ok((program, original)), Some(budget)) => (program, budget, original),
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
    let checked = match original {
        None => check_program(&program, budget),
        Some(mut original) => {
            let mut current = program.clone();
            current.requirements.retain(|r| !temporal_claim(&r.claim));
            original.requirements.retain(|r| temporal_claim(&r.claim));
            check_program(&current, budget).and_then(|mut checked| {
                if !original.requirements.is_empty() {
                    checked.claims.extend(check_program(&original, budget)?.claims);
                    checked.claims.sort_by_key(|r| program.requirements.iter().position(|wanted| wanted.name == r.requirement));
                }
                Ok(checked)
            })
        }
    };
    match checked {
        Ok(Checked { modes, claims: results }) => {
            for failure in &modes {
                println!("{}", galileo_core::modes::report(&program, &program.initially, failure, budget));
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

const ASKED: &str = "galileo-asked";
const ASKED_RULE: &str = "galileo-asked-question";

fn explain(arguments: &[String]) -> Result<u8, String> {
    let mut arguments = arguments.to_vec();
    let (events, as_of) = event_argument(&mut arguments)?;
    let budget = if let Some(index) = arguments.iter().position(|argument| argument == "--budget") {
        if index + 1 >= arguments.len() { return Err(USAGE.into()); }
        let work = arguments.remove(index + 1).parse().map_err(|_| USAGE.to_string())?;
        arguments.remove(index);
        Budget::work(work)
    } else { Budget::default() };
    let (negated, rest) = match arguments.as_slice() {
        [not, rest @ ..] if not == "not" => (true, rest),
        rest => (false, rest),
    };
    let mut rest = rest.to_vec();
    let via = if let Some(at) = rest.iter().position(|word| word == "--via") {
        if !negated || at + 1 == rest.len() || rest[at + 1].starts_with('-') {
            return Err(USAGE.into());
        }
        let rule = rest.remove(at + 1);
        rest.remove(at);
        Some(why_not::Via { rule, bindings: Vec::new() })
    } else { None };
    let split = rest
        .iter()
        .position(|a| a == "." || a.contains('/') || a.ends_with(".gal"))
        .unwrap_or(rest.len());
    let (words, paths) = rest.split_at(split);
    let all = !negated && words.iter().any(|w| w == "--all");
    let words: Vec<&str> = words.iter().map(String::as_str).filter(|w| *w != "--all").collect();
    if words.is_empty() {
        return Err(USAGE.into());
    }
    let sentence = words.join(" ");
    let mut read = modules(paths)?;
    let sources: Vec<_> = read.iter().map(|module| (module.name.as_str(), module.text.as_str())).collect();
    let linked = galileo_syntax::read_program(&sources).map_err(|(name, error)| located(&read, &galileo_syntax::Error { message: format!("{name}: {}", error.message), ..error }))?;
    let (scope, question) = galileo_syntax::read_question(&sentence, galileo_syntax::QuestionKind::Sentence, &linked).map_err(|error| error.to_string())?;
    let galileo_syntax::Question::Sentence(reading) = question else { unreachable!() };
    let sentence = scope.sentence(reading.shape, &reading.args);
    read.push(asking(&read, &format!("derived galileo-asked\n\nrule {ASKED_RULE}: galileo-asked\n  when {sentence}\n")));
    let mut program = link(&read)?.program;
    read.pop();
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
    let failed = |e: galileo_core::EvalError| format!("{e:?}");
    for source in program.facts.iter_mut().chain(&mut program.initially).filter_map(|fact| match &mut fact.origin { galileo_core::FactOrigin::Stated(source) => source.as_mut(), _ => None })
        .chain(program.rules.iter_mut().filter_map(|rule| rule.source.as_mut()))
        .chain(program.handlers.iter_mut().flat_map(|handler| handler.clauses.iter_mut().filter_map(|clause| clause.source.as_mut()))) {
        if let Some(module) = read.iter().find(|module| module.name == source.file) { source.file = module.file.clone(); }
    }
    let (text, code) = if let Some(file) = events {
        if all { return Err("temporal --all is not available".into()); }
        let occurrences = read_events(&program, &mut read, &file)?;
        let mut run = galileo_core::state::Run::new(&program);
        let mut sources = Vec::new();
        for (line, event, values) in occurrences {
            run.run(&program, event, &values, Budget::default()).map_err(failed)?;
            sources.push(galileo_core::SourceSpan { file: file.clone(), start: 0, end: 0, line, column: 1 });
        }
        let revision = as_of.unwrap_or(run.revision);
        if negated {
            let verdict = why_not::why_not_history(&program, &run, &sources, revision, relation, &values, via.as_ref(), budget).map_err(failed)?;
            match verdict {
                Verdict::Fails(text) => (text, 0),
                Verdict::Holds(()) => (format!("{} holds. Ask `why` for its proof.", why::sentence(&program, relation, &values)), 1),
                Verdict::Unknown(reason) => (format!("Why not {} is unknown: {reason}", why::sentence(&program, relation, &values)), 2),
            }
        } else {
            let verdict = why::why_history(&program, &run, &sources, revision, relation, &values, budget).map_err(failed)?;
            let text = match &verdict {
                Verdict::Holds(text) => text.clone(),
                Verdict::Fails(()) => format!("{} does not hold. Ask `why not` for its near misses.", why::sentence(&program, relation, &values)),
                Verdict::Unknown(reason) => format!("Why {} is unknown: {reason}", why::sentence(&program, relation, &values)),
            };
            (text, verdict_code(&verdict))
        }
    } else if negated {
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

fn compile(arguments: &[String]) -> Result<(), String> {
    let [module, flag, image] = arguments else { return Err(USAGE.into()); };
    if flag != "-o" || module.starts_with('-') || image.starts_with('-') {
        return Err(USAGE.into());
    }
    let read = modules(std::slice::from_ref(module))?;
    let program = link(&read)?.program;
    let sources: Vec<_> = read.iter().map(|module| (module.name.as_str(), module.text.as_str())).collect();
    let linked = galileo_syntax::read_program(&sources).map_err(|(name, error)| format!("{name}: {error}"))?;
    fn order<'a>(name: &'a str, linked: &'a std::collections::BTreeMap<String, galileo_syntax::Sentences>, names: &mut Vec<&'a str>) {
        if names.contains(&name) { return; }
        for used in &linked[name].uses { order(used, linked, names); }
        names.push(name);
    }
    let mut names = Vec::new();
    for module in &read { order(&module.name, &linked, &mut names); }
    let mut bytes = format!("galileo-image 1 {}\n", galileo_review::identity(&program)).into_bytes();
    for name in names {
        let module = read.iter().find(|module| module.name == name).expect("a linked module has source");
        bytes.extend_from_slice(format!("module {name} {}\n", module.text.len()).as_bytes());
        bytes.extend_from_slice(module.text.as_bytes());
    }
    std::fs::write(image, bytes).map_err(|error| format!("{image}: {error}"))
}

fn run(arguments: &[String]) -> Result<u8, String> {
    use galileo_core::{EvalError, Reason, Value};
    use galileo_runtime::{Error, Session};
    let (arguments, budget) = match arguments {
        [rest @ .., flag, work] if flag == "--budget" => (rest, Some(work.parse().ok().map(Budget::work).ok_or(USAGE)?)),
        rest => (rest, None),
    };
    let [paths @ .., flag, file] = arguments else { return Err(USAGE.into()); };
    if flag != "--events" || Path::new(file).extension().is_none_or(|extension| extension != "events") || paths.iter().any(|p| p.starts_with("--")) {
        return Err(USAGE.into());
    }
    let image = match paths {
        [path] if Path::new(path).is_file() => {
            let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
            bytes.starts_with(b"galileo-image ").then_some(bytes)
        }
        _ => None,
    };
    let (program, mut read) = match image {
        Some(bytes) => {
            let program = galileo_runtime::Program::load(&bytes).map_err(|error| format!("{error:?}"))?;
            let read = program.sources().iter().map(|(name, text)| Module {
                name: name.clone(), file: format!("{name}.gal"), text: text.clone(),
            }).collect();
            (program, read)
        }
        None => {
            let read = modules(paths)?;
            (galileo_runtime::Program::from(link(&read)?.program), read)
        }
    };
    let events = read_events(&program, &mut read, file)?;
    let mut session = Session::new(&program);
    println!("{{\"revision\":0,\"hash\":{}}}", json(&Value::Text(session.state().hash(&program))));
    let mut code = 0;
    for (line, relation, values) in events {
        let event = json(&Value::Text(galileo_core::why::sentence(&program, relation, &values)));
        let result = session.run_with_budget((relation, values), budget.unwrap_or_default());
        let revision = session.revision();
        let hash = session.state().hash(&program);
        let hash = json(&Value::Text(hash));
        match result {
            Err(Error::Evaluation(EvalError::Unknown(Reason::OutOfBudget { .. }))) => {
                println!("{{\"line\":{line},\"unknown\":\"budget\"}}");
                return Ok(2);
            }
            Ok(step) => {
                let effects = Value::List(step.effects.iter().map(|(relation, values)| Value::Text(galileo_core::why::sentence(&program, *relation, values))).collect());
                println!("{{\"line\":{line},\"event\":{event},\"admitted\":true,\"revision\":{revision},\"hash\":{hash},\"effects\":{}}}", json(&effects));
            }
            Err(Error::Rejected(rejection)) => {
                code = 1;
                let explanation = Value::List(rejection.explanation.iter().cloned().map(Value::Text).collect());
                println!("{{\"line\":{line},\"event\":{event},\"admitted\":false,\"rejection\":{},\"explanation\":{},\"revision\":{revision},\"hash\":{hash}}}", json(&Value::Text(rejection.kind.as_str().into())), json(&explanation));
            }
            Err(Error::Evaluation(error)) => return Err(format!("{error:?}")),
            Err(error) => return Err(format!("{error:?}")),
        }
    }
    Ok(code)
}

fn event_argument(arguments: &mut Vec<String>) -> Result<(Option<String>, Option<u64>), String> {
    let file = if let Some(index) = arguments.iter().position(|a| a == "--events") {
        if index + 1 >= arguments.len() || arguments[index + 1].starts_with("--") { return Err(USAGE.into()); }
        let file = arguments.remove(index + 1);
        arguments.remove(index);
        if arguments.iter().any(|a| a == "--events") { return Err(USAGE.into()); }
        Some(file)
    } else { None };
    let as_of = if let Some(index) = arguments.iter().position(|a| a == "--as-of") {
        if index + 1 >= arguments.len() { return Err(USAGE.into()); }
        let revision = arguments.remove(index + 1).parse().map_err(|_| USAGE.to_string())?;
        arguments.remove(index);
        if arguments.iter().any(|a| a == "--as-of") { return Err(USAGE.into()); }
        if file.is_none() { return Err("--as-of requires --events; last revision is 0".into()); }
        Some(revision)
    } else { None };
    Ok((file, as_of))
}

fn read_events(program: &galileo_core::Program, read: &mut Vec<Module>, file: &str) -> Result<Vec<(usize, galileo_core::RelationId, Vec<galileo_core::Value>)>, String> {
    use galileo_core::{ArgumentDomain, RelationKind};
    if Path::new(file).extension().is_none_or(|extension| extension != "events") { return Err(USAGE.into()); }
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let mut events = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let at = format!("{file}:{}", index + 1);
        read.push(asking(&read, &format!("derived galileo-asked\n\nrule {ASKED_RULE}: galileo-asked\n  when {line}\n")));
        let asked = link(&read).map_err(|e| format!("{at}: {e}"))?;
        read.pop();
        let rule = asked.program.rules.iter().find(|r| r.name == format!("{ASKED}/{ASKED_RULE}")).ok_or_else(|| format!("{at}: expected one ground event sentence"))?;
        let [Literal::Positive(atom)] = rule.body.as_slice() else { return Err(format!("{at}: expected one ground event sentence")); };
        if program.relations[atom.relation.0].kind != RelationKind::Event {
            return Err(format!("{at}: `{line}` is not an event sentence"));
        }
        let values = atom.arguments.iter().map(|a| match a { Expr::Value(v) => Some(v.clone()), _ => None }).collect::<Option<Vec<_>>>().ok_or_else(|| format!("{at}: an event sentence must be ground"))?;
        let event = program.events.iter().find(|e| e.relation == atom.relation).expect("an event shape is declared");
        for (slot, domain) in &event.domains {
            if let ArgumentDomain::Only(allowed) = domain {
                if !allowed.contains(&values[*slot]) {
                    return Err(format!("{at}: {} is outside the closed domain for {}", json(&values[*slot]), program.relations[atom.relation.0].slots[*slot].name));
                }
            }
        }
        events.push((index + 1, atom.relation, values));
    }
    if events.is_empty() {
        return Err(format!("{file}: no event occurrences"));
    }
    Ok(events)
}

fn apply_events(program: &mut galileo_core::Program, read: &[Module], file: &str, budget: Budget, as_of: Option<u64>) -> Result<(), String> {
    let events = read_events(program, &mut read.to_vec(), file)?;
    let mut run = galileo_core::state::Run::new(program);
    for (_, relation, values) in events {
        run.run(program, relation, &values, budget).map_err(|e| format!("{e:?}"))?;
    }
    let state = match as_of {
        Some(revision) => run.history.as_of(revision).ok_or_else(|| format!("revision {revision} is past the last revision {}", run.revision))?,
        None => run.state,
    };
    program.initially = state.facts.into_iter().map(|(relation, values)| galileo_core::Fact::new(relation, values)).collect();
    program.entities.retain(|entity| !program.entity_types.iter().any(|t| t.state && t.name == entity.entity_type));
    program.entities.extend(state.live);
    Ok(())
}

fn temporal_claim(claim: &galileo_core::Claim) -> bool {
    use galileo_core::Claim;
    match claim {
        Claim::State(_) | Claim::Requires { .. } => false,
        Claim::NoSinglePointOfFailure { claim, .. } => temporal_claim(claim),
        _ => true,
    }
}

fn lists_variables(rest: &str) -> bool {
    let mut words = rest.split_whitespace();
    loop {
        let Some(word) = words.next() else { return false };
        let name = word.trim_end_matches(',');
        if !name.starts_with('?') || name.len() == 1 || !name[1..].chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return false;
        }
        if !word.ends_with(',') {
            return words.next() == Some("where");
        }
    }
}

fn query(paths: &[String], question: &str, events: Option<&str>, as_of: Option<u64>, since: Option<u64>) -> Result<String, (u8, String)> {
    let mut read = modules(paths).map_err(|m| (1, m))?;
    let header = match question.strip_prefix("all ") {
        Some(rest) if lists_variables(rest) => format!("for {rest}"),
        _ => format!("for ?asked in {question}"),
    };
    read.push(asking(&read, &format!("nix file\n  value\n    {header}\n      galileo: true\n")));
    let mut lowered = link(&read).map_err(|m| (1, m))?;
    let Some((_, _, asked)) = lowered.for_queries.iter().find(|(file, _, _)| file == ASKED) else {
        return Err((1, format!("`{question}` is not a query, such as `all ?x where …` or `the ?x where …`")));
    };
    if let Some(since) = since {
        use galileo_core::{Fact, Value};
        use std::collections::BTreeSet;
        let file = events.expect("--since requires --events");
        let events = read_events(&lowered.program, &mut read[..read.len() - 1].to_vec(), file).map_err(|m| (2, m))?;
        let mut run = galileo_core::state::Run::new(&lowered.program);
        let mut occurrences = std::collections::BTreeMap::new();
        for (line, relation, values) in events {
            let record = run.run(&lowered.program, relation, &values, Budget::default()).map_err(|e| (2, format!("{e:?}")))?;
            if record.result.is_ok() {
                occurrences.insert(record.revision, (line, galileo_core::why::sentence(&lowered.program, relation, &values)));
            }
        }
        let until = as_of.unwrap_or(run.revision);
        if until > run.revision { return Err((2, format!("revision {until} is past the last revision {}", run.revision))); }
        if since > until { return Err((2, format!("--since {since} is past ending revision {until}; last revision {}", run.revision))); }
        let revisions = run.history.query_revisions(&lowered.program, asked, since, until);
        let mut previous = BTreeSet::new();
        let mut output = Vec::new();
        for revision in std::iter::once(since).chain(revisions) {
            let state = run.history.as_of(revision).expect("a selected revision is in the history");
            lowered.program.initially = state.facts.into_iter().map(|(relation, values)| Fact::new(relation, values)).collect();
            lowered.program.entities.retain(|entity| !lowered.program.entity_types.iter().any(|t| t.state && t.name == entity.entity_type));
            lowered.program.entities.extend(state.live);
            let model = galileo_core::evaluate(&lowered.program, &lowered.program.initially, Budget::default()).map_err(|e| (1, format!("{e:?}")))?;
            let value = galileo_core::answer(&lowered.program, &model, asked, &[], Budget::default()).map_err(|e| (1, format!("{e:?}")))?;
            let rows: BTreeSet<_> = match &value {
                Value::Null => BTreeSet::new(),
                Value::List(rows) if matches!(asked.kind, galileo_core::QueryKind::All(_)) => rows.iter().cloned().collect(),
                value => BTreeSet::from([value.clone()]),
            };
            if revision == since {
                output.push(format!("{{\"revision\":{revision},\"answer\":{}}}", json(&value)));
            } else if rows != previous {
                let (line, event) = &occurrences[&revision];
                let added = Value::List(rows.difference(&previous).cloned().collect());
                let removed = Value::List(previous.difference(&rows).cloned().collect());
                output.push(format!("{{\"revision\":{revision},\"line\":{line},\"event\":{},\"added\":{},\"removed\":{}}}", json(&Value::Text(event.clone())), json(&added), json(&removed)));
            }
            previous = rows;
        }
        return Ok(output.join("\n"));
    }
    if let Some(file) = events { apply_events(&mut lowered.program, &read[..read.len() - 1], file, Budget::default(), as_of).map_err(|m| (2, m))?; }
    let model = galileo_core::evaluate(&lowered.program, &lowered.program.initially, Budget::default()).map_err(|e| (1, format!("{e:?}")))?;
    let value = galileo_core::answer(&lowered.program, &model, asked, &[], Budget::default()).map_err(|e| (1, format!("{e:?}")))?;
    Ok(json(&value))
}

fn json(value: &galileo_core::Value) -> String {
    use galileo_core::Value;
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
            if entry.is_dir() && !hidden || entry.extension().is_some_and(|ext| ext == "gal") {
                sources(&entry, out)?;
            }
        }
    } else {
        out.push(path.to_path_buf());
    }
    Ok(())
}

fn load(paths: &[String]) -> Result<(Vec<PathBuf>, Vec<SourceFile>, Schema), String> {
    let read = modules(paths)?;
    let sources: Vec<(&str, &str)> = read.iter().map(|m| (m.name.as_str(), m.text.as_str())).collect();
    let mut sentences = galileo_syntax::read_program(&sources)
        .map_err(|(module, mut error)| {
            error.message = format!("{module}: {}", error.message);
            located(&read, &error)
        })?;
    let mut found = Vec::new();
    let mut files = Vec::new();
    for module in &read {
        if LIBRARY.iter().any(|(name, _)| *name == module.name) {
            continue;
        }
        let sentences = sentences.remove(&module.name).expect("read module has linked sentences");
        let file = SourceFile::linked(&module.file, &module.text, Some(sentences)).map_err(|e| e.to_string())?;
        found.push(PathBuf::from(&module.file));
        files.push(file);
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
    for (path, file) in found.iter().zip(&files) {
        let answers = |line: usize, outer: &[(String, galileo_core::Value)]| {
            let (read, lowered, model) = answered.get_or_init(|| answerable(paths)).as_ref().map_err(Clone::clone)?;
            let module = read.iter().find(|m| m.file == file.name).map_or(file.name.as_str(), |m| m.name.as_str());
            for_answer(lowered, model, module, line, outer)
        };
        let lower = galileo_syntax::nix::Lower {
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
        let text = galileo_nix::print(&lowered);
        parse_gate(&target, &text)?;
        out.push((target, text));
    }
    Ok(out)
}

type Answerable = (Vec<Module>, galileo_syntax::lower::Lowered, galileo_core::Model);

fn answerable(paths: &[String]) -> Result<Answerable, String> {
    let read = modules(paths)?;
    let lowered = link(&read)?;
    let model = match galileo_core::evaluate(&lowered.program, &[], Budget::default()) {
        Ok(model) => model,
        Err(galileo_core::EvalError::Mode(failures)) => {
            let program = &lowered.program;
            let reports: Vec<String> =
                failures.iter().map(|f| galileo_core::modes::report(program, &program.initially, f, Budget::default())).collect();
            return Err(reports.join("\n"));
        }
        Err(e) => return Err(format!("{e:?}")),
    };
    Ok((read, lowered, model))
}

fn for_answer(
    lowered: &galileo_syntax::lower::Lowered,
    model: &galileo_core::Model,
    file: &str,
    line: usize,
    outer: &[(String, galileo_core::Value)],
) -> Result<Vec<Vec<galileo_core::Value>>, String> {
    let Some((_, _, query)) = lowered.for_queries.iter().find(|(f, l, _)| f == file && *l == line) else {
        return Err("a `for` block reads a query, such as `all ?m where …`".into());
    };
    let outer: Vec<_> = outer.iter().map(|(name, value)| (galileo_core::Variable::new(name.as_str()), value.clone())).collect();
    let galileo_core::Value::List(rows) = galileo_core::answer(&lowered.program, model, query, &outer, Budget::default())
        .map_err(|e| format!("{e:?}"))?
    else {
        return Err("a `for` block reads an `all` query".into());
    };
    let variables = match &query.kind {
        galileo_core::QueryKind::All(variables) => variables.as_slice(),
        galileo_core::QueryKind::The(_) => &[],
    };
    rows.into_iter()
        .map(|row| match row {
            galileo_core::Value::Record(mut fields) if variables.len() > 1 => variables
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
                let formatted = galileo_syntax::fmt::format(&text).map_err(|e| format!("{}:{e}", path.display()))?;
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
