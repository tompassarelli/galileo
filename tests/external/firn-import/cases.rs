use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const FILES: [&str; 7] = [
    "modules/protonup-qt/default.nix",
    "modules/claude/default.nix",
    "modules/users/users.nix",
    "modules/beagle/default.nix",
    "modules/kanata/default.nix",
    "modules/kanata/kanata.nix",
    "flake.nix",
];

const MODULES: [&str; 40] = [
    "modules/agent-runtime-update/default.nix",
    "modules/alsa-lib/default.nix",
    "modules/beagle/default.nix",
    "modules/capacity-watchdog/default.nix",
    "modules/claude/default.nix",
    "modules/codex/default.nix",
    "modules/codex-runtime/default.nix",
    "modules/containers/default.nix",
    "modules/containers/devshell-flake.nix",
    "modules/firefox/default.nix",
    "modules/firefox/firefox.nix",
    "modules/glide/default.nix",
    "modules/glide/glide.nix",
    "modules/kanata/default.nix",
    "modules/kanata/kanata.nix",
    "modules/lane-gc/default.nix",
    "modules/machine-update/default.nix",
    "modules/niri/default.nix",
    "modules/niri/swaybg.nix",
    "modules/niri/swayidle.nix",
    "modules/niri/xwayland-satellite.nix",
    "modules/poppler-utils/default.nix",
    "modules/procps/default.nix",
    "modules/proton-log-watchdog/default.nix",
    "modules/protonup-qt/default.nix",
    "modules/river/default.nix",
    "modules/skill-review-queue/default.nix",
    "modules/socat/default.nix",
    "modules/styling/default.nix",
    "modules/stylix/default.nix",
    "modules/stylix/fonts.nix",
    "modules/stylix/stylix.nix",
    "modules/stylix/xdg-portal.nix",
    "modules/theme-switcher/default.nix",
    "modules/update-notify/default.nix",
    "modules/users/default.nix",
    "modules/users/users.nix",
    "modules/vast-reaper/default.nix",
    "modules/walker/default.nix",
    "modules/zen-browser/default.nix",
];

const HOSTS: [&str; 5] = [
    "hardware-configuration.nix",
    "hosts/ashashi/configuration.nix",
    "hosts/thinkpad-x1e/configuration.nix",
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
    let root = PathBuf::from(std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!(
            "{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9",
            std::env::var("HOME").unwrap()
        )
    }));
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join(directory);
    std::fs::create_dir_all(&scratch).unwrap();
    let mut compared = 0;
    let mut equal = 0;
    let mut parse_equal = 0;
    let mut formatted = 0;
    let mut rejected = 0;
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
        let built = Command::new(env!("CARGO_BIN_EXE_galileo"))
            .args(["build", "--stdout"])
            .arg(&source)
            .output()
            .unwrap();
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
    let summary = format!("{} files, {compared} comparisons, {equal} byte-identical, {parse_equal} parse-identical, {formatted} fmt checks, {mismatches} mismatches, {rejected} rejected files", files.len());
    let complete = failures.is_empty() && compared == files.len() && formatted == files.len();
    (summary, shapes, complete.then_some(()).ok_or(failures.join("\n")))
}
