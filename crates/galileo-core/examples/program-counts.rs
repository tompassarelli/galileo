use galileo_core::examples;

fn main() {
    for (name, program) in [
        ("dungeon-static", examples::dungeon_static()),
        ("tags", examples::tags()),
        ("R13", examples::r13()),
    ] {
        println!(
            "{name}: {} relations, {} rules, {} facts",
            program.relations.len(),
            program.rules.len(),
            program.facts.len()
        );
    }
}
