use galileo_syntax::{blocks, lex, Block, TokenKind};

fn golden_blocks(blocks: &[Block], out: &mut String, depth: usize) {
    for block in blocks {
        out.push_str(&format!(
            "{}@{} {}\n",
            block.line.no,
            depth * 2,
            block.line.text
        ));
        for token in &block.line.tokens {
            if let TokenKind::TextBlock(text) = &token.kind {
                out.push_str(&format!("  raw: {text:?}\n"));
            }
        }
        golden_blocks(&block.children, out, depth + 1);
    }
}

#[test]
fn spec_9_5_default_block_golden() {
    let source = include_str!("../../../tests/external/kanata/default.gal");
    let parsed = blocks(source).unwrap();
    let mut actual = String::new();
    golden_blocks(&parsed, &mut actual, 0);
    assert_eq!(actual, include_str!("golden/default.blocks"));
}

#[test]
fn spec_9_5_kanata_block_golden() {
    let source = include_str!("../../../tests/external/kanata/kanata.gal");
    let parsed = blocks(source).unwrap();
    let mut actual = String::new();
    golden_blocks(&parsed, &mut actual, 0);
    assert_eq!(actual, include_str!("golden/kanata.blocks"));
}

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
    for (source, line, message) in [
        ("a\n\tb\n", 2, "tabs"),
        ("a\n b\n", 2, "multiple of two"),
        ("a\n    b\n", 2, "two spaces"),
        ("  a\n", 1, "unexpected indentation"),
        ("a\r\n", 1, "LF"),
        ("says: \"unterminated\n", 1, "unterminated text literal"),
        ("says: \"\\q\"\n", 1, "invalid text escape"),
        ("named: `unterminated\n", 1, "unterminated quoted name"),
        ("raw: ''' trailing\n", 1, "end of a line"),
        ("raw: '''#attached\n", 1, "end of a line"),
        ("raw: '''\n  missing\n", 1, "unterminated text block"),
        ("raw: '''\n x\n  '''\n", 2, "left of its closing marker"),
        ("raw: '''\n'''\n", 2, "two spaces beneath its opening line"),
        ("raw: '''\n    x\n    '''\n", 3, "two spaces beneath its opening line"),
    ] {
        let error = blocks(source).unwrap_err();
        assert_eq!(error.line, line, "{source:?}");
        assert!(error.message.contains(message), "{source:?}: {error}");
    }
}

#[test]
fn spec_9_1_d59_text_block_continuations_build_reference_nix() {
    use galileo_syntax::record::{parse_record_file, RecordDecl};

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
        let emitted = galileo_nix::print(&lower.lower_file(file).unwrap());
        let parse = |nix: &str| {
            let output = std::process::Command::new("nix-instantiate")
                .args(["--parse", "--expr", nix])
                .output()
                .unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            output.stdout
        };
        assert_eq!(parse(&emitted), parse(reference));
    }
    for (source, line, column, message) in [
        (
            "value: f('''\n  echo hi\n    '''})\n",
            3,
            5,
            "indent the closing text marker two spaces beneath its opening line",
        ),
        (
            "value: f('''\n  echo hi\nafter: true\n",
            3,
            1,
            "text block content is left of its closing marker",
        ),
    ] {
        let error = lex(source).unwrap_err();
        assert_eq!(
            (error.line, error.column, error.message.as_str()),
            (line, column, message)
        );
        assert_eq!(galileo_syntax::fmt::format(source).unwrap_err(), error);
    }
}
