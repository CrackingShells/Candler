//! Evaluation of stored runs by plug-in evaluators: the plug-in registry in [`registry`], the run
//! context in [`context`], the protocol client in [`client`], selection, rates and the version diff
//! in [`selection`], and the engine that composes them in [`engine`].

#[cfg(unix)]
pub mod client;
pub mod context;
pub mod engine;
pub mod registry;
pub mod selection;
