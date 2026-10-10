use muove_syntax::check::{check, SourceFile};
use muove_syntax::lex;
use muove_syntax::record::{parse_record_file, ModuleItem, OptionKind, RecordDecl};
use muove_syntax::types::{parse_type, Type};

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
    let decls = parse_record_file("m.muove", &source).unwrap();
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
        let file = SourceFile::parse("d.muove", &source).unwrap();
        assert_eq!(show(&file.signatures[0].params[0].1), "(List (maybe Int))");
        assert!(check(&[file], &Default::default()).is_empty());
    }
    let lowered = muove_syntax::lower::lower(
        "define f(x: List (maybe Int)): Int\n  1\n",
        "d.muove",
    )
    .unwrap();
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    assert_eq!(
        lowered.program.definitions[0].parameters[0].ty,
        muove_core::Type::List(Box::new(muove_core::Type::Maybe(Box::new(muove_core::Type::Int))))
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
        first_error("f.muove", lambda).starts_with("f.muove:2:19: a type argument starting with `maybe`"),
        "{}",
        first_error("f.muove", lambda)
    );
    let option = "nix module\n  arguments: lib\n  option a.b: list of text\n";
    assert!(
        first_error("m.muove", option).starts_with("m.muove:3:15: expected a type; use `List Text`"),
        "{}",
        first_error("m.muove", option)
    );
}

#[test]
fn spec_9_4_a_name_that_is_not_a_type_is_a_checker_error() {
    assert_eq!(
        first_error("d.muove", "define f(x: Strng): Int\n"),
        "d.muove:1:13: `Strng` is not a type.\n    define f(x: Strng): Int"
    );
    assert!(first_error("d.muove", "define f(x: Lst Text): Int\n").contains("Did you mean `List`?"));
    assert_eq!(
        first_error("l.muove", "nix file\n  value: (x: maybe Txt) => x\n"),
        "l.muove:2:20: `Txt` is not a type. Did you mean `Text`?\n      value: (x: maybe Txt) => x"
    );
    let source = "nix module\n  arguments: lib\n  option a.b: enable \"{(x: Strng) => x}\"\n";
    assert_eq!(
        first_error("o.muove", source),
        "o.muove:3:28: `Strng` is not a type.\n      option a.b: enable \"{(x: Strng) => x}\""
    );
    assert!(first_error("o.muove", &source.replace("Strng", "Text")).is_empty());
}
