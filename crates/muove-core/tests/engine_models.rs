use muove_core::*;

#[test]
fn engine_models_equal_the_reference_interpreter() {
    for program in [examples::dungeon_static(), examples::tags()] {
        let reference = evaluate(&program, &[], Budget::default()).unwrap();
        let engine = engine::evaluate(&program, &[], Budget::default()).unwrap();
        assert!(!reference.relations.is_empty());
        assert_eq!(engine.relations, reference.relations);
    }
}
