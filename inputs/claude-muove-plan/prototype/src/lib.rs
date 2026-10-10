//! Muove: readable rules you can question.
//!
//! A program states facts as sentences, derives conclusions with rules, and
//! states what must stay true. It answers questions about all of it in the
//! same sentences:
//!
//! - `find`: what matches this pattern?
//! - `why`: every minimal set of stated facts a conclusion follows from;
//! - `why not`: for something false, how close each rule gets and what's missing;
//! - `prevent`: the minimal withdrawals that would make it false;
//! - `achieve`: the minimal additions that would make it true;
//! - `diff`: what an edit changed, including conclusions that stayed true
//!   but lost (or gained) reasons;
//! - `review`: a proposed change, checked against every must-sentence.
//!
//! Every search is bounded and says when it ran out of budget.

pub mod ask;
pub mod engine;
pub mod program;
pub mod render;
pub mod syntax;

pub use ask::{Answer, answer};
pub use engine::{Limits, Status};
pub use program::{Fact, Program};
pub use syntax::Error;

/// Answer every question in a source file, then check its must-sentences.
pub fn run(source: &str, limits: &Limits) -> Result<String, Error> {
    let program = Program::parse(source)?;
    let mut out = String::new();
    for item in &program.requests {
        if !out.is_empty() {
            out.push('\n');
        }
        let result = answer(&program, &item.request, limits);
        out.push_str(&render::render(&program, item, &result));
    }
    let (intents, _) = intents_report(&program, limits);
    if !intents.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&intents);
    }
    Ok(out)
}

/// Check every model's must-sentences in every world: models in full,
/// revisions only where something breaks. Returns the text and whether all hold.
pub fn intents_report(program: &Program, limits: &Limits) -> (String, bool) {
    let mut out = String::new();
    let mut all_hold = true;
    for world in 0..program.worlds.len() {
        let checks = ask::check_world(program, world, limits);
        if checks.is_empty() {
            continue;
        }
        let broken = checks.iter().any(|(_, c)| !c.holds);
        all_hold &= !broken;
        if program.worlds[world].base.is_none() || broken {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&render::render_world_intents(program, world, &checks));
        }
    }
    (out, all_hold)
}

/// Review every revision in `proposal` against the program in `base`.
/// Returns the review text and whether every proposal can be approved.
pub fn review(base: &str, proposal: &str, limits: &Limits) -> Result<(String, bool), Error> {
    let base = base.trim_end_matches('\n');
    let offset = base.lines().count() + 1;
    let combined = format!("{base}\n\n{proposal}");
    let program = Program::parse(&combined).map_err(|mut e| {
        if e.line > offset {
            e.line -= offset;
            e.message = format!("in the proposal: {}", e.message);
        }
        e
    })?;
    let proposed: Vec<usize> = (0..program.worlds.len())
        .filter(|&w| program.worlds[w].base.is_some() && program.worlds[w].line > offset)
        .collect();
    if proposed.is_empty() {
        return Err(Error::bare("the proposal defines no revision to review"));
    }
    let mut out = String::new();
    let mut approved = true;
    for successor in proposed {
        let base = program.worlds[successor].base.unwrap();
        let result = ask::review(&program, base, successor, limits);
        approved &= result.broken == 0;
        let item = program::RequestItem {
            line: 0,
            source: vec![format!("review {} -> {}", program.worlds[base].name, program.worlds[successor].name)],
            request: program::Request::Review { base, successor },
        };
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&render::render(&program, &item, &Answer::Review(result)));
    }
    Ok((out, approved))
}
