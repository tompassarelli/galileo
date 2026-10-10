#![forbid(unsafe_code)]

pub mod check;
pub mod examples;
pub mod ir;

pub use check::{check, CheckedProgram, Dependency, DependencyKind, StaticError, StaticErrorKind};
pub use ir::*;
