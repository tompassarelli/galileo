use muove_core::examples::dungeon_static;
use muove_core::supports::{achieve, AchieveMode, Intervention};
use muove_core::why::sentence;
use muove_core::*;
use std::collections::BTreeSet;

fn placements(program: &Program, sets: &[Vec<Fact>]) -> BTreeSet<String> {
    sets.iter()
        .map(|set| {
            assert_eq!(set.len(), 1, "{set:?}");
            sentence(program, set[0].relation, &set[0].values)
        })
        .collect()
}

#[test]
fn r17_achieve_acceptable_excludes_the_placements_that_break_never() {
    let program = dungeon_static();
    let reach = program.relation_id("level/reach").unwrap();
    let lies = program.relation_id("level/lies-in").unwrap();
    let shrine = [Value::entity("Hero", "Ash"), Value::entity("Room", "Shrine")];
    let budget = Budget::default();
    let six: BTreeSet<String> = ["Entrance", "Hall", "Library", "Armory", "Crypt", "Vault"]
        .iter()
        .map(|room| format!("Silver-Key lies in {room}"))
        .collect();

    let Intervention::Found { sets, excluded } =
        achieve(&program, &[], reach, &shrine, &[lies], AchieveMode::Possible, budget).unwrap()
    else {
        panic!("expected placements")
    };
    assert_eq!(placements(&program, &sets), six);
    assert!(excluded.is_empty());

    let Intervention::Found { sets, excluded } =
        achieve(&program, &[], reach, &shrine, &[lies], AchieveMode::Acceptable, budget).unwrap()
    else {
        panic!("expected placements")
    };
    assert!(sets.is_empty(), "{sets:?}");
    let broken: Vec<Vec<Fact>> = excluded.iter().map(|(set, _)| set.clone()).collect();
    assert_eq!(placements(&program, &broken), six);
    assert!(excluded.iter().all(|(_, names)| names == &["never-shrine".to_string()]));

    let mut permissive = program.clone();
    permissive.requirements.retain(|r| r.name != "never-shrine");
    let Intervention::Found { sets, excluded } =
        achieve(&permissive, &[], reach, &shrine, &[lies], AchieveMode::Acceptable, budget).unwrap()
    else {
        panic!("expected placements")
    };
    assert_eq!(placements(&permissive, &sets), six);
    assert!(excluded.is_empty());

    let starved = achieve(&program, &[], reach, &shrine, &[lies], AchieveMode::Acceptable, Budget::work(500)).unwrap();
    assert!(matches!(starved, Intervention::Unknown(_)), "{starved:?}");
}
