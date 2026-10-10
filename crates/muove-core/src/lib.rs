#![forbid(unsafe_code)]

pub mod check;
pub mod claims;
pub mod engine;
pub mod eval;
pub mod examples;
pub mod supports;
pub mod ir;
pub mod why;

pub use claims::{check_claims, exit_code, ClaimError, ClaimResult, ClaimVerdict, Evidence, Instance};
pub use check::{check, CheckedProgram, Dependency, DependencyKind, StaticError, StaticErrorKind};
pub use eval::{evaluate, evaluate_blocking, Budget, EvalError, Model, Reason, Resource, Tuples, Verdict};
pub use ir::*;
