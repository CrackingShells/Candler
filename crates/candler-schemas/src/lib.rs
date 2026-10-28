//! Candler wire schemas: the string formats every record shares in [`primitives`], the outcome
//! record in [`outcome`], the tool-catalog snapshot in [`catalog`], and in [`generate`] the JSON
//! Schema files generated from these types, committed under `schema/`, with the check that keeps
//! them in step with the types.
//!
//! JSON text that holds an unpaired surrogate escape, such as `\ud800`, is refused by the Rust
//! reader.

pub mod catalog;
pub mod generate;
mod map_only;
pub mod outcome;
pub mod primitives;
