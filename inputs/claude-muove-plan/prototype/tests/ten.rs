//! The 10 version: must-sentences, reviews of proposed changes, why-not,
//! and the compiler explaining its own mistakes.

use muove::ask::{self, CondState, Problem};
use muove::engine::{Limits, close};
use muove::program::{Pick, Program};

const DUNGEON: &str = include_str!("../examples/dungeon.clause");
const SECRET_PASSAGE: &str = include_str!("../examples/proposals/secret-passage.clause");
const ARMORY_COLLAPSE: &str = include_str!("../examples/proposals/armory-collapse.clause");
const PITFALL: &str = include_str!("../examples/proposals/pitfall.clause");
const CROWBAR: &str = include_str!("../examples/proposals/crowbar.clause");
const LIBRARY_LADDER: &str = include_str!("../examples/proposals/library-ladder.clause");

fn limits() -> Limits {
    Limits::default()
}

/// Parse the dungeon plus one proposal, and review the proposal's revision.
fn review(proposal: &str, revision: &str) -> (Program, ask::Review) {
    let p = Program::parse(&format!("{DUNGEON}\n{proposal}")).unwrap();
    let base = p.world("level").unwrap();
    let successor = p.world(revision).unwrap();
    let result = ask::review(&p, base, successor, &limits());
    (p, result)
}

fn broken_texts(p: &Program, review: &ask::Review) -> Vec<String> {
    review
        .intents
        .iter()
        .filter(|r| r.before.holds && !r.after.holds)
        .map(|r| p.intents[r.intent].text.clone())
        .collect()
}

#[test]
fn every_must_sentence_holds_in_the_level() {
    let p = Program::parse(DUNGEON).unwrap();
    let checks = ask::check_world(&p, p.world("level").unwrap(), &limits());
    assert_eq!(checks.len(), 5);
    assert!(checks.iter().all(|(_, c)| c.holds && !c.vacuous));
    let (report, ok) = muove::intents_report(&p, &limits());
    assert!(ok);
    assert!(report.contains("All 5 must-sentences hold."));
}

#[test]
fn a_secret_passage_breaks_the_library_requirement() {
    let (p, r) = review(SECRET_PASSAGE, "level/secret-passage");
    assert_eq!(r.says.as_deref(), Some("Adds a hidden shortcut for speedrunners. The main route is unchanged."));
    assert_eq!(broken_texts(&p, &r), ["Ash can reach Vault requires Ash can reach Library"]);
    let item = r.intents.iter().find(|i| !i.after.holds).unwrap();
    let Some(Problem::Bypassed { fact, needed, .. }) = &item.after.problem else { panic!() };
    assert_eq!(p.sentence(fact), "Ash can reach Vault");
    assert_eq!(needed, "Ash can reach Library");
}

#[test]
fn collapsing_the_armory_leaves_one_key_as_a_single_point_of_failure() {
    let (p, r) = review(ARMORY_COLLAPSE, "level/armory-collapse");
    assert_eq!(
        broken_texts(&p, &r),
        ["Ash can reach Vault has no single point of failure among ?key lies in ?room"]
    );
    let item = r.intents.iter().find(|i| !i.after.holds).unwrap();
    let Some(Problem::SinglePoints { points, .. }) = &item.after.problem else { panic!() };
    let points: Vec<String> = points.iter().map(|f| p.sentence(f)).collect();
    assert_eq!(points, ["Iron-Key lies in Crypt"]);

    // The level is still winnable, which the proposal truthfully claims, but weaker.
    let winnable = r.intents.iter().find(|i| p.intents[i.intent].text == "Ash can reach Vault").unwrap();
    assert!(winnable.after.holds);
    assert_eq!(winnable.weaker.as_ref().map(|w| (w.0, w.1)), Some((2, 1)));

    // Cross-check: the single points of failure are exactly the one-fact preventions.
    let world = p.world("level/armory-collapse").unwrap();
    let vault = p.fact("level", "Ash can reach Vault").unwrap();
    let prevent = ask::prevent(&p, world, &vault, &[p.relation("level/lies-in").unwrap()], Pick::All, &limits());
    let singles: Vec<String> = prevent.solutions.iter().filter(|s| s.len() == 1).map(|s| p.sentence(&s[0])).collect();
    assert_eq!(singles, points);
}

#[test]
fn an_untelegraphed_pit_breaks_fairness_and_says_what_else_it_touches() {
    let (p, r) = review(PITFALL, "level/pitfall");
    assert_eq!(broken_texts(&p, &r), ["?trap hurts ?hero requires ?trap is telegraphed"]);
    assert_eq!(r.diff.rules_added.len(), 1);
    let item = r.intents.iter().find(|i| !i.after.holds).unwrap();
    let Some(Problem::Bypassed { fact, others, .. }) = &item.after.problem else { panic!() };
    assert_eq!(p.sentence(fact), "Pit-Trap hurts Ash");
    let others: Vec<String> = others.iter().map(|f| p.sentence(f)).collect();
    assert_eq!(others, ["Spike-Trap hurts Ash"]);
}

#[test]
fn a_crowbar_rule_breaks_two_must_sentences() {
    let (p, r) = review(CROWBAR, "level/crowbar");
    assert_eq!(
        broken_texts(&p, &r),
        ["Ash can reach Vault requires Ash can reach Library", "never Ash can reach Shrine"]
    );
    assert_eq!(r.broken, 2);
}

#[test]
fn a_ladder_from_the_library_is_approved() {
    let (_, r) = review(LIBRARY_LADDER, "level/library-ladder");
    assert_eq!(r.broken, 0);
    assert!(r.intents.iter().all(|i| i.after.holds && i.weaker.is_none()));
}

#[test]
fn the_cli_review_rejects_and_approves() {
    let (text, ok) = muove::review(DUNGEON, SECRET_PASSAGE, &limits()).unwrap();
    assert!(!ok);
    assert!(text.contains("The proposal says: \"Adds a hidden shortcut"));
    assert!(text.contains("Verdict: reject. It breaks 1 of 5 must-sentences."));
    let (text, ok) = muove::review(DUNGEON, LIBRARY_LADDER, &limits()).unwrap();
    assert!(ok);
    assert!(text.contains("Verdict: approve."));
}

#[test]
fn removing_a_rule_is_reviewed_and_explained_with_why_not() {
    let proposal = "level/no-walking: Revision\n  from: level\n  remove rule level/walk\n";
    let (p, r) = review(proposal, "level/no-walking");
    assert_eq!(r.diff.rules_removed.len(), 1);
    assert!(broken_texts(&p, &r).contains(&"Ash can reach Vault".to_string()));
    let text = muove::review(DUNGEON, proposal, &limits()).unwrap().0;
    assert!(text.contains("− rule level/walk: ?hero can reach ?there when"));
    assert!(text.contains("It doesn't hold. The closest it gets:"));
}

#[test]
fn why_not_says_what_the_gatekeeper_would() {
    let p = Program::parse(DUNGEON).unwrap();
    let shrine = p.fact("level", "Ash can reach Shrine").unwrap();
    let answer = ask::why_not(&p, p.world("level").unwrap(), &shrine, &limits());
    assert!(!answer.holds);
    let closest = &answer.attempts[0];
    assert_eq!(p.rules[closest.rule].name, "level/unlock");
    assert_eq!((closest.missing, closest.met), (1, 3));
    let missing = closest.conditions.iter().find(|c| c.state != CondState::Met).unwrap();
    assert_eq!(missing.text, "Ash holds Silver-Key");
    let deeper = &missing.deeper[0];
    assert_eq!(p.rules[deeper.rule].name, "level/pick-up");
    assert_eq!(deeper.conditions[0].text, "Silver-Key lies in ?room");
    assert_eq!(deeper.conditions[0].state, CondState::Missing { stated: true });
}

#[test]
fn blocking_a_fact_never_concludes_it() {
    let p = Program::parse(DUNGEON).unwrap();
    let level = p.world("level").unwrap();
    let library = p.fact("level", "Ash can reach Library").unwrap();
    let blocked = |f: &muove::Fact| *f == library;
    let without = muove::engine::close_blocking(&p, level, &p.worlds[level].facts, &blocked, &limits());
    assert!(!without.holds(&library));
    assert!(!without.holds(&p.fact("level", "Ash can reach Vault").unwrap()));
    assert!(close(&p, level, &p.worlds[level].facts, &limits()).holds(&library));
}

#[test]
fn the_compiler_explains_its_own_mistakes() {
    let typo = DUNGEON.replace("  Ash can reach Vault\n  Ash can reach Vault requires", "  Ash can reach Valt\n  Ash can reach Vault requires");
    let error = Program::parse(&typo).unwrap_err();
    assert!(error.message.contains("Did you mean `Vault`?"), "{}", error.message);

    let shapeless = DUNGEON.replace("  Ash starts in Entrance\n", "  Ash begins at Entrance\n");
    let error = Program::parse(&shapeless).unwrap_err();
    assert!(error.message.contains("Closest:"), "{}", error.message);

    let proposal = "level/typo: Revision\n  from: level\n  admit:\n    Front-Door is unlockd\n";
    let error = muove::review(DUNGEON, proposal, &limits()).unwrap_err();
    assert_eq!(error.line, 4, "errors point into the proposal file");
    assert!(error.message.starts_with("in the proposal:"));

    let bad = DUNGEON.replace("never Ash can reach Shrine", "Ash can reach Vault requires Ash can fly");
    assert!(Program::parse(&bad).is_err());
}

#[test]
fn run_prints_questions_then_must_sentences() {
    let text = muove::run(DUNGEON, &limits()).unwrap();
    assert!(text.contains("▸ why not in level:"));
    assert!(text.contains("▸ level must:"));
    assert!(text.contains("All 5 must-sentences hold."));
}
