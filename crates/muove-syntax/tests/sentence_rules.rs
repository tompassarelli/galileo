use muove_syntax::expr::{parse_expr, BinOp, ExprKind};
use muove_syntax::{lex, read_program, read_sentences, Aggregate, Literal, Sentences};

const TAGS: &str = "\
type Module, Tag, Host, Platform

static {module: Module} is tagged {tag: Tag}
static {module: Module} can opt into {tag: Tag}
static {host: Host} enables {tag: Tag}
static {host: Host} drops {module: Module} from {tag: Tag}
static {host: Host} adds {module: Module} to {tag: Tag}
static {host: Host} disables {module: Module}
static {host: Host} runs {platform: Platform}
  exactly one platform per host
static {module: Module} supports {platform: Platform}
static {tag: Tag} sets {path: OptionPath} to {value: nix Any} for {module: Module}
derived {module: Module} is enabled on {host: Host} by {tag: Tag}
derived {module: Module} is enabled on {host: Host} by default through {tag: Tag}
derived checkpoint {module: Module} is enabled on {host: Host}
derived {host: Host} gets {path: OptionPath} set to {value: nix Any} by {module: Module}
  at most one value per host, path

rule enabled-by-default
  ?module is enabled on ?host by default through ?tag
  when
    ?host enables ?tag
    ?module is tagged ?tag
    not ?host drops ?module from ?tag

rule default-membership-enables
  ?module is enabled on ?host by ?tag
  when
    ?module is enabled on ?host by default through ?tag

rule enabled-by-opt-in
  ?module is enabled on ?host by ?tag
  when
    ?host enables ?tag
    ?host adds ?module to ?tag
    ?module can opt into ?tag

rule enabled
  ?module is enabled on ?host
  when
    ?module is enabled on ?host by ?tag
    not ?host disables ?module

rule tag-overrides
  ?host gets ?path set to ?value by ?module
  when
    ?module is enabled on ?host
    ?module is enabled on ?host by default through ?tag
    ?tag sets ?path to ?value for ?module

requirements for tags
  every ?host, ?module, ?tag where ?host adds ?module to ?tag: ?module can opt into ?tag
  every ?module, ?host, ?platform where ?module is enabled on ?host and ?host runs ?platform: ?module supports ?platform
";

const MENU: &str = "\
type North

state entity MenuRow
  key: Text
  label: Text
  description: Text
  annotation: Text
  order: Int

state {north: North} has menu kind {kind: Text}
  exactly one kind per north
state {north: North} has menu query {query: Text}
  exactly one query per north
state {north: North} has menu selection {index: Int}
  exactly one index per north
state {north: North} lists {row: MenuRow}

derived {row: MenuRow} matches the menu query of {north: North}
derived {north: North} has menu count {count: Int}
  exactly one count per north

rule matching-rows
  ?row matches the menu query of ?north
  when
    ?north lists ?row
    ?north has menu query ?query
    contains(lowercase(\"{?row.label} {?row.description} {?row.key}\"), lowercase(?query))

rule menu-count
  ?north has menu count ?count
  when
    ?north is a North
    ?count = count where ?row matches the menu query of ?north

define wrap(index: Int, count: Int): Int
  if index < 0 then count - 1 else if index >= count then 0 else index

event move-menu: {north: North} moves menu selection by {delta: Int}
event clear-menu-rows: {north: North} clears menu rows
";

fn shape_names(parsed: &Sentences, literals: &[Literal]) -> Vec<String> {
    literals
        .iter()
        .map(|l| match l {
            Literal::Sentence(r) => parsed.vocabulary.shapes[r.shape].name.clone(),
            Literal::Not(r) => format!("not {}", parsed.vocabulary.shapes[r.shape].name),
            other => format!("{other:?}"),
        })
        .collect()
}

fn query(parsed: &Sentences, text: &str) -> Vec<Literal> {
    let line = lex(text).unwrap().remove(0);
    let ExprKind::Query(q) = parse_expr(&line.tokens, &line).unwrap().kind else {
        panic!("not a query: {text}");
    };
    parsed.vocabulary.read_query(&line, &q).unwrap()
}

#[test]
fn spec_9_6_firn_tags_rules_and_generated_enables_queries() {
    let parsed = read_sentences(TAGS).unwrap();
    let names: Vec<&str> = parsed.rules.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "enabled-by-default",
            "default-membership-enables",
            "enabled-by-opt-in",
            "enabled",
            "tag-overrides"
        ]
    );
    let first = &parsed.rules[0];
    let body: Vec<Literal> = first.body.iter().map(|(_, l)| l.clone()).collect();
    assert_eq!(
        shape_names(&parsed, &body),
        ["enables", "is-tagged", "not drops-from"]
    );
    assert_eq!(
        parsed.vocabulary.shapes[first.head.shape].name,
        "is-enabled-on-by-default-through"
    );

    assert_eq!(
        shape_names(&parsed, &query(&parsed, "all ?m where ?m is enabled on whiterabbit")),
        ["is-enabled-on"]
    );
    assert_eq!(
        shape_names(
            &parsed,
            &query(&parsed, "all ?m, ?h where ?m is enabled on ?h and ?h runs linux, sorted by ?m")
        ),
        ["is-enabled-on", "runs"]
    );
    assert_eq!(
        shape_names(
            &parsed,
            &query(&parsed, "all ?p, ?v where whiterabbit gets ?p set to ?v by ?module")
        ),
        ["gets-set-to-by"]
    );

    let [Literal::In(_, inner)] = &query(&parsed, "all ?x where ?x in all ?y where ?y is a Host and ?y runs linux")[..] else {
        panic!("expected one `in` literal");
    };
    let ExprKind::Query(inner) = &inner.kind else {
        panic!("expected a nested query");
    };
    let line = lex("x").unwrap().remove(0);
    let inner = parsed.vocabulary.read_query(&line, inner).unwrap();
    assert_eq!(shape_names(&parsed, &inner), ["is-a-Host", "runs"]);
    assert_eq!(
        shape_names(&parsed, &query(&parsed, "all ?h where ?h runs linux and not ?h is a Host")),
        ["runs", "not is-a-Host"]
    );

    let [Literal::Sentence(outer), Literal::In(_, inner)] = &query(
        &parsed,
        "all ?x where ?x is a Host and ?x in all ?y where true or false",
    )[..] else {
        panic!("expected an outer type condition and a nested query");
    };
    assert_eq!(parsed.vocabulary.shapes[outer.shape].name, "is-a-Host");
    let ExprKind::Query(inner) = &inner.kind else {
        panic!("expected a nested query");
    };
    let inner = parsed.vocabulary.read_query(&line, inner).unwrap();
    assert!(matches!(inner.as_slice(), [Literal::Holds(e)] if matches!(&e.kind, ExprKind::Binary(BinOp::Or, _, _))));

    let [Literal::Sentence(outer), Literal::Aggregate { body, .. }] = &query(
        &parsed,
        "all ?x where ?x is a Host and ?n = count where true or false",
    )[..] else {
        panic!("expected an outer type condition and an aggregate");
    };
    assert_eq!(parsed.vocabulary.shapes[outer.shape].name, "is-a-Host");
    assert!(matches!(body.as_slice(), [Literal::Holds(e)] if matches!(&e.kind, ExprKind::Binary(BinOp::Or, _, _))));

    let text = "all ?x where ?x is a Host and ?x == a or ?x == b";
    let line = lex(text).unwrap().remove(0);
    let ExprKind::Query(q) = parse_expr(&line.tokens, &line).unwrap().kind else {
        panic!("not a query");
    };
    let error = parsed.vocabulary.read_query(&line, &q).unwrap_err();
    assert_eq!((error.line, error.column), (1, 39));
    assert!(error.message.contains("`(a or b) and c`"), "{error}");
}

#[test]
fn spec_9_7_menu_rules_and_view_queries() {
    let parsed = read_sentences(MENU).unwrap();
    let [matching, count] = parsed.rules.as_slice() else {
        panic!("expected two rules");
    };
    assert!(matches!(matching.body[2].1, Literal::Holds(_)));
    assert_eq!(shape_names(&parsed, &[count.body[0].1.clone()]), ["is-a-North"]);
    let Literal::Aggregate { var, op, of, body } = &count.body[1].1 else {
        panic!("expected an aggregate");
    };
    assert_eq!((var.as_str(), *op), ("count", Aggregate::Count));
    assert!(of.is_none());
    assert_eq!(
        shape_names(&parsed, body),
        ["matches-the-menu-query-of"]
    );

    for (text, shape) in [
        ("the ?k where north has menu kind ?k", "has-menu-kind"),
        ("the ?i where north has menu selection ?i", "has-menu-selection"),
        (
            "all ?row where ?row matches the menu query of north, sorted by ?row.order",
            "matches-the-menu-query-of",
        ),
    ] {
        assert_eq!(shape_names(&parsed, &query(&parsed, text)), [shape]);
    }
}

#[test]
fn spec_4_3_every_literal_reads() {
    let source = "\
type Item, Shop
static {shop: Shop} sells {item: Item} for {cost: Int}
static {shop: Shop} is closed
static {shop: Shop} stocks {items: nix Any}
static {item: Item} is named {name: Text}
derived {shop: Shop} offers {item: Item}
derived {shop: Shop} totals {n: Int}

rule every-literal
  ?shop offers ?item
  when
    ?shop sells ?item for ?cost
    not ?shop is closed
    ?cost <= 10
    ?item is named ?name
    contains(lowercase(?name), \"tea\")
    ?double = ?cost * 2
    ?shop stocks ?items
    ?item in ?items
    ?n = count where ?shop sells ?i for ?_
    ?total = sum ?c where ?shop sells ?i for ?c
    ?low = min ?c where ?shop sells ?i for ?c and ?c > 0
    ?high = max ?c where ?shop sells ?i for ?c
    ?cheapest = first ?i by ?c where ?shop sells ?i for ?c
";
    let parsed = read_sentences(source).unwrap();
    let body: Vec<&Literal> = parsed.rules[0].body.iter().map(|(_, l)| l).collect();
    assert!(matches!(body[0], Literal::Sentence(_)));
    assert!(matches!(body[1], Literal::Not(_)));
    assert!(matches!(body[2], Literal::Compare(BinOp::Le, _, _)));
    assert!(matches!(body[4], Literal::Holds(_)));
    assert!(matches!(body[5], Literal::Bind(v, _) if v == "double"));
    assert!(matches!(body[7], Literal::In(v, _) if v == "item"));
    let ops: Vec<Aggregate> = body[8..12]
        .iter()
        .map(|l| match l {
            Literal::Aggregate { op, .. } => *op,
            other => panic!("expected an aggregate, got {other:?}"),
        })
        .collect();
    assert_eq!(
        ops,
        [Aggregate::Count, Aggregate::Sum, Aggregate::Min, Aggregate::Max]
    );
    assert!(matches!(body[10], Literal::Aggregate { body, .. } if body.len() == 2));
    assert!(matches!(body[12], Literal::First { var, of, .. } if var == "cheapest" && of == "i"));

    let message = |text: &str| read_sentences(&source.replace("?shop offers ?item\n", text)).unwrap_err().message;
    assert_eq!(
        message("?shop is closed\n"),
        "a rule concludes a derived sentence, but `is-closed` is Static"
    );
    assert_eq!(
        read_sentences(&source.replace("?n = count where", "?n = sum where"))
            .unwrap_err()
            .message,
        "`sum` needs a value, as in `?n = sum ?x where …`"
    );
    let e = read_sentences(&source.replace("?n = count where", "?n = count ?i where")).unwrap_err();
    assert_eq!(
        (e.line, e.message.as_str()),
        (20, "`count` takes no value; write `?n = count where …`")
    );
    let body_error = |text: &str| {
        let e = read_sentences(&source.replace("?item in ?items\n", text)).unwrap_err();
        (e.line, e.message)
    };
    assert_eq!(body_error("? = 1\n"), (19, "expected a variable `?name`".into()));
    assert_eq!(
        body_error("length(all ?i where true and) == 0\n"),
        (19, "expected a condition".into())
    );
}

#[test]
fn k3_is_a_t_is_a_relation_exported_with_its_type() {
    let b = "type Dog\n";
    let a = "type Kind\nstatic {x: Kind} is a {kind: Kind}\nderived {x: Kind} is seen\nrule seen\n  ?x is seen\n  when\n    ?x is a Dog\n";
    read_program(&[("a", a), ("b", b)]).unwrap();
    let (module, error) = read_program(&[("a", &format!("use b\n{a}")), ("b", b)]).unwrap_err();
    assert_eq!((module.as_str(), error.line), ("a", 8));
    assert!(
        error.message.contains("as is-a: {x} is a {kind}") && error.message.contains("as is-a-Dog from b: {entity} is a Dog"),
        "{}",
        error.message
    );
}

#[test]
fn d49_each_anonymous_variable_under_not_is_existential() {
    use muove_core::why::{render, why};
    use muove_core::*;
    let lowered = |source: &str| {
        let lowered = muove_syntax::lower::lower(source, "anonymous.muove").unwrap();
        assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
        lowered.program
    };
    let derived = |program: &Program, words: &str| {
        let relation = RelationId(program.relations.iter().position(|r| r.template.contains(words)).unwrap());
        let reference = evaluate(program, &[], Budget::default()).unwrap();
        let engine = engine::evaluate(program, &[], Budget::default()).unwrap();
        let rows: Vec<Vec<Value>> = reference.tuples(relation).cloned().collect();
        assert_eq!(rows, engine.tuples(relation).cloned().collect::<Vec<_>>());
        (relation, rows)
    };
    let library = lowered(
        "type Person, Book\nstatic {person: Person} has read {book: Book}\nderived {person: Person} reads nothing\nana is a Person\nben is a Person\ndune is a Book\nana has read dune\n\nrule idle\n  ?p reads nothing\n  when\n    ?p is a Person\n    not ?p has read ?_\n",
    );
    let (reads_nothing, rows) = derived(&library, "reads nothing");
    let ben = vec![Value::entity("Person", "ben")];
    assert_eq!(rows, vec![ben.clone()]);
    let proof = why(&library, &[], reads_nothing, &ben, Budget::default()).unwrap();
    let shown = render(&library, reads_nothing, &ben, &proof, Budget::default());
    assert!(shown.contains("nothing states: ben has read ?_"), "{shown}");

    let seen = "static {a: Int} connects {b: Int}\nderived {n: Int} was seen\n\nrule seen\n  9 was seen\n  when\n    not ?_ connects ?_\n";
    assert_eq!(derived(&lowered(seen), "was seen").1, vec![vec![Value::Int(9)]]);
    assert!(derived(&lowered(&format!("{seen}\n1 connects 2\n")), "was seen").1.is_empty());
}
