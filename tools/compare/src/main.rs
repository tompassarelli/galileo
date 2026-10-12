mod blind;
mod replay;
mod scale;
mod size;
mod types;

use std::error::Error;
use std::process::ExitCode;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("size") => size::run(args.collect()),
        Some("replay") => replay::run(args.collect()),
        Some("blind") => blind::run(args.collect()),
        Some("scale") => scale::run(args.collect()),
        Some("types") => types::run(args.collect()),
        Some("--help" | "-h") => {
            println!("Usage: compare size [OPTIONS] FILE...\n       compare replay [OPTIONS]\n       compare blind [OPTIONS]\n       compare scale COUNT [OPTIONS]\n       compare types [OPTIONS]\n\nRun each command with --help for its options.");
            Ok(())
        }
        _ => Err("expected size, replay, blind, scale or types; see --help".into()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("compare: {error}");
            ExitCode::FAILURE
        }
    }
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| format!("{flag} needs a value").into())
}
