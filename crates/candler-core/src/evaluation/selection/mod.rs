//! Outcome selection with the I4 check in [`select`], the four compared outcome fields in
//! [`fields`], per-cell rates in [`rates`], and the diff of two evaluator versions in [`diff`].
//! The module reads outcome records only; it names no domain, no suite and no expectation.

pub mod diff;
pub mod fields;
pub mod rates;
pub mod select;
