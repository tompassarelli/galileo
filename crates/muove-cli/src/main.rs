use muove_core::{check_claims, claims, examples, exit_code, Budget, Program};
use std::process::ExitCode;

const USAGE: &str = "usage: muove check FIXTURE [--budget WORK]\nfixtures: gate, gate-never, chain, dungeon-static, tags, r13";

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
    let (program, budget) = match arguments.as_slice() {
        [command, name, rest @ ..] if command == "check" => {
            let budget = match rest {
                [] => Some(Budget::default()),
                [flag, work] if flag == "--budget" => work.parse().ok().map(Budget::work),
                _ => None,
            };
            match (fixture(name), budget) {
                (Some(program), Some(budget)) => (program, budget),
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
            }
            ExitCode::from(exit_code(&results) as u8)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}
