use galileo_syntax::check::{check, SourceFile};
use galileo_syntax::lex;
use galileo_syntax::record::{parse_record_file, ModuleItem, OptionKind, RecordDecl};
use galileo_syntax::types::{parse_type, Type};

fn show(ty: &Type) -> String {
    match ty {
        Type::Maybe(inner) => format!("(maybe {})", show(inner)),
        Type::Nix(inner) => format!("(nix {})", show(inner)),
        Type::Named { name, args, .. } if args.is_empty() => name.clone(),
        Type::Named { name, args, .. } => {
            format!("({name} {})", args.iter().map(show).collect::<Vec<_>>().join(" "))
        }
    }
}

fn value(source: &str) -> String {
    let line = &lex(source).unwrap()[0];
    show(&parse_type(&line.tokens, line).unwrap())
}

fn option(source: &str) -> String {
    let source = format!("nix module\n  arguments: lib\n  option x: {source}\n");
    let decls = parse_record_file("m.gal", &source).unwrap();
    let [RecordDecl::Module(module)] = decls.as_slice() else { panic!() };
    let ModuleItem::Option(option) = &module.items[0] else { panic!() };
    let OptionKind::Type(ty) = &option.kind else { panic!() };
    show(ty)
}

fn first_error(name: &str, source: &str) -> String {
    match SourceFile::parse(name, source) {
        Err(error) => error.to_string(),
        Ok(file) => check(&[file], &Default::default()).iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"),
    }
}

#[test]
fn spec_9_4_value_types_parse_into_trees() {
    assert_eq!(value("maybe List Text"), "(maybe (List Text))");
    assert_eq!(value("List (maybe Int)"), "(List (maybe Int))");
    assert_eq!(value("nix Any"), "(nix Any)");
    for body in ["", "\n  1"] {
        let source = format!("define f(x: List (maybe Int)): Int{body}\n");
        let file = SourceFile::parse("d.gal", &source).unwrap();
        assert_eq!(show(&file.signatures[0].params[0].1), "(List (maybe Int))");
        assert!(check(&[file], &Default::default()).is_empty());
    }
    let lowered = galileo_syntax::lower::lower(
        "define f(x: List (maybe Int)): Int\n  1\n",
        "d.gal",
    )
    .unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    assert_eq!(
        lowered.program.definitions[0].parameters[0].ty,
        galileo_core::Type::List(Box::new(galileo_core::Type::Maybe(Box::new(galileo_core::Type::Int))))
    );
}

#[test]
fn spec_10_2_option_types_parse_into_trees() {
    assert_eq!(option("List Text"), "(List Text)");
    assert_eq!(option("maybe Port"), "(maybe Port)");
}

#[test]
fn spec_9_4_malformed_types_are_parse_errors_with_location() {
    let lambda = "nix file\n  value: (x: List maybe Int) => x\n";
    assert!(
        first_error("f.gal", lambda).starts_with("f.gal:2:19: a type argument starting with `maybe`"),
        "{}",
        first_error("f.gal", lambda)
    );
    let option = "nix module\n  arguments: lib\n  option a.b: list of text\n";
    assert!(
        first_error("m.gal", option).starts_with("m.gal:3:15: expected a type; use `List Text`"),
        "{}",
        first_error("m.gal", option)
    );
}

#[test]
fn spec_9_4_a_name_that_is_not_a_type_is_a_checker_error() {
    let at = |name: &str, source: &str| -> Vec<(usize, usize, Vec<String>)> {
        let file = SourceFile::parse(name, source).unwrap();
        check(&[file], &Default::default())
            .iter()
            .map(|d| (d.line, d.column, d.message.split('`').skip(1).step_by(2).map(str::to_string).collect()))
            .collect()
    };
    let named = |names: &[&str]| names.iter().map(|name| name.to_string()).collect::<Vec<_>>();
    assert_eq!(at("d.gal", "define f(x: Strng): Int\n"), [(1, 13, named(&["Strng"]))]);
    assert_eq!(at("n.gal", "define f(x: Lst Text): Int\n"), [(1, 13, named(&["Lst", "List"]))]);
    assert_eq!(at("l.gal", "nix file\n  value: (x: maybe Txt) => x\n"), [(2, 20, named(&["Txt", "Text"]))]);
    let source = "nix module\n  arguments: lib\n  option a.b: enable \"{(x: Strng) => x}\"\n";
    assert_eq!(at("o.gal", source), [(3, 28, named(&["Strng"]))]);
    assert!(at("o.gal", &source.replace("Strng", "Text")).is_empty());
}
