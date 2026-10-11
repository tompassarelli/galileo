use crate::reads::claim_reads;
use galileo_core::claims::{check_claims, ClaimError, ClaimVerdict};
use galileo_core::{evaluate, Budget, EvalError, Fact, Program, RelationId, Verdict};
use std::collections::BTreeSet;

const BEFORE: &str = "?before ";

pub fn intended(baseline: &Program, contract: &Program, budget: Budget) -> Result<Vec<ClaimVerdict>, ClaimError> {
    let marked: Vec<(RelationId, Option<RelationId>)> = contract
        .relations
        .iter()
        .enumerate()
        .filter_map(|(index, relation)| relation.name.strip_prefix(BEFORE).map(|name| (RelationId(index), baseline.relations.iter().position(|r| r.name == name).map(RelationId))))
        .collect();
    if marked.is_empty() {
        return Ok(check_claims(contract, budget)?.into_iter().map(|result| result.verdict).collect());
    }
    let mut filled = contract.clone();
    match evaluate(baseline, &baseline.initially, budget) {
        Ok(model) => {
            for (before, relation) in &marked {
                let tuples = relation.into_iter().flat_map(|relation| model.tuples(*relation));
                filled.facts.extend(tuples.map(|values| Fact::new(*before, values.iter().cloned())));
            }
            Ok(check_claims(&filled, budget)?.into_iter().map(|result| result.verdict).collect())
        }
        Err(EvalError::Unknown(reason)) => {
            let marks: BTreeSet<RelationId> = marked.iter().map(|(before, _)| *before).collect();
            let reads_before: Vec<bool> = contract.requirements.iter().map(|r| !claim_reads(contract, &r.claim).is_disjoint(&marks)).collect();
            filled.requirements.retain(|r| claim_reads(contract, &r.claim).is_disjoint(&marks));
            let mut rest = check_claims(&filled, budget)?.into_iter().map(|result| result.verdict);
            Ok(reads_before
                .into_iter()
                .map(|before| if before { Verdict::Unknown(reason.clone()) } else { rest.next().expect("one verdict per unmarked claim") })
                .collect())
        }
        Err(error) => Err(ClaimError::Eval(error)),
    }
}
