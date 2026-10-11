use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

const LEVEL: &str = "type Hero, Room, Door, Key

Ash is a Hero
Entrance is a Room
Hall is a Room
Library is a Room
Armory is a Room
Crypt is a Room
Vault is a Room
Shrine is a Room
Front-Door is a Door
Library-Arch is a Door
Armory-Gate is a Door
Crypt-Stair is a Door
Vault-Gate is a Door
Crypt-Door is a Door
Shrine-Door is a Door
Bronze-Key is a Key
Iron-Key is a Key
Silver-Key is a Key

given Hero starts in Room
given Door leads from {here: Room} to {there: Room}
given Door is unlocked
given Door is locked by Key
  at most one key per door
given Key lies in Room
derived Hero can reach Room
derived Hero holds Key

Ash starts in Entrance
Front-Door
  leads from Entrance to Hall
  is unlocked
Library-Arch
  leads from Hall to Library
  is unlocked
Armory-Gate
  leads from Hall to Armory
  is locked by Bronze-Key
Crypt-Stair
  leads from Library to Crypt
  is unlocked
Vault-Gate
  leads from Armory to Vault
  is locked by Bronze-Key
Crypt-Door
  leads from Crypt to Vault
  is locked by Iron-Key
Shrine-Door
  leads from Crypt to Shrine
  is locked by Silver-Key
Bronze-Key lies in Library
Iron-Key lies in Crypt

rule start: ?hero can reach ?room
  when ?hero starts in ?room

rule walk: ?hero can reach ?there
  when ?hero can reach ?here
  and ?door leads from ?here to ?there
  and ?door is unlocked

rule unlock: ?hero can reach ?there
  when ?hero can reach ?here
  and ?door leads from ?here to ?there
  and ?door is locked by ?key
  and ?hero holds ?key

rule pick-up: ?hero holds ?key
  when ?key lies in ?room
  and ?hero can reach ?room

requirements for the level
  never Ash can reach Shrine
";

fn galileo(root: &Path, arguments: &str) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_galileo")).args(arguments.split(' ')).current_dir(root).output().unwrap();
    (output.status.code().unwrap(), String::from_utf8_lossy(&output.stdout).into_owned(), String::from_utf8_lossy(&output.stderr).into_owned())
}

fn sets(stdout: &str) -> BTreeSet<BTreeSet<String>> {
    let mut out = Vec::new();
    let mut open = false;
    for line in stdout.lines() {
        if line.ends_with('.') && line[..line.len() - 1].parse::<usize>().is_ok() {
            out.push(BTreeSet::new());
            open = true;
        } else if let (true, Some(sentence)) = (open, line.strip_prefix("  ")) {
            out.last_mut().unwrap().insert(sentence.to_string());
        } else {
            open = false;
        }
    }
    out.into_iter().collect()
}

#[test]
fn spec_7_4_prevent_and_achieve_print_the_intervention_sets_on_the_static_dungeon() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("interventions");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("level.gal"), LEVEL).unwrap();
    let placements: BTreeSet<BTreeSet<String>> = ["Entrance", "Hall", "Library", "Armory", "Crypt", "Vault"]
        .iter()
        .map(|room| BTreeSet::from([format!("Silver-Key lies in {room}")]))
        .collect();

    let (code, stdout, stderr) = galileo(&root, "achieve possible Ash can reach Shrine --using lies-in");
    assert_eq!((code, sets(&stdout)), (0, placements.clone()), "{stdout}{stderr}");

    let (code, stdout, stderr) = galileo(&root, "achieve acceptable Ash can reach Shrine --using lies-in");
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(sets(&stdout).is_empty(), "{stdout}");
    assert!(stdout.starts_with("no addition set within --using lies-in"), "{stdout}");

    let (code, stdout, stderr) = galileo(&root, "prevent level.gal Ash can reach Vault --using is-unlocked");
    let withdrawals = BTreeSet::from([
        BTreeSet::from(["Front-Door is unlocked".to_string()]),
        BTreeSet::from(["Library-Arch is unlocked".to_string()]),
    ]);
    assert_eq!((code, sets(&stdout)), (0, withdrawals), "{stdout}{stderr}");

    let (code, stdout, stderr) = galileo(&root, "prevent ?x holds --using lies-in");
    assert_eq!(code, 2, "{stdout}");
    assert!(stderr.contains("?x"), "{stderr}");

    let _ = std::fs::remove_dir_all(&root);
}
