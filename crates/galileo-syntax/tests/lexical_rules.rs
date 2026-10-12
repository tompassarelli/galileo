use galileo_syntax::{blocks, lex, TokenKind};

#[test]
fn spec_9_1_comments_respect_literals_and_raw_blocks() {
    let source = concat!(
        "# discarded\n",
        "says: \"Fixes issue #12\" # discarded\n",
        "escaped: \"a \\\" #12 \\\\ \\n \\t \\{\" # discarded\n",
        "named: `# quoted name` # discarded\n",
        "attached: issue#12 # discarded\n",
        "interpolation: \"{f(\" # nested \" )}\" # discarded\n",
        "raw: '''\n",
        "  # retained\n",
        "    \\n {{f(\"x\")}}\n",
        "\n",
        "  '''\n",
        "after: true\n",
    );
    let lines = lex(source).unwrap();
    assert_eq!(lines[0].text, "says: \"Fixes issue #12\"");
    assert_eq!(
        lines[0].tokens[2].kind,
        TokenKind::Text("Fixes issue #12".into())
    );
    assert_eq!(lines[1].text, "escaped: \"a \\\" #12 \\\\ \\n \\t \\{\"");
    assert_eq!(
        lines[2].tokens[2].kind,
        TokenKind::QuotedName("# quoted name".into())
    );
    assert_eq!(lines[3].tokens[2].kind, TokenKind::Word("issue#12".into()));
    assert_eq!(
        lines[4].tokens[2].kind,
        TokenKind::Text("{f(\" # nested \" )}".into())
    );
    assert_eq!(
        lines[5].tokens[2].kind,
        TokenKind::TextBlock("# retained\n  \\n {{f(\"x\")}}\n\n".into())
    );
    let span = &lines[5].tokens[2].span;
    assert_eq!(
        &source[span.start..span.end],
        "'''\n  # retained\n    \\n {{f(\"x\")}}\n\n  '''"
    );
    assert_eq!((span.line, span.column), (7, 6));
    assert_eq!(lines[6].no, 12);
}

#[test]
fn spec_9_1_tokens_preserve_spelling_and_source_coordinates() {
    let source = "says: \"雪\" ?name Front-Door 12 ./kanata.nix ../parent /root cfg!=[] ++ // =>\n";
    let line = lex(source).unwrap().remove(0);
    let expected = [
        TokenKind::Word("says".into()),
        TokenKind::Symbol(":".into()),
        TokenKind::Text("雪".into()),
        TokenKind::Variable("name".into()),
        TokenKind::Word("Front-Door".into()),
        TokenKind::Integer("12".into()),
        TokenKind::Path("./kanata.nix".into()),
        TokenKind::Path("../parent".into()),
        TokenKind::Path("/root".into()),
        TokenKind::Word("cfg".into()),
        TokenKind::Symbol("!=".into()),
        TokenKind::Symbol("[".into()),
        TokenKind::Symbol("]".into()),
        TokenKind::Symbol("++".into()),
        TokenKind::Symbol("//".into()),
        TokenKind::Symbol("=>".into()),
    ];
    assert_eq!(
        line.tokens.iter().map(|t| &t.kind).collect::<Vec<_>>(),
        expected.iter().collect::<Vec<_>>()
    );
    let variable = &line.tokens[3].span;
    assert_eq!(&source[variable.start..variable.end], "?name");
    assert_eq!((variable.line, variable.column), (1, 11));
}

#[test]
fn spec_9_1_rejects_malformed_layout_and_literals() {
    for (source, at) in [
        ("a\n\tb\n", (2, 1)),
        ("a\n b\n", (2, 2)),
        ("a\n    b\n", (2, 5)),
        ("  a\n", (1, 3)),
        ("a\r\n", (1, 2)),
        ("says: \"unterminated\n", (1, 7)),
        ("says: \"\\q\"\n", (1, 9)),
        ("named: `unterminated\n", (1, 8)),
        ("raw: ''' trailing\n", (1, 6)),
        ("raw: '''#attached\n", (1, 6)),
        ("raw: '''\n  missing\n", (1, 1)),
        ("raw: '''\n x\n  '''\n", (2, 2)),
        ("raw: '''\n'''\n", (2, 1)),
        ("raw: '''\n    x\n    '''\n", (3, 5)),
    ] {
        let error = blocks(source).unwrap_err();
        assert_eq!((error.line, error.column), at, "{source:?}: {error}");
    }
}

#[test]
fn spec_9_1_d59_text_block_continuations_build_reference_nix() {
    use galileo_syntax::record::{parse_record_file, RecordDecl};

    let mut pairs = Vec::new();
    for (source, reference) in [
        (
            "nix file\n  arguments: pkgs\n  value: pkgs.writeShellApplication({name: \"x\", text: '''\n    echo hi\n    '''})\n",
            "{ pkgs }: pkgs.writeShellApplication { name = \"x\"; text = \"echo hi\\n\"; }",
        ),
        (
            "nix file\n  value: ['''\n    a\n    ''', '''\n    b\n    ''']\n",
            "[ \"a\\n\" \"b\\n\" ]",
        ),
    ] {
        let lines = lex(source).unwrap();
        for token in lines.iter().flat_map(|line| &line.tokens) {
            let prefix = &source[..token.span.start];
            assert_eq!(token.span.line, prefix.bytes().filter(|b| *b == b'\n').count() + 1);
            assert_eq!(token.span.column, prefix.rsplit('\n').next().unwrap().chars().count() + 1);
            if !matches!(token.kind, TokenKind::TextBlock(_)) {
                assert!(!source[token.span.start..token.span.end].contains('\n'));
            }
        }
        assert_eq!(galileo_syntax::fmt::format(source).unwrap(), source);
        let parsed = parse_record_file("continuation.gal", source).unwrap();
        let [RecordDecl::File(file)] = parsed.as_slice() else { panic!() };
        let text_concat = Default::default();
        let lower = galileo_syntax::nix::Lower {
            text_concat: &text_concat,
            answers: &galileo_syntax::nix::no_answers,
        };
        pairs.push((galileo_nix::print(&lower.lower_file(file).unwrap()), reference.to_string()));
    }
    let parse = |nix: &str| {
        let output = std::process::Command::new("nix-instantiate")
            .args(["--parse", "--expr", nix])
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        output.stdout
    };
    let batch = std::process::Command::new("nix-instantiate")
        .arg("--parse")
        .args(pairs.iter().flat_map(|(emitted, reference)| ["--expr", emitted.as_str(), "--expr", reference.as_str()]))
        .output()
        .unwrap();
    let parsed: Vec<Vec<u8>> = batch.stdout.split_inclusive(|b| *b == b'\n').map(<[u8]>::to_vec).collect();
    let parsed = if batch.status.success() && parsed.len() == 2 * pairs.len() {
        parsed
    } else {
        pairs.iter().flat_map(|(emitted, reference)| [parse(emitted), parse(reference)]).collect()
    };
    for pair in parsed.chunks(2) {
        assert_eq!(pair[0], pair[1]);
    }
    for (source, line, column) in [
        ("value: f('''\n  echo hi\n    '''})\n", 3, 5),
        ("value: f('''\n  echo hi\nafter: true\n", 3, 1),
    ] {
        let error = lex(source).unwrap_err();
        assert_eq!((error.line, error.column), (line, column), "{error}");
        assert_eq!(galileo_syntax::fmt::format(source).unwrap_err(), error);
    }
}
