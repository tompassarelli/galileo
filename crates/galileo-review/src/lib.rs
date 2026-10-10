mod baseline;
mod patch;
mod truth;

pub use baseline::{identity, Baseline, Identity};
pub use patch::{parse, Declaration, Edit, Patch};
pub use truth::{truth, TruthChange};
