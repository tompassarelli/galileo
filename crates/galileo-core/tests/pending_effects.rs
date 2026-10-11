use galileo_core::history::Span;
use galileo_core::state::{RejectionKind, Run};
use galileo_core::*;

#[test]
fn spec_5_4_d55_receipts_match_shared_slots_by_name() {
    for reversed in [false, true] {
        let mut p = Program::builder();
        p.entity_type("File");
        for file in ["source", "destination", "other"] { p.entity(Entity::named("File", file)); }
        let slots = [Slot::new("src", Type::entity("File")), Slot::new("dst", Type::entity("File")), Slot::new("label", Type::Text)];
        let copy = p.relation(Relation::new("copy", RelationKind::Effect, slots.clone()));
        let request = p.relation(Relation::new("request", RelationKind::Event, slots.clone()));
        let mut receipt_slots = slots.to_vec();
        if reversed { receipt_slots.swap(0, 1); }
        receipt_slots.push(Slot::new("other", Type::entity("File")));
        let copied = p.relation(Relation::new("copied", RelationKind::Event, receipt_slots));
        p.event(Event::new(request));
        p.event(Event::new(copied));
        p.effect(Effect::new(copy).settled_by(copied));
        let args = [Expr::var("src"), Expr::var("dst"), Expr::var("label")];
        p.handler(Handler::new(Atom::new(request, args.clone())).clause(HandlerClause::new().action(Action::Emit(Atom::new(copy, args)))));
        let mut receipt_args = vec![Expr::var("src"), Expr::var("dst"), Expr::var("label")];
        if reversed { receipt_args.swap(0, 1); }
        receipt_args.push(Expr::var("other"));
        p.handler(Handler::new(Atom::new(copied, receipt_args)).clause(HandlerClause::new()));
        let p = p.build();
        check(&p).unwrap();
        let values = vec![Value::entity("File", "source"), Value::entity("File", "destination"), Value::text("copy")];
        let pending = (copy, values.clone());
        let mut run = Run::new(&p);
        assert_eq!(run.run(&p, request, &values, Budget::default()).unwrap().result, Ok(vec![pending.clone()]));
        let mut receipt = values.clone();
        if reversed { receipt.swap(0, 1); }
        receipt.push(Value::entity("File", "other"));
        let mut swapped = receipt.clone();
        swapped.swap(0, 1);
        let mut different_label = receipt.clone();
        different_label[2] = Value::text("different");
        for mismatch in [swapped, different_label] {
            assert_eq!(run.run(&p, copied, &mismatch, Budget::default()).unwrap().result, Ok(vec![]));
            assert_eq!(run.pending(), &[pending.clone()]);
        }
        assert_eq!(run.run(&p, copied, &receipt, Budget::default()).unwrap().result, Ok(vec![]));
        assert!(run.pending().is_empty());
        assert_eq!(Run::replay(&p, &run.log, Budget::default()).unwrap(), run);
    }
}

#[test]
fn spec_5_4_and_d52_pending_receipts_survive_restore_and_keep_occurrence_spans() {
    let mut p = Program::builder();
    p.entity_type("Turn");
    for turn in ["first", "other"] { p.entity(Entity::named("Turn", turn)); }
    p.entity_type("TurnStatus");
    for status in ["ok", "failed"] { p.entity(Entity::named("TurnStatus", status)); }
    let send = p.relation(Relation::new("send", RelationKind::Effect, [Slot::new("prompt", Type::Text), Slot::new("turn", Type::entity("Turn"))]));
    let request = p.relation(Relation::new("request", RelationKind::Event, [Slot::new("turn", Type::entity("Turn")), Slot::new("prompt", Type::Text)]));
    let finished = p.relation(Relation::new("finished", RelationKind::Event, [Slot::new("turn", Type::entity("Turn")), Slot::new("status", Type::entity("TurnStatus"))]));
    let burst = p.relation(Relation::new("burst", RelationKind::Event, [Slot::new("turn", Type::entity("Turn"))]));
    for event in [request, finished, burst] { p.event(Event::new(event)); }
    p.effect(Effect::new(send).at_most_one_per_step().settled_by(finished));
    p.handler(Handler::new(Atom::new(request, [Expr::var("turn"), Expr::var("prompt")])).clause(HandlerClause::new().action(Action::Emit(Atom::new(send, [Expr::var("prompt"), Expr::var("turn")])))));
    p.handler(Handler::new(Atom::new(finished, [Expr::var("turn"), Expr::value(Value::entity("TurnStatus", "ok"))])).clause(HandlerClause::new()));
    p.handler(Handler::new(Atom::new(burst, [Expr::var("turn")])).clause(HandlerClause::new()
        .action(Action::Emit(Atom::new(send, [Expr::value("a"), Expr::var("turn")])))
        .action(Action::Emit(Atom::new(send, [Expr::value("b"), Expr::var("turn")])))));
    let p = p.build();
    check(&p).unwrap();
    let turn = Value::entity("Turn", "first");
    let oldest = (send, vec![Value::text("z-oldest"), turn.clone()]);
    let newest = (send, vec![Value::text("a-newest"), turn.clone()]);
    let receipt = (finished, vec![turn.clone(), Value::entity("TurnStatus", "ok")]);
    let mut run = Run::new(&p);
    let initial_hash = run.state.hash(&p);
    for effect in [&oldest, &newest] {
        let record = run.run(&p, request, &[turn.clone(), effect.1[0].clone()], Budget::default()).unwrap();
        assert_eq!(record.result, Ok(vec![effect.clone()]));
    }
    assert_eq!(run.pending(), &[oldest.clone(), newest.clone()]);
    assert_ne!(run.state.hash(&p), initial_hash);
    let before_settlement = run.clone();
    assert_eq!(run.run(&p, receipt.0, &receipt.1, Budget::default()).unwrap().result, Ok(vec![]));
    let mut restored = Run::restore(run.checkpoint());
    assert_eq!(restored, run);
    assert_eq!(restored.pending(), &[newest.clone()]);
    assert_eq!(restored.history.as_of(2).unwrap().pending, vec![oldest.clone(), newest.clone()]);
    assert_eq!(restored.history.pending, vec![
        (oldest.clone(), Span { from: 1, until: Some(3) }),
        (newest.clone(), Span { from: 2, until: None }),
    ]);
    let admitted = restored.run(&p, finished, &[Value::entity("Turn", "other"), Value::entity("TurnStatus", "ok")], Budget::default()).unwrap();
    assert_eq!(admitted.result, Ok(vec![]));
    assert_eq!(restored.pending(), &[newest.clone()]);
    let history = restored.history.clone();
    let hash = restored.state.hash(&p);
    for (event, values, kind) in [
        (finished, vec![turn.clone(), Value::entity("TurnStatus", "failed")], RejectionKind::NoMatch),
        (burst, vec![turn.clone()], RejectionKind::CompetingEffects),
    ] {
        assert_eq!(restored.run(&p, event, &values, Budget::default()).unwrap().result.as_ref().unwrap_err().kind, kind);
        assert_eq!(restored.history, history);
        assert_eq!(restored.state.hash(&p), hash);
        assert_eq!(restored.pending(), &[newest.clone()]);
    }
    assert!(restored.run_all(&p, &[receipt.clone(), (burst, vec![turn.clone()])], Budget::default()).unwrap().result.is_err());
    assert_eq!(restored.history, history);
    assert_eq!(restored.state.hash(&p), hash);
    assert_eq!(Run::replay(&p, &restored.log, Budget::default()).unwrap(), restored);
    for record in &restored.log {
        assert_eq!(restored.history.as_of(record.revision).unwrap().hash(&p), record.hash);
    }
    let mut repeated = Run::restore(before_settlement.checkpoint());
    assert_eq!(repeated.run(&p, request, &[turn.clone(), oldest.1[0].clone()], Budget::default()).unwrap().result, Ok(vec![oldest.clone()]));
    assert_eq!(repeated.pending(), &[oldest.clone(), newest.clone(), oldest.clone()]);
    assert_eq!(repeated.history.as_of(3).unwrap().pending, vec![oldest.clone(), newest.clone(), oldest.clone()]);
    assert_eq!(repeated.run(&p, receipt.0, &receipt.1, Budget::default()).unwrap().result, Ok(vec![]));
    assert_eq!(repeated.pending(), &[newest.clone(), oldest.clone()]);
    assert_eq!(repeated.history.pending, vec![
        (oldest.clone(), Span { from: 1, until: Some(4) }),
        (newest.clone(), Span { from: 2, until: None }),
        (oldest.clone(), Span { from: 3, until: None }),
    ]);
    let mut duplicate = Run::restore(before_settlement.checkpoint());
    assert_eq!(duplicate.run_all(&p, &[
        (request, vec![turn.clone(), oldest.1[0].clone()]), receipt.clone(),
    ], Budget::default()).unwrap().result, Ok(vec![oldest.clone()]));
    assert_eq!(duplicate.pending(), &[newest.clone(), oldest.clone()]);
    assert_eq!(duplicate.history.pending, vec![
        (oldest.clone(), Span { from: 1, until: Some(3) }),
        (newest.clone(), Span { from: 2, until: None }),
        (oldest.clone(), Span { from: 3, until: None }),
    ]);
    assert_eq!(duplicate.history.as_of(3), Some(duplicate.state.clone()));
    let mut atomic = Run::new(&p);
    assert_eq!(atomic.run_all(&p, &[
        (request, vec![turn.clone(), oldest.1[0].clone()]), receipt,
    ], Budget::default()).unwrap().result, Ok(vec![oldest]));
    assert!(atomic.pending().is_empty());
    assert!(atomic.history.pending.is_empty());
    assert!(atomic.history.changes[0].requested.is_empty());
    assert!(atomic.history.changes[0].settled.is_empty());
    assert_eq!(atomic.state.hash(&p), initial_hash);
    let mut batched = Run::new(&p);
    let request_event = (request, vec![turn.clone(), newest.1[0].clone()]);
    assert_eq!(batched.run_all(&p, &[request_event.clone(), request_event], Budget::default()).unwrap().result, Ok(vec![newest.clone()]));
    assert_eq!(batched.pending(), &[newest.clone()]);
    assert_eq!(batched.history.pending, vec![(newest, Span { from: 1, until: None })]);
}
