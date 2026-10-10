use crate::check::{compare_symbol, render_atom, render_value};
use crate::engine::{derive, Derivation, Origin, Premise};
use crate::expr::Meter;
use crate::*;
use std::collections::BTreeSet;

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
        values: Vec<Value>,
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
                scope: scope(derivation.program(), derivation.stated_facts(), relation),
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

pub(crate) fn scope<'p>(
    program: &Program,
    stated: impl Iterator<Item = &'p Fact>,
    relation: RelationId,
) -> Vec<String> {
    let facts = stated
        .filter(|fact| fact.relation == relation)
        .filter_map(|fact| match &fact.origin {
            FactOrigin::Stated(source) => source.as_ref().map(|span| span.file.clone()),
            FactOrigin::Foreign { source, .. } => Some(source.clone()),
        });
    let rules = program
        .rules
        .iter()
        .filter(|rule| rule.head.relation == relation)
        .filter_map(|rule| rule.source.as_ref().map(|span| span.file.clone()));
    facts.chain(rules).collect::<BTreeSet<_>>().into_iter().collect()
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
            let checked = if scope.is_empty() {
                "every stated fact of the program".to_string()
            } else {
                scope.join(", ")
            };
            format!(
                "nothing states: {}    (checked: {checked})",
                sentence(program, *relation, values)
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
    let Proof::Derived { premises, .. } = proof else {
        return;
    };
    for (index, premise) in premises.iter().enumerate() {
        let last = index + 1 == premises.len();
        let (branch, carry) = if last { ("└─ ", "   ") } else { ("├─ ", "│  ") };
        out.push(format!("{prefix}{branch}{}", line(program, premise)));
        tree(program, premise, &format!("{prefix}{carry}"), out);
    }
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
