//! Resolution of an experiment: project and user configuration, layered values with their sources, and preflight steps.

pub mod config;
pub mod layers;
pub mod preflight;
pub(crate) mod refusal;
#[cfg(unix)]
pub mod steps;
