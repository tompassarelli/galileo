use std::io::{self, Write};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut ast = false;
    let mut check = false;
    let mut files = Vec::new();
    for arg in std::env::args_os().skip(1) {
        if arg == "--ast" {
            ast = true;
        } else if arg == "--check" {
            check = true;
        } else if arg == "--help" || arg == "-h" {
            println!("Usage: nix-roundtrip [--ast | --check] FILE [FILE ...]\n\nPrint one Nix file, inspect ASTs with --ast, or report byte identity and each first differing line with --check.");
            return Ok(());
        } else if arg.to_string_lossy().starts_with('-') {
            return Err(format!("unknown option: {}", arg.to_string_lossy()).into());
        } else {
            files.push(PathBuf::from(arg));
        }
    }
    if files.is_empty() || (ast && check) || (!ast && !check && files.len() != 1) {
        return Err("expected one FILE, or --ast/--check FILE [FILE ...]; see --help".into());
    }
    let mut output = io::stdout().lock();
    let total = files.len();
    let mut identical = 0;
    for file in files {
        let source = std::fs::read_to_string(&file)?;
        let parsed = nix_roundtrip::parse(&source);
        if check {
            match parsed {
                Ok(expr) => {
                    let printed = muove_nix::print(&expr);
                    if printed == source {
                        identical += 1;
                    } else {
                        let expected: Vec<_> = source.split_inclusive('\n').collect();
                        let actual: Vec<_> = printed.split_inclusive('\n').collect();
                        let line = (0..expected.len().max(actual.len()))
                            .find(|&line| expected.get(line) != actual.get(line))
                            .unwrap();
                        writeln!(output, "{}: first differing line {}", file.display(), line + 1)?;
                        writeln!(output, "  expected: {:?}", expected.get(line))?;
                        writeln!(output, "  actual:   {:?}", actual.get(line))?;
                    }
                }
                Err(error) => {
                    writeln!(output, "{}: first differing line 1 (no output)", file.display())?;
                    writeln!(output, "  expected: {:?}", source.split_inclusive('\n').next())?;
                    writeln!(output, "  error: {error}")?;
                }
            }
            continue;
        }
        let expr = parsed.map_err(|error| format!("{}: {error}", file.display()))?;
        if ast {
            writeln!(output, "{expr:#?}")?;
        } else {
            output.write_all(muove_nix::print(&expr).as_bytes())?;
        }
    }
    if check {
        writeln!(output, "identical: {identical}/{total}; failures: {}", total - identical)?;
        if identical != total {
            return Err("round-trip failures".into());
        }
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("nix-roundtrip: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
