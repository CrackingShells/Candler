//! Token counts of a usage record, and their sums.

use candler_schemas::outcome::Finite;
use candler_schemas::trace::{EventKind, UsageScope};
use serde_json::Value;

/// The four token counts of a usage record.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Tokens {
    /// Input tokens.
    input: u64,
    /// Output tokens.
    output: u64,
    /// Input tokens served from the cache.
    cache_read: u64,
    /// Input tokens written to the cache.
    cache_creation: u64,
}

impl Tokens {
    /// The counts of a `usage` object; a count that is missing or not a non-negative integer is 0.
    pub(crate) fn from_usage(usage: &Value) -> Self {
        let count = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
        Self {
            input: count("input_tokens"),
            output: count("output_tokens"),
            cache_read: count("cache_read_input_tokens"),
            cache_creation: count("cache_creation_input_tokens"),
        }
    }

    /// The counts of `self` and `other` added, each saturating at the largest count.
    pub(crate) const fn plus(self, other: Self) -> Self {
        Self {
            input: self.input.saturating_add(other.input),
            output: self.output.saturating_add(other.output),
            cache_read: self.cache_read.saturating_add(other.cache_read),
            cache_creation: self.cache_creation.saturating_add(other.cache_creation),
        }
    }

    /// A usage event of `scope` holding these counts and `cost`.
    pub(crate) const fn event(self, scope: UsageScope, cost: Option<Finite>) -> EventKind {
        EventKind::usage(
            scope,
            self.input,
            self.output,
            self.cache_read,
            self.cache_creation,
            cost,
        )
    }
}
