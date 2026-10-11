use galileo_syntax::record::{
    parse_record_file, print, Entry, FileValue, RecordDecl, Segment,
};

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
fn spec_9_4_nix_file_and_for_blocks() {
    let source = "nix file\n  arguments: self\n  let x = 1\n  value\n    for ?h in hosts\n      {?h}.\"name\": x when x != 0\n";
    let decls = parse_record_file("flake.gal", source).unwrap();
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
    for source in [
        source,
        include_str!("../../../tests/external/kanata/default.gal"),
        include_str!("../../../tests/external/kanata/kanata.gal"),
        "nix file\n  value\n    script: '''\n      echo {{cfg.name}} {plain}\n      '''\n    sub: nix module(config, ...)\n      enable: true\n",
        "nix file\n  value\n    makeHost: ({hostname, system = \"x86_64-linux\", extraModules = [], ...}) => hostname\n",
        "nix file\n  let x = \"b\"\n  value\n    \"a\\\"b\": 1\n    \"x\\\\y\": 2\n    \"text/html\": 3\n    \"a{x}\": 4\n    \"\\{literal}\": 5\n    \"\": 6\n    {x}: 7\n",
        "nix module\n  arguments: lib, config, ...\n  let description = \"feature\"\n  option feature.enable: enable description\n  option feature.items: List (maybe Int)\n    default: []\n  config\n    feature.value: if config.enabled then {a: 1, b.c: \"x\"} else {}\n",
    ] {
        let original = parse_record_file("record.gal", source).unwrap();
        let printed = original.iter().map(print).collect::<String>();
        let parsed = parse_record_file("printed.gal", &printed).unwrap();
        assert_eq!(galileo_syntax::fmt::format(&printed).unwrap(), printed);
        assert_eq!(parsed.iter().map(print).collect::<String>(), printed);
        let text_concat = Default::default();
        let answers = |_, _: &[(String, galileo_core::Value)]| Ok(Vec::new());
        let lower = galileo_syntax::nix::Lower {
            text_concat: &text_concat,
            answers: &answers,
        };
        let nix = |decl: &RecordDecl| {
            match decl {
                RecordDecl::Module(module) => lower.lower_module(module),
                RecordDecl::File(file) => lower.lower_file(file),
            }
            .map(|value| galileo_nix::print(&value))
            .unwrap()
        };
        assert_eq!(
            original.iter().map(nix).collect::<Vec<_>>(),
            parsed.iter().map(nix).collect::<Vec<_>>(),
            "{printed}"
        );
    }
}

#[test]
fn malformed_entry_names_file_line_and_form() {
    let source = "nix module\n  arguments: config, ...\n  config\n    users.users.kanata = true\n";
    let error = parse_record_file("modules/kanata/kanata.gal", source).unwrap_err();
    let shown = error.to_string();
    assert!(
        shown.starts_with(
            "modules/kanata/kanata.gal:4:5: expected an entry: `name: expr [when expr]`"
        ),
        "{shown}"
    );
}

#[test]
fn spec_9_4_rejects_mismatched_computed_segment_and_empty_for_variable() {
    for (source, line) in [
        (
            "nix module\n  arguments: config\n  config\n    {?h]: true\n",
            4,
        ),
        (
            "nix module\n  arguments: config\n  for ? in xs\n    x: true\n",
            3,
        ),
    ] {
        let error = parse_record_file("bad.gal", source).unwrap_err();
        assert_eq!(error.error.line, line, "{error}");
    }
    for key in ["\"c\"", "\"a{x}\"", "(\"c\")"] {
        let source = format!("nix file\n  value\n    {{{key}}}: 1\n");
        let error = parse_record_file("bad.gal", &source).unwrap_err();
        assert_eq!(error.error.line, 3, "{error}");
        assert_eq!(error.error.column, 5, "{error}");
        let quoted = key.trim_matches(['(', ')']);
        assert_eq!(error.error.message, format!("a text key takes no braces; write {quoted}"));
    }
}
