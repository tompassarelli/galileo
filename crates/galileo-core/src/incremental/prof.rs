use std::cell::RefCell;
use std::time::Instant;

pub const N: usize = 18;
pub const NAMES: [&str; N] = ["step_other", "recompute_other", "clone_live", "stated", "delta_strata", "restate_delta", "dred_maintain", "restate_dred", "dred_insert", "rr_copy", "rr_eval", "rr_writeback", "mode_failures", "changed_pass", "head_delta", "step_start", "index_build", "aggregate_update"];
pub const STEP_OTHER: usize = 0;
pub const RECOMPUTE_OTHER: usize = 1;
pub const CLONE_LIVE: usize = 2;
pub const STATED: usize = 3;
pub const DELTA: usize = 4;
pub const RESTATE_DELTA: usize = 5;
pub const DRED_MAINTAIN: usize = 6;
pub const RESTATE_DRED: usize = 7;
pub const DRED_INSERT: usize = 8;
pub const RR_COPY: usize = 9;
pub const RR_EVAL: usize = 10;
pub const RR_WRITEBACK: usize = 11;
pub const MODE: usize = 12;
pub const CHANGED_PASS: usize = 13;
pub const HEAD_DELTA: usize = 14;
pub const STEP_START: usize = 15;
pub const INDEX: usize = 16;
pub const AGG: usize = 17;

#[derive(Default)]
struct Prof {
    ns: [u128; N],
    rows: [u64; N],
    calls: [u64; N],
    stack: Vec<(usize, Option<Instant>, u128)>,
    strata: Vec<String>,
}

thread_local! { static P: RefCell<Prof> = RefCell::new(Prof::default()); }

pub struct G;

pub fn enter(phase: usize) -> G {
    P.with(|p| p.borrow_mut().stack.push((phase, Some(Instant::now()), 0)));
    G
}

impl Drop for G {
    fn drop(&mut self) {
        P.with(|p| {
            let mut p = p.borrow_mut();
            let (phase, start, child) = p.stack.pop().unwrap();
            let elapsed = start.unwrap().elapsed().as_nanos();
            p.ns[phase] += elapsed - child;
            p.calls[phase] += 1;
            if let Some(top) = p.stack.last_mut() { top.2 += elapsed; }
        })
    }
}

pub const P_TOP: usize = usize::MAX;
pub fn rows(phase: usize, n: u64) { P.with(|p| { let mut p = p.borrow_mut(); let phase = if phase == P_TOP { p.stack.last().map_or(0, |t| t.0) } else { phase }; p.rows[phase] += n; }); }

pub fn stratum(line: String) { P.with(|p| p.borrow_mut().strata.push(line)); }

pub fn take() -> ([u128; N], [u64; N], [u64; N], Vec<String>) {
    P.with(|p| {
        let mut p = p.borrow_mut();
        let out = (p.ns, p.rows, p.calls, std::mem::take(&mut p.strata));
        p.ns = [0; N]; p.rows = [0; N]; p.calls = [0; N];
        out
    })
}
