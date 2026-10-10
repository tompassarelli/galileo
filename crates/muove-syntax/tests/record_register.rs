use muove_syntax::record::{
    parse_record_file, Entry, FileValue, ModuleItem, NixModule, RecordDecl, Segment,
};

fn module(file: &str, source: &str) -> NixModule {
    match parse_record_file(file, source).unwrap().as_slice() {
        [RecordDecl::Module(module)] => module.clone(),
        other => panic!("expected one nix module, got {other:?}"),
    }
}

fn path(entry: &Entry) -> String {
    let path = match entry {
        Entry::Value { path, .. } | Entry::Nested { path, .. } => path,
        Entry::For(_) => return "for".into(),
    };
    path.segments
        .iter()
        .map(|s| match s {
            Segment::Name(n) | Segment::Text(n) => n.clone(),
            Segment::Computed(_) => "{}".into(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

#[test]
fn spec_9_5_default_muove_declarations() {
    let m = module(
        "default.muove",
        include_str!("../../../tests/external/kanata/default.muove"),
    );
    assert_eq!(m.arguments.names, ["config", "lib", "pkgs"]);
    assert!(m.arguments.open);
    let fields: Vec<Vec<&str>> = m
        .items
        .iter()
        .filter_map(|item| match item {
            ModuleItem::Option(o) => Some(o.fields.iter().map(|f| f.name.as_str()).collect()),
            _ => None,
        })
        .collect();
    assert_eq!(
        fields,
        [
            vec![],
            vec!["default", "description"],
            vec!["description"],
            vec!["default", "description"],
            vec!["default", "description"],
        ]
    );
    assert!(matches!(m.items.last(), Some(ModuleItem::Imports(_))));
}

#[test]
fn spec_9_5_kanata_muove_declarations() {
    let m = module(
        "kanata.muove",
        include_str!("../../../tests/external/kanata/kanata.muove"),
    );
    let [ModuleItem::Let(cfg), ModuleItem::Config {
        when: Some(_),
        entries,
        ..
    }] = m.items.as_slice()
    else {
        panic!("expected `let` then `config when`: {:?}", m.items)
    };
    assert_eq!(cfg.name, "cfg");
    let paths: Vec<String> = entries.iter().map(path).collect();
    assert_eq!(
        paths,
        [
            "hardware.uinput.enable",
            "services.udev.extraRules",
            "users.groups.uinput",
            "users.users.kanata",
            "users.groups.kanata",
            "services.kanata",
            "systemd.services.kanata-main.serviceConfig",
        ]
    );
    let Entry::Nested { entries, .. } = &entries[5] else {
        panic!()
    };
    let Entry::Nested {
        when: Some(_),
        entries,
        ..
    } = &entries[2]
    else {
        panic!()
    };
    let Entry::Nested { entries: main, .. } = &entries[0] else {
        panic!()
    };
    assert!(matches!(&main[1], Entry::Value { when: Some(_), .. }));
}

#[test]
fn spec_9_4_nix_file_and_for_blocks() {
    let source = "nix file\n  arguments: self\n  let x = 1\n  value\n    for ?h in hosts\n      {?h}.\"name\": x when x != 0\n";
    let decls = parse_record_file("flake.muove", source).unwrap();
    let [RecordDecl::File(file)] = decls.as_slice() else {
        panic!()
    };
    assert_eq!(file.lets.len(), 1);
    let FileValue::Entries(entries) = &file.value else {
        panic!()
    };
    let Entry::For(f) = &entries[0] else { panic!() };
    assert_eq!(f.vars, ["h"]);
    assert_eq!(path(&f.entries[0]), "{}.name");
}

#[test]
fn malformed_entry_names_file_line_and_form() {
    let source = "nix module\n  arguments: config, ...\n  config\n    users.users.kanata = true\n";
    let error = parse_record_file("modules/kanata/kanata.muove", source).unwrap_err();
    let shown = error.to_string();
    assert!(
        shown.starts_with(
            "modules/kanata/kanata.muove:4:5: expected an entry: `name: expr [when expr]`"
        ),
        "{shown}"
    );
}
