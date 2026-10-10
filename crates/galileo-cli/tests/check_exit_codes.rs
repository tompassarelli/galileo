use std::path::{Path, PathBuf};
use std::process::Command;

const GATE: &str = "\
type Node

given {node: Node} is listed
given {node: Node} is seeded
derived {node: Node} holds

Seed is a Node
Gate is a Node
Goal is a Node
Seed is listed
Gate is listed
Goal is listed
Seed is seeded

rule seed: ?n holds
  when ?n is seeded

rule make-gate: Gate holds
  when Seed holds

rule make-goal: Goal holds
  when Gate holds

requirements
  Goal holds
  every ?n where ?n is listed: ?n holds
";

const TAGS_USER: &str = "use firn/tags\n";

const FIREFOX: &str = "\
firefox is a Module
browsers is a Tag
firefox is tagged browsers
browsers sets myConfig.modules.firefox.default to true for firefox
";

const WHITERABBIT: &str = "\
whiterabbit is a Host
whiterabbit runs linux
whiterabbit enables browsers
linux is a Platform
linux allows every module
";

fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    for (file, text) in files {
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    assert!(Command::new("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
    dir
}

fn check(cwd: &Path, arguments: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).arg("check").args(arguments).current_dir(cwd).output().unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    (out.status.code().unwrap(), text)
}

#[test]
fn galileo_check_exits_0_when_holding_1_when_failing_and_2_when_unknown() {
    let rooms: String = (0..101).map(|i| format!("r{i} is a Room\n")).collect();
    let doors: String = (1..101).map(|i| format!("r{} opens to r{i}\n", i - 1)).collect();
    let chain = format!(
        "type Room\n\ngiven {{here: Room}} opens to {{there: Room}}\nderived {{here: Room}} reaches {{there: Room}}\n\n{rooms}{doors}\nrule step: ?a reaches ?b\n  when ?a opens to ?b\n\nrule walk: ?a reaches ?c\n  when ?a reaches ?b\n  and ?b opens to ?c\n\nrequirements\n  r0 reaches r100\n"
    );
    let never = format!("{GATE}  never Goal holds\n");
    let dir = scratch("exit-codes", &[("gate.gal", GATE), ("gate-never.gal", &never), ("chain.gal", &chain)]);
    assert_eq!(check(&dir, &["gate.gal"]).0, 0);
    assert_eq!(check(&dir, &["gate-never.gal"]).0, 1);
    assert_eq!(check(&dir, &["chain.gal", "--budget", "1000"]).0, 2);
}

#[test]
fn a_project_reads_its_modules_and_the_embedded_library_from_any_directory() {
    let firefox = format!("{TAGS_USER}\n{FIREFOX}");
    let whiterabbit = format!("{TAGS_USER}\n{WHITERABBIT}");
    let requirements = format!("{TAGS_USER}\nrequirements\n  firefox is enabled on whiterabbit\n");
    let dir = scratch(
        "linked-project",
        &[
            ("modules/firefox/default.gal", &firefox),
            ("hosts/whiterabbit/tags.gal", &whiterabbit),
            ("requirements.gal", &requirements),
        ],
    );
    let (code, text) = check(&dir, &[]);
    assert_eq!(code, 0, "{text}");
    let (code, text) = check(&dir.join("hosts"), &[]);
    assert_eq!(code, 0, "{text}");

    let one = "type Thing\ngiven {thing: Thing} is shiny\nlamp is a Thing\nlamp is shiny\n";
    let two = "type Thing\ngiven {thing: Thing} is shiny\nspoon is a Thing\nspoon is shiny\n";
    let dir = scratch("same-template", &[("one.gal", one), ("two.gal", two)]);
    let (code, text) = check(&dir, &[]);
    assert_eq!(code, 0, "{text}");
}
