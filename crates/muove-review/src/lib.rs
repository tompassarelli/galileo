mod baseline;
mod patch;

pub use baseline::{identity, Baseline, Identity};
pub use patch::{parse, Declaration, Edit, Patch};
