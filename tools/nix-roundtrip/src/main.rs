use std::io::{self, Write};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut ast = false;
    let mut files = Vec::new();
    for arg in std::env::args_os().skip(1) {
        if arg == "--ast" {
            ast = true;
        } else if arg == "--help" || arg == "-h" {
            println!("Usage: nix-roundtrip [--ast] FILE [FILE ...]\n\nPrint a Nix file with muove-nix, or inspect ASTs with --ast.");
            return Ok(());
        } else if arg.to_string_lossy().starts_with('-') {
            return Err(format!("unknown option: {}", arg.to_string_lossy()).into());
        } else {
            files.push(PathBuf::from(arg));
        }
    }
    if files.is_empty() || (!ast && files.len() != 1) {
        return Err("expected one FILE, or --ast FILE [FILE ...]; see --help".into());
    }
    let mut output = io::stdout().lock();
    for file in files {
        let source = std::fs::read_to_string(&file)?;
        let expr = nix_roundtrip::parse(&source)
            .map_err(|error| format!("{}: {error}", file.display()))?;
        if ast {
            writeln!(output, "{expr:#?}")?;
        } else {
            output.write_all(muove_nix::print(&expr).as_bytes())?;
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
