//! What a session records about how the agent stopped: its turns, when it last yielded, what was
//! pending then and which way the run ended.
//!
//! The values are `measured[]` entries under `session.*`. A value the run did not give is left
//! out, and an absent entry reads as unrecorded: a pending list that could not be read is never
//! recorded as an empty one.

use candler_harness::spec::{PendingItem, PendingKind};
use candler_schemas::values::MeasuredValue;
use serde_json::{Map, Number, Value};

use super::measure::measured;

/// The preflight step the session values name as their source.
const STEP: &str = "session@1";

/// How a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SessionEnd {
    /// The runner closed the input after the agent had yielded, and the harness exited.
    Closed,
    /// A cap of the cell ended the run: the turn limit across the session, or the wall.
    Cap,
    /// The harness's own dollar budget ended the work.
    Budget,
    /// The harness exited before the runner closed the input.
    HarnessExit,
}

impl SessionEnd {
    /// The text recorded at `session.end`.
    #[must_use]
    pub const fn as_text(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Cap => "cap",
            Self::Budget => "budget",
            Self::HarnessExit => "harness_exit",
        }
    }
}

/// How long a session stays open after the agent yields with nothing pending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Idle {
    /// This many whole seconds.
    Seconds(u64),
    /// Never: only a cap, the budget or the harness's own exit ends the session.
    Never,
}

/// What the runner observed of a session, as the measures read it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SessionMeasures {
    /// The effective `runner.idle_s`.
    pub idle: Idle,
    /// The turns that ended (`result` events of the harness's stream).
    pub turns: u64,
    /// Milliseconds from the start of the attempt to the end of the last turn; none when no turn
    /// ended.
    pub yielded_at_ms: Option<u64>,
    /// What was pending at the end of the run; none when it could not be read.
    pub pending_at_end: Option<Vec<PendingItem>>,
    /// How the session ended; none when the run is not the arm's own end (the experimenter
    /// stopped it, or it failed).
    pub end: Option<SessionEnd>,
    /// Whether the harness had to be stopped after the input was closed.
    pub close_forced: bool,
}

impl SessionMeasures {
    /// The measures of a session.
    #[must_use]
    pub const fn new(
        idle: Idle,
        turns: u64,
        yielded_at_ms: Option<u64>,
        pending_at_end: Option<Vec<PendingItem>>,
        end: Option<SessionEnd>,
        close_forced: bool,
    ) -> Self {
        Self {
            idle,
            turns,
            yielded_at_ms,
            pending_at_end,
            end,
            close_forced,
        }
    }
}

/// The text of a kind of pending work.
const fn kind_text(kind: PendingKind) -> &'static str {
    match kind {
        PendingKind::Task => "task",
        PendingKind::Monitor => "monitor",
        PendingKind::Cron => "cron",
        PendingKind::WakeUp => "wake_up",
        // EXPLICIT: a kind a newer harness crate adds is recorded as other, never dropped
        _ => "other",
    }
}

/// The `session.*` values of a run: `session.idle_s`, `session.turns`, `session.yielded_at_ms`,
/// `session.pending_at_end` (a list of `{kind, id}`), `session.end` and `session.close_forced`.
#[must_use]
pub fn session_values(facts: &SessionMeasures) -> Vec<MeasuredValue> {
    let pending = facts.pending_at_end.as_ref().map(|items| {
        Value::Array(
            items
                .iter()
                .map(|item| {
                    let mut entry = Map::new();
                    entry.insert(
                        "kind".to_owned(),                              // BORROW: the entry owns its key
                        Value::String(kind_text(item.kind).to_owned()), // BORROW: owned text
                    );
                    entry.insert("id".to_owned(), Value::String(item.id.clone())); // BORROW: the entry owns its id
                    Value::Object(entry)
                })
                .collect(),
        )
    });
    let idle = match facts.idle {
        Idle::Seconds(seconds) => Value::Number(Number::from(seconds)),
        Idle::Never => Value::String("none".to_owned()), // BORROW: owned text
    };
    let entries = [
        ("session.idle_s", Some(idle)),
        (
            "session.turns",
            Some(Value::Number(Number::from(facts.turns))),
        ),
        (
            "session.yielded_at_ms",
            facts
                .yielded_at_ms
                .map(|ms| Value::Number(Number::from(ms))),
        ),
        ("session.pending_at_end", pending),
        (
            "session.end",
            facts.end.map(|end| Value::String(end.as_text().to_owned())), // BORROW: owned text
        ),
        (
            "session.close_forced",
            Some(Value::Bool(facts.close_forced)),
        ),
    ];
    entries
        .into_iter()
        .filter_map(|(path, value)| measured(STEP, path, value?))
        .collect()
}
