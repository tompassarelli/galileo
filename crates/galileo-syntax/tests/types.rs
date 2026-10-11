use galileo_syntax::check::{check, SourceFile};
use galileo_syntax::lex;
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

#[test]
fn spec_9_4_value_types_parse_into_trees() {
    assert_eq!(value("maybe List Text"), "(maybe (List Text))");
    assert_eq!(value("List (maybe Int)"), "(List (maybe Int))");
    assert_eq!(value("nix Any"), "(nix Any)");
    for body in ["", "\n  1"] {
        let source = format!("define f(x: List (maybe Int)): Int{body}\n");
        let file = SourceFile::parse("d.gal", &source).unwrap();
        assert_eq!(show(&file.signatures[0].params[0].1), "(List (maybe Int))");
        let found = check(&[file], &Default::default());
        if body.is_empty() {
            assert_eq!(found.len(), 1);
            assert_eq!(found[0].line, 1);
            assert!(found[0].message.contains("needs a body"));
        } else {
            assert!(found.is_empty());
        }
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
