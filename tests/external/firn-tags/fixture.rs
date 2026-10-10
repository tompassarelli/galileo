use galileo_import::bnix_tags::{catalog, clause_facts, module_facts, platform_facts};
use std::path::Path;

pub fn firn_root() -> String {
    std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!("{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9", std::env::var("HOME").unwrap())
    })
}

pub fn module_sources(root: &Path) -> String {
    let mut names: Vec<String> = std::fs::read_dir(root.join("modules"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    let mut facts = String::new();
    for name in &names {
        let directory = root.join("modules").join(name);
        let clause = directory.join("tags.clause");
        let beagle = directory.join("default.bnix");
        if clause.exists() {
            facts += &clause_facts(name, &std::fs::read_to_string(&clause).unwrap()).unwrap();
        } else if beagle.exists() {
            facts += &module_facts(name, &std::fs::read_to_string(&beagle).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", beagle.display()));
        }
    }
    let hosts = ["whiterabbit", "ashashi"]
        .map(|host| std::fs::read_to_string(root.join(format!("hosts/{host}/enabled-tags.bnix"))).unwrap());
    let declared = catalog(&facts, &hosts.each_ref().map(String::as_str)).unwrap();
    declared + &platform_facts(&std::fs::read_to_string(root.join("config/darwin-modules.json")).unwrap()).unwrap() + &facts
}
