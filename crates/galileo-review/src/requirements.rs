use crate::baseline::normalized_claim;
use crate::Edit;
use galileo_core::claims::{enumerate_claim_failures, failure_sentence, ClaimError, ClaimVerdict, Instance};
use galileo_core::{Budget, Program, Reason, RelationId, Requirement, Variable, Verdict};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureChange {
    Introduced(String),
    Fixed(String),
    PreExisting(String),
}

impl fmt::Display for FailureChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (label, sentence) = match self {
            Self::Introduced(sentence) => ("introduced", sentence),
            Self::Fixed(sentence) => ("fixed", sentence),
            Self::PreExisting(sentence) => ("pre-existing", sentence),
        };
        write!(f, "{sentence}: {label}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequirementChange {
    Added(String),
    Removed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequirementComparison {
    pub requirement_changes: Vec<RequirementChange>,
    pub failures: Vec<FailureChange>,
    pub before: Vec<ClaimVerdict>,
    pub after: Vec<ClaimVerdict>,
    pub incomplete: Vec<Reason>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReviewOptions {
    pub allow_baseline_failures: bool,
    pub allow_requirement_changes: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewVerdict {
    Approve,
    ApproveNoNewFailures,
    Reject,
    Unknown,
}

impl RequirementComparison {
    pub fn verdict(&self, intended: &[ClaimVerdict], edits: Option<&[Edit]>, options: ReviewOptions) -> ReviewVerdict {
        if !self.incomplete.is_empty() {
            return ReviewVerdict::Unknown;
        }
        if intended.iter().any(|v| matches!(v, Verdict::Fails(_))) {
            return ReviewVerdict::Reject;
        }
        if intended.iter().chain(&self.after).any(|v| matches!(v, Verdict::Unknown(_))) {
            return ReviewVerdict::Unknown;
        }
        let declared = edits.is_some_and(|edits| edits.iter().any(|edit| matches!(edit, Edit::ChangeRequirements { .. })));
        if !self.requirement_changes.is_empty() && !declared && !options.allow_requirement_changes {
            return ReviewVerdict::Reject;
        }
        if self.failures.iter().any(|failure| matches!(failure, FailureChange::Introduced(_))) {
            return ReviewVerdict::Reject;
        }
        if self.after.iter().any(|v| matches!(v, Verdict::Fails(_))) {
            if options.allow_baseline_failures {
                ReviewVerdict::ApproveNoNewFailures
            } else {
                ReviewVerdict::Reject
            }
        } else {
            ReviewVerdict::Approve
        }
    }
}

pub fn requirements(before: &Program, after: &Program, budget: Budget) -> Result<RequirementComparison, ClaimError> {
    fn vocabulary(program: &Program) -> Vec<String> {
        program.relations.iter().map(|relation| {
            let mut relation = relation.clone();
            relation.source = None;
            for (index, slot) in relation.slots.iter_mut().enumerate() {
                let name = format!("s{index}");
                relation.template = relation.template.replace(&format!("{{{}}}", slot.name), &format!("{{{name}}}"));
                slot.name = name;
            }
            format!("{relation:?}")
        }).collect()
    }
    fn keys(program: &Program, vocabulary: &[String], all: &BTreeMap<String, RelationId>) -> BTreeMap<String, usize> {
        let relations: BTreeMap<_, _> = vocabulary.iter().enumerate().map(|(index, key)| (RelationId(index), all[key])).collect();
        program.requirements.iter().enumerate().map(|(index, requirement)| {
            let (claim, _) = normalized_claim(&requirement.claim, relations.clone());
            (format!("{}:{claim:?}", requirement.invariant), index)
        }).collect()
    }
    fn sentence(program: &Program, requirement: &Requirement, instance: &Instance) -> String {
        let (claim, names) = normalized_claim(&requirement.claim, BTreeMap::new());
        let instance = instance.iter().map(|(variable, value)| (Variable(names[&variable.0].clone()), value.clone())).collect();
        let requirement = Requirement { claim, ..requirement.clone() };
        failure_sentence(program, &requirement, &instance)
    }
    let before_vocabulary = vocabulary(before);
    let after_vocabulary = vocabulary(after);
    let all: BTreeMap<_, _> = before_vocabulary.iter().chain(&after_vocabulary).cloned().collect::<BTreeSet<_>>().into_iter().enumerate().map(|(index, key)| (key, RelationId(index))).collect();
    let before_keys = keys(before, &before_vocabulary, &all);
    let after_keys = keys(after, &after_vocabulary, &all);
    let mut requirement_changes = Vec::new();
    for (key, index) in &before_keys {
        if !after_keys.contains_key(key) {
            requirement_changes.push(RequirementChange::Removed(before.requirements[*index].name.clone()));
        }
    }
    for (key, index) in &after_keys {
        if !before_keys.contains_key(key) {
            requirement_changes.push(RequirementChange::Added(after.requirements[*index].name.clone()));
        }
    }
    let before_results = enumerate_claim_failures(before, budget)?;
    let after_results = enumerate_claim_failures(after, budget)?;
    let incomplete = before_results.iter().chain(&after_results).filter_map(|result| result.incomplete.clone()).collect::<Vec<_>>();
    let mut failures = Vec::new();
    if incomplete.is_empty() {
        let collect = |program: &Program, keys: &BTreeMap<String, usize>, results: &[galileo_core::claims::ClaimFailures]| {
            keys.iter().flat_map(|(key, index)| results[*index].instances.iter().map(move |instance| (key.clone(), sentence(program, &program.requirements[*index], instance)))).collect::<BTreeSet<_>>()
        };
        let before_failures = collect(before, &before_keys, &before_results);
        let after_failures = collect(after, &after_keys, &after_results);
        for failure in &before_failures {
            failures.push(if after_failures.contains(failure) {
                FailureChange::PreExisting(failure.1.clone())
            } else {
                FailureChange::Fixed(failure.1.clone())
            });
        }
        for failure in after_failures.difference(&before_failures) {
            failures.push(FailureChange::Introduced(failure.1.clone()));
        }
    }
    Ok(RequirementComparison {
        requirement_changes,
        failures,
        before: before_results.into_iter().map(|result| result.verdict).collect(),
        after: after_results.into_iter().map(|result| result.verdict).collect(),
        incomplete,
    })
}
