//! Proposed regression tests for the uploaded Muove snapshot.
//!
//! Copy to tests/evolution_regressions.rs and run:
//!     cargo test --test evolution_regressions
//!
//! IMPORTANT: These Rust tests were NOT compiled or executed in this review
//! environment, which has no Rust toolchain. Tests encode desired behavior;
//! several are expected to fail on the uploaded snapshot. The characterization
//! test deliberately records a distinction, not a failure of fact provenance.

use std::collections::BTreeSet;
use muove::{Limits, Program};
use muove::ask::{self, Proof};
use muove::engine;
use muove::program::Pick;

const BASE: &str = r#"Node

w/holds: RelationShape
  {node: Node} holds

w
  Seed ∈ Node
  Gate ∈ Node
  Goal ∈ Node
  Seed holds

w/make-gate: DerivationRule
  Gate holds
  when:
    Seed holds

w/make-goal: DerivationRule
  Goal holds
  when:
    Gate holds
"#;

const NOOP: &str = "w/unchanged: Revision\n  from: w\n";

#[test]
fn check_cannot_succeed_when_a_negative_requirement_is_unknown() {
    let source = format!("{BASE}\nw must:\n  never Goal holds\n");
    let p = Program::parse(&source).unwrap();
    let limits = Limits { max_work: 0, ..Limits::default() };
    let checks = ask::check_world(&p, p.world("w").unwrap(), &limits);
    assert!(!checks[0].1.status.is_complete());
    let (_, accepted) = muove::intents_report(&p, &limits);
    assert!(!accepted, "Unknown is not an approval");
}

#[test]
fn review_cannot_approve_an_already_invalid_baseline_as_valid() {
    let source = format!("{BASE}\nw must:\n  never Goal holds\n");
    let (text, accepted) = muove::review(&source, NOOP, &Limits::default()).unwrap();
    assert!(!accepted, "An unchanged failure is still a failure: {text}");
}

#[test]
fn review_cannot_approve_when_its_analysis_is_incomplete() {
    let source = format!("{BASE}\nw must:\n  never Goal holds\n");
    let limits = Limits { max_work: 0, ..Limits::default() };
    let (text, accepted) = muove::review(&source, NOOP, &limits).unwrap();
    assert!(!accepted, "Incomplete analysis is not an approval: {text}");
}

#[test]
fn a_proposal_must_not_rewrite_the_baseline_it_is_compared_against() {
    let source = format!("{BASE}\nw must:\n  Goal holds requires Gate holds\n");
    // This is outside a Revision block. In the uploaded implementation the
    // combined parse installs it into the base model before doing the review.
    let proposal = r#"w/outside-revision: DerivationRule
  Goal holds
  when:
    Seed holds

w/unchanged: Revision
  from: w
"#;
    match muove::review(&source, proposal, &Limits::default()) {
        Err(_) => {}, // Correct to reject the proposal's grammar/scope.
        Ok((text, accepted)) => assert!(!accepted, "Baseline contamination: {text}"),
    }
}

#[test]
fn derivation_becoming_an_assertion_is_not_a_loss_of_truth() {
    let proposal = "w/assert-goal: Revision\n  from: w\n  admit:\n    Goal holds\n";
    let p = Program::parse(&format!("{BASE}\n{proposal}")).unwrap();
    let goal = p.fact("w", "Goal holds").unwrap();
    let before = p.world("w").unwrap();
    let after = p.world("w/assert-goal").unwrap();
    let limits = Limits::default();
    assert!(engine::close(&p, before, &p.worlds[before].facts, &limits).holds(&goal));
    assert!(engine::close(&p, after, &p.worlds[after].facts, &limits).holds(&goal));
    let diff = ask::diff(&p, before, after, &limits);
    assert!(!diff.lost.iter().any(|lost| lost.fact == goal), "Still true, now also asserted");
}

#[test]
fn characterization_equal_fact_support_can_hide_changed_checkpoint_dependence() {
    let source = format!("{BASE}\nw must:\n  Goal holds requires Gate holds\n");
    let proposal = r#"w/bypass: Revision
  from: w
  add rule w/bypass-gate:
    Goal holds
    when:
      Seed holds
"#;
    let p = Program::parse(&format!("{source}\n{proposal}")).unwrap();
    let before = p.world("w").unwrap();
    let after = p.world("w/bypass").unwrap();
    let goal = p.fact("w", "Goal holds").unwrap();
    let limits = Limits::default();
    let a = ask::why(&p, before, &goal, Pick::All, &limits);
    let b = ask::why(&p, after, &goal, Pick::All, &limits);
    assert_eq!(a.reasons.len(), 1);
    assert_eq!(b.reasons.len(), 1);
    assert_eq!(a.reasons[0].facts, b.reasons[0].facts);
    let review = ask::review(&p, before, after, &limits);
    assert_eq!(review.broken, 1, "The reason-level requirement catches the bypass");
    // This currently has no Goal entry. Evolving diff to expose checkpoint
    // dependence can change this expectation; the fact-support equality above
    // is the enduring mathematical distinction.
    assert!(!review.diff.changed.iter().any(|c| c.fact == goal));
}

#[test]
fn proof_exhaustion_must_not_invent_an_asserted_goal() {
    let p = Program::parse(BASE).unwrap();
    let w = p.world("w").unwrap();
    let goal = p.fact("w", "Goal holds").unwrap();
    let seed = p.fact("w", "Seed holds").unwrap();
    let support = BTreeSet::from([seed]);
    let limits = Limits { max_work: 0, ..Limits::default() };
    let proof = ask::prove(&p, w, &support, &goal, &limits);
    if let Proof::Stated(fact) = proof {
        assert!(support.contains(&fact), "A stated proof leaf must actually be stated");
    }
}

fn shortcut_model(requirement: &str) -> String {
    let mut s = String::from("Node\n\nw/holds: RelationShape\n  {node: Node} holds\n\nw\n  Seed ∈ Node\n  Gate ∈ Node\n  Goal ∈ Node\n");
    for i in 1..=5 { s.push_str(&format!("  C{i} ∈ Node\n")); }
    s.push_str("  Seed holds\n  Gate holds\n\n");
    for i in 1..=5 {
        let prev = if i == 1 { "Seed".to_string() } else { format!("C{}", i - 1) };
        s.push_str(&format!("w/shortcut-{i}: DerivationRule\n  C{i} holds\n  when:\n    Gate holds\n\nw/chain-{i}: DerivationRule\n  C{i} holds\n  when:\n    {prev} holds\n\n"));
    }
    s.push_str("w/finish: DerivationRule\n  Goal holds\n  when:\n    C5 holds\n\nw must:\n  ");
    s.push_str(requirement);
    s.push('\n');
    s
}

#[test]
fn requires_must_propagate_its_counterfactual_budget() {
    let p = Program::parse(&shortcut_model("Goal holds requires Gate holds")).unwrap();
    let w = p.world("w").unwrap();
    let gate = p.fact("w", "Gate holds").unwrap();
    let goal = p.fact("w", "Goal holds").unwrap();
    // Base: 11 constant unary rules scan 2 + 7 + 8 facts = 187 steps.
    // Without Gate the slow chain requires more rounds, and Goal is not yet
    // committed to the closure when the same budget is exhausted.
    let tight = Limits { max_work: 187, ..Limits::default() };
    assert!(engine::close(&p, w, &p.worlds[w].facts, &tight).status.is_complete());
    let blocked = engine::close_blocking(&p, w, &p.worlds[w].facts, &|f| f == &gate, &tight);
    assert!(!blocked.status.is_complete());
    assert!(!blocked.holds(&goal));
    let full = engine::close_blocking(&p, w, &p.worlds[w].facts, &|f| f == &gate, &Limits::default());
    assert!(full.status.is_complete() && full.holds(&goal));
    let check = ask::check_world(&p, w, &tight).remove(0).1;
    assert!(!check.holds || !check.status.is_complete(), "A truncated search cannot prove necessity");
}

#[test]
fn no_single_point_must_propagate_its_counterfactual_budget() {
    let p = Program::parse(&shortcut_model("Goal holds has no single point of failure among Gate holds")).unwrap();
    let w = p.world("w").unwrap();
    let tight = Limits { max_work: 187, ..Limits::default() };
    let check = ask::check_world(&p, w, &tight).remove(0).1;
    assert!(check.holds || !check.status.is_complete(), "A truncated search cannot prove a single point of failure");
}
