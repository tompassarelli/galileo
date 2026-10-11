use galileo_syntax::expr::{parse_expr, Expr, ExprKind, Param, Query, TextPart};
use galileo_syntax::record::{parse_record_file, Entry, FileValue, ModuleItem, RecordDecl, Segment};
use galileo_syntax::{lex, Line};

fn show(e: &Expr) -> String {
    let all = |xs: &[Expr]| xs.iter().map(show).collect::<Vec<_>>().join(" ");
    match &e.kind {
        ExprKind::Null => "null".into(),
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Int(i) => i.to_string(),
        ExprKind::Text(parts) => format!("(text {})", text(parts)),
        ExprKind::TextBlock(parts) => format!("(block {})", text(parts)),
        ExprKind::Path(p) => p.clone(),
        ExprKind::Name(n) => n.clone(),
        ExprKind::Var(v) => format!("?{v}"),
        ExprKind::List(xs) => format!("[{}]", all(xs)),
        ExprKind::Record(fields) => format!(
            "{{{}}}",
            fields
                .iter()
                .map(|f| format!("{}={}", segments(&f.path.segments), show(&f.value)))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        ExprKind::Field(e, name) => format!("(. {} {name})", show(e)),
        ExprKind::Call(f, args) => format!("(call {} {})", show(f), all(args)),
        ExprKind::Lambda(params, body) => format!(
            "(fn [{}] {})",
            params
                .iter()
                .map(|p| match p {
                    Param::Name { name, ty: None } => name.clone(),
                    Param::Name { name, ty: Some(galileo_syntax::types::Type::Named { name: t, .. }) } => format!("{name}:{t}"),
                    Param::Name { name, ty: Some(t) } => format!("{name}:{t:?}"),
                    Param::Formals { fields, open } => format!(
                        "{{{}{}}}",
                        fields.iter().map(|field| match &field.default {
                            Some(default) => format!("{}={}", field.name, show(default)),
                            None => field.name.clone(),
                        }).collect::<Vec<_>>().join(","),
                        if *open { ",..." } else { "" }
                    ),
                })
                .collect::<Vec<_>>()
                .join(" "),
            show(body)
        ),
        ExprKind::If(c, t, f) => format!("(if {} {} {})", show(c), show(t), show(f)),
        ExprKind::Binary(op, l, r) => format!("({op:?} {} {})", show(l), show(r)),
        ExprKind::Not(e) => format!("(not {})", show(e)),
        ExprKind::With(s, b) => format!("(with {} {})", show(s), show(b)),
        ExprKind::Query(Query::All {
            vars,
            conj,
            sorted_by,
            head,
        }) => format!(
            "(all {} {}{})",
            head.as_ref().map_or_else(|| vars.join(","), |head| show(head)),
            conj.len(),
            sorted_by
                .as_ref()
                .map_or(String::new(), |s| format!(" by {}", show(s)))
        ),
        ExprKind::Query(Query::The { var, conj }) => format!("(the {var} {})", conj.len()),
        ExprKind::NixModule(args, entries) => {
            format!("(module {} {})", args.names.join(","), entries.len())
        }
    }
}

fn text(parts: &[TextPart]) -> String {
    parts
        .iter()
        .map(|p| match p {
            TextPart::Text(t) => format!("{t:?}"),
            TextPart::Interpolation(e) => format!("${}", show(e)),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn segments(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|s| match s {
            Segment::Name(n) => n.clone(),
            Segment::Text(t) => format!("{t:?}"),
            Segment::Computed(e) => format!("{{{}}}", show(e)),
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn parse(source: &str) -> Result<String, String> {
    let lines: Vec<Line> = lex(source).map_err(|e| e.to_string())?;
    parse_expr(&lines[0].tokens, &lines[0])
        .map(|e| show(&e))
        .map_err(|e| e.to_string())
}

#[test]
fn spec_9_4_expr_forms_parse_from_one_line_examples() {
    for (source, expected) in [
        ("null", "null"),
        ("true", "true"),
        ("42", "42"),
        (
            r#""port {cfg.port}\n""#,
            r#"(text "port " $(. cfg port) "\n")"#,
        ),
        (
            r#""a \{b} {f("x")}""#,
            r#"(text "a {b} " $(call f (text "x")))"#,
        ),
        ("./kanata.nix", "./kanata.nix"),
        (
            "[\"input\", \"uinput\"]",
            r#"[(text "input") (text "uinput")]"#,
        ),
        ("[]", "[]"),
        ("{}", "{}"),
        (
            "{a: 1, b.c: \"x\", {?h}: ?h}",
            r#"{a=1 b.c=(text "x") {?h}=?h}"#,
        ),
        ("pkgs.unstable.kanata", "(. (. pkgs unstable) kanata)"),
        ("lib.mkForce(false)", "(call (. lib mkForce) false)"),
        ("f(a, b)(c)", "(call (call f a b) c)"),
        ("(x) => x + 1", "(fn [x] (Add x 1))"),
        ("(x: Int, y) => x", "(fn [x:Int y] x)"),
        (
            "({self, nixpkgs, ...}) => self",
            "(fn [{self,nixpkgs,...}] self)",
        ),
        ("if a then b else c", "(if a b c)"),
        ("a // b // c", "(Update a (Update b c))"),
        ("a ++ b ++ c", "(Concat a (Concat b c))"),
        ("a + b * c - d", "(Sub (Add a (Mul b c)) d)"),
        ("a // b ++ c", "(Update a (Concat b c))"),
        (
            "a == b and c != d or e < f",
            "(Or (And (Eq a b) (Ne c d)) (Lt e f))",
        ),
        (
            "a <= b and a >= c and a > d",
            "(And (And (Le a b) (Ge a c)) (Gt a d))",
        ),
        ("not a == b and c", "(And (not (Eq a b)) c)"),
        ("(a or b) and c", "(And (Or a b) c)"),
        ("kanata-main - 1", "(Sub kanata-main 1)"),
        ("with pkgs: [git, vim]", "(with pkgs [git vim])"),
        ("all ?m where ?m is enabled on whiterabbit", "(all m 5)"),
        (
            "all ?a, ?b where ?a leads ?b, sorted by ?a",
            "(all a,b 3 by ?a)",
        ),
        ("all ?row.label where ?north lists ?row", "(all (. ?row label) 3)"),
        ("all {key: ?row.key} where ?north lists ?row", "(all {key=(. ?row key)} 3)"),
        ("the ?x where ?x is first", "(the x 3)"),
        (
            "count(all ?m where ?m is on, sorted by ?m)",
            "(call count (all m 3 by ?m))",
        ),
    ] {
        assert_eq!(parse(source).as_deref(), Ok(expected), "{source}");
    }
}

#[test]
fn spec_9_4_malformed_expressions_name_their_column() {
    for (source, column, names) in [
        ("a == b == c", 8, ""),
        ("if a then b", 12, ""),
        ("[a b]", 4, ""),
        ("f(", 3, ""),
        ("\"x {}\"", 1, ""),
        ("a then", 3, ""),
        ("() => 1", 2, ""),
        ("a == not b", 6, "`a == (not b)`"),
        ("-x", 1, "`0 - x`"),
        ("-(x)", 1, "`0 - x`"),
        ("-1", 1, "`0 - x`"),
        ("a%b", 2, "`%` is reserved"),
        ("x;y", 2, "`;` is reserved"),
        ("!x", 1, "`!` is reserved"),
    ] {
        let error = parse(source).unwrap_err();
        assert!(
            error.starts_with(&format!("line 1:{column}:")) && error.contains(names),
            "{source}: {error}"
        );
    }
}

#[test]
fn spec_9_1_text_block_interpolation_and_inline_nix_module() {
    let source = "nix file\n  value\n    script: '''\n      echo {{cfg.name}} {plain}\n      '''\n    sub: nix module(config, ...)\n      enable: true\n";
    let decls = parse_record_file("t.gal", source).unwrap();
    let [RecordDecl::File(file)] = decls.as_slice() else {
        panic!()
    };
    let FileValue::Entries(entries) = &file.value else {
        panic!()
    };
    let values: Vec<String> = entries
        .iter()
        .map(|e| match e {
            Entry::Value { value, .. } => show(value),
            _ => panic!(),
        })
        .collect();
    assert_eq!(
        values,
        [
            r#"(block "echo " $(. cfg name) " {plain}\n")"#,
            "(module config 1)"
        ]
    );
}

#[test]
fn spec_9_5_kanata_expressions() {
    let mut shown = Vec::new();
    for source in [
        include_str!("../../../tests/external/kanata/default.gal"),
        include_str!("../../../tests/external/kanata/kanata.gal"),
    ] {
        let decls = parse_record_file("k.gal", source).unwrap();
        let [RecordDecl::Module(m)] = decls.as_slice() else {
            panic!()
        };
        for item in &m.items {
            match item {
                ModuleItem::Let(l) => shown.push(show(&l.value)),
                ModuleItem::Option(o) => shown.extend(o.fields.iter().map(|f| show(&f.value))),
                ModuleItem::Imports(e) => shown.push(show(e)),
                ModuleItem::Config { when, entries, .. } => {
                    shown.extend(when.iter().map(show));
                    walk(entries, &mut shown);
                }
                ModuleItem::For(_) => panic!(),
            }
        }
    }
    let shown: Vec<&str> = shown
        .iter()
        .map(String::as_str)
        .filter(|s| !s.starts_with("(text"))
        .collect();
    assert_eq!(
        shown,
        [
            "[]", "null", "[]", "[./kanata.nix]",
            "(. (. (. config myConfig) modules) kanata)",
            "(. cfg enable)",
            "true",
            "(block \"KERNEL==\\\"uinput\\\", MODE=\\\"0660\\\", GROUP=\\\"uinput\\\", OPTIONS+=\\\"static_node=uinput\\\"\\n\\n\")",
            "{}", "true", "[(text \"input\") (text \"uinput\")]", "{}", "true",
            "(. (. pkgs unstable) kanata)",
            "(Ne (. cfg devices) [])",
            "(. cfg devices)", "(. cfg port)", "(Ne (. cfg port) null)",
            "(. cfg extraArgs)",
            "(call (. builtins readFile) (. cfg configFile))",
            "(Ne (. cfg devices) [])",
            "(call (. lib mkForce) false)",
        ]
    );
}

fn walk(entries: &[Entry], shown: &mut Vec<String>) {
    for entry in entries {
        match entry {
            Entry::Value { value, when, .. } => {
                shown.push(show(value));
                shown.extend(when.iter().map(show));
            }
            Entry::Nested { when, entries, .. } => {
                shown.extend(when.iter().map(show));
                walk(entries, shown);
            }
            Entry::For(_) => panic!(),
        }
    }
}
