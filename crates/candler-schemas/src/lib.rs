//! Candler wire schemas: the string formats every record shares in [`primitives`], the outcome
//! record in [`outcome`], the tool-catalog snapshot in [`catalog`], resolved values in [`values`],
//! canonical JSON in [`canonical`], process records in [`process`], container records in
//! [`container`], raw frames in [`raw`], the normalised trace in [`trace`], execution records in
//! [`execution`], evaluator protocol records in [`evaluation`], and in [`generate`] the JSON Schema
//! files generated from these types, committed under `schema/`, with the check that keeps them in
//! step with the types.
//!
//! JSON text that holds an unpaired surrogate escape, such as `\ud800`, is refused by the Rust
//! reader.

pub mod canonical;
pub mod catalog;
pub mod container;
pub mod evaluation;
pub mod execution;
pub mod generate;
mod map_only;
pub mod outcome;
pub mod primitives;
pub mod process;
pub mod raw;
pub mod trace;
pub mod values;
