//! What the run's own end measured: how it ended, its exit, its length, what the harness reports
//! it spent and the tools it called.

use std::collections::BTreeMap;

use candler_harness::spec::RunTotals;
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
    /// What the live reader reports the run spent.
    pub totals: &'a RunTotals,
}

impl<'a> RunFacts<'a> {
    /// The facts of a finished attempt.
    #[must_use]
    pub const fn new(
        ended_by: &'a EndedBy,
        client_exit: Option<i32>,
        duration_ms: u64,
        events: &'a [SessionEvent],
        totals: &'a RunTotals,
    ) -> Self {
        Self {
            ended_by,
            client_exit,
            duration_ms,
            events,
            totals,
        }
    }
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
/// out: a signalled client has no exit, and a run whose harness reported no totals has no usage,
/// cost or turn count.
#[must_use]
pub fn run_values(facts: &RunFacts<'_>) -> Vec<MeasuredValue> {
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
        ("run.usage", facts.totals.usage.clone()), // BORROW: the value owns its copy
        ("run.cost_usd", facts.totals.cost_usd.clone()), // BORROW: the value owns its copy
        ("run.num_turns", facts.totals.num_turns.clone()), // BORROW: the value owns its copy
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
