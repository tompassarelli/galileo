use galileo_import::bnix_tags::{catalog, clause_facts, host_facts, module_facts, platform_facts};
use std::path::Path;
use std::process::Command;

const TAGS: &str = include_str!("../../../lib/firn/tags.gal");
const REQUIREMENT: &str = "?module is enabled on ?host requires ?module is supported on ?host";

fn firn_root() -> String {
    std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!("{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9", std::env::var("HOME").unwrap())
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

fn galileo(repo: &Path, arguments: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).args(arguments).current_dir(repo).output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    eprint!("{stderr}");
    (out.status.code().unwrap(), String::from_utf8(out.stdout).unwrap())
}

fn patched(tags: &str) -> String {
    let shape = "given Host turns off Module\n";
    let rule = "rule enabled-directly: ?module is enabled on ?host\n  when ?host turns on ?module\n";
    assert!(tags.contains(shape) && tags.contains(rule));
    tags.replacen(shape, &format!("{shape}given Host pins Module\n"), 1).replacen(
        rule,
        &format!("{rule}\nrule enabled-by-pin: ?module is tag-enabled on ?host\n  when ?host pins ?module\n\nashashi pins git\nwhiterabbit pins git\n"),
        1,
    )
}

fn built(repo: &Path, hosts: &[String]) -> Vec<String> {
    assert_eq!(galileo(repo, &["build"]).0, 0);
    hosts.iter().map(|h| std::fs::read_to_string(repo.join(format!("hosts/{h}/_generated-enables.nix"))).unwrap()).collect()
}

#[test]
fn a_patch_to_firn_tags_that_keeps_every_generated_enables_byte_identical_is_rejected_for_bypassing_the_platform_gate() {
    let root = firn_root();
    let root = Path::new(&root);
    let repo = Path::new(env!("CARGO_TARGET_TMPDIR")).join("thesis-demo");
    let _ = std::fs::remove_dir_all(&repo);
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    let header = "use lib/firn/tags\n\n";
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
        write(&repo.join(format!("modules/{name}/default.gal")), &format!("{header}{facts}"));
        all += &facts;
    }
    let mut hosts: Vec<String> = std::fs::read_dir(root.join("hosts"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|h| root.join(format!("hosts/{h}/enabled-tags.bnix")).exists())
        .collect();
    hosts.sort();
    let tags: Vec<String> = hosts.iter().map(|h| std::fs::read_to_string(root.join(format!("hosts/{h}/enabled-tags.bnix"))).unwrap()).collect();
    let platforms = platform_facts(&std::fs::read_to_string(root.join("config/darwin-modules.json")).unwrap()).unwrap();
    let declared = catalog(&all, &tags.iter().map(String::as_str).collect::<Vec<_>>()).unwrap();
    write(&repo.join("config/tags.gal"), &format!("{header}{declared}{platforms}"));
    for (host, tags) in hosts.iter().zip(&tags) {
        write(
            &repo.join(format!("hosts/{host}/_generated-enables.gal")),
            &format!(
                "{header}{}\nnix module\n  arguments: config, lib, pkgs, ...\n  for ?module where ?module is tag-enabled on {host}\n    myConfig.modules.{{?module}}.enable: lib.mkDefault(true)\n    for ?path, ?value where {host} gets ?path set to ?value by ?module\n      {{?path}}: lib.mkDefault(?value)\n",
                host_facts(host, tags).unwrap()
            ),
        );
    }
    write(&repo.join("lib/firn/tags.gal"), TAGS);
    write(&repo.join("lib/firn/thesis.gal"), &format!("{header}requirements for thesis\n  {REQUIREMENT}\n"));

    let pin: Vec<String> = hosts.iter().map(|h| std::fs::read_to_string(root.join(format!("hosts/{h}/_generated-enables.nix"))).unwrap()).collect();
    assert_eq!(hosts.len(), 2, "{hosts:?}");
    assert_eq!(built(&repo, &hosts), pin);
    let (code, check) = galileo(&repo, &["check"]);
    println!("{check}");
    assert_eq!(code, 0, "{check}");
    assert!(check.contains(&format!("{REQUIREMENT} holds for all 155 of its instances.")), "{check}");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "base"]);

    write(&repo.join("lib/firn/tags.gal"), &patched(TAGS));
    assert_eq!(built(&repo, &hosts), pin);
    git(&repo, &["commit", "-qam", "pin git on both hosts"]);

    let (code, review) = galileo(&repo, &["review", "--base", "HEAD~1"]);
    println!("{review}");
    assert_eq!(code, 1, "{review}");
    for host in ["ashashi", "whiterabbit"] {
        assert!(review.contains(&format!("  git is enabled on {host} requires git is supported on {host}: introduced\n")), "{review}");
    }
    assert!(review.contains("git is enabled on ashashi still holds with every git is supported on ashashi blocked:"), "{review}");
    assert!(review.contains("← lib/firn/tags/enabled-by-pin (lib/firn/tags:62)"), "{review}");
    assert!(
        review.contains(&format!("`{REQUIREMENT}` reads rule `lib/firn/tags/enabled-by-pin`, which this patch adds")),
        "{review}"
    );
    assert!(review.ends_with("verdict: reject\n"), "{review}");
}
