use crate::reads::claim_reads;
use crate::requirements::{requirement_changes, RequirementChange};
use crate::{Intent, Patch};
use galileo_core::claims::{check_claims, failure_sentence, ClaimError, ClaimVerdict, Evidence, Instance};
use galileo_core::why::sentence;
use galileo_core::{evaluate, Budget, EvalError, Expr, Fact, Model, Program, Reason, RelationId, Value, Variable, Verdict};
use std::collections::{BTreeMap, BTreeSet};

const BEFORE: &str = "?before ";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntendedVerdict {
    pub verdict: ClaimVerdict,
    pub failure: Option<String>,
}

pub fn intended(baseline: &Program, patch: &Patch, budget: Budget) -> Result<Vec<IntendedVerdict>, ClaimError> {
    let Some(contract) = &patch.contract else {
        return Ok(Vec::new());
    };
    let marked: Vec<(RelationId, Option<RelationId>)> = contract
        .relations
        .iter()
        .enumerate()
        .filter_map(|(index, relation)| relation.name.strip_prefix(BEFORE).map(|name| (RelationId(index), relation_named(baseline, name))))
        .collect();
    let only = patch.intends.iter().any(|intent| matches!(intent, Intent::Only { .. }));
    let model = |program: &Program| match evaluate(program, &program.initially, budget) {
        Ok(model) => Ok(Ok(model)),
        Err(EvalError::Unknown(reason)) => Ok(Err(reason)),
        Err(error) => Err(ClaimError::Eval(error)),
    };
    let before = if marked.is_empty() && !only { None } else { Some(model(baseline)?) };
    let after = if only { Some(model(&patch.program)?) } else { None };
    let claims = claims(contract, &marked, before.as_ref(), budget)?;
    Ok(patch
        .intends
        .iter()
        .map(|intent| match intent {
            Intent::Claim(index) => claims[*index].clone(),
            Intent::Only { relation, pattern } => only_changes(baseline, contract, *relation, pattern, before.as_ref().unwrap(), after.as_ref().unwrap()),
            Intent::RequirementsUnchanged => match requirement_changes(baseline, &patch.program).first() {
                None => IntendedVerdict { verdict: Verdict::Holds(Evidence::Complete), failure: None },
                Some(RequirementChange::Added(name) | RequirementChange::Removed(name)) => {
                    IntendedVerdict { verdict: Verdict::Fails(Instance::new()), failure: Some(name.clone()) }
                }
            },
        })
        .collect())
}

fn relation_named(program: &Program, name: &str) -> Option<RelationId> {
    program.relations.iter().position(|r| r.name == name).map(RelationId)
}

fn claims(contract: &Program, marked: &[(RelationId, Option<RelationId>)], before: Option<&Result<Model, Reason>>, budget: Budget) -> Result<Vec<IntendedVerdict>, ClaimError> {
    let named = |program: &Program, verdicts: Vec<ClaimVerdict>| -> Vec<IntendedVerdict> {
        verdicts
            .into_iter()
            .zip(&program.requirements)
            .map(|(verdict, requirement)| {
                let failure = match &verdict {
                    Verdict::Fails(instance) => Some(failure_sentence(program, requirement, instance)),
                    _ => None,
                };
                IntendedVerdict { verdict, failure }
            })
            .collect()
    };
    let check = |program: &Program| -> Result<Vec<IntendedVerdict>, ClaimError> {
        Ok(named(program, check_claims(program, budget)?.into_iter().map(|result| result.verdict).collect()))
    };
    if marked.is_empty() {
        return check(contract);
    }
    let mut filled = contract.clone();
    match before.expect("a marked contract has the baseline's model") {
        Ok(model) => {
            for (before, relation) in marked {
                let tuples = relation.iter().flat_map(|relation| model.tuples(*relation));
                filled.facts.extend(tuples.map(|values| Fact::new(*before, values.iter().cloned())));
            }
            check(&filled)
        }
        Err(reason) => {
            let marks: BTreeSet<RelationId> = marked.iter().map(|(before, _)| *before).collect();
            let reads_before: Vec<bool> = contract.requirements.iter().map(|r| !claim_reads(contract, &r.claim).is_disjoint(&marks)).collect();
            filled.requirements.retain(|r| claim_reads(contract, &r.claim).is_disjoint(&marks));
            let mut rest = check(&filled)?.into_iter();
            Ok(reads_before
                .into_iter()
                .map(|before| if before { IntendedVerdict { verdict: Verdict::Unknown(reason.clone()), failure: None } } else { rest.next().expect("one verdict per unmarked claim") })
                .collect())
        }
    }
}

fn only_changes(baseline: &Program, contract: &Program, relation: RelationId, pattern: &[Expr], before: &Result<Model, Reason>, after: &Result<Model, Reason>) -> IntendedVerdict {
    let unknown = |reason: &Reason| IntendedVerdict { verdict: Verdict::Unknown(reason.clone()), failure: None };
    let (before, after) = match (before, after) {
        (Err(reason), _) | (_, Err(reason)) => return unknown(reason),
        (Ok(before), Ok(after)) => (before, after),
    };
    let lost: BTreeSet<&Vec<Value>> = relation_named(baseline, &contract.relations[relation.0].name).into_iter().flat_map(|r| before.tuples(r)).collect();
    let gained: BTreeSet<&Vec<Value>> = after.tuples(relation).collect();
    let mut matched = 0;
    for tuple in lost.symmetric_difference(&gained) {
        if !matches(pattern, tuple) {
            let instance = contract.relations[relation.0].slots.iter().zip(tuple.iter()).map(|(slot, value)| (Variable(slot.name.clone()), value.clone())).collect();
            return IntendedVerdict { verdict: Verdict::Fails(instance), failure: Some(sentence(contract, relation, tuple)) };
        }
        matched += 1;
    }
    IntendedVerdict { verdict: Verdict::Holds(Evidence::Every { instances: matched }), failure: None }
}

fn matches(pattern: &[Expr], tuple: &[Value]) -> bool {
    let mut bound: BTreeMap<&Variable, &Value> = BTreeMap::new();
    pattern.iter().zip(tuple).all(|(expr, value)| match expr {
        Expr::Value(expected) => expected == value,
        Expr::Variable(variable) if variable.0.is_empty() || variable.0.starts_with('_') => true,
        Expr::Variable(variable) => *bound.entry(variable).or_insert(value) == value,
        _ => false,
    })
}
