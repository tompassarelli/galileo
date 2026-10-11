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

fn active(resolve: &str) -> BTreeSet<String> {
    let section = resolve.split("\nActive modules").nth(1).unwrap();
    section.lines().skip(1).filter_map(|line| line.strip_prefix("  ").filter(|l| !l.starts_with(' '))).map(str::to_owned).collect()
}

fn firn_resolve(root: &Path, scratch: &Path, untag: bool) -> String {
    let query = scratch.join(if untag { "firn-after" } else { "firn-before" });
    let _ = std::fs::remove_dir_all(&query);
    std::fs::create_dir_all(query.join(".firn-build")).unwrap();
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() != ".firn-build" && entry.file_name() != "modules" {
            std::os::unix::fs::symlink(entry.path(), query.join(entry.file_name())).unwrap();
        }
    }
    assert!(Command::new("cp").arg("-r").arg(root.join("modules")).arg(query.join("modules")).status().unwrap().success());
    if untag {
        let file = query.join("modules/firefox/default.bnix");
        let mut permissions = std::fs::metadata(&file).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        std::fs::set_permissions(&file, permissions).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, text.replacen("{:tags [browsers]", "{", 1)).unwrap();
    }
    let out = Command::new(root.join("dotfiles/bin/firn")).args(["tag", "resolve", "whiterabbit"]).env_remove("FIRN_TAG_EMIT").env("FIRN_REPO", &query).current_dir(&query).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn reviewing_a_commit_that_removes_firefoxs_browsers_tag_turns_firefox_off_as_firn_tag_resolve_does() {
    let root = firn_root();
    let root = Path::new(&root);
    let repo = Path::new(env!("CARGO_TARGET_TMPDIR")).join("review-firefox-tag");
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
            "use firn/tags\n\n{}\nnix module\n  arguments: config, lib, pkgs, ...\n  for ?module where ?module is enabled on whiterabbit\n    myConfig.modules.{{?module}}.enable: lib.mkDefault(true)\n",
            host_facts("whiterabbit", &tags).unwrap()
        ),
    );
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "base"]);
    let firefox = repo.join("modules/firefox/default.gal");
    let text = std::fs::read_to_string(&firefox).unwrap();
    assert!(text.contains("firefox is tagged browsers\n"));
    std::fs::write(&firefox, text.replacen("firefox is tagged browsers\n", "", 1)).unwrap();
    git(&repo, &["commit", "-qam", "untag firefox"]);

    let started = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).args(["review", "--base", "HEAD~1"]).current_dir(&repo).output().unwrap();
    let elapsed = started.elapsed();
    let stdout = String::from_utf8_lossy(&out.stdout);
    println!("{stdout}review took {} ms", elapsed.as_millis());
    assert!(out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));

    let section = |name: &str| -> Vec<String> {
        let rest = stdout.split(&format!("\n{name} (")).nth(1).or_else(|| stdout.strip_prefix(&format!("{name} ("))).unwrap();
        rest.lines().skip(1).take_while(|l| l.starts_with("  ")).map(|l| l.trim().to_owned()).collect()
    };
    let edits = section("edits");
    assert_eq!(edits.len(), 1, "{stdout}");
    assert!(edits[0].starts_with("removes the fact `firefox is tagged browsers`") && edits[0].contains("modules/firefox/default.gal"), "{stdout}");

    let full = std::env::var_os("GALILEO_FULL").is_some();
    let (before, after) = if full {
        let scratch = Path::new(env!("CARGO_TARGET_TMPDIR"));
        (firn_resolve(root, scratch, false), firn_resolve(root, scratch, true))
    } else {
        (
            include_str!("whiterabbit-tag-resolve-4bcfad4c.txt").to_owned(),
            include_str!("whiterabbit-tag-resolve-4bcfad4c-firefox-untagged.txt").to_owned(),
        )
    };
    let (before, after) = (active(&before), active(&after));
    let mut expected: BTreeSet<String> = before.difference(&after).map(|m| format!("whiterabbit: turns off {m}")).collect();
    expected.extend(after.difference(&before).map(|m| format!("whiterabbit: turns on {m}")));
    let hosts: BTreeSet<String> = section("per host").into_iter().filter(|l| l.contains(": turns ")).collect();
    assert_eq!(hosts, expected, "{stdout}");
    assert!(expected.contains("whiterabbit: turns off firefox"));
    assert_eq!(section("generated files"), ["hosts/whiterabbit/_generated-enables.nix changes"], "{stdout}");
    assert!(elapsed.as_secs_f64() < 5.0, "review took {elapsed:?}");
}
