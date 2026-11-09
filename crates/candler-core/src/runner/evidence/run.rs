//! What the run's own end measured: how it ended, its exit, its length, what the result frame
//! says it spent and the tools it called.

use std::collections::BTreeMap;

use candler_schemas::trace::{EndedBy, EventKind, SessionEvent};
use candler_schemas::values::MeasuredValue;
use serde_json::{Map, Number, Value};

use super::measure::measured;

/// The preflight step the run values name as their source.
const STEP: &str = "run@1";

/// What a finished attempt gives the measurement.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct RunFacts<'a> {
    /// How the session ended.
    pub ended_by: &'a EndedBy,
    /// The client's exit code; none when a signal ended it.
    pub client_exit: Option<i32>,
    /// Milliseconds from the start of the attempt to the end of the client.
    pub duration_ms: u64,
    /// The events of the trace, in feed order.
    pub events: &'a [SessionEvent],
}

/// The last result line of the trace: the keys of the last event that is a `result` frame.
fn result_frame(events: &[SessionEvent]) -> Option<&Map<String, Value>> {
    events
        .iter()
        .rev()
        .map(|entry| &entry.event.ext)
        .find(|ext| ext.get("type").and_then(Value::as_str) == Some("result"))
}

/// The tools the stream called, by name, with the number of calls of each.
fn tool_calls(events: &[SessionEvent]) -> Map<String, Value> {
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    for entry in events {
        if let EventKind::ToolCall { name, .. } = &entry.event.kind {
            // BORROW: .as_str() keys the count by the name the event holds
            let seen = counts.entry(name.as_str()).or_insert(0);
            *seen = seen.saturating_add(1);
        }
    }
    counts
        .into_iter()
        // BORROW: .to_owned() gives the map its own names
        .map(|(name, count)| (name.to_owned(), Value::Number(Number::from(count))))
        .collect()
}

/// The `run.*` values of an attempt: `run.ended_by`, `run.exit`, `run.duration_ms`, `run.usage`,
/// `run.cost_usd`, `run.num_turns` and `run.tool_calls`. A value the run did not give is left
/// out: a signalled client has no exit, and a run with no result frame has no usage, cost or turn
/// count.
#[must_use]
pub fn run_values(facts: &RunFacts<'_>) -> Vec<MeasuredValue> {
    let frame = result_frame(facts.events);
    let from_frame = |key: &str| frame.and_then(|line| line.get(key)).cloned();
    let entries = [
        ("run.ended_by", serde_json::to_value(facts.ended_by).ok()),
        (
            "run.exit",
            facts
                .client_exit
                .map(|code| Value::Number(Number::from(code))),
        ),
        (
            "run.duration_ms",
            Some(Value::Number(Number::from(facts.duration_ms))),
        ),
        ("run.usage", from_frame("usage")),
        ("run.cost_usd", from_frame("total_cost_usd")),
        ("run.num_turns", from_frame("num_turns")),
        (
            "run.tool_calls",
            Some(Value::Object(tool_calls(facts.events))),
        ),
    ];
    entries
        .into_iter()
        .filter_map(|(path, value)| measured(STEP, path, value?))
        .collect()
}
