use std::cell::RefCell;
use std::time::Instant;

pub const N: usize = 20;
pub const NAMES: [&str; N] = ["step_total", "match", "touched_modes", "ensures", "advance", "invariant", "run_total", "apply", "history", "hash", "log_push", "commit", "change_extend", "create", "actions", "change", "post_live", "checks", "tail", "undo_drop"];
pub const STEP_TOTAL: usize = 0;
pub const MATCH: usize = 1;
pub const TOUCHED: usize = 2;
pub const ENSURES: usize = 3;
pub const ADVANCE: usize = 4;
pub const INVARIANT: usize = 5;
pub const RUN_TOTAL: usize = 6;
pub const APPLY: usize = 7;
pub const HISTORY: usize = 8;
pub const HASH: usize = 9;
pub const LOG: usize = 10;
pub const COMMIT: usize = 11;
pub const EXTEND: usize = 12;
pub const CREATE: usize = 13;
pub const ACTIONS: usize = 14;
pub const CHANGE: usize = 15;
pub const LIVE: usize = 16;
pub const CHECKS: usize = 17;
pub const TAIL: usize = 18;
pub const UNDO: usize = 19;

thread_local! { static P: RefCell<[u128; N]> = const { RefCell::new([0; N]) }; }

pub struct G(usize, Instant);

pub fn enter(part: usize) -> G { G(part, Instant::now()) }

impl Drop for G {
    fn drop(&mut self) {
        let elapsed = self.1.elapsed().as_nanos();
        P.with(|p| p.borrow_mut()[self.0] += elapsed);
    }
}

pub fn take() -> [u128; N] { P.with(|p| std::mem::replace(&mut *p.borrow_mut(), [0; N])) }
