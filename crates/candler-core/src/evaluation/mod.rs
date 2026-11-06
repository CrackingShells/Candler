//! Evaluation of stored runs by plug-in evaluators: the plug-in registry in [`registry`], the run
//! context in [`context`], the protocol client in [`client`], selection, rates and the version diff
//! in [`selection`], and the engine that composes them in [`engine`]. The registry, the client and
//! the engine spawn through [`crate::supervise`], so they are built on Unix only, as it is.

#[cfg(unix)]
pub mod client;
pub mod context;
#[cfg(unix)]
pub mod engine;
#[cfg(unix)]
pub mod registry;
pub mod selection;
