use galileo_import::bnix_tags::host_facts;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "../../../tests/external/firn-tags/fixture.rs"]
mod fixture;
use fixture::{firn_root, module_sources};

const WITHDRAWN: (&str, &str, &str) = ("obsidian", "is_realized_by", "obsidian-module");

const LIBRARY_DECLARED: [&str; 3] = ["boot is a Domain", "environment is a Domain", "long-lived-system-service is a Lifecycle"];

type Triple = (String, String, String);

fn triples() -> Vec<Triple> {
    let text = std::fs::read_to_string(format!("{}/hosts/whiterabbit/responsibilities.bnix", firn_root())).unwrap();
    bracketed(&text)
}

fn bracketed(text: &str) -> Vec<Triple> {
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

fn module(name: &str) -> &str {
    name.strip_suffix("-module").unwrap_or(name)
}

fn sentence((s, p, o): &Triple) -> Option<String> {
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
        "member_of" if o == "firn-modules" => return Some(format!("{} is a Module", module(s))),
        "member_of" => return class(o).map(|t| format!("{s} is a {t}")),
        "is_realized_by" => return Some(format!("{s} is realized by {}", module(o))),
        "has_responsibility" => "has responsibility",
        "depends_on" => "depends on",
        "requires_capability" => "requires capability",
        "provides_capability" => "provides capability",
        "requires_software" => "requires software",
        "is_realized_in" => "is realized in",
        "has_criticality" => "has criticality",
        "has_lifecycle" => "has lifecycle",
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
    run(name, &facts.join("\n"))
}

fn run(name: &str, facts: &str) -> Output {
    let host: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.gal"));
    let facts: Vec<&str> = facts.lines().filter(|line| !LIBRARY_DECLARED.contains(line)).collect();
    std::fs::write(&host, format!("use firn/responsibilities\nuse firn/tags\n\n{}\n", facts.join("\n"))).unwrap();
    Command::new(env!("CARGO_BIN_EXE_galileo"))
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
        stdout.contains("firn/responsibilities:106: whiterabbit must realize obsidian, but not obsidian has a realizing module."),
        "{stdout}"
    );
}

fn declarations(triples: &[Triple]) -> BTreeSet<String> {
    let slot = |p: &str| match p {
        "has_responsibility" => Some(("Host", "Responsibility")),
        "depends_on" => Some(("Responsibility", "Responsibility")),
        "requires_capability" => Some(("Responsibility", "Capability")),
        "provides_capability" => Some(("Software", "Capability")),
        "requires_software" => Some(("Responsibility", "Software")),
        "is_realized_by" => Some(("Software", "Module")),
        "is_realized_in" => Some(("Responsibility", "Domain")),
        "has_criticality" => Some(("Responsibility", "Criticality")),
        "has_lifecycle" => Some(("Responsibility", "Lifecycle")),
        _ => None,
    };
    triples
        .iter()
        .filter_map(|(s, p, o)| slot(p).map(|(st, ot)| [format!("{s} is a {st}"), format!("{} is a {ot}", if ot == "Module" { module(o) } else { o })]))
        .flatten()
        .collect()
}

fn keywords(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || c == ')' || c == '(').filter_map(|w| w.strip_prefix(':')).map(String::from).collect()
}

fn scenarios() -> Vec<(String, Vec<Triple>, Vec<String>)> {
    let text = std::fs::read_to_string(format!("{}/native/responsibility_projection_test.bjs", firn_root())).unwrap();
    let source = &text[text.find("(def source").unwrap()..text.find("(defn expected-obligations").unwrap()];
    let base = bracketed(source);
    let behavior = &text[text.find("(defn projection-behavior?").unwrap()..text.find("(defn module-names-from-paths").unwrap()];
    let mut variants = vec![("propositions".to_string(), base.clone())];
    let bindings: Vec<&str> = behavior.split(" (Vec t/Triple) (").skip(1).collect();
    let names: Vec<String> = behavior
        .split(" (Vec t/Triple) (")
        .map(|chunk| chunk.split_whitespace().last().unwrap().to_string())
        .collect();
    for (index, body) in bindings.iter().enumerate() {
        let body = body.split(" (Vec t/Triple) (").next().unwrap();
        let body = body.split("(and (= (projection/realization-obligations").next().unwrap();
        let mut triples = base.clone();
        for chunk in body.split("(t/triple").skip(1) {
            let words = keywords(chunk);
            triples.push((words[0].clone(), words[1].clone(), words[2].clone()));
        }
        variants.push((names[index].clone(), triples));
    }
    let mut seen = BTreeSet::new();
    behavior
        .split("(projection/assess-boot-coupling")
        .skip(1)
        .map(|call| {
            let mut words = call.split_whitespace();
            let variant = words.next().unwrap().to_string();
            let list = &call[call.find('[').unwrap() + 1..call.find(']').unwrap()];
            let modules = list.split('"').skip(1).step_by(2).map(String::from).collect();
            let name = if seen.insert(variant.clone()) { variant.clone() } else { format!("{variant}-blender") };
            let triples = variants.iter().find(|(n, _)| *n == variant).unwrap().1.clone();
            (name, triples, modules)
        })
        .collect()
}

fn scenario_facts(triples: &[Triple], boot: &[String]) -> String {
    let mut lines: Vec<String> = declarations(triples).into_iter().collect();
    lines.extend(triples.iter().filter(|(_, p, _)| p != "member_of").filter_map(sentence));
    lines.extend(["linux is a Platform", "linux allows every module", "boot-set is a Tag", "whiterabbit runs linux", "whiterabbit enables boot-set"].map(String::from));
    for m in boot {
        let declared = format!("{m} is a Module");
        if !lines.contains(&declared) {
            lines.push(declared);
        }
        lines.push(format!("{m} is tagged boot-set"));
    }
    lines.join("\n")
}

const BUDGET: [(&str, &str, &str); 5] = [
    ("environment module enabled", "for environment software ?software fails: ", "all ?software where ?host enables ?module for environment software ?software"),
    ("startup reaches user session", "reaching environment responsibility ?dependency fails: ", "all ?responsibility where ?host has boot responsibility ?responsibility reaching environment responsibility ?dependency"),
    ("incomplete software", "must realize incomplete software ?software for ?responsibility fails: ", "all ?software where ?host must realize incomplete software ?software for ?responsibility"),
    ("uncovered capability", "?responsibility gets capability ?capability fails: ", "all ?capability where ?host needs capability ?capability for ?responsibility and not ?responsibility gets capability ?capability"),
    ("inadmissible startup software", "as a long-lived system service for ?responsibility fails: ", "all ?software where ?host must start ?software at boot for ?responsibility and not ?host starts ?software as a long-lived system service for ?responsibility"),
];

fn budget_failures(name: &str, output: &Output) -> Vec<(String, BTreeSet<String>)> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let host = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.gal"));
    BUDGET
        .iter()
        .filter(|(_, marker, _)| stdout.contains(marker))
        .map(|(part, _, question)| {
            let rows = Command::new(env!("CARGO_BIN_EXE_galileo")).arg("query").arg(&host).arg(question).arg("--json").output().unwrap();
            assert!(rows.status.success(), "{question}: {}", String::from_utf8_lossy(&rows.stderr));
            let rows = String::from_utf8(rows.stdout).unwrap();
            let items = rows.trim().trim_matches(['[', ']']).split(',').map(|i| i.trim().trim_matches('"').to_string()).filter(|i| !i.is_empty()).collect();
            (part.to_string(), items)
        })
        .collect()
}

fn expect(parts: &[(&str, &[&str])]) -> Vec<(String, BTreeSet<String>)> {
    let mut expected: Vec<(String, BTreeSet<String>)> =
        parts.iter().map(|(p, items)| (p.to_string(), items.iter().map(|i| i.to_string()).collect())).collect();
    expected.sort_by_key(|(p, _)| BUDGET.iter().position(|(b, _, _)| b == p));
    expected
}

#[test]
fn boot_coupling_budget_fails_the_parts_the_merge_base_calculation_fails() {
    let root = firn_root();
    let root = Path::new(&root);
    let tags = module_sources(root)
        + &host_facts("whiterabbit", &std::fs::read_to_string(root.join("hosts/whiterabbit/enabled-tags.bnix")).unwrap()).unwrap();
    let declared: BTreeSet<&str> = tags.lines().collect();
    let facts: Vec<String> = triples().iter().filter_map(sentence).filter(|s| !declared.contains(s.as_str())).collect();
    let whiterabbit = run("whiterabbit-budget", &format!("{}\n{tags}", facts.join("\n")));
    print!("{}", String::from_utf8_lossy(&whiterabbit.stdout));
    assert_eq!(whiterabbit.status.code(), Some(1), "{}", String::from_utf8_lossy(&whiterabbit.stderr));
    assert_eq!(budget_failures("whiterabbit-budget", &whiterabbit), expect(&[("environment module enabled", &["blender", "obsidian"])]));

    let expected: [(&str, Vec<(String, BTreeSet<String>)>); 7] = [
        ("propositions", expect(&[])),
        ("propositions-blender", expect(&[("environment module enabled", &["blender"])])),
        ("dev-lifecycle", expect(&[("inadmissible startup software", &["project-toolchain"])])),
        ("unowned", expect(&[("incomplete software", &["unowned-toolchain"]), ("inadmissible startup software", &["unowned-toolchain"])])),
        ("inverted", expect(&[("startup reaches user session", &["boot-workflow"])])),
        ("incomplete", expect(&[("incomplete software", &["unrealized"])])),
        ("uncovered", expect(&[("uncovered capability", &["missing-capability"])])),
    ];
    let scenarios = scenarios();
    assert_eq!(scenarios.iter().map(|(n, _, _)| n.as_str()).collect::<Vec<_>>(), expected.iter().map(|(n, _)| *n).collect::<Vec<_>>());
    for ((name, triples, boot), (_, failures)) in scenarios.iter().zip(expected) {
        let output = run(name, &scenario_facts(triples, boot));
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("{name}: {:?}", budget_failures(name, &output));
        assert!(output.status.code().is_some_and(|c| c < 2), "{name}: {}{}", stdout, String::from_utf8_lossy(&output.stderr));
        assert_eq!(budget_failures(name, &output), failures, "{name}: {stdout}");
    }
    let host = Path::new(env!("CARGO_TARGET_TMPDIR")).join("boot-domain-warning.gal");
    std::fs::write(&host, "use firn/responsibilities\nuse firn/tags\nboot is a Domain\nwhiterabbit is a Host\nlinux is a Platform\nlinux allows every module\nwhiterabbit runs linux\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_galileo")).arg("check").arg(&host).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    assert!(stderr.contains("lib/firn/responsibilities.gal:5:1: warning: `boot` is already declared a Domain"), "{stderr}");
    for (kind, ty, verb) in [
        ("domain", "Domain", "is realized in"),
        ("criticality", "Criticality", "has criticality"),
        ("lifecycle", "Lifecycle", "has lifecycle"),
    ] {
        let output = run(
            &format!("duplicate-{kind}"),
            &format!("backup is a Responsibility\nfirst is a {ty}\nsecond is a {ty}\nbackup {verb} first\nbackup {verb} second"),
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{kind}: {stderr}");
        assert!(stderr.contains(&format!("at most one {kind} per responsibility")), "{stderr}");
        assert!(stderr.contains(&format!("line 7: backup {verb} first")), "{stderr}");
        assert!(stderr.contains(&format!("line 8: backup {verb} second")), "{stderr}");
    }

}
