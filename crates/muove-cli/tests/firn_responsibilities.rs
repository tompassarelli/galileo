use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const WITHDRAWN: (&str, &str, &str) = ("obsidian", "is_realized_by", "obsidian-module");

fn triples() -> Vec<(String, String, String)> {
    let firn = std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!("{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9", std::env::var("HOME").unwrap())
    });
    let text = std::fs::read_to_string(format!("{firn}/hosts/whiterabbit/responsibilities.bnix")).unwrap();
    text.split('[')
        .filter_map(|chunk| {
            let words: Vec<&str> = chunk.split(']').next()?.split_whitespace().collect();
            match words.as_slice() {
                [s, p, o] if [s, p, o].iter().all(|w| w.starts_with(':')) => {
                    Some((s[1..].to_string(), p[1..].to_string(), o[1..].to_string()))
                }
                _ => None,
            }
        })
        .collect()
}

fn sentence((s, p, o): &(String, String, String)) -> Option<String> {
    let class = |class: &str| match class {
        "machines" => Some("Host"),
        "responsibilities" => Some("Responsibility"),
        "capabilities" => Some("Capability"),
        "software-units" => Some("Software"),
        "firn-modules" => Some("Module"),
        "realization-domains" => Some("Domain"),
        "criticalities" => Some("Criticality"),
        "relationships" => None,
        other => panic!("unknown class {other}"),
    };
    let verb = match p.as_str() {
        "member_of" => return class(o).map(|t| format!("{s} is a {t}")),
        "has_responsibility" => "has responsibility",
        "depends_on" => "depends on",
        "requires_capability" => "requires capability",
        "provides_capability" => "provides capability",
        "requires_software" => "requires software",
        "is_realized_by" => "is realized by",
        "is_realized_in" => "is realized in",
        "has_criticality" => "has criticality",
        other => panic!("unknown relation {other}"),
    };
    Some(format!("{s} {verb} {o}"))
}

fn check(name: &str, withdraw: Option<(&str, &str, &str)>) -> Output {
    let facts: Vec<String> = triples()
        .iter()
        .filter(|(s, p, o)| withdraw != Some((s.as_str(), p.as_str(), o.as_str())))
        .filter_map(sentence)
        .collect();
    let host: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.muove"));
    std::fs::write(&host, format!("use firn/responsibilities\n\n{}\n", facts.join("\n"))).unwrap();
    Command::new(env!("CARGO_BIN_EXE_muove"))
        .arg("check")
        .arg(&host)
        .output()
        .unwrap()
}

#[test]
fn a5_realization_holds_on_whiterabbit_and_fails_with_a_proof_after_the_withdrawal() {
    let holds = check("whiterabbit", None);
    let stdout = String::from_utf8_lossy(&holds.stdout);
    print!("{stdout}");
    assert_eq!(holds.status.code(), Some(0), "{}", String::from_utf8_lossy(&holds.stderr));
    assert!(stdout.contains("holds for all 2 of its instances"), "{stdout}");

    let fails = check("whiterabbit-withdrawn", Some(WITHDRAWN));
    let stdout = String::from_utf8_lossy(&fails.stdout);
    print!("{stdout}");
    assert_eq!(fails.status.code(), Some(1), "{}", String::from_utf8_lossy(&fails.stderr));
    assert!(
        stdout.contains("firn/responsibilities:44: whiterabbit must realize obsidian, but not obsidian has a realizing module."),
        "{stdout}"
    );
}
