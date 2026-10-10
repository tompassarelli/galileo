#![forbid(unsafe_code)]

pub mod check;
pub mod eval;
pub mod examples;
pub mod ir;

pub use check::{check, CheckedProgram, Dependency, DependencyKind, StaticError, StaticErrorKind};
pub use eval::{evaluate, Budget, EvalError, Model, Reason, Resource, Tuples, Verdict};
pub use ir::*;
