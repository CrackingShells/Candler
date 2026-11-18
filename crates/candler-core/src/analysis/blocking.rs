//! The blocks of a comparison: the level combinations of the declared factors other than the
//! compared one.

use candler_schemas::primitives::FieldPath;
use serde_json::Value;

/// One block of a comparison between arm A and arm B.
#[derive(Debug, Clone, PartialEq)]
pub struct Stratum {
    /// The level of each declared factor other than the compared one, which names the block.
    pub key: Vec<(FieldPath, Value)>,
    /// Arm A's passes and runs in this block.
    pub a: (u64, u64),
    /// Arm B's passes and runs in this block.
    pub b: (u64, u64),
}
