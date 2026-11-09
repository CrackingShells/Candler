//! One `measured[]` entry, built from a step name, a dotted path and a JSON value.

use candler_schemas::primitives::{FieldPath, NameToken};
use candler_schemas::values::{JsonValue, MeasuredValue, ValueSource};
use serde_json::Value;

/// The entry at `path` holding `value`, derived by the step `step`. A value that cannot be
/// measured (`null`, or a number outside the canonical range) gives no entry: an absent entry
/// reads as unrecorded.
pub(super) fn measured(step: &str, path: &str, value: Value) -> Option<MeasuredValue> {
    let path = path.parse::<FieldPath>().ok()?;
    let step = step.parse::<NameToken>().ok()?;
    let value = JsonValue::new(value).ok()?;
    Some(MeasuredValue::new(path, value, ValueSource::derived(step)))
}
