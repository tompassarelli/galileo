use muove::{Limits, Program};
use std::process::ExitCode;

const USAGE: &str = "\
usage: muove <file>                      answer every question, then check the must-sentences
       muove check <file>                check the file and its must-sentences (exit 1 if any break)
       muove review <file> <proposal>... review proposed revisions (exit 1 if any break a must-sentence)

limits (each bounded search reports when it runs out):
  --max-reasons N      minimal reasons kept per conclusion (default 1000)
  --max-work N         elementary steps per computation (default 20000000)
  --max-facts N        facts per closure (default 1000000)
  --max-candidates N   possible additions for achieve (default 100000)";

fn read(path: &str) -> Result<String, ExitCode> {
    std::fs::read_to_string(path).map_err(|error| {
        eprintln!("{path}: {error}");
        ExitCode::FAILURE
    })
}

fn main() -> ExitCode {
    let mut limits = Limits::default();
    let mut words = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let slot = match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--max-reasons" => &mut limits.max_reasons,
            "--max-work" => &mut limits.max_work,
            "--max-facts" => &mut limits.max_facts,
            "--max-candidates" => &mut limits.max_candidates,
            _ if arg.starts_with("--") => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
            _ => {
                words.push(arg);
                continue;
            }
        };
        match args.next().and_then(|n| n.parse().ok()) {
            Some(n) => *slot = n,
            None => {
                eprintln!("{arg} needs a number");
                return ExitCode::from(2);
            }
        }
    }

    match words.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["check", file] => {
            let source = match read(file) {
                Ok(s) => s,
                Err(code) => return code,
            };
            match Program::parse(&source) {
                Ok(p) => {
                    println!(
                        "{file}: reads correctly ({} types, {} relations, {} rules, {} must-sentences, {} questions)",
                        p.types.len(),
                        p.relations.len(),
                        p.rules.len(),
                        p.intents.len(),
                        p.requests.len()
                    );
                    let (report, ok) = muove::intents_report(&p, &limits);
                    if !report.is_empty() {
                        print!("\n{report}");
                    }
                    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
                }
                Err(error) => {
                    eprintln!("{file}: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        ["review", file, proposals @ ..] if !proposals.is_empty() => {
            let base = match read(file) {
                Ok(s) => s,
                Err(code) => return code,
            };
            let mut all_ok = true;
            for (i, path) in proposals.iter().enumerate() {
                let proposal = match read(path) {
                    Ok(s) => s,
                    Err(code) => return code,
                };
                match muove::review(&base, &proposal, &limits) {
                    Ok((text, ok)) => {
                        if i > 0 {
                            println!();
                        }
                        println!("━━ {path}\n");
                        print!("{text}");
                        all_ok &= ok;
                    }
                    Err(error) => {
                        eprintln!("{path}: {error}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            if all_ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        [file] if *file != "check" && *file != "review" => {
            let source = match read(file) {
                Ok(s) => s,
                Err(code) => return code,
            };
            match muove::run(&source, &limits) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{file}: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
