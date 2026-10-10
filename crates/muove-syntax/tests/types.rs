use muove_syntax::check::{check, SourceFile};
use muove_syntax::expr::ExprKind;
use muove_syntax::lex;
use muove_syntax::record::TextPart;
use muove_syntax::types::{parse_option_type, parse_type, OptionType, Type};

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

fn show_option(ty: &OptionType) -> String {
    let text = |e: &muove_syntax::expr::Expr| match &e.kind {
        ExprKind::Text(parts) => match parts.as_slice() {
            [TextPart::Text(t)] => format!("{t:?}"),
            other => format!("{other:?}"),
        },
        other => format!("{other:?}"),
    };
    match ty {
        OptionType::ListOf(inner) => format!("(listOf {})", show_option(inner)),
        OptionType::AttrsOf(inner) => format!("(attrsOf {})", show_option(inner)),
        OptionType::Maybe(inner) => format!("(nullOr {})", show_option(inner)),
        OptionType::OneOf(choices) => {
            format!("(enum {})", choices.iter().map(text).collect::<Vec<_>>().join(" "))
        }
        OptionType::Enable(description) => format!("(enable {})", text(description)),
        leaf => format!("{leaf:?}").to_lowercase(),
    }
}

fn value(source: &str) -> String {
    let line = &lex(source).unwrap()[0];
    show(&parse_type(&line.tokens, line).unwrap())
}

fn option(source: &str) -> String {
    let line = &lex(source).unwrap()[0];
    show_option(&parse_option_type(&line.tokens, line).unwrap())
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
}

#[test]
fn spec_10_2_option_types_parse_into_trees() {
    assert_eq!(option("list of text"), "(listOf text)");
    assert_eq!(option("maybe port"), "(nullOr port)");
    assert_eq!(option("one of \"a\" | \"b\""), "(enum \"a\" \"b\")");
}

#[test]
fn spec_9_4_malformed_types_are_parse_errors_with_location() {
    let lambda = "nix file\n  value: (x: List maybe Int) => x\n";
    assert!(
        first_error("f.muove", lambda).starts_with("f.muove:2:19: a type argument starting with `maybe`"),
        "{}",
        first_error("f.muove", lambda)
    );
    let option = "nix module\n  arguments: lib\n  option a.b: lsit of text\n";
    assert!(
        first_error("m.muove", option).starts_with("m.muove:3:15: expected an option type"),
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
}
