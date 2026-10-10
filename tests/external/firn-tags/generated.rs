use muove_import::bnix_tags::host_facts;
use std::path::Path;
use std::process::Command;

#[path = "fixture.rs"]
mod fixture;
use fixture::{firn_root, module_sources};

const TAGS: &str = include_str!("../../../lib/firn/tags.muove");

fn generated_enables(host: &str) -> String {
    format!(
        "nix module
  arguments: config, lib, pkgs, ...
  for ?module in all ?m where ?m is enabled on {host}
    myConfig.modules.{{?module}}.enable: lib.mkDefault(true)
    for ?path, ?value in all ?p, ?v where {host} gets ?p set to ?v by ?module
      {{?path}}: lib.mkDefault(?value)
"
    )
}

#[test]
fn muove_build_emits_generated_enables_byte_for_byte() {
    let root = firn_root();
    let root = Path::new(&root);
    let modules = module_sources(root);
    for host in ["whiterabbit"] {
        let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("generated-enables-{host}"));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).unwrap();
        let tags = std::fs::read_to_string(root.join(format!("hosts/{host}/enabled-tags.bnix"))).unwrap();
        let source = scratch.join("_generated-enables.muove");
        let text = format!("{TAGS}\n{modules}{}\n{}", host_facts(host, &tags).unwrap(), generated_enables(host));
        std::fs::write(&source, text).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_muove")).arg("build").arg(&scratch).output().unwrap();
        assert!(out.status.success(), "{host}: {}", String::from_utf8_lossy(&out.stderr));
        let built = std::fs::read_to_string(source.with_extension("nix")).unwrap();
        let expected = std::fs::read_to_string(root.join(format!("hosts/{host}/_generated-enables.nix"))).unwrap();
        assert_eq!(built, expected, "{host}");
    }
}
