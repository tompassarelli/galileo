use crate::{value, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

struct Unit {
    id: String,
    file: String,
    first: usize,
    last: usize,
}

fn units(path: &Path) -> Result<Vec<Unit>> {
    let source = fs::read_to_string(path)?;
    let mut lines = source.lines();
    let header: Vec<_> = lines.next().ok_or("empty units TSV")?.split('\t').collect();
    let column = |name: &str| -> Result<usize> {
        header
            .iter()
            .position(|field| *field == name)
            .ok_or_else(|| format!("missing TSV column: {name}").into())
    };
    let id = column("id")?;
    let file = column(if header.contains(&"pin_file") {
        "pin_file"
    } else {
        "file"
    })?;
    let first = column("start_line")?;
    let last = column("end_line")?;
    let mut result = Vec::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let row: Vec<_> = line.split('\t').collect();
        if row.len() != header.len() {
            return Err("unit row has the wrong field count".into());
        }
        let unit = Unit {
            id: row[id].into(),
            file: row[file].into(),
            first: row[first].parse()?,
            last: row[last].parse()?,
        };
        if unit.first == 0 || unit.last < unit.first {
            return Err("unit range must be inclusive and one-based".into());
        }
        result.push(unit);
    }
    Ok(result)
}

fn token_character(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '#' | '\\')
}

fn citation(token: &str, units: &[Unit]) -> Option<String> {
    let token = token.trim_end_matches(['.', ':']);
    let (file, location) = token
        .split_once(':')
        .or_else(|| token.split_once("#L"))
        .map_or((token, None), |(file, line)| (file, Some(line)));
    let extension = file.rsplit_once('.').map(|(_, extension)| extension);
    if !file.contains(['/', '\\'])
        && !extension.is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "gal"
                    | "bnix"
                    | "clause"
                    | "nix"
                    | "ncl"
                    | "cue"
                    | "dhall"
                    | "k"
                    | "bjs"
                    | "json"
                    | "txt"
                    | "sh"
                    | "rs"
            )
        })
    {
        return None;
    }
    let range = location.and_then(|line| {
        let line = line.split(':').next()?;
        let mut numbers = line.split(['-', '#']);
        let first: usize = numbers.next()?.trim_start_matches('L').parse().ok()?;
        let last = numbers
            .next()
            .and_then(|part| part.trim_start_matches('L').parse().ok())
            .unwrap_or(first);
        Some((first, last))
    });
    let ids: BTreeSet<_> = units
        .iter()
        .filter(|unit| {
            (file == unit.file || file.ends_with(&format!("/{}", unit.file)))
                && range.is_none_or(|(first, last)| first <= unit.last && last >= unit.first)
        })
        .map(|unit| unit.id.as_str())
        .collect();
    Some(if !ids.is_empty() {
        ids.into_iter().collect::<Vec<_>>().join(", ")
    } else if file.to_ascii_lowercase().contains("generated") {
        "a generated file".into()
    } else {
        "another file".into()
    })
}

fn scrub(source: &str, units: &[Unit]) -> Result<String> {
    let commands = [
        "galileo why not",
        "galileo why",
        "firn tag resolve",
        "firn module status",
        "nix eval",
        "nickel query",
        "cue eval",
        "kcl run",
    ];
    let mut source = source.to_owned();
    for command in commands {
        while let Some(at) = source.to_ascii_lowercase().find(command) {
            source.replace_range(at..at + command.len(), "");
        }
    }
    let mut result = String::new();
    let mut at = 0;
    while at < source.len() {
        let ch = source[at..].chars().next().unwrap();
        if !token_character(ch) {
            result.push(ch);
            at += ch.len_utf8();
            continue;
        }
        let end = source[at..]
            .char_indices()
            .find(|(_, ch)| !token_character(*ch))
            .map_or(source.len(), |(offset, _)| at + offset);
        let token = &source[at..end];
        if let Some(description) = citation(token, units) {
            result.push_str(&description);
        } else if !matches!(
            token.trim_matches(['.', ':']).to_ascii_lowercase().as_str(),
            "galileo"
                | "firn"
                | "clause"
                | "beagle"
                | "clause-beagle"
                | "nix"
                | "nickel"
                | "cue"
                | "dhall"
                | "kcl"
                | "dhall-json"
                | "why"
                | "query"
                | "eval"
                | "--json"
                | "--via"
                | "-e"
                | "-s"
        ) {
            if token.split(['-', '_', '.']).any(|part| {
                matches!(
                    part.to_ascii_lowercase().as_str(),
                    "galileo"
                        | "firn"
                        | "clause"
                        | "beagle"
                        | "nix"
                        | "nickel"
                        | "cue"
                        | "dhall"
                        | "kcl"
                )
            }) {
                return Err(format!("answer names a column: {token}").into());
            }
            result.push_str(token);
        }
        at = end;
    }
    Ok(result)
}

pub fn run(args: Vec<String>) -> Result<()> {
    let mut bench = PathBuf::from("bench");
    let mut answers = None;
    let mut out = None;
    let mut seal = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bench" => bench = PathBuf::from(value(&mut args, &arg)?),
            "--answers" => answers = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--out" => out = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--seal" => seal = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--help" | "-h" => {
                println!("Usage: compare blind [--bench BENCH] [--answers DIR] [--out DIR] [--seal FILE]\n\nDefaults: bench/questions/answers, bench/questions/blinded, bench/questions/seal.tsv.\nRead COLUMN/Q<n>.txt answers and bench/COLUMN/units.tsv. Print blinded answers\nand write one file per question; create a private letter map without overwriting it.");
                return Ok(());
            }
            _ => return Err(format!("unknown option: {arg}").into()),
        }
    }
    let answers = answers.unwrap_or_else(|| bench.join("questions/answers"));
    let out = out.unwrap_or_else(|| bench.join("questions/blinded"));
    let seal = seal.unwrap_or_else(|| bench.join("questions/seal.tsv"));
    let mut questions: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for column in [
        "galileo",
        "clause-beagle",
        "nix",
        "nickel",
        "cue",
        "dhall",
        "kcl",
    ] {
        let root = answers.join(column);
        if !root.exists() {
            continue;
        }
        let units = units(&bench.join(column).join("units.tsv"))?;
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("txt") {
                continue;
            }
            let question = path
                .file_stem()
                .and_then(|name| name.to_str())
                .ok_or("invalid answer name")?;
            if !question.starts_with('Q') || question[1..].parse::<usize>().is_err() {
                return Err("answer name must be Q<n>.txt".into());
            }
            let answer = scrub(&fs::read_to_string(&path)?, &units)?;
            questions
                .entry(question.into())
                .or_default()
                .push((column.into(), answer));
        }
    }
    if questions.is_empty() {
        return Err("no answers found".into());
    }
    let mut random = File::open("/dev/urandom")?;
    let mut mapping = String::from("question\tletter\tcolumn\n");
    let mut blinded = Vec::new();
    for (question, mut answers) in questions {
        for index in (1..answers.len()).rev() {
            let mut bytes = [0; 8];
            random.read_exact(&mut bytes)?;
            answers.swap(
                index,
                (u64::from_ne_bytes(bytes) % (index as u64 + 1)) as usize,
            );
        }
        let mut text = format!("{question}\n");
        for (index, (column, answer)) in answers.iter().enumerate() {
            let letter = char::from(b'A' + index as u8);
            mapping.push_str(&format!("{question}\t{letter}\t{column}\n"));
            text.push_str(&format!("\n{letter}\n{}\n", answer.trim()));
        }
        blinded.push((question, text));
    }
    if out.exists() {
        return Err("blinded output directory already exists".into());
    }
    let mut sealed = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(seal)?;
    sealed.write_all(mapping.as_bytes())?;
    fs::create_dir_all(&out)?;
    for (question, text) in blinded {
        fs::write(out.join(format!("{question}.txt")), &text)?;
        print!("{text}");
    }
    Ok(())
}
