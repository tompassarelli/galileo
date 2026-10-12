use crate::check::{compare_symbol, render_atom, render_value};
use crate::engine::{derive, Derivation, Origin, Premise};
use crate::expr::Meter;
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Proof {
    Stated {
        fact: Fact,
    },
    Derived {
        relation: RelationId,
        values: Vec<Value>,
        rule: usize,
        premises: Vec<Proof>,
    },
    Absent {
        relation: RelationId,
        values: Vec<Option<Value>>,
        scope: Vec<String>,
    },
    Computed(Premise),
}

pub use crate::supports::Support;

pub fn why(
    program: &Program,
    state: &[Fact],
    relation: RelationId,
    values: &[Value],
    budget: Budget,
) -> Result<Verdict<Proof, ()>, EvalError> {
    let derivation = match derive(program, state, budget) {
        Err(EvalError::Unknown(reason)) => return Ok(Verdict::Unknown(reason)),
        other => other?,
    };
    let mut meter = Meter {
        limit: budget.work,
        spent: derivation.work,
    };
    let Some(origin) = derivation.origin(relation, values) else {
        return Ok(Verdict::Fails(()));
    };
    match prove(&derivation, relation, values, origin, &mut meter) {
        Err(EvalError::Unknown(reason)) => Ok(Verdict::Unknown(reason)),
        other => Ok(Verdict::Holds(other?)),
    }
}

fn prove(
    derivation: &Derivation,
    relation: RelationId,
    values: &[Value],
    origin: Origin,
    meter: &mut Meter,
) -> Result<Proof, EvalError> {
    meter.charge("reconstructing a proof")?;
    let (rule, height) = match origin {
        Origin::Stated(index) => {
            let fact = derivation.stated(index).expect("a recorded stated fact exists");
            return Ok(Proof::Stated { fact: fact.clone() });
        }
        Origin::Derived { rule, height } => (rule, height),
    };
    let firing = derivation
        .firings(relation, values, Some((rule, height)), meter)?
        .into_iter()
        .next()
        .ok_or_else(|| Reason::Unsupported {
            construct: "a proof the engine did not record".into(),
            at: derivation.program().rules[rule].name.clone(),
        })?;
    let mut premises = Vec::new();
    for premise in firing.premises {
        premises.push(match premise {
            Premise::Present(relation, row) => {
                let origin = derivation
                    .origin(relation, &row)
                    .expect("a matched premise is in the model");
                prove(derivation, relation, &row, origin, meter)?
            }
            Premise::Absent(relation, row) => Proof::Absent {
                relation,
                scope: scope(derivation, relation, row.first().and_then(Option::as_ref), meter)?,
                values: row,
            },
            other => Proof::Computed(other),
        });
    }
    Ok(Proof::Derived {
        relation,
        values: values.to_vec(),
        rule,
        premises,
    })
}

pub(crate) fn scope(
    derivation: &Derivation,
    relation: RelationId,
    subject: Option<&Value>,
    meter: &mut Meter,
) -> Result<Vec<String>, EvalError> {
    let program = derivation.program();
    match program.relations[relation.0].kind {
        RelationKind::State => Ok(vec!["current state".into()]),
        RelationKind::Derived => program
            .rules
            .iter()
            .filter(|rule| rule.head.relation == relation)
            .map(|rule| {
                meter.charge("checking absence scope")?;
                Ok(format!("{} ({})", rule.name, located(&rule.source)))
            })
            .collect(),
        _ => {
            let mut files = BTreeSet::new();
            for fact in derivation.stated_for_relation(relation) {
                meter.charge("checking absence scope")?;
                if subject.is_none() || fact.values.first() != subject {
                    continue;
                }
                match &fact.origin {
                    FactOrigin::Stated(Some(span)) => {
                        files.insert(span.file.clone());
                    }
                    FactOrigin::Foreign { source, .. } => {
                        files.insert(source.clone());
                    }
                    FactOrigin::Stated(None) => {}
                }
            }
            Ok(files.into_iter().collect())
        }
    }
}

pub fn why_all(
    program: &Program,
    state: &[Fact],
    relation: RelationId,
    values: &[Value],
    budget: Budget,
) -> Result<Verdict<Vec<Support>, ()>, EvalError> {
    crate::supports::minimal_supports(program, state, relation, values, budget)
}

pub fn sentence(program: &Program, relation: RelationId, values: &[Value]) -> String {
    let atom = Atom::new(relation, values.iter().cloned().map(Expr::Value));
    render_atom(program, &atom)
}

pub(crate) fn located(span: &Option<SourceSpan>) -> String {
    match span {
        Some(span) => format!("{}:{}", span.file, span.line),
        None => "stated without a source".into(),
    }
}

pub(crate) fn fact_line(program: &Program, fact: &Fact) -> String {
    let text = sentence(program, fact.relation, &fact.values);
    match &fact.origin {
        FactOrigin::Stated(span) => format!("{text}    {}", located(span)),
        FactOrigin::Foreign {
            source, revision, ..
        } => format!("{text}    from {source}, extracted at {revision}"),
    }
}

fn line(program: &Program, proof: &Proof) -> String {
    match proof {
        Proof::Stated { fact } => fact_line(program, fact),
        Proof::Derived {
            relation,
            values,
            rule,
            ..
        } => {
            let rule = &program.rules[*rule];
            let at = rule
                .source
                .as_ref()
                .map(|span| format!(" ({}:{})", span.file, span.line))
                .unwrap_or_default();
            format!("{}    ← {}{at}", sentence(program, *relation, values), rule.name)
        }
        Proof::Absent {
            relation,
            values,
            scope,
        } => {
            let checked = if scope.is_empty() && program.relations[relation.0].kind == RelationKind::Derived {
                "no rule concludes it".to_string()
            } else if scope.is_empty() {
                "every stated fact of the program".to_string()
            } else {
                scope.join(", ")
            };
            format!(
                "nothing {}: {}    (checked: {checked})",
                if program.relations[relation.0].kind == RelationKind::Derived { "derives" } else { "states" },
                render_atom(
                    program,
                    &Atom::new(
                        *relation,
                        values.iter().map(|value| value.clone().map_or_else(|| Expr::var("_#"), Expr::Value))
                    )
                )
            )
        }
        Proof::Computed(premise) => match premise {
            Premise::Compare(op, left, right) => format!(
                "computed: {} {} {}",
                render_value(left),
                compare_symbol(*op),
                render_value(right)
            ),
            Premise::Bound(variable, value) => {
                format!("computed: ?{} = {}", variable.0, render_value(value))
            }
            Premise::Member(variable, value) => {
                format!("computed: ?{} = {} is in the list", variable.0, render_value(value))
            }
            Premise::Condition(_) => "computed: the condition is true".into(),
            Premise::Present(..) | Premise::Absent(..) => unreachable!("facts are not computed"),
        },
    }
}

fn tree(program: &Program, proof: &Proof, prefix: &str, out: &mut Vec<String>) {
    () /* ~ changed by cargo-mutants ~ */ // MUTANT:9e796a2f7-1632
}

pub fn render(
    program: &Program,
    relation: RelationId,
    values: &[Value],
    verdict: &Verdict<Proof, ()>,
    budget: Budget,
) -> String {
    let asked = sentence(program, relation, values);
    match verdict {
        Verdict::Holds(proof) => {
            let mut out = vec![line(program, proof)];
            tree(program, proof, "", &mut out);
            out.join("\n")
        }
        Verdict::Fails(()) => format!("{asked} does not hold. Ask `why not` for its near misses."),
        Verdict::Unknown(reason) => unknown(&asked, reason, budget),
    }
}

pub fn render_all(
    program: &Program,
    relation: RelationId,
    values: &[Value],
    verdict: &Verdict<Vec<Support>, ()>,
    budget: Budget,
) -> String {
    let asked = sentence(program, relation, values);
    match verdict {
        Verdict::Holds(sets) => {
            let mut out = vec![format!(
                "{asked} has {} minimal supports, all of them within a work budget of {} units:",
                sets.len(),
                budget.work
            )];
            for (index, set) in sets.iter().enumerate() {
                out.push(format!("{}. {}", index + 1, set.iter().map(|fact| fact_line(program, fact)).collect::<Vec<_>>().join("; ")));
            }
            out.join("\n")
        }
        Verdict::Fails(()) => format!("{asked} does not hold, so it has no supports."),
        Verdict::Unknown(reason) => unknown(&asked, reason, budget),
    }
}

fn unknown(asked: &str, reason: &Reason, budget: Budget) -> String {
    format!(
        "Why {asked} is unknown: {reason}, within a work budget of {} units over the whole program. Unknown is not an approval.",
        budget.work
    )
}

pub fn why_history(
    program: &Program,
    run: &crate::state::Run,
    sources: &[SourceSpan],
    revision: u64,
    relation: RelationId,
    values: &[Value],
    budget: Budget,
) -> Result<Verdict<String, ()>, EvalError> {
    let mut meter = Meter { limit: budget.work, spent: 0 };
    history_explanation(program, run, sources, revision, Some((relation, values)), BTreeMap::new(), &mut meter)
}

pub(crate) fn history_explanation(
    program: &Program,
    run: &crate::state::Run,
    sources: &[SourceSpan],
    revision: u64,
    asked: Option<(RelationId, &[Value])>,
    mut pending: BTreeMap<u64, BTreeSet<crate::state::Tuple>>,
    meter: &mut Meter,
) -> Result<Verdict<String, ()>, EvalError> {
    let mut out = Vec::new();
    let mut at = revision;
    let mut requested = if asked.is_none() { pending.pop_last() } else { None };
    if let Some((from, _)) = &requested { at = from - 1; }
    if asked.is_none() && requested.is_none() { return Ok(Verdict::Holds(String::new())); }
    loop {
        let state = run.history.as_of(at).ok_or_else(|| EvalError::Type { at: "chain of title".into(), message: format!("revision {at} is past the last revision {}", run.revision) })?;
        let mut current = program.clone();
        current.entities.retain(|entity| !program.entity_types.iter().any(|t| t.state && t.name == entity.entity_type));
        current.entities.extend(state.live.iter().cloned());
        let facts: Vec<_> = state.facts.iter().map(|(relation, values)| {
            program.initially.iter().find(|fact| fact.relation == *relation && fact.values == *values).cloned().unwrap_or_else(|| Fact::new(*relation, values.clone()))
        }).collect();
        let result = (|| {
            let causes = match requested.take() {
                Some((from, reached)) => {
                    let index = run.log.iter().position(|record| record.revision == from && record.result.is_ok()).expect("an opened span has an admitted occurrence");
                    let record = &run.log[index];
                    let [(event, arguments)] = record.events.as_slice() else {
                        return Err(EvalError::Type { at: "chain of title".into(), message: "expected one event occurrence".into() });
                    };
                    let mut causes = crate::state::step_causes(program, &state, *event, arguments, meter)?;
                    let mut selected = Vec::new();
                    for fact in reached {
                        let cause = causes.remove(&fact).expect("a changed fact has its clause and substitution");
                        if !selected.contains(&cause) { selected.push(cause); }
                    }
                    selected.sort_by_cached_key(|(clause, binding)| (crate::state::clause_text(program, clause), binding.clone()));
                    let source = &sources[index];
                    out.push(String::new());
                    let clause = if selected.len() == 1 { format!(", by the clause at {}", located(&selected[0].0.source)) } else { String::new() };
                    out.push(format!("revision {from}: {} ({}:{}){clause}, when at revision {at}:", sentence(program, *event, arguments), source.file, source.line));
                    Some(selected)
                }
                None => None,
            };
            let derivation = derive(&current, &facts, Budget::work(meter.limit.saturating_sub(meter.spent)))?;
            meter.spent += derivation.work;
            if let Some(causes) = causes {
                let model = derive(&current, &facts, Budget::work(meter.limit.saturating_sub(meter.spent)))?;
                meter.spent += model.work;
                let model = model.model();
                let grouped = causes.len() > 1;
                for (clause, binding) in causes {
                    if grouped { out.push(format!("  by the clause at {}:", located(&clause.source))); }
                    for literal in &clause.body {
                        meter.charge("explaining a when condition")?;
                        let proof = match literal {
                            Literal::Positive(atom) => {
                                let values = atom.arguments.iter().map(|argument| crate::eval::value(&current, &model, argument, &binding, Budget::work(meter.limit.saturating_sub(meter.spent)))).collect::<Result<Vec<_>, _>>()?;
                                let origin = derivation.origin(atom.relation, &values).expect("a matching condition holds in its stored pre-state");
                                prove(&derivation, atom.relation, &values, origin, meter)?
                            }
                            Literal::Negative(atom) => {
                                let values = atom.arguments.iter().map(|argument| if matches!(argument, Expr::Variable(variable) if variable.is_anonymous()) { Ok(None) } else { crate::eval::value(&current, &model, argument, &binding, Budget::work(meter.limit.saturating_sub(meter.spent))).map(Some) }).collect::<Result<Vec<_>, EvalError>>()?;
                                Proof::Absent { relation: atom.relation, scope: scope(&derivation, atom.relation, values.first().and_then(Option::as_ref), meter)?, values }
                            }
                            Literal::Compare { op, left, right } => Proof::Computed(Premise::Compare(*op, crate::eval::value(&current, &model, left, &binding, Budget::work(meter.limit.saturating_sub(meter.spent)))?, crate::eval::value(&current, &model, right, &binding, Budget::work(meter.limit.saturating_sub(meter.spent)))?)),
                            Literal::Boolean(expression) => Proof::Computed(Premise::Condition(crate::eval::value(&current, &model, expression, &binding, Budget::work(meter.limit.saturating_sub(meter.spent)))?)),
                            Literal::Bind { variable, .. } | Literal::Aggregate { variable, .. } | Literal::ArgMin { variable, .. } => Proof::Computed(Premise::Bound(variable.clone(), binding[variable].clone())),
                            Literal::Member { variable, .. } => Proof::Computed(Premise::Member(variable.clone(), binding[variable].clone())),
                        };
                        history_tree(program, &run.history, at, &proof, if grouped { "    " } else { "  " }, &mut pending, &mut out);
                    }
                }
            } else {
                let (relation, values) = asked.expect("the first proof has an asked sentence");
                let Some(origin) = derivation.origin(relation, values) else { return Ok(false); };
                let proof = prove(&derivation, relation, values, origin, meter)?;
                history_tree(program, &run.history, at, &proof, "", &mut pending, &mut out);
            }
            Ok(true)
        })();
        match result {
            Ok(false) => return Ok(Verdict::Fails(())),
            Ok(true) => {}
            Err(EvalError::Unknown(reason)) if out.is_empty() => return Ok(Verdict::Unknown(reason)),
            Err(EvalError::Unknown(Reason::OutOfBudget { .. })) => {
                out.push("earlier causes not shown: out of budget".into());
                break;
            }
            Err(error) => return Err(error),
        }
        let Some((from, reached)) = pending.pop_last() else { break; };
        at = from - 1;
        requested = Some((from, reached));
    }
    Ok(Verdict::Holds(out.join("\n")))
}

fn history_tree(
    program: &Program,
    history: &crate::history::History,
    revision: u64,
    proof: &Proof,
    prefix: &str,
    pending: &mut BTreeMap<u64, BTreeSet<crate::state::Tuple>>,
    out: &mut Vec<String>,
) {
    let text = match proof {
        Proof::Stated { fact } if program.relations[fact.relation.0].kind == RelationKind::State => {
            let tuple = (fact.relation, fact.values.clone());
            let span = history.facts[&tuple].iter().find(|span| span.contains(revision)).expect("a state leaf has its holding span");
            let mut text = format!("{}, since revision {}", sentence(program, fact.relation, &fact.values), span.from);
            if span.from == 0 {
                text.push_str(&format!(" ({})", located(&program.initially.iter().find(|initial| initial.relation == fact.relation && initial.values == fact.values).and_then(|initial| match &initial.origin { FactOrigin::Stated(source) => source.clone(), _ => None }))));
            } else {
                pending.entry(span.from).or_default().insert(tuple);
            }
            text
        }
        _ => line(program, proof),
    };
    out.push(format!("{prefix}{text}"));
    if let Proof::Derived { premises, .. } = proof {
        for premise in premises { history_tree(program, history, revision, premise, &format!("{prefix}  "), pending, out); }
    }
}
