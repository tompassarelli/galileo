use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[path = "../firn-tags/fixture.rs"]
mod fixture;

const FILES: [&str; 7] = [
    "modules/protonup-qt/default.nix",
    "modules/claude/default.nix",
    "modules/users/users.nix",
    "modules/beagle/default.nix",
    "modules/kanata/default.nix",
    "modules/kanata/kanata.nix",
    "flake.nix",
];

const MODULES: [&str; 64] = [
    "modules/agent-runtime-update/default.nix",
    "modules/airplane/default.nix",
    "modules/alsa-lib/default.nix",
    "modules/beagle/default.nix",
    "modules/capacity-watchdog/default.nix",
    "modules/checkout-sync/default.nix",
    "modules/claude/default.nix",
    "modules/codex/default.nix",
    "modules/codex-runtime/default.nix",
    "modules/containers/default.nix",
    "modules/containers/devshell-flake.nix",
    "modules/firefox/default.nix",
    "modules/firefox/firefox.nix",
    "modules/github-pat/default.nix",
    "modules/glide/default.nix",
    "modules/glide/glide.nix",
    "modules/job-run/default.nix",
    "modules/job-watch/default.nix",
    "modules/kanata/default.nix",
    "modules/kanata/kanata.nix",
    "modules/keyring-headless/default.nix",
    "modules/lane-gc/default.nix",
    "modules/laptop-runner/default.nix",
    "modules/machine-update/default.nix",
    "modules/model-watch/default.nix",
    "modules/networkd/default.nix",
    "modules/nexus-alerts/default.nix",
    "modules/nexus-backup/default.nix",
    "modules/nexus-checks/default.nix",
    "modules/nexus-hardening/default.nix",
    "modules/nexus-matrix/default.nix",
    "modules/nexus-pull/default.nix",
    "modules/niri/default.nix",
    "modules/niri/swaybg.nix",
    "modules/niri/swayidle.nix",
    "modules/niri/xwayland-satellite.nix",
    "modules/plato/default.nix",
    "modules/poppler-utils/default.nix",
    "modules/procps/default.nix",
    "modules/proton-log-watchdog/default.nix",
    "modules/protonup-qt/default.nix",
    "modules/quickshell/default.nix",
    "modules/river/default.nix",
    "modules/skill-review-queue/default.nix",
    "modules/socat/default.nix",
    "modules/socrates-chat/default.nix",
    "modules/sops-host-key/default.nix",
    "modules/styling/default.nix",
    "modules/stylix/default.nix",
    "modules/stylix/fonts.nix",
    "modules/stylix/stylix.nix",
    "modules/stylix/xdg-portal.nix",
    "modules/supply-chain-scan/default.nix",
    "modules/theme-switcher/default.nix",
    "modules/update-notify/default.nix",
    "modules/users/default.nix",
    "modules/users/users.nix",
    "modules/vast-reaper/default.nix",
    "modules/walker/default.nix",
    "modules/watch-alerts/default.nix",
    "modules/wg-nexus/default.nix",
    "modules/worker-ledger/default.nix",
    "modules/zen-browser/default.nix",
    "modules/zen-browser/zen-browser.nix",
];

const HOSTS: [&str; 3] = [
    "hardware-configuration.nix",
    "hosts/whiterabbit/configuration.nix",
    "template/hosts/my-machine/configuration.nix",
];

#[test]
fn f3_f4_committed_firn_constructs_rebuild_against_d59_reference() {
    let (summary, _, outcome) = rebuild(&FILES, "firn-constructs");
    println!("firn construct coverage: 8 required constructs, {summary}");
    outcome.unwrap();
}

#[test]
fn every_bnix_module_output_rebuilds_byte_identical() {
    let (summary, shapes, outcome) = rebuild(&MODULES, "firn-modules");
    println!("firn module rows: {summary}, {} nix module, {} nix file", shapes[0], shapes[1]);
    outcome.unwrap();
}

#[test]
fn every_bnix_host_output_rebuilds_byte_identical() {
    let (summary, shapes, outcome) = rebuild(&HOSTS, "firn-hosts");
    println!("firn host rows: {summary}, {} nix module, {} nix file", shapes[0], shapes[1]);
    outcome.unwrap();
}

fn rebuild(files: &[&str], directory: &str) -> (String, [usize; 2], Result<(), String>) {
    let root = PathBuf::from(fixture::firn_root());
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join(directory);
    std::fs::create_dir_all(&scratch).unwrap();
    let mut compared = 0;
    let mut equal = 0;
    let mut parse_equal = 0;
    let mut formatted = 0;
    let mut rejected = 0;
    let mut eval_equal = 0;
    let mut mismatches = 0;
    let mut failures = Vec::new();
    let mut shapes = [0, 0];
    for (index, file) in files.iter().enumerate() {
        let original = root.join(file);
        let imported = Command::new(env!("CARGO_BIN_EXE_galileo"))
            .args(["import", "nix"])
            .arg(&original)
            .output()
            .unwrap();
        if !imported.status.success() {
            rejected += 1;
            failures.push(format!(
                "{file}: import: {}",
                String::from_utf8_lossy(&imported.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
            ));
            continue;
        }
        let source = scratch.join(format!("case-{index}.gal"));
        match imported.stdout.split(|&b| b == b'\n').find(|line| line.starts_with(b"nix ")) {
            Some(line) if line.starts_with(b"nix module") => shapes[0] += 1,
            Some(line) if line.starts_with(b"nix file") => shapes[1] += 1,
            _ => {}
        }
        std::fs::write(&source, imported.stdout).unwrap();
        let format = Command::new(env!("CARGO_BIN_EXE_galileo"))
            .args(["fmt", "--check"])
            .arg(&source)
            .output()
            .unwrap();
        if format.status.success() {
            formatted += 1;
            println!("{file}: fmt --check exit 0");
        } else {
            failures.push(format!(
                "{file}: fmt --check: {}",
                String::from_utf8_lossy(&format.stderr)
            ));
        }
        let host = file.strip_prefix("hosts/").and_then(|f| f.strip_suffix("/configuration.nix"));
        let built = match host {
            Some(host) => host_build(&root, &scratch.join(format!("host-{index}")), host, &std::fs::read(&source).unwrap()),
            None => Command::new(env!("CARGO_BIN_EXE_galileo"))
                .args(["build", "--stdout"])
                .arg(&source)
                .output()
                .unwrap(),
        };
        if !built.status.success() {
            rejected += 1;
            failures.push(format!(
                "{file}: build: {}",
                String::from_utf8_lossy(&built.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
            ));
            continue;
        }
        let expected = std::fs::read(&original).unwrap();
        compared += 1;
        if *file == "flake.nix" {
            let mut parsed = Vec::new();
            for text in [&expected, &built.stdout] {
                let mut child = Command::new("nix-instantiate")
                    .args(["--parse", "-"])
                    .current_dir(&root)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                child.stdin.take().unwrap().write_all(text).unwrap();
                let output = child.wait_with_output().unwrap();
                assert!(output.status.success(), "{file}: Nix parse: {}", String::from_utf8_lossy(&output.stderr));
                parsed.push(output.stdout);
            }
            if parsed[0] == parsed[1] {
                parse_equal += 1;
                println!("{file}: parse-identical, stdin in {}", root.display());
            } else {
                mismatches += 1;
                failures.push(format!("{file}: parsed Nix differs"));
            }
        } else if host.is_some() {
            if eval_equal_files(&scratch.join(format!("host-{index}")), &expected, &built.stdout) {
                eval_equal += 1;
                println!("{file}: eval-equal under stub config, lib and pkgs");
            } else {
                mismatches += 1;
                failures.push(format!("{file}: evaluated Nix differs"));
            }
        } else if expected == built.stdout {
            equal += 1;
            println!("{file}: {} bytes, byte-identical", expected.len());
        } else {
            mismatches += 1;
            let first = expected
                .iter()
                .zip(&built.stdout)
                .position(|(a, b)| a != b)
                .unwrap_or(expected.len().min(built.stdout.len()));
            failures.push(format!(
                "{file}: byte {first}, original {} bytes, rebuilt {} bytes",
                expected.len(),
                built.stdout.len()
            ));
        }
    }
    for failure in &failures {
        println!("{failure}");
    }
    let summary = format!("{} files, {compared} comparisons, {equal} byte-identical, {parse_equal} parse-identical, {eval_equal} eval-equal, {formatted} fmt checks, {mismatches} mismatches, {rejected} rejected files", files.len());
    let complete = failures.is_empty() && compared == files.len() && formatted == files.len();
    (summary, shapes, complete.then_some(()).ok_or(failures.join("\n")))
}

fn host_build(root: &Path, directory: &Path, host: &str, imported: &[u8]) -> std::process::Output {
    let _ = std::fs::remove_dir_all(directory);
    std::fs::create_dir_all(directory.join(".git")).unwrap();
    std::fs::write(directory.join("configuration.gal"), imported).unwrap();
    let tags = std::fs::read_to_string(root.join(format!("hosts/{host}/enabled-tags.bnix"))).unwrap();
    let program = format!(
        "use firn/tags\n\n{}{}",
        fixture::module_sources(root),
        galileo_import::bnix_tags::host_facts(host, &tags).unwrap()
    );
    std::fs::write(directory.join("tags.gal"), program).unwrap();
    let mut output = Command::new(env!("CARGO_BIN_EXE_galileo")).arg("build").arg(directory).output().unwrap();
    if output.status.success() {
        output.stdout = std::fs::read(directory.join("configuration.nix")).unwrap();
    }
    output
}

fn eval_equal_files(directory: &Path, expected: &[u8], built: &[u8]) -> bool {
    std::fs::write(directory.join("a.nix"), expected).unwrap();
    std::fs::write(directory.join("b.nix"), built).unwrap();
    let text = String::from_utf8_lossy(expected);
    let mut members: Vec<String> = text
        .match_indices("lib.")
        .filter(|(at, _)| !text[..*at].ends_with(|c: char| c.is_alphanumeric() || c == '.' || c == '_'))
        .map(|(at, _)| text[at + 4..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect())
        .filter(|m: &String| !m.is_empty())
        .collect();
    members.sort();
    members.dedup();
    let lib: String = members.iter().map(|m| format!("{m} = v: {{ _{m} = v; }}; ")).collect();
    let expression = format!(
        "let args = {{ config = {{ }}; pkgs = {{ }}; lib = {{ {lib}}}; }}; in import ./a.nix args == import ./b.nix args"
    );
    let output = Command::new("nix-instantiate")
        .args(["--eval", "--strict", "--expr", &expression])
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(output.status.success(), "eval: {}", String::from_utf8_lossy(&output.stderr));
    output.stdout.trim_ascii() == b"true"
}
