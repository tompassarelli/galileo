use galileo_core::*;
use std::collections::BTreeMap;

#[test]
fn clause_8_4_recorded_text_builtins_match_all_144_cases() {
    let cases = include_str!("../../../tests/external/clause-builtins/cases.jsonl");
    let mut groups = BTreeMap::new();
    for line in cases.lines() {
        let case: serde_json::Value = serde_json::from_str(line).unwrap();
        let original = case["function"].as_str().unwrap();
        let function = match original {
            "contains-text" => "contains",
            "remaining-words" => "rest-words",
            name => name,
        };
        let expected = match &case["output"] {
            serde_json::Value::Bool(value) => Value::Bool(*value),
            serde_json::Value::String(value) => Value::Text(value.clone()),
            value => panic!("unexpected reference output: {value}"),
        };
        let arguments = case["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| Expr::value(value.as_str().unwrap()))
            .collect::<Vec<_>>();
        let mut builder = Program::builder();
        let result = builder.relation(Relation::new(
            "result",
            RelationKind::Derived,
            [Slot::new(
                "value",
                if matches!(expected, Value::Bool(_)) {
                    Type::Bool
                } else {
                    Type::Text
                },
            )],
        ));
        builder.rule(Rule::new(
            "recorded",
            Atom::new(result, [Expr::call(function, arguments)]),
        ));
        let program = builder.build();
        let model = evaluate(&program, &[], Budget::default()).unwrap();
        assert_eq!(
            model.tuples(result).collect::<Vec<_>>(),
            [&vec![expected]],
            "first mismatch: {}",
            case["id"]
        );
        *groups.entry(original.to_string()).or_insert(0) += 1;
    }
    assert_eq!(groups.len(), 6);
    assert!(groups.values().all(|count| *count == 24));
    eprintln!("Clause external reference: groups={groups:?}; cases=144; mismatches=0");
}
