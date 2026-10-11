use galileo_import::bnix_tags::{catalog, host_facts, module_facts, platform_facts};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

fn firn_root() -> String {
    std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!("{}/code/nixos-config/pins/4bcfad4cdb806dd7f66401dad5581037cb996ff0", std::env::var("HOME").unwrap())
    })
}

fn galileo(arguments: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).args(arguments).output().unwrap();
    assert!(out.status.success(), "{arguments:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

fn generated_enables(host: &str) -> String {
    format!(
        "nix module
  arguments: config, lib, pkgs, ...
  for ?module where ?module is tag-enabled on {host}
    myConfig.modules.{{?module}}.enable: lib.mkDefault(true)
    for ?path, ?value where {host} gets ?path set to ?value by ?module
      {{?path}}: lib.mkDefault(?value)
"
    )
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn module_gal_files_with_moved_clause_tags_build_generated_enables_byte_for_byte() {
    let root = firn_root();
    let root = Path::new(&root);
    let mut names: Vec<String> =
        std::fs::read_dir(root.join("modules")).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    let mut modules = Vec::new();
    for name in &names {
        let directory = root.join("modules").join(name);
        let text = if directory.join("tags.clause").exists() {
            galileo(&["import", "nix", directory.join("default.nix").to_str().unwrap()])
        } else if directory.join("default.bnix").exists() {
            format!("use firn/tags\n\n{}", module_facts(name, &std::fs::read_to_string(directory.join("default.bnix")).unwrap()).unwrap())
        } else {
            continue;
        };
        modules.push((name.clone(), text));
    }
    let moved = names.iter().filter(|n| root.join("modules").join(n).join("tags.clause").exists()).count();
    let all: String = modules.iter().map(|(_, text)| text.as_str()).collect();
    let hosts = ["whiterabbit"]
        .map(|host| std::fs::read_to_string(root.join(format!("hosts/{host}/enabled-tags.bnix"))).unwrap());
    let shared = format!(
        "use firn/tags\n\n{}{}",
        catalog(&all, &hosts.each_ref().map(String::as_str)).unwrap(),
        platform_facts(&std::fs::read_to_string(root.join("config/darwin-modules.json")).unwrap()).unwrap()
    );
    for (host, tags) in ["whiterabbit"].iter().zip(&hosts) {
        let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("generated-enables-{host}"));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(scratch.join(".git")).unwrap();
        for (name, text) in &modules {
            write(&scratch.join(format!("modules/{name}/default.gal")), text);
        }
        write(&scratch.join("config/tags.gal"), &shared);
        let source = scratch.join("_generated-enables.gal");
        write(&source, &format!("use firn/tags\n\n{}\n{}", host_facts(host, tags).unwrap(), generated_enables(host)));
        let expected = std::fs::read_to_string(root.join(format!("hosts/{host}/_generated-enables.nix"))).unwrap();
        let listed: BTreeSet<String> = expected
            .lines()
            .filter_map(|l| l.trim().strip_prefix("myConfig.modules.")?.strip_suffix(".enable = lib.mkDefault true;").map(|m| m.trim_matches('"').to_owned()))
            .collect();
        let json = galileo(&["query", scratch.to_str().unwrap(), &format!("all ?m where ?m is enabled on {host}"), "--json"]);
        let enabled: BTreeSet<String> =
            json.split('"').skip(1).step_by(2).map(str::to_owned).collect();
        println!("{host}: {moved} tags.clause files moved; all ?m where ?m is enabled on {host}: {} modules; _generated-enables: {} modules", enabled.len(), listed.len());
        assert_eq!(enabled, listed, "{host}");
        galileo(&["build", scratch.to_str().unwrap()]);
        let built = std::fs::read_to_string(source.with_extension("nix")).unwrap();
        assert_eq!(built, expected, "{host}");
    }
}
