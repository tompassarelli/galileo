#![allow(dead_code)]

pub const HERO: &str = "Ash";
pub const WALK: [(&str, &str, &str); 6] = [
    ("moves through", "Door", "Front-Door"),
    ("moves through", "Door", "Armory-Gate"),
    ("moves through", "Door", "Library-Arch"),
    ("picks up", "Key", "Bronze-Key"),
    ("moves through", "Door", "Crypt-Stair"),
    ("picks up", "Key", "Iron-Key"),
];
pub const LOCKED: usize = 1;
pub const REVISIONS: [u64; 6] = [1, 1, 2, 3, 4, 5];

pub fn events_text() -> String {
    WALK.iter().map(|(verb, _, name)| format!("{HERO} {verb} {name}\n")).collect()
}

pub fn template(verb: &str) -> &'static str {
    if verb == "moves through" { "{hero} moves through {door}" } else { "{hero} picks up {key}" }
}
