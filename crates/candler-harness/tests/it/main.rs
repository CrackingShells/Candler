//! The integration tests of this crate, one binary: one module per former test file.
#![cfg(test)]

mod support;

#[cfg(feature = "conformance")]
mod conformance_shapes;
mod guard_two_shapes;
mod guard_unsupported_setting;
