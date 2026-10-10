use muove_core::{examples, Literal};

#[test]
fn prototype_dungeon_and_spec_9_6_and_r13_program_sizes() {
    let dungeon = examples::dungeon_static();
    assert_eq!(
        (
            dungeon.relations.len(),
            dungeon.rules.len(),
            dungeon.facts.len()
        ),
        (10, 5, 19)
    );
    let tags = examples::tags();
    assert_eq!((tags.relations.len(), tags.rules.len()), (13, 5));
    let r13 = examples::r13();
    assert_eq!(r13.facts.len(), 40);
    let goal = r13
        .rules
        .iter()
        .find(|rule| rule.name == "r13/all-stages")
        .unwrap();
    assert_eq!(goal.body.len(), 20);
    assert!(goal
        .body
        .iter()
        .all(|literal| matches!(literal, Literal::Positive(_))));
}
