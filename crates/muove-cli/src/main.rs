use muove_core::{check_claims, claims, examples, exit_code, Budget, Program};
use muove_syntax::check::{check, SourceFile};
use muove_syntax::record::RecordDecl;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: muove check FIXTURE [--budget WORK]\n       muove build PATH...\nfixtures: gate, gate-never, chain, dungeon-static, tags, r13";
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
    if let [command, paths @ ..] = arguments.as_slice() {
        if command == "build" && !paths.is_empty() {
            return match build(paths) {
                Ok(()) => ExitCode::SUCCESS,
                Err(message) => {
                    eprintln!("{message}");
                    ExitCode::from(1)
                }
            };
        }
    }
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

fn sources(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<_, _>>()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        entries.sort();
        for entry in entries {
            if entry.is_dir() || entry.extension().is_some_and(|ext| ext == "muove") {
                sources(&entry, out)?;
            }
        }
    } else {
        out.push(path.to_path_buf());
    }
    Ok(())
}

fn build(paths: &[String]) -> Result<(), String> {
    let mut found = Vec::new();
    for path in paths {
        sources(Path::new(path), &mut found)?;
    }
    let mut files = vec![SourceFile::parse("lib/firn/nix-lib.muove", NIX_LIB).map_err(|e| e.to_string())?];
    for path in &found {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        files.push(SourceFile::parse(&path.display().to_string(), &text).map_err(|e| e.to_string())?);
    }
    let diagnostics = check(&files);
    if !diagnostics.is_empty() {
        return Err(diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"));
    }
    for (path, file) in found.iter().zip(&files[1..]) {
        let module = match file.decls.as_slice() {
            [] => continue,
            [RecordDecl::Module(module)] => module,
            [RecordDecl::File(_)] => return Err(format!("{}: `nix file` has no Nix lowering yet", file.name)),
            _ => return Err(format!("{}: one `nix module` per source file", file.name)),
        };
        let lowered = muove_syntax::nix::lower_module(module).map_err(|e| format!("{}:{e}", file.name))?;
        write_if_changed(&path.with_extension("nix"), muove_nix::print(&lowered).as_bytes())?;
    }
    Ok(())
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
