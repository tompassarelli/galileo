//! Answering questions: find, why, prevent, achieve, diff.

use crate::engine::{Closure, Limits, Status, Support, close, close_blocking, minimal_hitting_sets, reasons};
use crate::program::{Atom, Fact, Intent, IntentKind, Pick, Program, Request, Term};
use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Debug)]
pub enum Answer {
    Find(Found),
    Why(Why),
    Prevent(Change),
    Achieve(Change),
    Diff(Diff),
    WhyNot(WhyNot),
    Review(Review),
}

#[derive(Debug)]
pub struct Found {
    pub matches: Vec<Fact>,
    /// Distinct values of the sought variable, by name.
    pub values: Vec<String>,
    pub status: Status,
}

#[derive(Debug)]
pub struct Why {
    pub holds: bool,
    pub reasons: Vec<Reason>,
    /// Total number of minimal reasons (more than `reasons.len()` for `why in`).
    pub total: usize,
    /// Facts that every reason depends on.
    pub shared: Vec<Fact>,
    pub status: Status,
}

#[derive(Debug)]
pub struct Reason {
    pub facts: Vec<Fact>,
    pub proof: Proof,
}

#[derive(Debug)]
pub enum Proof {
    Stated(Fact),
    Derived { fact: Fact, rule: usize, from: Vec<Proof> },
}

#[derive(Debug)]
pub struct Change {
    /// Prevent: it's already false. Achieve: it's already true.
    pub already: bool,
    /// No change to the `using` relations can do it.
    pub impossible: bool,
    pub solutions: Vec<Vec<Fact>>,
    pub total: usize,
    pub status: Status,
}

#[derive(Debug)]
pub struct Diff {
    /// Names that exist only in the successor.
    pub new_names: Vec<usize>,
    pub rules_added: Vec<usize>,
    pub rules_removed: Vec<usize>,
    pub added: Vec<Fact>,
    pub removed: Vec<Fact>,
    pub lost: Vec<Lost>,
    pub gained: Vec<Gained>,
    pub changed: Vec<Changed>,
    pub status: Status,
}

#[derive(Debug)]
pub struct Lost {
    pub fact: Fact,
    pub reasons_before: Vec<Vec<Fact>>,
    /// Withdrawn facts its reasons relied on.
    pub because: Vec<Fact>,
}

#[derive(Debug)]
pub struct Gained {
    pub fact: Fact,
    pub reasons_after: Vec<Vec<Fact>>,
    /// Admitted facts its reasons rely on.
    pub thanks_to: Vec<Fact>,
}

/// A conclusion that holds on both sides but for different reasons.
#[derive(Debug)]
pub struct Changed {
    pub fact: Fact,
    pub kept: Vec<Vec<Fact>>,
    pub lost: Vec<Vec<Fact>>,
    pub new: Vec<Vec<Fact>>,
    /// Facts every reason now depends on that it didn't before.
    pub now_required: Vec<Fact>,
    /// Facts every reason used to depend on that are now optional.
    pub no_longer_required: Vec<Fact>,
}

impl Answer {
    pub fn status(&self) -> &Status {
        match self {
            Answer::Find(a) => &a.status,
            Answer::Why(a) => &a.status,
            Answer::Prevent(a) | Answer::Achieve(a) => &a.status,
            Answer::Diff(a) => &a.status,
            Answer::WhyNot(a) => &a.status,
            Answer::Review(a) => &a.status,
        }
    }
}

pub fn answer(program: &Program, request: &Request, limits: &Limits) -> Answer {
    match request {
        Request::Find { world, var, pattern, .. } => Answer::Find(find(program, *world, *var, pattern, limits)),
        Request::Why { world, target, pick } => Answer::Why(why(program, *world, target, *pick, limits)),
        Request::Prevent { world, target, using, pick } => {
            Answer::Prevent(prevent(program, *world, target, using, *pick, limits))
        }
        Request::Achieve { world, target, using, pick } => {
            Answer::Achieve(achieve(program, *world, target, using, *pick, limits))
        }
        Request::Diff { base, successor } => Answer::Diff(diff(program, *base, *successor, limits)),
        Request::WhyNot { world, target } => Answer::WhyNot(why_not(program, *world, target, limits)),
        Request::Review { base, successor } => Answer::Review(review(program, *base, *successor, limits)),
    }
}

fn sort_facts(program: &Program, facts: &mut [Fact]) {
    facts.sort_by_cached_key(|f| program.sentence(f));
}

fn ordered(program: &Program, sets: impl IntoIterator<Item = Support>) -> Vec<Vec<Fact>> {
    let mut out: Vec<Vec<Fact>> = sets
        .into_iter()
        .map(|set| {
            let mut facts: Vec<Fact> = set.into_iter().collect();
            sort_facts(program, &mut facts);
            facts
        })
        .collect();
    out.sort_by_cached_key(|facts| {
        (
            facts.len(),
            facts.iter().map(|f| program.sentence(f)).collect::<Vec<_>>(),
        )
    });
    out
}

fn intersection(sets: &[Support]) -> Support {
    let mut iter = sets.iter();
    let Some(first) = iter.next() else {
        return Support::new();
    };
    iter.fold(first.clone(), |acc, set| acc.intersection(set).cloned().collect())
}

pub fn find(program: &Program, world: usize, var: usize, pattern: &Atom, limits: &Limits) -> Found {
    let w = &program.worlds[world];
    let closure = close(program, world, &w.facts, limits);
    let mut matches = Vec::new();
    let mut values = BTreeSet::new();
    let width = pattern
        .terms
        .iter()
        .filter_map(|t| match t {
            Term::Var(v) => Some(v + 1),
            Term::Const(_) => None,
        })
        .max()
        .unwrap_or(0);
    for fact in closure.facts.iter().filter(|f| f.rel == pattern.rel) {
        let mut binding: Vec<Option<usize>> = vec![None; width];
        let fits = pattern.terms.iter().zip(&fact.args).all(|(term, &value)| match *term {
            Term::Const(c) => c == value,
            Term::Var(v) => *binding[v].get_or_insert(value) == value,
        });
        if fits {
            matches.push(fact.clone());
            if let Some(value) = binding.get(var).copied().flatten() {
                values.insert(program.entities[value].name.clone());
            }
        }
    }
    sort_facts(program, &mut matches);
    Found {
        matches,
        values: values.into_iter().collect(),
        status: closure.status,
    }
}

pub fn why(program: &Program, world: usize, target: &Fact, pick: Pick, limits: &Limits) -> Why {
    let w = &program.worlds[world];
    let closure = close(program, world, &w.facts, limits);
    if !closure.holds(target) {
        return Why {
            holds: false,
            reasons: Vec::new(),
            total: 0,
            shared: Vec::new(),
            status: closure.status,
        };
    }
    let found = reasons(&closure, &w.facts, &|_| true, limits);
    let sets = found.of.get(target).cloned().unwrap_or_default();
    let mut shared: Vec<Fact> = intersection(&sets).into_iter().collect();
    sort_facts(program, &mut shared);
    let mut ordered_sets = ordered(program, sets);
    let total = ordered_sets.len();
    if pick == Pick::One {
        ordered_sets.truncate(1);
    }
    let reasons = ordered_sets
        .into_iter()
        .map(|facts| {
            let support: Support = facts.iter().cloned().collect();
            let proof = prove(program, world, &support, target, limits);
            Reason { facts, proof }
        })
        .collect();
    Why {
        holds: true,
        reasons,
        total,
        shared,
        status: found.status,
    }
}

/// A derivation of `target` that uses only the facts in `support`.
pub fn prove(program: &Program, world: usize, support: &Support, target: &Fact, limits: &Limits) -> Proof {
    prove_avoiding(program, world, support, target, &|_| false, limits)
}

/// Like [`prove`], but never passing through a `blocked` fact.
pub fn prove_avoiding(
    program: &Program,
    world: usize,
    support: &Support,
    target: &Fact,
    blocked: &dyn Fn(&Fact) -> bool,
    limits: &Limits,
) -> Proof {
    let closure = close_blocking(program, world, support, blocked, limits);
    build_proof(&closure, support, target)
}

fn build_proof(closure: &Closure, support: &Support, fact: &Fact) -> Proof {
    if support.contains(fact) {
        return Proof::Stated(fact.clone());
    }
    let Some(&rank) = closure.rank.get(fact) else {
        return Proof::Stated(fact.clone());
    };
    // Pick an application whose premises all appeared strictly earlier, so the
    // proof is well-founded even when rules are recursive.
    let firing = closure.firings.iter().find(|firing| {
        &firing.head == fact
            && firing
                .body
                .iter()
                .all(|b| closure.rank.get(b).is_some_and(|&r| r < rank))
    });
    match firing {
        Some(firing) => Proof::Derived {
            fact: fact.clone(),
            rule: firing.rule,
            from: firing.body.iter().map(|b| build_proof(closure, support, b)).collect(),
        },
        None => Proof::Stated(fact.clone()),
    }
}

pub fn prevent(
    program: &Program,
    world: usize,
    target: &Fact,
    using: &[usize],
    pick: Pick,
    limits: &Limits,
) -> Change {
    let w = &program.worlds[world];
    let closure = close(program, world, &w.facts, limits);
    if !closure.holds(target) {
        return Change {
            already: true,
            impossible: false,
            solutions: Vec::new(),
            total: 0,
            status: closure.status,
        };
    }
    // Reasons counted over the facts we're allowed to withdraw.
    let found = reasons(&closure, &w.facts, &|f| using.contains(&f.rel), limits);
    let family = found.of.get(target).cloned().unwrap_or_default();
    let mut status = found.status;
    if family.iter().any(|reason| reason.is_empty()) {
        return Change {
            already: false,
            impossible: true,
            solutions: Vec::new(),
            total: 0,
            status,
        };
    }
    let (hitting, hit_status) = minimal_hitting_sets(&family, limits);
    status = status.and(hit_status);
    // Check every answer by actually withdrawing it.
    let mut verified = Vec::new();
    for withdrawal in hitting {
        let remaining: BTreeSet<Fact> = w.facts.difference(&withdrawal).cloned().collect();
        let after = close(program, world, &remaining, limits);
        if after.status.is_complete() && !after.holds(target) {
            verified.push(withdrawal);
        } else {
            status = status.and(Status::Incomplete(
                "a candidate withdrawal didn't verify; some reasons were probably cut off".into(),
            ));
        }
    }
    finish(program, verified, pick, status)
}

pub fn achieve(
    program: &Program,
    world: usize,
    target: &Fact,
    using: &[usize],
    pick: Pick,
    limits: &Limits,
) -> Change {
    let w = &program.worlds[world];
    let closure = close(program, world, &w.facts, limits);
    if closure.holds(target) {
        return Change {
            already: true,
            impossible: false,
            solutions: Vec::new(),
            total: 0,
            status: closure.status,
        };
    }
    // Every fact of the `using` relations we could add, over the entities
    // that exist in this world.
    let mut candidates: BTreeSet<Fact> = BTreeSet::new();
    for &rel in using {
        let pools: Vec<Vec<usize>> = program.relations[rel]
            .roles
            .iter()
            .map(|role| {
                w.entities
                    .iter()
                    .copied()
                    .filter(|&e| program.entities[e].ty == role.ty)
                    .collect()
            })
            .collect();
        let count = pools.iter().map(Vec::len).product::<usize>();
        if candidates.len() + count > limits.max_candidates {
            return Change {
                already: false,
                impossible: false,
                solutions: Vec::new(),
                total: 0,
                status: Status::Incomplete(format!(
                    "more than {} possible additions; raise --max-candidates",
                    limits.max_candidates
                )),
            };
        }
        let mut args = vec![0usize; pools.len()];
        product(&pools, 0, &mut args, &mut |args| {
            let fact = Fact { rel, args: args.to_vec() };
            if !w.facts.contains(&fact) {
                candidates.insert(fact);
            }
        });
    }
    // Minimal additions are the minimal reasons in the world where every
    // candidate is present, counting only candidates.
    let everything: BTreeSet<Fact> = w.facts.union(&candidates).cloned().collect();
    let augmented = close(program, world, &everything, limits);
    if !augmented.holds(target) {
        return Change {
            already: false,
            impossible: augmented.status.is_complete(),
            solutions: Vec::new(),
            total: 0,
            status: augmented.status,
        };
    }
    let found = reasons(&augmented, &everything, &|f| candidates.contains(f), limits);
    let mut status = found.status;
    let mut verified = Vec::new();
    for addition in found.of.get(target).cloned().unwrap_or_default() {
        let with: BTreeSet<Fact> = w.facts.union(&addition).cloned().collect();
        if close(program, world, &with, limits).holds(target) {
            verified.push(addition);
        } else {
            status = status.and(Status::Incomplete("a candidate addition didn't verify".into()));
        }
    }
    finish(program, verified, pick, status)
}

fn product(pools: &[Vec<usize>], i: usize, args: &mut Vec<usize>, emit: &mut dyn FnMut(&[usize])) {
    if i == pools.len() {
        emit(args);
        return;
    }
    for &value in &pools[i] {
        args[i] = value;
        product(pools, i + 1, args, emit);
    }
}

fn finish(program: &Program, solutions: Vec<Support>, pick: Pick, status: Status) -> Change {
    let mut solutions = ordered(program, solutions);
    let total = solutions.len();
    if pick == Pick::One {
        solutions.truncate(1);
    }
    Change {
        already: false,
        impossible: false,
        solutions,
        total,
        status,
    }
}

pub fn diff(program: &Program, base: usize, successor: usize, limits: &Limits) -> Diff {
    let (a, b) = (&program.worlds[base], &program.worlds[successor]);
    let mut added: Vec<Fact> = b.facts.difference(&a.facts).cloned().collect();
    let mut removed: Vec<Fact> = a.facts.difference(&b.facts).cloned().collect();
    sort_facts(program, &mut added);
    sort_facts(program, &mut removed);

    let closure_a = close(program, base, &a.facts, limits);
    let closure_b = close(program, successor, &b.facts, limits);
    let reasons_a = reasons(&closure_a, &a.facts, &|_| true, limits);
    let reasons_b = reasons(&closure_b, &b.facts, &|_| true, limits);
    let status = reasons_a.status.clone().and(reasons_b.status.clone());

    let derived_a: BTreeSet<&Fact> = closure_a.facts.iter().filter(|f| !a.facts.contains(f)).collect();
    let derived_b: BTreeSet<&Fact> = closure_b.facts.iter().filter(|f| !b.facts.contains(f)).collect();
    let of = |r: &crate::engine::Reasons, f: &Fact| r.of.get(f).cloned().unwrap_or_default();

    let mut lost = Vec::new();
    for &fact in derived_a.difference(&derived_b) {
        let before = of(&reasons_a, fact);
        let mut because: Vec<Fact> = removed
            .iter()
            .filter(|r| before.iter().any(|reason| reason.contains(r)))
            .cloned()
            .collect();
        sort_facts(program, &mut because);
        lost.push(Lost {
            fact: fact.clone(),
            reasons_before: ordered(program, before),
            because,
        });
    }
    let mut gained = Vec::new();
    for &fact in derived_b.difference(&derived_a) {
        let after = of(&reasons_b, fact);
        let mut thanks_to: Vec<Fact> = added
            .iter()
            .filter(|r| after.iter().any(|reason| reason.contains(r)))
            .cloned()
            .collect();
        sort_facts(program, &mut thanks_to);
        gained.push(Gained {
            fact: fact.clone(),
            reasons_after: ordered(program, after),
            thanks_to,
        });
    }
    let mut changed = Vec::new();
    for &fact in derived_a.intersection(&derived_b) {
        let before = of(&reasons_a, fact);
        let after = of(&reasons_b, fact);
        let before_set: BTreeSet<&Support> = before.iter().collect();
        let after_set: BTreeSet<&Support> = after.iter().collect();
        if before_set == after_set {
            continue;
        }
        let required_before = intersection(&before);
        let required_after = intersection(&after);
        let mut now_required: Vec<Fact> = required_after.difference(&required_before).cloned().collect();
        let mut no_longer_required: Vec<Fact> = required_before.difference(&required_after).cloned().collect();
        sort_facts(program, &mut now_required);
        sort_facts(program, &mut no_longer_required);
        changed.push(Changed {
            fact: fact.clone(),
            kept: ordered(program, before_set.intersection(&after_set).map(|s| (*s).clone())),
            lost: ordered(program, before_set.difference(&after_set).map(|s| (*s).clone())),
            new: ordered(program, after_set.difference(&before_set).map(|s| (*s).clone())),
            now_required,
            no_longer_required,
        });
    }
    lost.sort_by_cached_key(|l| program.sentence(&l.fact));
    gained.sort_by_cached_key(|g| program.sentence(&g.fact));
    changed.sort_by_cached_key(|c| program.sentence(&c.fact));
    let new_names = b.entities.difference(&a.entities).copied().collect();
    Diff {
        new_names,
        rules_added: b.rules.difference(&a.rules).copied().collect(),
        rules_removed: a.rules.difference(&b.rules).copied().collect(),
        added,
        removed,
        lost,
        gained,
        changed,
        status,
    }
}

// ---------------------------------------------------------------- why not

#[derive(Debug)]
pub struct WhyNot {
    pub holds: bool,
    /// The closest each rule gets, closest first.
    pub attempts: Vec<Attempt>,
    pub status: Status,
}

#[derive(Debug)]
pub struct Attempt {
    pub rule: usize,
    pub conditions: Vec<Condition>,
    pub missing: usize,
    pub met: usize,
}

#[derive(Debug)]
pub struct Condition {
    pub text: String,
    pub state: CondState,
    /// For a missing conclusion of another rule: how close that one gets.
    pub deeper: Vec<Attempt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CondState {
    Met,
    /// Missing. `stated` when no rule here could conclude it: someone has to state it.
    Missing { stated: bool },
    /// Depends on a missing condition's unknown value.
    Waiting,
}

/// Why something does not hold: for each rule that could conclude it, the
/// closest the current facts get, and what is missing.
pub fn why_not(program: &Program, world: usize, target: &Fact, limits: &Limits) -> WhyNot {
    let w = &program.worlds[world];
    let closure = close(program, world, &w.facts, limits);
    if closure.holds(target) {
        return WhyNot {
            holds: true,
            attempts: Vec::new(),
            status: closure.status,
        };
    }
    let mut index: HashMap<usize, Vec<&Fact>> = HashMap::new();
    for fact in &closure.facts {
        index.entry(fact.rel).or_default().push(fact);
    }
    let concluded: HashSet<usize> = w.rules.iter().map(|&r| program.rules[r].head.rel).collect();
    let mut search = NearMiss {
        program,
        world,
        index,
        concluded,
        work: 0,
        max_work: limits.max_work,
        exhausted: false,
    };
    let attempts = search.explain(target, 0, &mut vec![target.clone()]);
    let status = if search.exhausted {
        Status::Incomplete(format!("stopped after {} steps; raise --max-work", limits.max_work))
    } else {
        Status::Complete
    };
    WhyNot {
        holds: false,
        attempts,
        status: closure.status.and(status),
    }
}

struct Best {
    missing: usize,
    met: usize,
    binding: Vec<Option<usize>>,
    states: Vec<CondState>,
}

struct NearMiss<'a> {
    program: &'a Program,
    world: usize,
    index: HashMap<usize, Vec<&'a Fact>>,
    concluded: HashSet<usize>,
    work: usize,
    max_work: usize,
    exhausted: bool,
}

impl NearMiss<'_> {
    fn explain(&mut self, target: &Fact, depth: usize, path: &mut Vec<Fact>) -> Vec<Attempt> {
        let program = self.program;
        let mut attempts = Vec::new();
        let rules: Vec<usize> = program.worlds[self.world].rules.iter().copied().collect();
        for r in rules {
            let rule = &program.rules[r];
            let mut binding = vec![None; rule.vars.len()];
            if !fits(&rule.head, target, &mut binding) {
                continue;
            }
            let mut best = None;
            let mut poisoned = vec![false; rule.vars.len()];
            let mut states = vec![CondState::Waiting; rule.body.len()];
            self.search(&rule.body, 0, &mut binding, &mut poisoned, &mut states, 0, &mut best);
            let Some(best) = best else { continue };
            let mut conditions = Vec::new();
            for (atom, &state) in rule.body.iter().zip(&best.states) {
                let mut deeper = Vec::new();
                if let (CondState::Missing { stated: false }, true) = (state, depth < 2)
                    && let Some(fact) = ground_partial(atom, &best.binding)
                    && !path.contains(&fact)
                {
                    path.push(fact.clone());
                    deeper = self.explain(&fact, depth + 1, path);
                    path.pop();
                }
                conditions.push(Condition {
                    text: partial_sentence(program, atom, &best.binding, &rule.vars),
                    state,
                    deeper,
                });
            }
            let met = best.states.iter().filter(|s| **s == CondState::Met).count();
            attempts.push(Attempt {
                rule: r,
                conditions,
                missing: best.missing,
                met,
            });
        }
        attempts.sort_by_key(|a| (a.missing, Reverse(a.met), a.rule));
        attempts
    }

    #[allow(clippy::too_many_arguments)]
    fn search(
        &mut self,
        atoms: &[Atom],
        i: usize,
        binding: &mut Vec<Option<usize>>,
        poisoned: &mut Vec<bool>,
        states: &mut Vec<CondState>,
        missing: usize,
        best: &mut Option<Best>,
    ) {
        if self.exhausted || best.as_ref().is_some_and(|b| missing > b.missing) {
            return;
        }
        let Some(atom) = atoms.get(i) else {
            // Fewest missing wins; among those, the one that gets furthest.
            let met = states.iter().filter(|s| **s == CondState::Met).count();
            if best.as_ref().is_none_or(|b| (missing, Reverse(met)) < (b.missing, Reverse(b.met))) {
                *best = Some(Best {
                    missing,
                    met,
                    binding: binding.clone(),
                    states: states.clone(),
                });
            }
            return;
        };
        let waits = atom
            .terms
            .iter()
            .any(|t| matches!(*t, Term::Var(v) if binding[v].is_none() && poisoned[v]));
        if waits {
            states[i] = CondState::Waiting;
            self.search(atoms, i + 1, binding, poisoned, states, missing, best);
            return;
        }
        let candidates = self.index.get(&atom.rel).cloned().unwrap_or_default();
        for fact in candidates {
            self.work += 1;
            if self.work > self.max_work {
                self.exhausted = true;
                return;
            }
            let saved = binding.clone();
            if fits(atom, fact, binding) {
                states[i] = CondState::Met;
                self.search(atoms, i + 1, binding, poisoned, states, missing, best);
            }
            *binding = saved;
        }
        let fresh: Vec<usize> = atom
            .terms
            .iter()
            .filter_map(|t| match *t {
                Term::Var(v) if binding[v].is_none() && !poisoned[v] => Some(v),
                _ => None,
            })
            .collect();
        for &v in &fresh {
            poisoned[v] = true;
        }
        states[i] = CondState::Missing {
            stated: !self.concluded.contains(&atom.rel),
        };
        self.search(atoms, i + 1, binding, poisoned, states, missing + 1, best);
        for v in fresh {
            poisoned[v] = false;
        }
    }
}

/// Does `fact` match `atom` given `binding`? Binds unbound variables on success.
fn fits(atom: &Atom, fact: &Fact, binding: &mut [Option<usize>]) -> bool {
    if atom.rel != fact.rel {
        return false;
    }
    let saved: Vec<Option<usize>> = binding.to_vec();
    for (term, &value) in atom.terms.iter().zip(&fact.args) {
        let ok = match *term {
            Term::Const(c) => c == value,
            Term::Var(v) => *binding[v].get_or_insert(value) == value,
        };
        if !ok {
            binding.copy_from_slice(&saved);
            return false;
        }
    }
    true
}

fn ground_partial(atom: &Atom, binding: &[Option<usize>]) -> Option<Fact> {
    let args = atom
        .terms
        .iter()
        .map(|t| match *t {
            Term::Const(c) => Some(c),
            Term::Var(v) => binding[v],
        })
        .collect::<Option<Vec<usize>>>()?;
    Some(Fact { rel: atom.rel, args })
}

/// Render a pattern with what's known filled in and the rest as `?name`.
pub fn partial_sentence(program: &Program, atom: &Atom, binding: &[Option<usize>], vars: &[String]) -> String {
    let filled = Atom {
        rel: atom.rel,
        terms: atom
            .terms
            .iter()
            .map(|t| match *t {
                Term::Var(v) => binding.get(v).copied().flatten().map_or(*t, Term::Const),
                c => c,
            })
            .collect(),
    };
    program.atom_sentence(&filled, vars)
}

fn matches_of(facts: &BTreeSet<Fact>, atom: &Atom, width: usize) -> Vec<(Fact, Vec<Option<usize>>)> {
    facts
        .iter()
        .filter_map(|fact| {
            let mut binding = vec![None; width];
            fits(atom, fact, &mut binding).then(|| (fact.clone(), binding))
        })
        .collect()
}

// ---------------------------------------------------------------- intents

#[derive(Debug)]
pub struct IntentCheck {
    pub holds: bool,
    /// It holds only because nothing triggers it yet.
    pub vacuous: bool,
    pub problem: Option<Problem>,
    pub status: Status,
}

#[derive(Debug)]
pub enum Problem {
    /// A must-hold sentence doesn't hold.
    Missing { why_not: Option<WhyNot> },
    /// A `never` sentence holds.
    Present { facts: Vec<Fact>, proof: Proof },
    /// A conclusion holds without what it requires; `proof` avoids it.
    /// `others` are further conclusions with the same problem.
    Bypassed { fact: Fact, needed: String, proof: Proof, others: Vec<Fact> },
    /// Each of these stated facts can stop the conclusion on its own.
    SinglePoints { fact: Fact, points: Vec<Fact> },
}

/// Check one must-sentence in one world.
pub fn check_intent(program: &Program, world: usize, intent: &Intent, limits: &Limits) -> IntentCheck {
    let w = &program.worlds[world];
    let closure = close(program, world, &w.facts, limits);
    let width = intent.vars.len();
    let ok = |vacuous| IntentCheck {
        holds: true,
        vacuous,
        problem: None,
        status: closure.status.clone(),
    };
    let broken = |problem| IntentCheck {
        holds: false,
        vacuous: false,
        problem: Some(problem),
        status: closure.status.clone(),
    };
    match &intent.kind {
        IntentKind::Holds(atom) => {
            if !matches_of(&closure.facts, atom, width).is_empty() {
                return ok(false);
            }
            let why_not = ground_partial(atom, &vec![None; width]).map(|f| why_not(program, world, &f, limits));
            broken(Problem::Missing { why_not })
        }
        IntentKind::Never(atom) => {
            let mut facts: Vec<Fact> = matches_of(&closure.facts, atom, width).into_iter().map(|(f, _)| f).collect();
            if facts.is_empty() {
                return ok(false);
            }
            sort_facts(program, &mut facts);
            let first = why(program, world, &facts[0], Pick::One, limits);
            let proof = first
                .reasons
                .into_iter()
                .next()
                .map(|r| r.proof)
                .unwrap_or(Proof::Stated(facts[0].clone()));
            broken(Problem::Present { facts, proof })
        }
        IntentKind::Requires(subject, needs) => {
            let found = matches_of(&closure.facts, subject, width);
            let vacuous = found.is_empty();
            let mut cases: Vec<(Fact, Vec<Option<usize>>)> = found
                .into_iter()
                .filter(|(fact, binding)| {
                    let blocked = |f: &Fact| fits(needs, f, &mut binding.clone());
                    close_blocking(program, world, &w.facts, &blocked, limits).holds(fact)
                })
                .collect();
            if cases.is_empty() {
                return ok(vacuous);
            }
            cases.sort_by_cached_key(|(fact, _)| program.sentence(fact));
            let others = cases[1..].iter().map(|(fact, _)| fact.clone()).collect();
            let (fact, binding) = cases.swap_remove(0);
            // Show the simplest way it holds while avoiding what it requires.
            let blocked = |f: &Fact| fits(needs, f, &mut binding.clone());
            let without = close_blocking(program, world, &w.facts, &blocked, limits);
            let base: BTreeSet<Fact> = w.facts.iter().filter(|f| !blocked(f)).cloned().collect();
            let simplest = reasons(&without, &base, &|_| true, limits)
                .of
                .get(&fact)
                .and_then(|r| r.iter().min_by_key(|s| s.len()).cloned())
                .unwrap_or_default();
            let proof = prove_avoiding(program, world, &simplest, &fact, &blocked, limits);
            let needed = partial_sentence(program, needs, &binding, &intent.vars);
            broken(Problem::Bypassed { fact, needed, proof, others })
        }
        IntentKind::NoSinglePoint(subject, among) => {
            let found = matches_of(&closure.facts, subject, width);
            let vacuous = found.is_empty();
            for (fact, binding) in found {
                let mut points: Vec<Fact> = w
                    .facts
                    .iter()
                    .filter(|f| fits(among, f, &mut binding.clone()))
                    .filter(|candidate| {
                        let rest: BTreeSet<Fact> = w.facts.iter().filter(|f| f != candidate).cloned().collect();
                        !close(program, world, &rest, limits).holds(&fact)
                    })
                    .cloned()
                    .collect();
                if !points.is_empty() {
                    sort_facts(program, &mut points);
                    return broken(Problem::SinglePoints { fact, points });
                }
            }
            ok(vacuous)
        }
    }
}

/// Every must-sentence of the world's model, checked in that world.
pub fn check_world(program: &Program, world: usize, limits: &Limits) -> Vec<(usize, IntentCheck)> {
    let model = program.worlds[world].model;
    program
        .intents
        .iter()
        .enumerate()
        .filter(|(_, intent)| intent.model == model)
        .map(|(i, intent)| (i, check_intent(program, world, intent, limits)))
        .collect()
}

// ---------------------------------------------------------------- review

#[derive(Debug)]
pub struct Review {
    /// What the author says the change does. Unchecked.
    pub says: Option<String>,
    pub diff: Diff,
    pub intents: Vec<IntentReview>,
    /// Intents that held before and don't after.
    pub broken: usize,
    pub status: Status,
}

#[derive(Debug)]
pub struct IntentReview {
    pub intent: usize,
    pub before: IntentCheck,
    pub after: IntentCheck,
    /// A must-hold sentence that still holds for fewer reasons: (before, after, now required).
    pub weaker: Option<(usize, usize, Vec<Fact>)>,
}

/// Review a proposed revision: what it changes, what follows, and which
/// must-sentences it keeps or breaks.
pub fn review(program: &Program, base: usize, successor: usize, limits: &Limits) -> Review {
    let diff = diff(program, base, successor, limits);
    let mut status = diff.status.clone();
    let mut intents = Vec::new();
    for ((i, before), (_, after)) in check_world(program, base, limits).into_iter().zip(check_world(program, successor, limits)) {
        status = status.and(before.status.clone()).and(after.status.clone());
        let mut weaker = None;
        if let IntentKind::Holds(atom) = &program.intents[i].kind
            && after.holds
            && let Some(fact) = ground_partial(atom, &vec![None; program.intents[i].vars.len()])
            && let Some(change) = diff.changed.iter().find(|c| c.fact == fact && !c.lost.is_empty())
        {
            let was = change.kept.len() + change.lost.len();
            let now = change.kept.len() + change.new.len();
            if now < was {
                weaker = Some((was, now, change.now_required.clone()));
            }
        }
        intents.push(IntentReview {
            intent: i,
            before,
            after,
            weaker,
        });
    }
    let broken = intents.iter().filter(|r| r.before.holds && !r.after.holds).count();
    Review {
        says: program.worlds[successor].says.clone(),
        diff,
        intents,
        broken,
        status,
    }
}
