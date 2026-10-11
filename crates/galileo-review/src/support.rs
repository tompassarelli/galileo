use galileo_core::engine::evaluate;
use galileo_core::supports::{failure_sets, strength_sentence, Strength};
use galileo_core::why::sentence;
use galileo_core::{
    evaluate_blocking, Atom, Budget, Claim, EvalError, Expr, Fact, Literal, Model, Program, Reason,
    Relation, RelationId, RelationKind, Rule, StateClaim, Value,
};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupportView {
    pub subject: String,
    pub before: Option<Strength>,
    pub after: Option<Strength>,
    pub before_sentence: Option<String>,
    pub after_sentence: Option<String>,
}

impl fmt::Display for SupportView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:", self.subject)?;
        if let Some(sentence) = &self.before_sentence {
            write!(f, " Before: {sentence}")?;
        }
        if let Some(sentence) = &self.after_sentence {
            write!(f, " After: {sentence}")?;
        }
        if let (Some(before), Some(after)) = (
            self.before.as_ref().and_then(Strength::size),
            self.after.as_ref().and_then(Strength::size),
        ) {
            let direction = if after < before { "fell" } else { "rose" };
            if before != after {
                let noun = if before == 1 { "fact" } else { "facts" };
                write!(
                    f,
                    " The smallest failure set {direction} from {before} {noun} to {after}."
                )?;
            }
        }
        Ok(())
    }
}

pub fn support(
    before: &Program,
    after: &Program,
    pattern: &[String],
    budget: Budget,
) -> Result<Vec<SupportView>, EvalError> {
    let before_subjects = subjects(before, budget)?;
    let after_subjects = subjects(after, budget)?;
    let mut views = BTreeMap::new();
    for (program, subjects, is_before) in [
        (before, before_subjects, true),
        (after, after_subjects, false),
    ] {
        let pattern: Vec<_> = pattern
            .iter()
            .filter_map(|name| program.relation_id(name))
            .collect();
        for (key, fact) in subjects {
            let strength = failure_sets(
                program,
                &program.initially,
                fact.relation,
                &fact.values,
                &pattern,
                budget,
            )?;
            let text = strength_sentence(program, fact.relation, &fact.values, &pattern, &strength);
            let view = views.entry(key).or_insert_with(|| SupportView {
                subject: sentence(program, fact.relation, &fact.values),
                before: None,
                after: None,
                before_sentence: None,
                after_sentence: None,
            });
            if is_before {
                view.before = Some(strength);
                view.before_sentence = Some(text);
            } else {
                view.after = Some(strength);
                view.after_sentence = Some(text);
            }
        }
    }
    Ok(views.into_values().collect())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MilestoneChange {
    Bypassed {
        fact: String,
        milestone: String,
    },
    Unknown {
        fact: String,
        milestone: String,
        reason: Reason,
    },
}

impl fmt::Display for MilestoneChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bypassed { fact, milestone } => {
                write!(f, "{fact} now bypasses milestone {milestone}.")
            }
            Self::Unknown {
                fact,
                milestone,
                reason,
            } => write!(
                f,
                "Whether {fact} newly bypasses milestone {milestone} is unknown: {reason}."
            ),
        }
    }
}

pub fn milestones(
    before: &Program,
    after: &Program,
    budget: Budget,
) -> Result<Vec<MilestoneChange>, EvalError> {
    let before_model = evaluate(before, &before.initially, budget)?;
    let after_model = evaluate(after, &after.initially, budget)?;
    let before_facts = named_facts(before, &before_model);
    let after_facts = named_facts(after, &after_model);
    let mut changes = Vec::new();
    for (milestone_key, milestone) in &before_facts {
        let relation = &before.relations[milestone.relation.0];
        if !relation.checkpoint {
            continue;
        }
        let Some(after_relation) = after.relation_id(&relation.name) else {
            continue;
        };
        let after_milestone = Fact::new(after_relation, milestone.values.clone());
        for (key, fact) in &before_facts {
            let Some(after_fact) = after_facts.get(key) else {
                continue;
            };
            if key == milestone_key
                || before.relations[fact.relation.0].kind != RelationKind::Derived
                || after.relations[after_fact.relation.0].kind != RelationKind::Derived
            {
                continue;
            }
            let old = dependent(before, &before_model, fact, milestone, budget);
            let new = dependent(after, &after_model, after_fact, &after_milestone, budget);
            let fact_text = sentence(after, after_fact.relation, &after_fact.values);
            let milestone_text = sentence(before, milestone.relation, &milestone.values);
            match (old, new) {
                (Ok(true), Ok(false)) => changes.push(MilestoneChange::Bypassed {
                    fact: fact_text,
                    milestone: milestone_text,
                }),
                (Err(EvalError::Unknown(reason)), _) | (_, Err(EvalError::Unknown(reason))) => {
                    changes.push(MilestoneChange::Unknown {
                        fact: fact_text,
                        milestone: milestone_text,
                        reason,
                    });
                }
                (Err(error), _) | (_, Err(error)) => return Err(error),
                _ => {}
            }
        }
    }
    Ok(changes)
}

fn dependent(
    program: &Program,
    model: &Model,
    target: &Fact,
    milestone: &Fact,
    budget: Budget,
) -> Result<bool, EvalError> {
    let changed = evaluate_blocking(
        program,
        &program.initially,
        Budget::work(budget.work.saturating_sub(model.work)),
        &|relation, values| relation == milestone.relation && values == milestone.values,
    )?;
    Ok(!changed.contains(target.relation, &target.values))
}

type FactKey = (String, Vec<Value>);

fn named_facts(program: &Program, model: &Model) -> BTreeMap<FactKey, Fact> {
    let mut facts = BTreeMap::new();
    for (id, values) in &model.relations {
        for values in values.iter() {
            facts.insert(
                (program.relations[id.0].name.clone(), values.clone()),
                Fact::new(*id, values.clone()),
            );
        }
    }
    facts
}

fn subjects(program: &Program, budget: Budget) -> Result<BTreeMap<FactKey, Fact>, EvalError> {
    let mut projected = program.clone();
    let mut projections = Vec::new();
    let mut subjects = BTreeMap::new();
    for requirement in &program.requirements {
        for (atom, body) in subject_atoms(&requirement.claim) {
            if let Some(values) = atom
                .arguments
                .iter()
                .map(|e| match e {
                    Expr::Value(value) => Some(value.clone()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
            {
                subjects.insert(
                    (
                        program.relations[atom.relation.0].name.clone(),
                        values.clone(),
                    ),
                    Fact::new(atom.relation, values),
                );
            } else {
                let id = RelationId(projected.relations.len());
                let mut name = format!("review-subject-{}", id.0);
                while projected.relation_id(&name).is_some() {
                    name.push('_');
                }
                projected.relations.push(Relation::new(
                    name.clone(),
                    RelationKind::Derived,
                    program.relations[atom.relation.0].slots.clone(),
                ));
                projected
                    .rules
                    .push(Rule::new(name, Atom::new(id, atom.arguments.clone())).when(body));
                projections.push((id, atom.relation));
            }
        }
    }
    let model = evaluate(&projected, &projected.initially, budget)?;
    for (projection, relation) in projections {
        for values in model.tuples(projection) {
            subjects.insert(
                (program.relations[relation.0].name.clone(), values.clone()),
                Fact::new(relation, values.clone()),
            );
        }
    }
    for (index, relation) in program
        .relations
        .iter()
        .enumerate()
        .filter(|(_, r)| r.checkpoint)
    {
        let id = RelationId(index);
        for values in model.tuples(id) {
            subjects.insert(
                (relation.name.clone(), values.clone()),
                Fact::new(id, values.clone()),
            );
        }
    }
    Ok(subjects)
}

fn subject_atoms(claim: &Claim) -> Vec<(Atom, Vec<Literal>)> {
    match claim {
        Claim::Requires { target, .. } => vec![(target.clone(), vec![target.clone().positive()])],
        Claim::NoSinglePointOfFailure { claim, .. } => subject_atoms(claim),
        Claim::State(state)
        | Claim::Eventually(state)
        | Claim::Always(state)
        | Claim::Never(state)
        | Claim::EveryTracePasses { target: state, .. } => state_atoms(state),
        Claim::EveryEventWhen { event, .. } => {
            vec![(event.clone(), vec![event.clone().positive()])]
        }
    }
}

fn state_atoms(claim: &StateClaim) -> Vec<(Atom, Vec<Literal>)> {
    let body = match claim {
        StateClaim::Holds(atom) => return vec![(atom.clone(), vec![atom.clone().positive()])],
        StateClaim::Some { body, .. } | StateClaim::No { body, .. } => body.clone(),
        StateClaim::Every { domain, body, .. } => domain.iter().chain(body).cloned().collect(),
        StateClaim::Before(claim) | StateClaim::After(claim) => return state_atoms(claim),
    };
    body.iter()
        .filter_map(|literal| match literal {
            Literal::Positive(atom) => Some((atom.clone(), body.clone())),
            _ => None,
        })
        .collect()
}
