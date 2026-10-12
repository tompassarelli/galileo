use crate::{value, Result};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    fn read(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)?;
        let mut lines = source.lines();
        let header: Vec<_> = lines
            .next()
            .ok_or("empty TSV")?
            .split('\t')
            .map(str::to_owned)
            .collect();
        let mut rows = Vec::new();
        for (index, line) in lines.enumerate() {
            let row: Vec<_> = line.split('\t').map(str::to_owned).collect();
            if row.len() != header.len() || row.iter().any(String::is_empty) {
                return Err(format!(
                    "{}:{}: expected {} nonempty fields",
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

fn counts(packet: &[u8]) -> (usize, usize) {
    (
        packet.iter().filter(|byte| **byte == b'\n').count(),
        packet.len(),
    )
}

fn source_tree(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            source_tree(&entry.path())?;
        } else if !kind.is_file() || entry.path().extension().is_none_or(|ext| ext != "gal") {
            return Err(format!(
                "source tree needs only Galileo files: {}",
                entry.path().display()
            )
            .into());
        }
    }
    Ok(())
}

fn prepare(
    sources: &Path,
    reviews: &Path,
    intents: &Path,
    guarantees: &Path,
    out: &Path,
    cards: &Table,
    id: usize,
    scaled: bool,
) -> Result<()> {
    if out.exists() {
        return Err("packet output directory already exists".into());
    }
    let text = fs::read_to_string(intents)?;
    let words: Vec<_> = text
        .lines()
        .filter(|line| (1..=4).any(|number| line.starts_with(&format!("{number}. **I{number} — "))))
        .collect();
    if words.len() != 4 {
        return Err("intents must contain the four numbered I1–I4 statements".into());
    }
    let prefix = format!("{}\n\n", words.join("\n"));
    let guarantees = fs::read(guarantees)?;
    let mut packets = Vec::new();
    for card in &cards.rows {
        let name = &card[id];
        source_tree(&sources.join(name).join("base"))?;
        source_tree(&sources.join(name).join("patched"))?;
        let diff = Command::new("diff")
            .args(["-ru", "base", "patched"])
            .current_dir(sources.join(name))
            .output()?;
        if diff.status.code() != Some(1) || !diff.stdout.windows(3).any(|bytes| bytes == b"@@ ") {
            return Err(format!(
                "{name}: diff -ru must produce a source hunk (exit {:?}): {}",
                diff.status.code(),
                String::from_utf8_lossy(&diff.stderr)
            )
            .into());
        }
        if scaled
            && diff
                .stdout
                .split(|byte| *byte == b'\n')
                .filter(|line| line.starts_with(b"@@ "))
                .count()
                < 210
        {
            return Err(format!(
                "{name}: scaled implementation packet needs at least 210 diff hunks"
            )
            .into());
        }
        let stdout = fs::read(reviews.join(format!("{name}.stdout")))?;
        let stderr = fs::read(reviews.join(format!("{name}.stderr")))?;
        let output = std::str::from_utf8(&stdout)?;
        if !scaled
            && !output.lines().any(|line| {
                matches!(
                    line,
                    "verdict: approve"
                        | "verdict: reject"
                        | "verdict: unknown"
                        | "verdict: approve: no new failures"
                )
            })
        {
            return Err(format!("{name}: native review stdout has no complete verdict").into());
        }
        let mut implementation = prefix.as_bytes().to_vec();
        implementation.extend_from_slice(&diff.stdout);
        let mut requirement = prefix.as_bytes().to_vec();
        requirement.extend_from_slice(&guarantees);
        requirement.push(b'\n');
        requirement.extend_from_slice(&stdout);
        requirement.extend_from_slice(&stderr);
        if requirement
            .split(|byte| *byte == b'\n')
            .any(|line| line.starts_with(b"@@"))
        {
            return Err(format!("{name}: requirement packet contains a diff hunk").into());
        }
        packets.push((name, "I", implementation));
        packets.push((name, "R", requirement));
    }
    fs::create_dir_all(out)?;
    let size = if scaled { 210 } else { 10 };
    println!("card\tcondition\tsize\tlines\tbytes\tpacket");
    for (card, condition, packet) in packets {
        let path = out.join(format!("{card}-{condition}.txt"));
        fs::write(&path, &packet)?;
        let (lines, bytes) = counts(&packet);
        println!(
            "{card}\t{condition}\t{size}\t{lines}\t{bytes}\t{}",
            path.display()
        );
    }
    Ok(())
}

fn median(values: &mut [u64]) -> f64 {
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        values[middle - 1] as f64 / 2.0 + values[middle] as f64 / 2.0
    } else {
        values[middle] as f64
    }
}

fn score(
    answers: &Path,
    out: &Path,
    results: &Path,
    cards: &Table,
    id: usize,
    scaled: bool,
) -> Result<()> {
    let answers = Table::read(answers)?;
    let answer_card = answers.column("card")?;
    let condition = answers.column("condition")?;
    let verdict = answers.column("verdict")?;
    let intent = answers.column("intent")?;
    let tokens = answers.column("tokens")?;
    let wall = answers.column("wall_ms")?;
    let kind = cards.column("kind")?;
    let mut seen = BTreeSet::new();
    let header = "card\tcondition\tsize\tlines\tbytes\tverdict\tintent\tright\ttokens\twall_ms\n";
    let mut rows = String::from(if scaled { "" } else { header });
    let mut sample = [
        (Vec::new(), Vec::new(), Vec::new()),
        (Vec::new(), Vec::new(), Vec::new()),
    ];
    if scaled {
        let original = fs::read_to_string(results)?;
        let table = Table::read(results)?;
        if !original.starts_with(header) || !original.ends_with('\n') || table.rows.len() != 18 {
            return Err(
                "scaled scoring requires the unchanged eighteen-row sample result table".into(),
            );
        }
        let size = table.column("size")?;
        let condition = table.column("condition")?;
        let tokens = table.column("tokens")?;
        let bytes = table.column("bytes")?;
        let wall = table.column("wall_ms")?;
        for row in &table.rows {
            if row[size] != "10" {
                return Err(
                    "scaled scoring requires only size-10 sample rows before appending".into(),
                );
            }
            let index = match row[condition].as_str() {
                "I" => 0,
                "R" => 1,
                other => return Err(format!("unknown sample condition: {other}").into()),
            };
            sample[index].0.push(row[tokens].parse::<u64>()?);
            sample[index].1.push(row[bytes].parse::<u64>()?);
            sample[index].2.push(row[wall].parse::<u64>()?);
        }
        if sample.iter().any(|total| total.0.len() != 9) {
            return Err("scaled scoring requires nine sample rows per condition".into());
        }
    }
    let size = if scaled { 210 } else { 10 };
    let mut totals = [
        (0usize, Vec::new(), Vec::new(), Vec::new()),
        (0usize, Vec::new(), Vec::new(), Vec::new()),
    ];
    for answer in &answers.rows {
        let name = &answer[answer_card];
        let card = cards
            .rows
            .iter()
            .find(|card| card[id] == *name)
            .ok_or_else(|| format!("unknown answer card: {name}"))?;
        let index = match answer[condition].as_str() {
            "I" => 0,
            "R" => 1,
            other => return Err(format!("unknown condition: {other}").into()),
        };
        if !seen.insert((name.clone(), index)) {
            return Err(format!("duplicate answer: {name} {}", answer[condition]).into());
        }
        if !matches!(answer[verdict].as_str(), "approve" | "reject") {
            return Err(format!("{name}: reviewer must answer approve or reject").into());
        }
        if !matches!(answer[intent].as_str(), "none" | "I1" | "I2" | "I3" | "I4")
            || (answer[verdict] == "reject") == (answer[intent] == "none")
        {
            return Err(format!("{name}: reject must name I1–I4; approve must name none").into());
        }
        let used_tokens: u64 = answer[tokens].parse()?;
        let wall_ms: u64 = answer[wall].parse()?;
        if used_tokens == 0 || wall_ms == 0 {
            return Err(
                format!("{name}: tokens and wall_ms must be actual positive measurements").into(),
            );
        }
        let packet = fs::read(out.join(format!("{name}-{}.txt", answer[condition])))?;
        let (lines, bytes) = counts(&packet);
        let right = answer[verdict]
            == if card[kind] == "breaking" {
                "reject"
            } else {
                "approve"
            };
        let total = &mut totals[index];
        total.0 += usize::from(right);
        total.1.push(used_tokens);
        total.2.push(bytes as u64);
        total.3.push(wall_ms);
        rows.push_str(&format!(
            "{name}\t{}\t{size}\t{lines}\t{bytes}\t{}\t{}\t{}\t{used_tokens}\t{wall_ms}\n",
            answer[condition],
            answer[verdict],
            answer[intent],
            if right { "right" } else { "wrong" }
        ));
    }
    if seen.len() != cards.rows.len() * 2 {
        return Err("answers must contain exactly one cold session per card and condition".into());
    }
    if scaled {
        fs::OpenOptions::new()
            .append(true)
            .open(results)?
            .write_all(rows.as_bytes())?;
    } else {
        fs::write(results, rows)?;
    }
    let medians = totals.each_mut().map(|total| {
        (
            median(&mut total.1),
            median(&mut total.2),
            median(&mut total.3),
        )
    });
    println!("condition\tright\ttotal\tmedian_tokens\tmedian_bytes\tmedian_wall_ms");
    for (index, condition) in ["I", "R"].into_iter().enumerate() {
        let total = &totals[index];
        println!(
            "{condition}\t{}\t{}\t{}\t{}\t{}",
            total.0,
            cards.rows.len(),
            medians[index].0,
            medians[index].1,
            medians[index].2
        );
    }
    println!(
        "{}\t{}",
        if scaled { "loss2" } else { "loss1" },
        if totals[1].0 < totals[0].0 || (scaled && medians[1].0 >= medians[0].0) {
            "tripped"
        } else {
            "not tripped"
        }
    );
    if scaled {
        println!("growth\tcondition\ttokens\tbytes\twall_ms");
        for (index, condition) in ["I", "R"].into_iter().enumerate() {
            println!(
                "growth\t{condition}\t{}\t{}\t{}",
                medians[index].0 / median(&mut sample[index].0),
                medians[index].1 / median(&mut sample[index].1),
                medians[index].2 / median(&mut sample[index].2)
            );
        }
    }
    Ok(())
}

pub fn run(args: Vec<String>) -> Result<()> {
    let mut sources = None;
    let mut reviews = None;
    let mut answers = None;
    let mut cards_path = PathBuf::from("bench/review/cases.tsv");
    let mut intents = PathBuf::from("bench/review/intents.md");
    let mut guarantees = PathBuf::from("bench/galileo/guarantees.gal");
    let mut out = PathBuf::from("bench/review/effort");
    let mut results = PathBuf::from("bench/results/review-effort.tsv");
    let mut scaled = false;
    let mut out_set = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sources" => sources = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--reviews" => reviews = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--answers" => answers = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--cards" => cards_path = PathBuf::from(value(&mut args, &arg)?),
            "--intents" => intents = PathBuf::from(value(&mut args, &arg)?),
            "--guarantees" => guarantees = PathBuf::from(value(&mut args, &arg)?),
            "--out" => {
                out = PathBuf::from(value(&mut args, &arg)?);
                out_set = true;
            }
            "--results" => results = PathBuf::from(value(&mut args, &arg)?),
            "--scaled" => scaled = true,
            "--help" | "-h" => {
                println!(
                    "Usage: compare packets --sources SOURCES --reviews REVIEWS [OPTIONS]\n       compare packets --answers ANSWERS.tsv [OPTIONS]\n\nSOURCES/<card>/base and patched contain only the actual Galileo sources.\nREVIEWS/<card>.stdout and .stderr contain the whole native review output.\nPackets contain only four intents plus diff -ru (I), or guarantees and output (R).\nAnswers need card, condition, verdict, intent, tokens, wall_ms from one cold\nsession per packet. Score verdicts against the committed card kinds.\nDefaults: --cards bench/review/cases.tsv, --intents bench/review/intents.md,\n--guarantees bench/galileo/guarantees.gal, --out bench/review/effort,\n--results bench/results/review-effort.tsv. The packet directory must be new.\n--scaled uses the three cards registered in bench/review/scale.md at size 210,\ndefaults --out to bench/review/effort/210, and appends six result rows."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown option: {arg}").into()),
        }
    }
    let mut cards = Table::read(&cards_path)?;
    let id = cards.column("id")?;
    let kind = cards.column("kind")?;
    let mut seen = BTreeSet::new();
    for card in &cards.rows {
        if card[id].is_empty()
            || !card[id]
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
            || !seen.insert(&card[id])
            || !matches!(card[kind].as_str(), "breaking" | "control")
        {
            return Err("cards need unique simple ids and breaking/control kinds".into());
        }
    }
    if scaled {
        let registration = fs::read_to_string("bench/review/scale.md")?;
        let selected: Vec<_> = registration
            .lines()
            .filter_map(|line| line.strip_prefix("Cards: "))
            .collect();
        if selected.len() != 1 {
            return Err("scale.md must register one Cards: line".into());
        }
        let names: Vec<_> = selected[0].split(", ").collect();
        if names.len() != 3 || names.iter().collect::<BTreeSet<_>>().len() != 3 {
            return Err("scale.md must register exactly three distinct cards".into());
        }
        cards.rows.retain(|card| names.contains(&card[id].as_str()));
        if cards.rows.len() != 3
            || cards
                .rows
                .iter()
                .filter(|card| card[kind] == "control")
                .count()
                != 1
            || cards
                .rows
                .iter()
                .filter(|card| {
                    card[kind] == "breaking"
                        && matches!(card[id].as_str(), "B1" | "B2" | "B3" | "B4")
                })
                .count()
                != 2
        {
            return Err(
                "scaled packets require two registered B1–B4 breaking cards and one control".into(),
            );
        }
        if !out_set {
            out.push("210");
        }
    } else if cards.rows.len() != 9 {
        return Err("sample packets require exactly nine cards".into());
    }
    if let Some(answers) = answers {
        if sources.is_some() || reviews.is_some() {
            return Err(
                "--answers scores existing packets; do not combine it with --sources or --reviews"
                    .into(),
            );
        }
        score(&answers, &out, &results, &cards, id, scaled)
    } else {
        prepare(
            &sources.ok_or("packets requires --sources")?,
            &reviews.ok_or("packets requires --reviews")?,
            &intents,
            &guarantees,
            &out,
            &cards,
            id,
            scaled,
        )
    }
}
