//! The original Clause acceptance oracles (hospital, impact, achievement),
//! ported from the pre-reset test suite, plus the dungeon example and
//! brute-force cross-checks of every prevent/achieve answer.

use muove::ask::{self, Answer};
use muove::engine::{Limits, close};
use muove::program::{Pick, Program, Request};
use muove::{Fact, Status};
use std::collections::BTreeSet;

const HOSPITAL: &str = include_str!("../examples/hospital.clause");
const IMPACT: &str = include_str!("../examples/impact.clause");
const DUNGEON: &str = include_str!("../examples/dungeon.clause");

const ACHIEVEMENT: &str = r#"Start
Option
State

choice/selects: RelationShape
  {start: Start} selects {option: Option}
  mode start -> option: many

choice/reached: RelationShape
  {start: Start} reached {state: State}
  mode start -> state: many

choice
  South ∈ Start
  Beagle ∈ Option
  North ∈ Option
  Relay ∈ Option
  Store ∈ Option
  Ready ∈ State

choice/selection-reaches-ready: DerivationRule
  ?start reached Ready
  when:
    ?start selects ?option
"#;

fn sets(p: &Program, world: &str, groups: &[&[&str]]) -> BTreeSet<BTreeSet<Fact>> {
    groups
        .iter()
        .map(|group| group.iter().map(|text| p.fact(world, text).unwrap()).collect())
        .collect()
}

fn as_sets(groups: &[Vec<Fact>]) -> BTreeSet<BTreeSet<Fact>> {
    groups.iter().map(|g| g.iter().cloned().collect()).collect()
}

fn world(p: &Program, name: &str) -> usize {
    p.world(name).unwrap()
}

fn rel(p: &Program, name: &str) -> usize {
    p.relation(name).unwrap()
}

fn limits() -> Limits {
    Limits::default()
}

/// Every minimal subset of the `using` facts whose withdrawal stops `target`.
fn brute_prevent(p: &Program, w: usize, target: &Fact, using: &[usize]) -> BTreeSet<BTreeSet<Fact>> {
    let world = &p.worlds[w];
    let pool: Vec<Fact> = world.facts.iter().filter(|f| using.contains(&f.rel)).cloned().collect();
    assert!(pool.len() <= 16, "brute force is for small pools");
    let mut hits = Vec::new();
    for mask in 0u32..(1 << pool.len()) {
        let chosen: BTreeSet<Fact> = (0..pool.len()).filter(|i| mask & (1 << i) != 0).map(|i| pool[i].clone()).collect();
        let rest: BTreeSet<Fact> = world.facts.difference(&chosen).cloned().collect();
        if !close(p, w, &rest, &limits()).holds(target) {
            hits.push(chosen);
        }
    }
    minimal(hits)
}

/// Every minimal set of at most `cap` new `using` facts that makes `target` hold.
fn brute_achieve(p: &Program, w: usize, target: &Fact, using: &[usize], cap: usize) -> BTreeSet<BTreeSet<Fact>> {
    let world = &p.worlds[w];
    let mut pool = Vec::new();
    for &r in using {
        let roles = &p.relations[r].roles;
        let pools: Vec<Vec<usize>> = roles
            .iter()
            .map(|role| world.entities.iter().copied().filter(|&e| p.entities[e].ty == role.ty).collect())
            .collect();
        let mut args = vec![Vec::new()];
        for options in &pools {
            args = args
                .into_iter()
                .flat_map(|prefix: Vec<usize>| options.iter().map(move |&o| [prefix.clone(), vec![o]].concat()))
                .collect();
        }
        for a in args {
            let fact = Fact { rel: r, args: a };
            if !world.facts.contains(&fact) {
                pool.push(fact);
            }
        }
    }
    let mut hits = Vec::new();
    let mut choose = |set: BTreeSet<Fact>| {
        let with: BTreeSet<Fact> = world.facts.union(&set).cloned().collect();
        if close(p, w, &with, &limits()).holds(target) {
            hits.push(set);
        }
    };
    for i in 0..pool.len() {
        choose(BTreeSet::from([pool[i].clone()]));
        if cap >= 2 {
            for j in i + 1..pool.len() {
                choose(BTreeSet::from([pool[i].clone(), pool[j].clone()]));
            }
        }
    }
    minimal(hits)
}

fn minimal(sets: Vec<BTreeSet<Fact>>) -> BTreeSet<BTreeSet<Fact>> {
    sets.iter()
        .filter(|s| !sets.iter().any(|o| o.len() < s.len() && o.is_subset(s)))
        .cloned()
        .collect()
}

/// Run every prevent/achieve question in a program and compare with brute force.
fn cross_check(source: &str, achieve_cap: usize) {
    let p = Program::parse(source).unwrap();
    for item in &p.requests {
        match &item.request {
            Request::Prevent { world, target, using, .. } => {
                let got = ask::prevent(&p, *world, target, using, Pick::All, &limits());
                assert!(got.status.is_complete());
                if got.already || got.impossible {
                    continue;
                }
                assert_eq!(as_sets(&got.solutions), brute_prevent(&p, *world, target, using), "line {}", item.line);
            }
            Request::Achieve { world, target, using, .. } => {
                let got = ask::achieve(&p, *world, target, using, Pick::All, &limits());
                assert!(got.status.is_complete());
                if got.already || got.impossible {
                    continue;
                }
                assert!(got.solutions.iter().all(|s| s.len() <= achieve_cap));
                assert_eq!(as_sets(&got.solutions), brute_achieve(&p, *world, target, using, achieve_cap), "line {}", item.line);
            }
            Request::Why { world, target, .. } => {
                // Each reason suffices on its own, and dropping any one fact breaks it.
                let got = ask::why(&p, *world, target, Pick::All, &limits());
                for reason in &got.reasons {
                    let set: BTreeSet<Fact> = reason.facts.iter().cloned().collect();
                    assert!(close(&p, *world, &set, &limits()).holds(target));
                    for fact in &set {
                        let mut smaller = set.clone();
                        smaller.remove(fact);
                        assert!(!close(&p, *world, &smaller, &limits()).holds(target));
                    }
                }
            }
            _ => {}
        }
    }
}

#[test]
fn hospital_matches_the_original_oracle() {
    let p = Program::parse(HOSPITAL).unwrap();
    let base = world(&p, "egress");
    let successor = world(&p, "egress/door-101-withdrawn");
    // The original counted memberships and facts together: 12 + 10.
    let size = |w: usize| p.worlds[w].facts.len() + p.worlds[w].entities.len();
    assert_eq!(size(base), 22);
    assert_eq!(size(successor), 21);
    assert_eq!(p.requests.len(), 6);

    let answers: Vec<Answer> = p.requests.iter().map(|r| ask::answer(&p, &r.request, &limits())).collect();
    assert!(answers.iter().all(|a| a.status().is_complete()));

    let Answer::Find(found) = &answers[0] else { panic!() };
    assert_eq!(found.values, ["East-Corridor", "North-Exit", "West-Corridor"]);

    let Answer::Why(why) = &answers[1] else { panic!() };
    assert!(why.holds);
    assert_eq!(
        as_sets(&why.reasons.iter().map(|r| r.facts.clone()).collect::<Vec<_>>()),
        sets(&p, "egress", &[
            &[
                "Door 101 connects ICU-A to East-Corridor",
                "Door 101 passed Fire-Marshal-Inspection",
                "Door 102 connects East-Corridor to North-Exit",
                "Door 102 passed Fire-Marshal-Inspection",
            ],
            &[
                "Door 103 connects ICU-A to West-Corridor",
                "Door 103 passed Fire-Marshal-Inspection",
                "Door 104 connects West-Corridor to North-Exit",
                "Door 104 passed Fire-Marshal-Inspection",
            ],
        ]),
    );

    let passed = |n: u32| format!("Door {n} passed Fire-Marshal-Inspection");
    let Answer::Prevent(prevent) = &answers[2] else { panic!() };
    assert_eq!(
        as_sets(&prevent.solutions),
        sets(&p, "egress", &[
            &[&passed(101), &passed(103)],
            &[&passed(101), &passed(104)],
            &[&passed(102), &passed(103)],
            &[&passed(102), &passed(104)],
        ]),
    );
    let Answer::Prevent(prevent) = &answers[3] else { panic!() };
    assert_eq!(as_sets(&prevent.solutions), sets(&p, "egress", &[&[&passed(103)], &[&passed(104)]]));

    let Answer::Achieve(achieve) = &answers[4] else { panic!() };
    assert_eq!(as_sets(&achieve.solutions), sets(&p, "egress", &[&[&passed(105)], &[&passed(106)]]));

    let Answer::Diff(diff) = &answers[5] else { panic!() };
    assert!(diff.added.is_empty());
    assert_eq!(diff.removed, vec![p.fact("egress", &passed(101)).unwrap()]);
    assert!(diff.gained.is_empty());
    assert_eq!(diff.lost.len(), 1);
    let east = &diff.lost[0];
    assert_eq!(east.fact, p.fact("egress", "ICU-A has a usable egress path to East-Corridor").unwrap());
    assert_eq!(
        as_sets(&east.reasons_before),
        sets(&p, "egress", &[&["Door 101 connects ICU-A to East-Corridor", &passed(101)]]),
    );
    assert_eq!(diff.changed.len(), 1);
    let north = &diff.changed[0];
    assert_eq!(north.fact, p.fact("egress", "ICU-A has a usable egress path to North-Exit").unwrap());
    assert!(north.new.is_empty());
    assert_eq!(
        as_sets(&north.lost),
        sets(&p, "egress", &[&[
            "Door 101 connects ICU-A to East-Corridor",
            &passed(101),
            "Door 102 connects East-Corridor to North-Exit",
            &passed(102),
        ]]),
    );
    assert_eq!(
        as_sets(&north.kept),
        sets(&p, "egress", &[&[
            "Door 103 connects ICU-A to West-Corridor",
            &passed(103),
            "Door 104 connects West-Corridor to North-Exit",
            &passed(104),
        ]]),
    );
    assert_eq!(north.now_required.len(), 4, "the west path became a chain of single points of failure");
}

#[test]
fn impact_matches_the_original_oracle() {
    let p = Program::parse(IMPACT).unwrap();
    let base = world(&p, "impact");
    assert_eq!(p.worlds[base].facts.len() + p.worlds[base].entities.len(), 11);
    assert_eq!(p.requests.len(), 5);
    let target = p.fact("impact", "compiler-change affects North").unwrap();

    let why = ask::why(&p, base, &target, Pick::All, &limits());
    assert!(why.status.is_complete());
    assert_eq!(
        as_sets(&why.reasons.iter().map(|r| r.facts.clone()).collect::<Vec<_>>()),
        sets(&p, "impact", &[
            &["compiler-change changes Beagle", "North imports Relay", "Relay imports Beagle"],
            &["compiler-change changes Beagle", "North imports Store", "Store imports Beagle"],
        ]),
    );

    let prevent = ask::prevent(&p, base, &target, &[rel(&p, "impact/imports")], Pick::All, &limits());
    assert_eq!(
        as_sets(&prevent.solutions),
        sets(&p, "impact", &[
            &["North imports Relay", "North imports Store"],
            &["North imports Relay", "Store imports Beagle"],
            &["North imports Store", "Relay imports Beagle"],
            &["Relay imports Beagle", "Store imports Beagle"],
        ]),
    );

    let successor = world(&p, "impact/adopt-south");
    let diff = ask::diff(&p, base, successor, &limits());
    assert_eq!(diff.added, vec![p.fact("impact/adopt-south", "South imports North").unwrap()]);

    // The original suite's "redundant path withdrawn" revision.
    let source = IMPACT.replace(
        "impact/adopt-south: Revision",
        "impact/redundant-path-withdrawn: Revision\n  from: impact\n  withdraw:\n    North imports Relay\n    Relay imports Beagle\n\nimpact/adopt-south: Revision",
    );
    let p = Program::parse(&source).unwrap();
    let target = p.fact("impact", "compiler-change affects North").unwrap();
    let diff = ask::diff(&p, world(&p, "impact"), world(&p, "impact/redundant-path-withdrawn"), &limits());
    assert!(!diff.lost.iter().any(|l| l.fact == target));
    let change = diff.changed.iter().find(|c| c.fact == target).expect("support changed");
    assert!(change.new.is_empty());
    assert_eq!(change.lost.len(), 1);
    assert_eq!(change.kept.len(), 1);
}

#[test]
fn typed_active_domain_yields_four_additions() {
    let p = Program::parse(ACHIEVEMENT).unwrap();
    let target = p.fact("choice", "South reached Ready").unwrap();
    let got = ask::achieve(&p, world(&p, "choice"), &target, &[rel(&p, "choice/selects")], Pick::All, &limits());
    assert!(got.status.is_complete());
    assert_eq!(
        as_sets(&got.solutions),
        sets(&p, "choice", &[
            &["South selects Beagle"],
            &["South selects North"],
            &["South selects Relay"],
            &["South selects Store"],
        ]),
    );
}

#[test]
fn dungeon_answers_the_designer_questions() {
    let p = Program::parse(DUNGEON).unwrap();
    let level = world(&p, "level");
    let vault = p.fact("level", "Ash can reach Vault").unwrap();

    let Request::Find { world: w, var, pattern, .. } = &p.requests[0].request else { panic!() };
    let found = ask::find(&p, *w, *var, pattern, &limits());
    assert_eq!(found.values, ["Armory", "Crypt", "Entrance", "Hall", "Library", "Vault"]);

    let why = ask::why(&p, level, &vault, Pick::All, &limits());
    assert_eq!(why.total, 2);
    assert_eq!(why.shared.len(), 5);

    let chokepoints = ask::prevent(&p, level, &vault, &[rel(&p, "level/unlocked")], Pick::All, &limits());
    assert_eq!(
        as_sets(&chokepoints.solutions),
        sets(&p, "level", &[&["Front-Door is unlocked"], &["Library-Arch is unlocked"]]),
    );

    let shrine = p.fact("level", "Ash can reach Shrine").unwrap();
    let keys = ask::achieve(&p, level, &shrine, &[rel(&p, "level/lies-in")], Pick::All, &limits());
    assert_eq!(keys.solutions.len(), 6);
    // A key behind its own door can't open that door: cycles never support themselves.
    let inside = p.fact("level", "Silver-Key lies in Shrine").unwrap();
    assert!(!keys.solutions.iter().any(|s| s.contains(&inside)));

}

#[test]
fn prevent_and_achieve_agree_with_brute_force() {
    cross_check(HOSPITAL, 2);
    cross_check(IMPACT, 2);
    cross_check(DUNGEON, 2);
}

#[test]
fn running_out_of_budget_is_reported_not_hidden() {
    let p = Program::parse(HOSPITAL).unwrap();
    let base = world(&p, "egress");
    let target = p.fact("egress", "ICU-A has a usable egress path to North-Exit").unwrap();
    let tight = Limits { max_reasons: 1, ..Limits::default() };
    let why = ask::why(&p, base, &target, Pick::All, &tight);
    assert!(matches!(why.status, Status::Incomplete(_)));

    let starved = Limits { max_work: 10, ..Limits::default() };
    let closure = close(&p, base, &p.worlds[base].facts, &starved);
    assert!(matches!(closure.status, Status::Incomplete(_)));
    let text = muove::run(HOSPITAL, &starved).unwrap();
    assert!(text.contains("Incomplete"));
}

#[test]
fn readable_output_for_the_hospital() {
    let text = muove::run(HOSPITAL, &limits()).unwrap();
    assert!(text.contains("Yes, for 2 independent reasons."));
    assert!(text.contains("Each of these is a single point of failure."));
    assert!(text.contains("Still true, but weaker:"));
    assert!(text.contains("reasons: 2 → 1"));
    assert!(!text.contains("sha256"));
}

#[test]
fn mistakes_get_plain_explanations() {
    let error = |from: &str, to: &str| Program::parse(&HOSPITAL.replacen(from, to, 1)).unwrap_err().message;

    assert!(error("Door 102 connects East-Corridor", "Door 107 connects East-Corridor").contains("`Door 107` isn't declared"));
    assert!(error("Door 102 connects East-Corridor", "ICU-A connects East-Corridor").contains("is a Space, but `door`"));
    assert!(error("    Door 101 passed Fire-Marshal-Inspection\n\nfind", "    Door 105 passed Fire-Marshal-Inspection\n\nfind")
        .contains("can't be withdrawn"));
    assert!(error("  ?origin has a usable egress path to ?destination\n  when:\n    ?door connects ?origin to ?destination",
        "  ?origin has a usable egress path to ?nowhere\n  when:\n    ?door connects ?origin to ?destination")
        .contains("`?nowhere` appears in the conclusion"));
    assert!(error("using:\n  egress/passed", "using:\n  egress/route").contains("concluded by rules"));
}
