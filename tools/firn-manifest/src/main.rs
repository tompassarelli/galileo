use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

type Outcome = Result<String, String>;

fn main() -> ExitCode {
    let root = match env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| env::var_os("FIRN_ROOT").map(PathBuf::from))
    {
        Some(root) => root,
        None => {
            eprintln!("usage: firn-manifest <FIRN_ROOT>   (or set FIRN_ROOT)");
            return ExitCode::from(2);
        }
    };
    match run(&root) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("firn-manifest: {}: {error}", root.display());
            ExitCode::FAILURE
        }
    }
}

fn run(root: &Path) -> io::Result<()> {
    let mut files = Vec::new();
    walk(root, "", &mut files)?;
    files.sort();

    let bnix: Vec<&str> = files
        .iter()
        .map(String::as_str)
        .filter(|f| f.ends_with(".bnix"))
        .collect();
    let clause: Vec<&str> = files
        .iter()
        .map(String::as_str)
        .filter(|f| f.starts_with("native/nix/") && f.ends_with(".clause"))
        .collect();
    let tags: Vec<&str> = files
        .iter()
        .map(String::as_str)
        .filter(|f| f.starts_with("modules/") && f.ends_with("/tags.clause"))
        .collect();

    let clause_names: BTreeSet<&str> = clause
        .iter()
        .map(|s| &s["native/nix/".len()..s.len() - ".clause".len()])
        .collect();
    let tag_names: BTreeSet<&str> = tags
        .iter()
        .map(|s| &s["modules/".len()..s.len() - "/tags.clause".len()])
        .collect();

    let mut out = io::stdout().lock();
    let mut counts = [0usize; 6];

    for source in &bnix {
        let outcome = bnix_outcome(source);
        counts[0] += 1;
        counts[1] += usize::from(outcome.is_ok());
        write_row(&mut out, "bnix", source, "", &outcome)?;
    }
    for source in &clause {
        let name = &source["native/nix/".len()..source.len() - ".clause".len()];
        let outcome = if tag_names.contains(name) {
            Ok(module_output(name))
        } else {
            Err(format!("no modules/{name}/tags.clause, so the build selects no output"))
        };
        counts[2] += 1;
        counts[3] += usize::from(outcome.is_ok());
        write_row(&mut out, "module", source, "", &outcome)?;
    }
    for source in &tags {
        let name = &source["modules/".len()..source.len() - "/tags.clause".len()];
        let outcome = if clause_names.contains(name) {
            Ok(module_output(name))
        } else {
            Err(format!("no native/nix/{name}.clause, so the build rejects the module"))
        };
        counts[4] += 1;
        counts[5] += usize::from(outcome.is_ok());
        let extra = format!(" module={name}");
        write_row(&mut out, "tags", source, &extra, &outcome)?;
    }
    writeln!(
        out,
        "summary bnix={} with_output={} module={} with_output={} tags={} with_output={}",
        counts[0], counts[1], counts[2], counts[3], counts[4], counts[5]
    )
}

fn module_output(name: &str) -> String {
    format!("modules/{name}/default.nix")
}

fn bnix_outcome(source: &str) -> Outcome {
    let excluded = if source.starts_with("scripts/") {
        Some("under scripts/")
    } else if source.starts_with("tests/") {
        Some("under tests/")
    } else if source.starts_with("docs/fixtures/") {
        Some("under docs/fixtures/")
    } else if source == "enabled-tags.bnix" || source.ends_with("/enabled-tags.bnix") {
        Some("enabled-tags.bnix selects tags and is not a source")
    } else if source == "responsibilities.bnix" || source.ends_with("/responsibilities.bnix") {
        Some("responsibilities.bnix is not a source")
    } else {
        None
    };
    match excluded {
        Some(reason) => Err(format!("excluded: {reason}")),
        None => Ok(format!("{}.nix", &source[..source.len() - ".bnix".len()])),
    }
}

fn write_row(
    out: &mut impl Write,
    kind: &str,
    source: &str,
    extra: &str,
    outcome: &Outcome,
) -> io::Result<()> {
    match outcome {
        Ok(output) => writeln!(out, "{kind} {source}{extra} -> {output}"),
        Err(reason) => writeln!(out, "{kind} {source}{extra} -- none: {reason}"),
    }
}

fn excluded_dir(relative: &str) -> bool {
    matches!(relative, ".git" | ".direnv" | ".firn-build") || relative.starts_with("result")
}

fn walk(root: &Path, relative: &str, files: &mut Vec<String>) -> io::Result<()> {
    let directory = if relative.is_empty() {
        root.to_path_buf()
    } else {
        root.join(relative)
    };
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = if relative.is_empty() {
            name
        } else {
            format!("{relative}/{name}")
        };
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if !excluded_dir(&child) {
                walk(root, &child, files)?;
            }
        } else if file_type.is_file() {
            files.push(child);
        }
    }
    Ok(())
}
