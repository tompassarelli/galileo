//! Evaluation: bounded closure, minimal reasons, and minimal hitting sets.
//!
//! Every computation is bounded. When a bound is hit the result says so in
//! its [`Status`]; running out of budget never turns into "no" or "complete".

use crate::program::{Atom, Fact, Program, Term};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

#[derive(Clone, Debug)]
pub struct Limits {
    /// Most facts a closure may hold.
    pub max_facts: usize,
    /// Most distinct minimal reasons kept for any one conclusion.
    pub max_reasons: usize,
    /// Most elementary steps (join attempts, set unions) per computation.
    pub max_work: usize,
    /// Most hypothetical facts `achieve` may consider.
    pub max_candidates: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_facts: 1_000_000,
            max_reasons: 1_000,
            max_work: 20_000_000,
            max_candidates: 100_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Complete,
    Incomplete(String),
}

impl Status {
    pub fn is_complete(&self) -> bool {
        matches!(self, Status::Complete)
    }

    pub fn and(self, other: Status) -> Status {
        match self {
            Status::Complete => other,
            incomplete => incomplete,
        }
    }
}

/// A set of stated facts that together suffice for a conclusion.
pub type Support = BTreeSet<Fact>;

/// One rule application: `body` facts produced `head` through `rule`.
#[derive(Clone, Debug)]
pub struct Firing {
    pub rule: usize,
    pub head: Fact,
    pub body: Vec<Fact>,
}

#[derive(Debug)]
pub struct Closure {
    /// Stated plus derived facts.
    pub facts: BTreeSet<Fact>,
    /// Round in which each fact first appeared (stated facts are round 0).
    pub rank: HashMap<Fact, usize>,
    /// Every rule application over the final fact set.
    pub firings: Vec<Firing>,
    pub status: Status,
}

impl Closure {
    pub fn holds(&self, fact: &Fact) -> bool {
        self.facts.contains(fact)
    }
}

/// Everything that follows from `base` under the rules in force in `world`.
pub fn close(program: &Program, world: usize, base: &BTreeSet<Fact>, limits: &Limits) -> Closure {
    close_blocking(program, world, base, &|_| false, limits)
}

/// Like [`close`], but `blocked` facts are never stated or concluded. This
/// answers "does it still follow without these?" without editing any rule.
pub fn close_blocking(
    program: &Program,
    world: usize,
    base: &BTreeSet<Fact>,
    blocked: &dyn Fn(&Fact) -> bool,
    limits: &Limits,
) -> Closure {
    let rules: Vec<usize> = program.worlds[world].rules.iter().copied().collect();
    let base: BTreeSet<Fact> = base.iter().filter(|f| !blocked(f)).cloned().collect();
    let mut facts = base.clone();
    let mut rank: HashMap<Fact, usize> = base.iter().map(|f| (f.clone(), 0)).collect();
    let mut work = 0usize;
    let mut round = 0;
    loop {
        round += 1;
        let mut index: HashMap<usize, Vec<&Fact>> = HashMap::new();
        for fact in &facts {
            index.entry(fact.rel).or_default().push(fact);
        }
        let mut firings = Vec::new();
        let mut fresh = BTreeSet::new();
        let mut exhausted = false;
        for &r in &rules {
            let rule = &program.rules[r];
            let mut binding = vec![None; rule.vars.len()];
            let mut body = Vec::with_capacity(rule.body.len());
            let mut emit = |binding: &[Option<usize>], body: &[Fact]| {
                let head = ground(&rule.head, binding);
                if blocked(&head) {
                    return;
                }
                if !facts.contains(&head) {
                    fresh.insert(head.clone());
                }
                firings.push(Firing {
                    rule: r,
                    head,
                    body: body.to_vec(),
                });
            };
            if !join(&rule.body, &index, &mut binding, &mut body, &mut work, limits.max_work, &mut emit) {
                exhausted = true;
                break;
            }
        }
        if exhausted {
            let status = Status::Incomplete(format!(
                "stopped after {} join steps; raise --max-work for a complete answer",
                limits.max_work
            ));
            return Closure { facts, rank, firings, status };
        }
        if fresh.is_empty() {
            return Closure {
                facts,
                rank,
                firings,
                status: Status::Complete,
            };
        }
        for fact in fresh {
            rank.insert(fact.clone(), round);
            facts.insert(fact);
        }
        if facts.len() > limits.max_facts {
            let status = Status::Incomplete(format!(
                "stopped at {} facts; raise --max-facts for a complete answer",
                limits.max_facts
            ));
            return Closure {
                facts,
                rank,
                firings: Vec::new(),
                status,
            };
        }
    }
}

fn ground(atom: &Atom, binding: &[Option<usize>]) -> Fact {
    Fact {
        rel: atom.rel,
        args: atom
            .terms
            .iter()
            .map(|term| match *term {
                Term::Const(c) => c,
                Term::Var(v) => binding[v].expect("rule conclusions only use bound variables"),
            })
            .collect(),
    }
}

/// Receives each complete match: the variable binding and the matched facts.
type Emit<'a> = dyn FnMut(&[Option<usize>], &[Fact]) + 'a;

/// Nested-loop join of `atoms` against `index`. Returns false when the work
/// budget runs out.
fn join(
    atoms: &[Atom],
    index: &HashMap<usize, Vec<&Fact>>,
    binding: &mut Vec<Option<usize>>,
    body: &mut Vec<Fact>,
    work: &mut usize,
    max_work: usize,
    emit: &mut Emit,
) -> bool {
    let Some((atom, rest)) = atoms.split_first() else {
        emit(binding, body);
        return true;
    };
    let Some(candidates) = index.get(&atom.rel) else {
        return true;
    };
    for fact in candidates {
        *work += 1;
        if *work > max_work {
            return false;
        }
        let mut newly = Vec::new();
        let mut fits = true;
        for (term, &value) in atom.terms.iter().zip(&fact.args) {
            match *term {
                Term::Const(c) if c != value => fits = false,
                Term::Const(_) => {}
                Term::Var(v) => match binding[v] {
                    Some(bound) if bound != value => fits = false,
                    Some(_) => {}
                    None => {
                        binding[v] = Some(value);
                        newly.push(v);
                    }
                },
            }
            if !fits {
                break;
            }
        }
        let mut keep_going = true;
        if fits {
            body.push((*fact).clone());
            keep_going = join(rest, index, binding, body, work, max_work, emit);
            body.pop();
        }
        for v in newly {
            binding[v] = None;
        }
        if !keep_going {
            return false;
        }
    }
    true
}

/// Add `set` to an antichain of minimal sets. Returns true if it was new and
/// not implied by a smaller set already present.
pub fn insert_minimal(list: &mut Vec<Support>, set: Support) -> bool {
    if list.iter().any(|existing| existing.is_subset(&set)) {
        return false;
    }
    list.retain(|existing| !set.is_subset(existing));
    list.push(set);
    true
}

#[derive(Debug)]
pub struct Reasons {
    /// For each fact in the closure, its minimal reasons over tracked facts.
    pub of: HashMap<Fact, Vec<Support>>,
    pub status: Status,
}

/// Minimal reasons for every fact, counting only `tracked` stated facts.
///
/// Untracked stated facts are treated as free (they contribute nothing to a
/// reason). With everything tracked this is the classic minimal-witness
/// provenance; with only some facts tracked it answers "which of *these*
/// facts does the conclusion depend on?".
pub fn reasons(
    closure: &Closure,
    base: &BTreeSet<Fact>,
    tracked: &dyn Fn(&Fact) -> bool,
    limits: &Limits,
) -> Reasons {
    let mut of: HashMap<Fact, Vec<Support>> = HashMap::new();
    for fact in base {
        let reason = if tracked(fact) {
            BTreeSet::from([fact.clone()])
        } else {
            BTreeSet::new()
        };
        of.insert(fact.clone(), vec![reason]);
    }
    let mut dependents: HashMap<&Fact, Vec<usize>> = HashMap::new();
    for (i, firing) in closure.firings.iter().enumerate() {
        for fact in &firing.body {
            dependents.entry(fact).or_default().push(i);
        }
    }
    let mut queue: VecDeque<usize> = (0..closure.firings.len()).collect();
    let mut queued = vec![true; closure.firings.len()];
    let mut capped: HashSet<Fact> = HashSet::new();
    let mut work = 0usize;
    let mut status = closure.status.clone();

    while let Some(i) = queue.pop_front() {
        queued[i] = false;
        let firing = &closure.firings[i];
        if capped.contains(&firing.head) {
            continue;
        }
        let mut combined: Vec<Support> = vec![Support::new()];
        for fact in &firing.body {
            let Some(options) = of.get(fact) else {
                combined.clear();
                break;
            };
            let mut next = Vec::new();
            for partial in &combined {
                for option in options {
                    work += 1;
                    let mut union = partial.clone();
                    union.extend(option.iter().cloned());
                    insert_minimal(&mut next, union);
                }
            }
            combined = next;
            if work > limits.max_work {
                let status = status.and(Status::Incomplete(format!(
                    "stopped after {} steps while collecting reasons; raise --max-work",
                    limits.max_work
                )));
                return Reasons { of, status };
            }
        }
        if combined.is_empty() {
            continue;
        }
        let list = of.entry(firing.head.clone()).or_default();
        let mut changed = false;
        for reason in combined {
            changed |= insert_minimal(list, reason);
        }
        if list.len() > limits.max_reasons {
            list.sort_by_key(|r| r.len());
            list.truncate(limits.max_reasons);
            capped.insert(firing.head.clone());
            status = status.and(Status::Incomplete(format!(
                "kept only {} reasons for one conclusion; raise --max-reasons",
                limits.max_reasons
            )));
        }
        if changed && let Some(deps) = dependents.get(&firing.head) {
            for &d in deps {
                if !queued[d] {
                    queued[d] = true;
                    queue.push_back(d);
                }
            }
        }
    }
    Reasons { of, status }
}

/// All inclusion-minimal sets that share at least one element with every
/// set in `family` (Berge's algorithm).
pub fn minimal_hitting_sets(family: &[Support], limits: &Limits) -> (Vec<Support>, Status) {
    let mut sets: Vec<&Support> = family.iter().collect();
    sets.sort_by_key(|s| s.len());
    let mut hitting: Vec<Support> = vec![Support::new()];
    let mut work = 0usize;
    for set in sets {
        let mut next = Vec::new();
        for partial in &hitting {
            if partial.iter().any(|x| set.contains(x)) {
                insert_minimal(&mut next, partial.clone());
                continue;
            }
            for x in set {
                work += 1;
                let mut extended = partial.clone();
                extended.insert(x.clone());
                insert_minimal(&mut next, extended);
            }
            if work > limits.max_work {
                return (
                    next,
                    Status::Incomplete(format!(
                        "stopped after {} steps; raise --max-work",
                        limits.max_work
                    )),
                );
            }
        }
        hitting = next;
    }
    (hitting, Status::Complete)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(n: usize) -> Fact {
        Fact { rel: 0, args: vec![n] }
    }

    fn set(ns: &[usize]) -> Support {
        ns.iter().map(|&n| f(n)).collect()
    }

    #[test]
    fn antichain_keeps_only_minimal_sets() {
        let mut list = Vec::new();
        assert!(insert_minimal(&mut list, set(&[1, 2])));
        assert!(!insert_minimal(&mut list, set(&[1, 2, 3])));
        assert!(insert_minimal(&mut list, set(&[1])));
        assert_eq!(list, vec![set(&[1])]);
    }

    #[test]
    fn hitting_sets_of_two_disjoint_paths() {
        let (mut hs, status) =
            minimal_hitting_sets(&[set(&[1, 2]), set(&[3, 4])], &Limits::default());
        hs.sort();
        assert!(status.is_complete());
        assert_eq!(hs, vec![set(&[1, 3]), set(&[1, 4]), set(&[2, 3]), set(&[2, 4])]);
    }

    #[test]
    fn shared_element_is_a_single_point_of_failure() {
        let (hs, _) = minimal_hitting_sets(&[set(&[1, 2]), set(&[1, 3])], &Limits::default());
        assert!(hs.contains(&set(&[1])));
        assert!(hs.contains(&set(&[2, 3])));
        assert_eq!(hs.len(), 2);
    }
}
