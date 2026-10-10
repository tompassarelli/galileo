//! Plain-language output: every answer is printed as sentences, through the
//! same relation shapes the rules were written in.

use crate::ask::{
    Answer, Attempt, Change, Changed, CondState, Diff, Found, IntentCheck, Problem, Proof, Review, Why, WhyNot,
};
use crate::engine::Status;
use crate::program::{Fact, Pick, Program, Request, RequestItem};
use std::collections::HashSet;
use std::fmt::Write;

pub fn render(program: &Program, item: &RequestItem, answer: &Answer) -> String {
    let mut out = String::new();
    for (i, line) in item.source.iter().enumerate() {
        if i == 0 {
            let _ = writeln!(out, "▸ {line}");
        } else {
            let _ = writeln!(out, "  {line}");
        }
    }
    out.push('\n');
    match (&item.request, answer) {
        (Request::Find { .. }, Answer::Find(found)) => find(program, &mut out, found),
        (Request::Why { world, pick, .. }, Answer::Why(why)) => {
            render_why(program, &mut out, &program.worlds[*world].name, *pick, why)
        }
        (Request::Prevent { world, using, .. }, Answer::Prevent(change)) => {
            change_answer(program, &mut out, &program.worlds[*world].name, using, change, false)
        }
        (Request::Achieve { world, using, .. }, Answer::Achieve(change)) => {
            change_answer(program, &mut out, &program.worlds[*world].name, using, change, true)
        }
        (Request::Diff { .. }, Answer::Diff(diff)) => render_diff(program, &mut out, diff),
        (Request::WhyNot { world, .. }, Answer::WhyNot(why_not)) => {
            render_why_not(program, &mut out, &program.worlds[*world].name, why_not)
        }
        (Request::Review { .. }, Answer::Review(review)) => render_review(program, &mut out, review),
        _ => unreachable!("answers match their requests"),
    }
    if let Status::Incomplete(why) = answer.status() {
        let _ = writeln!(out, "  ⚠ Incomplete: {why}.");
        let _ = writeln!(out, "    Treat this as a partial answer, not a final one.");
    }
    out
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

fn find(program: &Program, out: &mut String, found: &Found) {
    if found.matches.is_empty() {
        let _ = writeln!(out, "  Nothing matches.");
        return;
    }
    let _ = writeln!(out, "  {}:", plural(found.matches.len(), "match", "matches"));
    for fact in &found.matches {
        let _ = writeln!(out, "    {}", program.sentence(fact));
    }
}

fn render_why(program: &Program, out: &mut String, world: &str, pick: Pick, why: &Why) {
    if !why.holds {
        let _ = writeln!(out, "  No: this doesn't follow in `{world}`.");
        let _ = writeln!(out, "  (An `achieve` question can find what would make it true.)");
        return;
    }
    match (pick, why.total) {
        (_, 1) => {
            let _ = writeln!(out, "  Yes, for exactly one reason.");
        }
        (Pick::One, n) => {
            let _ = writeln!(out, "  Yes. Here is the simplest of {n} independent reasons.");
        }
        (Pick::All, n) => {
            let _ = writeln!(out, "  Yes, for {n} independent reasons.");
        }
    }
    let shown = why.reasons.len();
    for (i, reason) in why.reasons.iter().enumerate() {
        out.push('\n');
        let label = if shown > 1 { format!("Reason {} of {}", i + 1, why.total) } else { "Reason".into() };
        let _ = writeln!(out, "  {label} ({}):", plural(reason.facts.len(), "stated fact", "stated facts"));
        tree(program, out, &reason.proof, "    ", None, &mut HashSet::new());
    }
    if why.total > 1 && pick == Pick::All {
        out.push('\n');
        if why.shared.is_empty() {
            let _ = writeln!(out, "  No single stated fact is needed by every reason.");
        } else {
            let _ = writeln!(out, "  Every reason depends on:");
            for fact in &why.shared {
                let _ = writeln!(out, "    {}", program.sentence(fact));
            }
        }
    }
}

/// Draw a proof as a tree. `branch` is `Some(is_last)` for non-root nodes.
/// A conclusion already drawn earlier in the same tree is not expanded again.
fn tree(
    program: &Program,
    out: &mut String,
    proof: &Proof,
    prefix: &str,
    branch: Option<bool>,
    shown: &mut HashSet<Fact>,
) {
    let (lead, child_prefix) = match branch {
        None => (prefix.to_string(), prefix.to_string()),
        Some(true) => (format!("{prefix}└─ "), format!("{prefix}   ")),
        Some(false) => (format!("{prefix}├─ "), format!("{prefix}│  ")),
    };
    match proof {
        Proof::Stated(fact) => {
            let _ = writeln!(out, "{lead}{}", program.sentence(fact));
        }
        Proof::Derived { fact, .. } if shown.contains(fact) => {
            let _ = writeln!(out, "{lead}{}   (shown above)", program.sentence(fact));
        }
        Proof::Derived { fact, rule, from } => {
            shown.insert(fact.clone());
            let _ = writeln!(out, "{lead}{}   ← {}", program.sentence(fact), program.rules[*rule].name);
            for (i, child) in from.iter().enumerate() {
                tree(program, out, child, &child_prefix, Some(i + 1 == from.len()), shown);
            }
        }
    }
}

fn list_block(program: &Program, out: &mut String, label: &str, facts: &[Fact]) {
    let pad = " ".repeat(label.chars().count());
    for (i, fact) in facts.iter().enumerate() {
        let head = if i == 0 { label } else { pad.as_str() };
        let _ = writeln!(out, "{head}{}", program.sentence(fact));
    }
}

fn change_answer(
    program: &Program,
    out: &mut String,
    world: &str,
    using: &[usize],
    change: &Change,
    achieve: bool,
) {
    let names: Vec<String> = using.iter().map(|&r| format!("`{}`", program.relations[r].name)).collect();
    let names = names.join(", ");
    let (goal, verb) = if achieve { ("true", "add") } else { ("false", "withdraw") };
    if change.already {
        let _ = writeln!(out, "  It's already {goal} in `{world}`; nothing to {verb}.");
        return;
    }
    if change.impossible {
        if achieve {
            let _ = writeln!(out, "  No combination of {names} facts would make it true.");
        } else {
            let _ = writeln!(out, "  It can't be prevented by withdrawing only {names} facts:");
            let _ = writeln!(out, "  it holds without any of them.");
        }
        return;
    }
    if change.solutions.is_empty() {
        let _ = writeln!(out, "  No way to make it {goal} was found.");
        return;
    }
    let singles = change.solutions.iter().all(|s| s.len() == 1);
    if change.solutions.len() < change.total {
        let _ = writeln!(out, "  The smallest way to make it {goal} (1 of {}):", change.total);
    } else if singles && change.total > 1 {
        let _ = writeln!(out, "  {} to make it {goal}; any one of these alone does it:", plural(change.total, "minimal way", "minimal ways"));
    } else if change.total > 1 {
        let _ = writeln!(out, "  {} to make it {goal}; {verb} everything in any one of them:", plural(change.total, "minimal way", "minimal ways"));
    } else {
        let _ = writeln!(out, "  One minimal way to make it {goal}:");
    }
    let width = change.solutions.len().to_string().len();
    for (i, solution) in change.solutions.iter().enumerate() {
        let label = format!("    {:>width$}. {verb} ", i + 1);
        list_block(program, out, &label, solution);
    }
    if !achieve && change.solutions.len() == change.total && change.total > 0 {
        let single: Vec<&Fact> = change.solutions.iter().filter(|s| s.len() == 1).map(|s| &s[0]).collect();
        if single.is_empty() {
            let _ = writeln!(out, "  No single withdrawal is enough.");
        } else if single.len() == change.total {
            let _ = writeln!(out, "  Each of these is a single point of failure.");
        } else {
            let _ = writeln!(out, "  Single points of failure:");
            for fact in single {
                let _ = writeln!(out, "    {}", program.sentence(fact));
            }
        }
    }
}

/// Facts in `set` that appear in none of `others`: what makes this reason distinct.
fn distinct(set: &[Fact], others: &[Vec<Fact>]) -> Vec<Fact> {
    let only: Vec<Fact> = set
        .iter()
        .filter(|f| !others.iter().any(|o| o.contains(f)))
        .cloned()
        .collect();
    if only.is_empty() { set.to_vec() } else { only }
}

fn render_diff(program: &Program, out: &mut String, diff: &Diff) {
    if !render_changes(program, out, diff, "  You changed:") {
        let _ = writeln!(out, "  The two are identical.");
        return;
    }

    if !diff.lost.is_empty() {
        let _ = writeln!(out, "\n  No longer true:");
        for lost in &diff.lost {
            let _ = writeln!(out, "    − {}", program.sentence(&lost.fact));
            list_block(program, out, "        it needed: ", &lost.because);
        }
    }
    if !diff.gained.is_empty() {
        let _ = writeln!(out, "\n  Newly true:");
        for gained in &diff.gained {
            let _ = writeln!(out, "    + {}", program.sentence(&gained.fact));
            list_block(program, out, "        thanks to: ", &gained.thanks_to);
        }
    }

    let weaker: Vec<&Changed> = diff.changed.iter().filter(|c| c.new.is_empty()).collect();
    let stronger: Vec<&Changed> = diff.changed.iter().filter(|c| c.lost.is_empty()).collect();
    let mixed: Vec<&Changed> = diff
        .changed
        .iter()
        .filter(|c| !c.new.is_empty() && !c.lost.is_empty())
        .collect();
    for (title, group) in [
        ("Still true, but weaker:", weaker),
        ("Still true, with more ways now:", stronger),
        ("Still true, for different reasons:", mixed),
    ] {
        if group.is_empty() {
            continue;
        }
        let _ = writeln!(out, "\n  {title}");
        for change in group {
            let before = change.kept.len() + change.lost.len();
            let after = change.kept.len() + change.new.len();
            let _ = writeln!(out, "    ~ {}", program.sentence(&change.fact));
            let _ = writeln!(out, "        reasons: {before} → {after}");
            let now: Vec<Vec<Fact>> = change.kept.iter().chain(&change.new).cloned().collect();
            let then: Vec<Vec<Fact>> = change.kept.iter().chain(&change.lost).cloned().collect();
            for lost in &change.lost {
                list_block(program, out, "        lost the way through: ", &distinct(lost, &now));
            }
            for new in &change.new {
                list_block(program, out, "        new way through: ", &distinct(new, &then));
            }
            if !change.now_required.is_empty() {
                list_block(program, out, "        now depends on every one of: ", &change.now_required);
            }
            if !change.no_longer_required.is_empty() {
                list_block(program, out, "        no longer requires: ", &change.no_longer_required);
            }
        }
    }
    if diff.lost.is_empty() && diff.gained.is_empty() && diff.changed.is_empty() {
        let _ = writeln!(out, "\n  No conclusions changed.");
    }
}

/// The edit itself: names, facts and rules. Returns false when nothing changed.
fn render_changes(program: &Program, out: &mut String, diff: &Diff, title: &str) -> bool {
    if diff.added.is_empty()
        && diff.removed.is_empty()
        && diff.new_names.is_empty()
        && diff.rules_added.is_empty()
        && diff.rules_removed.is_empty()
    {
        return false;
    }
    let _ = writeln!(out, "{title}");
    for &name in &diff.new_names {
        let entity = &program.entities[name];
        let _ = writeln!(out, "    + {} ∈ {}", entity.name, program.types[entity.ty]);
    }
    for fact in &diff.removed {
        let _ = writeln!(out, "    − {}", program.sentence(fact));
    }
    for fact in &diff.added {
        let _ = writeln!(out, "    + {}", program.sentence(fact));
    }
    for &rule in &diff.rules_removed {
        let _ = writeln!(out, "    − rule {}", program.rule_text(rule));
    }
    for &rule in &diff.rules_added {
        let _ = writeln!(out, "    + rule {}", program.rule_text(rule));
    }
    true
}

fn render_why_not(program: &Program, out: &mut String, world: &str, why_not: &WhyNot) {
    if why_not.holds {
        let _ = writeln!(out, "  It does hold in `{world}`. Ask `why` to see the reasons.");
        return;
    }
    if why_not.attempts.is_empty() {
        let _ = writeln!(out, "  It doesn't hold: nothing states it, and no rule here concludes it.");
        return;
    }
    let _ = writeln!(out, "  It doesn't hold. Here is how close each rule gets:");
    for attempt in &why_not.attempts {
        out.push('\n');
        attempt_block(program, out, attempt, "  ");
    }
}

fn attempt_block(program: &Program, out: &mut String, attempt: &Attempt, indent: &str) {
    let _ = writeln!(out, "{indent}{} needs:", program.rules[attempt.rule].name);
    for condition in &attempt.conditions {
        let (mark, note) = match condition.state {
            CondState::Met => ("✓", ""),
            CondState::Missing { stated: true } => ("✗", "   (nothing states this)"),
            CondState::Missing { stated: false } => ("✗", ""),
            CondState::Waiting => ("·", ""),
        };
        let _ = writeln!(out, "{indent}  {mark} {}{note}", condition.text);
        // Below the top level, show only the closest way for each missing piece.
        if let Some(deeper) = condition.deeper.first() {
            attempt_block(program, out, deeper, &format!("{indent}      "));
        }
    }
}

/// One must-sentence and, when it's broken, why.
fn render_intent(program: &Program, out: &mut String, text: &str, check: &IntentCheck, note: &str, indent: &str) {
    let mark = if check.holds { "✓" } else { "✗" };
    let vacuous = if check.vacuous { "   (nothing triggers it yet)" } else { "" };
    let _ = writeln!(out, "{indent}{mark} {text}{vacuous}{note}");
    let deeper = format!("{indent}    ");
    match &check.problem {
        None => {}
        Some(Problem::Missing { why_not: Some(why_not) }) => {
            let _ = writeln!(out, "{deeper}It doesn't hold. The closest it gets:");
            for attempt in why_not.attempts.iter().take(2) {
                attempt_block(program, out, attempt, &format!("{deeper}  "));
            }
        }
        Some(Problem::Missing { why_not: None }) => {
            let _ = writeln!(out, "{deeper}Nothing matches it.");
        }
        Some(Problem::Present { facts, proof }) => {
            let more = if facts.len() > 1 { format!(" (and {} more)", facts.len() - 1) } else { String::new() };
            let _ = writeln!(out, "{deeper}But it does hold{more}:");
            tree(program, out, proof, &format!("{deeper}  "), None, &mut HashSet::new());
        }
        Some(Problem::Bypassed { fact, needed, proof, others }) => {
            let _ = writeln!(out, "{deeper}{} holds without {needed}:", program.sentence(fact));
            tree(program, out, proof, &format!("{deeper}  "), None, &mut HashSet::new());
            if !others.is_empty() {
                list_block(program, out, &format!("{deeper}The same goes for: "), others);
            }
        }
        Some(Problem::SinglePoints { fact, points }) => {
            let lead = if points.len() == 1 { "This alone would stop" } else { "Each of these alone would stop" };
            let _ = writeln!(out, "{deeper}{lead} {}:", program.sentence(fact));
            for point in points {
                let _ = writeln!(out, "{deeper}  {}", program.sentence(point));
            }
        }
    }
}

/// The must-sentences of one world, as `run` and `check` print them.
pub fn render_world_intents(program: &Program, world: usize, checks: &[(usize, IntentCheck)]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "▸ {} must:", program.worlds[world].name);
    for (i, check) in checks {
        render_intent(program, &mut out, &program.intents[*i].text, check, "", "  ");
    }
    let broken = checks.iter().filter(|(_, c)| !c.holds).count();
    out.push('\n');
    if broken == 0 {
        let _ = writeln!(out, "  All {} hold.", plural(checks.len(), "must-sentence", "must-sentences"));
    } else {
        let _ = writeln!(out, "  {} of {} broken.", broken, plural(checks.len(), "must-sentence is", "must-sentences are"));
    }
    out
}

fn short_list(out: &mut String, sign: &str, label: &str, items: Vec<String>) {
    const SHOWN: usize = 8;
    for item in items.iter().take(SHOWN) {
        let _ = writeln!(out, "    {sign} {label}{item}");
    }
    if items.len() > SHOWN {
        let _ = writeln!(out, "      … and {} more (ask `diff` for all of them)", items.len() - SHOWN);
    }
}

fn render_review(program: &Program, out: &mut String, review: &Review) {
    let diff = &review.diff;
    if let Some(says) = &review.says {
        let _ = writeln!(out, "  The proposal says: \"{says}\"");
        let _ = writeln!(out, "  That's the author's claim, and nothing checks it. Here is what the change actually does.");
        out.push('\n');
    }
    if !render_changes(program, out, diff, "  It changes:") {
        let _ = writeln!(out, "  It changes nothing.");
    }

    let _ = writeln!(out, "\n  Consequences:");
    let before = out.len();
    let lost: Vec<String> = diff.lost.iter().map(|l| program.sentence(&l.fact)).collect();
    let gained: Vec<String> = diff.gained.iter().map(|g| program.sentence(&g.fact)).collect();
    let counts = |c: &Changed| (c.kept.len() + c.lost.len(), c.kept.len() + c.new.len());
    let weaker: Vec<String> = diff
        .changed
        .iter()
        .filter(|c| counts(c).1 < counts(c).0)
        .map(|c| format!("{}   (reasons {} → {})", program.sentence(&c.fact), counts(c).0, counts(c).1))
        .collect();
    let stronger: Vec<String> = diff
        .changed
        .iter()
        .filter(|c| counts(c).1 > counts(c).0)
        .map(|c| format!("{}   (reasons {} → {})", program.sentence(&c.fact), counts(c).0, counts(c).1))
        .collect();
    // Same number of reasons, different ones: say what it stopped (or started) needing.
    let changed: Vec<String> = diff
        .changed
        .iter()
        .filter(|c| counts(c).1 == counts(c).0)
        .map(|c| {
            let mut detail = Vec::new();
            if !c.no_longer_required.is_empty() {
                let facts: Vec<String> = c.no_longer_required.iter().map(|f| program.sentence(f)).collect();
                detail.push(format!("no longer needs {}", facts.join(", ")));
            }
            if !c.now_required.is_empty() {
                let facts: Vec<String> = c.now_required.iter().map(|f| program.sentence(f)).collect();
                detail.push(format!("now needs {}", facts.join(", ")));
            }
            let detail = if detail.is_empty() { String::new() } else { format!("   ({})", detail.join("; ")) };
            format!("{}{detail}", program.sentence(&c.fact))
        })
        .collect();
    short_list(out, "−", "no longer true: ", lost);
    short_list(out, "+", "newly true: ", gained);
    short_list(out, "~", "weaker: ", weaker);
    short_list(out, "~", "more ways: ", stronger);
    short_list(out, "~", "changed: ", changed);
    if out.len() == before {
        let _ = writeln!(out, "    No conclusions change.");
    }

    let _ = writeln!(out, "\n  Must-sentences:");
    let mut warnings = 0;
    for item in &review.intents {
        let text = &program.intents[item.intent].text;
        let note = match (item.before.holds, item.after.holds) {
            (true, false) => "   ← broken by this change",
            (false, false) => "   (already broken before this change)",
            (false, true) => "   ← fixed by this change",
            (true, true) => "",
        };
        if let (Some((was, now, required)), true) = (&item.weaker, item.after.holds) {
            warnings += 1;
            let _ = writeln!(out, "    ⚠ {text}");
            let _ = writeln!(out, "        Still holds, but for fewer reasons: {was} → {now}.");
            if !required.is_empty() {
                list_block(program, out, "        It now depends on every one of: ", required);
            }
            continue;
        }
        render_intent(program, out, text, &item.after, note, "    ");
    }

    let total = review.intents.len();
    out.push('\n');
    if total == 0 {
        let _ = writeln!(out, "  Verdict: nothing to check. This model has no must-sentences yet.");
    } else if review.broken > 0 {
        let _ = writeln!(
            out,
            "  Verdict: reject. It breaks {} of {}.",
            review.broken,
            plural(total, "must-sentence", "must-sentences")
        );
    } else if warnings > 0 {
        let _ = writeln!(out, "  Verdict: approve, with {}.", plural(warnings, "warning", "warnings"));
    } else {
        let _ = writeln!(out, "  Verdict: approve. Every must-sentence still holds.");
    }
}
