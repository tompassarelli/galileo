use muove_core::why::{render, why, why_all, Proof};
use muove_core::*;

fn leaves<'a>(proof: &'a Proof, out: &mut Vec<&'a Proof>) {
    match proof {
        Proof::Derived { premises, .. } => premises.iter().for_each(|p| leaves(p, out)),
        leaf => out.push(leaf),
    }
}

#[test]
fn r10_reconstruction_never_invents_a_stated_leaf() {
    let program = examples::gate();
    let holds = program.relation_id("w/holds").unwrap();
    let goal = [Value::entity("Node", "Goal")];
    let mut outcomes = (0, 0);
    for work in 0..=60 {
        match why(&program, &[], holds, &goal, Budget::work(work)).unwrap() {
            Verdict::Holds(proof) => {
                outcomes.0 += 1;
                let mut found = Vec::new();
                leaves(&proof, &mut found);
                for leaf in found {
                    let Proof::Stated { fact } = leaf else { continue };
                    assert!(program.facts.contains(fact), "invented leaf at budget {work}");
                }
                assert!(render(&program, holds, &goal, &Verdict::Holds(proof), Budget::work(work))
                    .contains("└─ Gate holds    ← w/make-gate\n   └─ Seed holds    stated without a source"));
            }
            Verdict::Unknown(_) => outcomes.1 += 1,
            Verdict::Fails(()) => panic!("Goal holds"),
        }
    }
    assert!(outcomes.0 > 0 && outcomes.1 > 0, "{outcomes:?}");
}

#[test]
fn a_not_proof_shows_its_absence_leaf_and_scope() {
    let mut program = examples::tags();
    for (line, fact) in program.facts.iter_mut().enumerate() {
        let host = fact.values.iter().any(|v| *v == Value::entity("Host", "whiterabbit"));
        let file = if host { "hosts/whiterabbit/tags.muove" } else { "modules/default.muove" };
        *fact = fact.clone().at(SourceSpan {
            file: file.into(),
            start: 0,
            end: 0,
            line: line + 1,
            column: 1,
        });
    }
    let enabled = program.relation_id("firn/enabled").unwrap();
    let asked = [Value::entity("Module", "firefox"), Value::entity("Host", "whiterabbit")];
    let verdict = why(&program, &[], enabled, &asked, Budget::default()).unwrap();
    let text = render(&program, enabled, &asked, &verdict, Budget::default());
    println!("{text}");
    assert!(text.starts_with("firefox is enabled on whiterabbit    ← enabled\n"));
    assert!(text.contains("nothing states: whiterabbit drops firefox from browsers    (checked: hosts/whiterabbit/tags.muove)"));
    assert!(text.contains("└─ nothing states: whiterabbit disables firefox    (checked: hosts/whiterabbit/tags.muove)"));
    assert!(text.contains("whiterabbit enables browsers    hosts/whiterabbit/tags.muove:"));
}

#[test]
fn r13_why_gives_one_proof_and_why_all_runs_out_of_work() {
    let program = examples::r13();
    let goal = program.relation_id("r13/goal").unwrap();
    let Verdict::Holds(proof) = why(&program, &[], goal, &[], Budget::default()).unwrap() else {
        panic!("the goal holds");
    };
    let mut found = Vec::new();
    leaves(&proof, &mut found);
    assert_eq!(found.len(), 20);
    let all = why_all(&program, &[], goal, &[], Budget::default()).unwrap();
    let Verdict::Unknown(Reason::OutOfBudget { resource: Resource::Work, .. }) = &all else {
        panic!("expected Unknown(work), got {all:?}");
    };
    println!("{}", why::render_all(&program, goal, &[], &all, Budget::default()));
}
