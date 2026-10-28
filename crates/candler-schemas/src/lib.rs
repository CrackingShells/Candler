//! Candler wire schemas: the string formats every record shares, and the generation of the
//! committed JSON Schema files with the check that keeps them in step with the structs.

pub mod catalog;
pub mod generate;
mod map_only;
pub mod outcome;
pub mod primitives;
