use crate::{value, Result};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    fn read(path: &Path) -> Result<Self> {
        let source = std::fs::read_to_string(path)?;
        let mut lines = source.lines();
        let header: Vec<_> = lines
            .next()
            .ok_or("empty TSV")?
            .split('\t')
            .map(str::to_owned)
            .collect();
        let mut rows = Vec::new();
        for (index, line) in lines.enumerate() {
            if line.is_empty() {
                continue;
            }
            let row: Vec<_> = line.split('\t').map(str::to_owned).collect();
            if row.len() != header.len() {
                return Err(format!(
                    "{}:{}: expected {} fields",
                    path.display(),
                    index + 2,
                    header.len()
                )
                .into());
            }
            rows.push(row);
        }
        Ok(Self { header, rows })
    }
    fn column(&self, name: &str) -> Result<usize> {
        self.header
            .iter()
            .position(|field| field == name)
            .ok_or_else(|| format!("missing TSV column: {name}").into())
    }
}

fn check(root: &Path, args: &[String]) -> std::io::Result<Output> {
    Command::new(&args[0])
        .args(&args[1..])
        .current_dir(root)
        .output()
}

pub fn run(args: Vec<String>) -> Result<()> {
    let mut dry_run = false;
    let mut lang = String::from("clause-beagle");
    let mut root = None;
    let mut units = None;
    let mut cards = PathBuf::from("bench/corpus/cases.tsv");
    let mut checker = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--lang" => lang = value(&mut args, &arg)?,
            "--root" => root = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--units" => units = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--cards" => cards = PathBuf::from(value(&mut args, &arg)?),
            "--check" => {
                checker.extend(args);
                break;
            }
            "--help" | "-h" => {
                println!("Usage: compare replay [--dry-run] [--lang COLUMN] --root SOURCE_ROOT\n       [--units UNITS.tsv] [--cards CASES.tsv] [--check PROGRAM ARG...]\n\nDefaults: --lang clause-beagle, --units bench/<lang>/units.tsv,\n--cards bench/corpus/cases.tsv. Token cards need id, unit, anchor, replacement.\nDry run edits an in-memory scratch source and prints case, file, line, text.\nChecked replay requires a scratch root and --check; it restores each source.\nPut --check last. Each card runs independently against the original source.");
                return Ok(());
            }
            _ => return Err(format!("unknown option: {arg}").into()),
        }
    }
    let root = root.ok_or("replay requires --root")?.canonicalize()?;
    if !dry_run {
        if checker.is_empty() {
            return Err("checked replay requires --check PROGRAM ARG...".into());
        }
        if root
            .components()
            .any(|part| part == Component::Normal("pins".as_ref()))
        {
            return Err("checked replay requires a scratch root, never an immutable pin".into());
        }
    }
    let units =
        Table::read(&units.unwrap_or_else(|| PathBuf::from(format!("bench/{lang}/units.tsv"))))?;
    let cards = Table::read(&cards)?;
    let unit_id = units.column("id")?;
    let file = if units.header.iter().any(|name| name == "pin_file") {
        units.column("pin_file")?
    } else {
        units.column("file")?
    };
    let start = units.column("start_line")?;
    let end = units.column("end_line")?;
    let id = cards.column("id")?;
    let unit = cards.column("unit")?;
    let anchor = cards.column("anchor")?;
    let replacement = cards.column("replacement")?;
    if cards.rows.is_empty() {
        return Err("cards TSV contains no token cards".into());
    }
    if !dry_run && !check(&root, &checker)?.status.success() {
        return Err("column check failed on the unmutated source".into());
    }
    if dry_run {
        println!("case\tfile\tline\ttext");
    } else {
        println!("case\tcolumn\texit_status\tfirst_diagnostic\tnames_file\twall_ms");
    }
    for card in &cards.rows {
        if card[anchor].is_empty()
            || card[anchor].contains(['\r', '\n'])
            || card[replacement].contains(['\r', '\n'])
        {
            return Err(format!("{}: token edits must have a nonempty single-line anchor and single-line replacement", card[id]).into());
        }
        let mappings: Vec<_> = units
            .rows
            .iter()
            .filter(|row| row[unit_id] == card[unit])
            .collect();
        if mappings.is_empty() {
            return Err(format!("{}: unknown unit {}", card[id], card[unit]).into());
        }
        let mut matches = Vec::new();
        for mapping in mappings {
            let relative = Path::new(&mapping[file]);
            if relative.is_absolute()
                || relative
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
            {
                return Err(format!(
                    "unit file must be relative to source root: {}",
                    relative.display()
                )
                .into());
            }
            let path = root.join(relative).canonicalize()?;
            if !path.starts_with(&root) {
                return Err("unit file is outside source root".into());
            }
            let original = std::fs::read_to_string(&path)?;
            let first: usize = mapping[start].parse()?;
            let last: usize = mapping[end].parse()?;
            if first == 0 || last < first || last > original.lines().count() {
                return Err(
                    format!("{}: invalid unit range {first}..{last}", relative.display()).into(),
                );
            }
            let mut offset = 0;
            for (index, line) in original.split_inclusive('\n').enumerate() {
                if (first..=last).contains(&(index + 1)) {
                    for (position, _) in line.match_indices(&card[anchor]) {
                        matches.push((
                            path.clone(),
                            mapping[file].clone(),
                            original.clone(),
                            offset + position,
                            index + 1,
                        ));
                    }
                }
                offset += line.len();
            }
        }
        matches.sort_by(|a, b| (&a.0, a.3).cmp(&(&b.0, b.3)));
        matches.dedup_by(|a, b| a.0 == b.0 && a.3 == b.3);
        if matches.len() != 1 {
            return Err(format!(
                "{}: anchor matched {} sites in unit {}; expected exactly one",
                card[id],
                matches.len(),
                card[unit]
            )
            .into());
        }
        let (path, relative, original, position, line_number) = matches.pop().unwrap();
        let mut edited = original.clone();
        edited.replace_range(position..position + card[anchor].len(), &card[replacement]);
        if dry_run {
            let line = edited.lines().nth(line_number - 1).unwrap_or("");
            println!("{}\t{relative}\t{line_number}\t{line}", card[id]);
        } else {
            if let Err(error) = std::fs::write(&path, &edited) {
                std::fs::write(&path, &original)?;
                return Err(error.into());
            }
            let began = Instant::now();
            let output = check(&root, &checker);
            let wall = began.elapsed().as_millis();
            std::fs::write(&path, &original)?;
            let output = output?;
            let diagnostic = if output.stderr.is_empty() {
                &output.stdout
            } else {
                &output.stderr
            };
            let diagnostic = String::from_utf8_lossy(diagnostic);
            let first = diagnostic
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("");
            let names_file = first.contains(&relative);
            let first = first.replace(['\t', '\r'], " ");
            let status = output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".into());
            println!(
                "{}\t{lang}\t{status}\t{first}\t{names_file}\t{wall}",
                card[id]
            );
        }
    }
    Ok(())
}
