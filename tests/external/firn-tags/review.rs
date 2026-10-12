#[path = "../../golden/harness.rs"]
mod harness;

use galileo_import::bnix_tags::{catalog, clause_facts, host_facts, module_facts, platform_facts};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

fn firn_root() -> String {
    std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!("{}/code/nixos-config/pins/4bcfad4cdb806dd7f66401dad5581037cb996ff0", std::env::var("HOME").unwrap())
    })
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn git(repo: &Path, arguments: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "user.name=review", "-c", "user.email=review@example.invalid", "-c", "commit.gpgsign=false"])
        .args(arguments)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {arguments:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn untagged_repo(root: &Path, name: &str) -> std::path::PathBuf {
    let repo = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&repo);
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    let mut names: Vec<String> =
        std::fs::read_dir(root.join("modules")).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    let mut all = String::new();
    for name in &names {
        let directory = root.join("modules").join(name);
        let facts = if let Ok(clause) = std::fs::read_to_string(directory.join("tags.clause")) {
            clause_facts(name, &clause).unwrap()
        } else if let Ok(text) = std::fs::read_to_string(directory.join("default.bnix")) {
            module_facts(name, &text).unwrap()
        } else {
            continue;
        };
        write(&repo.join(format!("modules/{name}/default.gal")), &format!("use firn/tags\n\n{facts}"));
        all += &facts;
    }
    let tags = std::fs::read_to_string(root.join("hosts/whiterabbit/enabled-tags.bnix")).unwrap();
    let platforms = platform_facts(&std::fs::read_to_string(root.join("config/darwin-modules.json")).unwrap()).unwrap();
    write(&repo.join("config/tags.gal"), &format!("use firn/tags\n\n{}{platforms}", catalog(&all, &[&tags]).unwrap()));
    write(
        &repo.join("hosts/whiterabbit/_generated-enables.gal"),
        &format!(
            "use firn/tags\n\n{}\nnix module\n  arguments: config, lib, pkgs, ...\n  for ?module where ?module is tag-enabled on whiterabbit\n    myConfig.modules.{{?module}}.enable: lib.mkDefault(true)\n",
            host_facts("whiterabbit", &tags).unwrap()
        ),
    );
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "base"]);
    let firefox = repo.join("modules/firefox/default.gal");
    let text = std::fs::read_to_string(&firefox).unwrap();
    assert!(text.contains("\n  is tagged browsers\n"));
    std::fs::write(&firefox, text.replacen("\n  is tagged browsers\n", "\n", 1)).unwrap();
    git(&repo, &["commit", "-qam", "untag firefox"]);
    repo
}

fn section(stdout: &str, name: &str) -> Vec<String> {
    let rest = stdout.split(&format!("\n{name} (")).nth(1).or_else(|| stdout.strip_prefix(&format!("{name} ("))).unwrap();
    rest.lines().skip(1).take_while(|l| l.starts_with("  ")).map(|l| l.trim().to_owned()).collect()
}

fn galileo(repo: &Path, arguments: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).args(arguments).current_dir(repo).output().unwrap();
    (out.status.code(), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn reviewing_a_commit_that_removes_firefoxs_browsers_tag_turns_firefox_off() {
    let root = firn_root();
    let root = Path::new(&root);
    let repo = untagged_repo(root, "review-firefox-tag");

    let started = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["review", "--base", "HEAD~1"]).current_dir(&repo).output().unwrap();
    let elapsed = started.elapsed();
    let stdout = String::from_utf8_lossy(&out.stdout);
    println!("{stdout}review took {} ms", elapsed.as_millis());
    assert!(out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));

    let section = |name: &str| section(&stdout, name);
    let edits = section("edits");
    assert_eq!(edits.len(), 1, "{stdout}");
    assert!(edits[0].starts_with("removes the fact `firefox is tagged browsers`") && edits[0].contains("modules/firefox/default.gal"), "{stdout}");

    let hosts: BTreeSet<String> = section("per host").into_iter().filter(|l| l.contains(": turns ")).collect();
    let mut golden = harness::Golden::new();
    golden.compare(&harness::area("firn-tags").join("review-firefox-untagged-per-host.txt"), &hosts.iter().map(|line| format!("{line}\n")).collect::<String>());
    golden.finish();
    assert!(hosts.contains("whiterabbit: turns off firefox"));
    assert_eq!(section("generated files"), ["hosts/whiterabbit/_generated-enables.nix changes"], "{stdout}");
    assert_eq!(section("definitions read"), Vec::<String>::new(), "{stdout}");
}

#[test]
fn a_written_patch_untagging_firefox_is_approved_by_its_contract_and_matches_the_commits_consequences() {
    let root = firn_root();
    let repo = untagged_repo(Path::new(&root), "review-written-patch");
    let patches = Path::new(env!("CARGO_TARGET_TMPDIR")).join("review-written-patch-files");
    let patch = |name: &str, body: &str| {
        let path = patches.join(format!("{name}.gal"));
        write(&path, &format!("patch {name}\n{body}"));
        path.display().to_string()
    };
    let untag = |claim: &str| format!("  says \"firefox leaves the browsers tag\"\n  remove\n    firefox is tagged browsers\n  intends\n    {claim}\n");

    let (code, base, _) = galileo(&repo, &["review", "--base", "HEAD~1"]);
    assert_eq!(code, Some(0), "{base}");

    let leaves = patch("firefox-leaves-browsers", &untag("firefox is enabled on whiterabbit becomes false"));
    let (code, stdout, stderr) = galileo(&repo, &["review", "--base", "HEAD~1", &leaves]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    assert!(stdout.contains("\nverdict: approve\n") && stdout.contains("firefox leaves the browsers tag"), "{stdout}");
    let edits = section(&stdout, "edits");
    assert!(edits.len() == 1 && edits[0].contains("firefox is tagged browsers") && edits[0].ends_with("patch:4"), "{stdout}");
    assert_eq!(section(&stdout, "intends"), ["holds: firefox is enabled on whiterabbit becomes false"], "{stdout}");
    assert_eq!(section(&stdout, "per host"), section(&base, "per host"));
    assert_eq!(section(&stdout, "generated files"), section(&base, "generated files"));

    let stays = patch("firefox-stays", &untag("firefox is enabled on whiterabbit stays"));
    let (code, stdout, stderr) = galileo(&repo, &["review", "--base", "HEAD~1", &stays]);
    assert_eq!(code, Some(1), "{stdout}{stderr}");
    assert!(stdout.ends_with("verdict: reject\n"), "{stdout}");
    assert!(section(&stdout, "intends")[0].starts_with("fails: firefox is enabled on whiterabbit"), "{stdout}");

    let never = patch("never", "  remove\n    firefox is tagged browsers\n    whiterabbit turns on firefox\n");
    let (code, stdout, stderr) = galileo(&repo, &["review", "--base", "HEAD~1", &never]);
    assert_eq!(code, Some(2), "{stdout}{stderr}");
    assert!(stderr.starts_with(&format!("{never}:4:")) && !stdout.contains("verdict:"), "{stdout}{stderr}");

    let tags = include_str!("../../../lib/firn/tags.gal");
    let rule = tags.split("rule tag-enabled: ").nth(1).unwrap().split("\n\n").next().unwrap();
    let body: String = rule.lines().map(|line| format!("    {}\n", line.trim())).collect();
    let library = patch("library", &format!("  replace rule tag-enabled\n{body}"));
    let (code, stdout, stderr) = galileo(&repo, &["review", &library]);
    assert_eq!(code, Some(2), "{stdout}{stderr}");
    assert!(stderr.contains("`firn/tags`") && !stdout.contains("verdict:"), "{stdout}{stderr}");

    let (code, _, stderr) = galileo(&repo, &["review"]);
    assert_eq!(code, Some(2));
    assert!(stderr.starts_with("usage: "), "{stderr}");
}
