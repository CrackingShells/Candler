//! The transcript of a stored trace: its events, one line each, in the order the file holds them.

use candler_schemas::trace::{SessionEvent, TraceFile, TraceRecord};

/// How a transcript line is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TranscriptForm {
    /// The event as the trace file stores it: one JSON object.
    Json,
    /// The event as one line for a person: session, `seq`, time, kind and the kind's fields.
    Text,
}

/// The line of one event as the trace file stores it.
fn stored_line(event: &SessionEvent) -> Result<String, serde_json::Error> {
    serde_json::to_string(&TraceRecord::Event(event.clone())) // BORROW: the record owns its event
}

/// The line of one event for a person.
fn text_line(event: &SessionEvent) -> String {
    let held = &event.event;
    let at = held
        .at
        .as_ref()
        .map_or_else(|| "-".to_owned(), ToString::to_string);
    let mut fields = serde_json::to_value(&held.kind)
        .ok()
        .and_then(|value| match value {
            serde_json::Value::Object(map) => Some(map),
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_)
            | serde_json::Value::Array(_) => None,
        })
        .unwrap_or_default();
    let kind = fields
        .remove("kind")
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    let mut line = format!("{} {} {at} {kind}", event.session, held.seq);
    if let Some(span) = &held.span {
        line.push_str(" span=");
        line.push_str(span.to_string().as_str());
    }
    if !fields.is_empty() {
        line.push(' ');
        line.push_str(&serde_json::Value::Object(fields).to_string());
    }
    line
}

/// One line per event of `trace`, in the stored order, none dropped. A JSON line is the event
/// serialised as the trace file writes it.
///
/// # Errors
/// Returns the serialiser's error when an event cannot be written as JSON.
pub fn render_transcript(
    trace: &TraceFile,
    form: TranscriptForm,
) -> Result<Vec<String>, serde_json::Error> {
    trace
        .events()
        .iter()
        .map(|event| match form {
            TranscriptForm::Json => stored_line(event),
            TranscriptForm::Text => Ok(text_line(event)),
        })
        .collect()
}
