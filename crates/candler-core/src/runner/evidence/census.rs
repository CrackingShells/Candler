//! The census of a run: the tools and MCP servers the agent saw at its start.

use candler_schemas::primitives::{FieldPath, NameToken};
use candler_schemas::trace::{Session, SessionEvent};
use candler_schemas::values::{JsonValue, MeasuredValue, ValueSource};
use serde_json::Value;

/// The preflight step the census values name as their source.
const STEP: &str = "census@1";

/// Why the census could not be taken.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CensusError {
    /// The first session has no init frame.
    #[error("the first session has no init frame")]
    NoInit,
    /// The init frame lacks a list, or the list holds an entry of the wrong kind.
    #[error("the init frame has no usable `{key}`")]
    Malformed {
        /// The key of the init frame.
        key: &'static str,
    },
    /// A census value could not be built.
    #[error("a census value is not valid: {detail}")]
    Value {
        /// What is wrong.
        detail: String,
    },
}

/// Whether `ext`, the keys of a line, is an init line.
fn is_init(ext: &serde_json::Map<String, Value>) -> bool {
    ext.get("type").and_then(Value::as_str) == Some("system")
        && ext.get("subtype").and_then(Value::as_str) == Some("init")
}

/// One census value.
fn measured(path: &str, value: Value) -> Result<MeasuredValue, CensusError> {
    let value_error = |detail: String| CensusError::Value { detail };
    let path = path
        .parse::<FieldPath>()
        .map_err(|error| value_error(error.to_string()))?; // BORROW: .to_string() gives owned text
    let step = STEP
        .parse::<NameToken>()
        .map_err(|error| value_error(error.to_string()))?; // BORROW: .to_string() gives owned text
    let value = JsonValue::new(value).map_err(|error| value_error(error.to_string()))?; // BORROW: .to_string() gives owned text
    Ok(MeasuredValue::new(path, value, ValueSource::derived(step)))
}

/// The three census values of a run, from the first init of its first session: `census.tools`
/// and `census.mcp_servers` as the frame lists them, and `census.init_tools`, the tool names
/// sorted and deduplicated. The server list never enters `census.init_tools`.
///
/// # Errors
/// [`CensusError::NoInit`] when the first session has no init frame, and
/// [`CensusError::Malformed`] when the frame's `tools` is not a list of names or its
/// `mcp_servers` is not a list.
pub fn census(
    sessions: &[Session],
    events: &[SessionEvent],
) -> Result<Vec<MeasuredValue>, CensusError> {
    let first = sessions.first().ok_or(CensusError::NoInit)?;
    let init = events
        .iter()
        .filter(|entry| entry.session == *first.id())
        .map(|entry| &entry.event.ext)
        .find(|ext| is_init(ext))
        .ok_or(CensusError::NoInit)?;
    let tools = init
        .get("tools")
        .filter(|tools| tools.is_array())
        .ok_or(CensusError::Malformed { key: "tools" })?;
    let servers = init
        .get("mcp_servers")
        .filter(|servers| servers.is_array())
        .ok_or(CensusError::Malformed { key: "mcp_servers" })?;
    let mut names: Vec<String> = Vec::new();
    for tool in tools.as_array().into_iter().flatten() {
        let name = tool
            .as_str()
            .ok_or(CensusError::Malformed { key: "tools" })?;
        names.push(name.to_owned()); // BORROW: .to_owned() gives the sorted list its own names
    }
    names.sort();
    names.dedup();
    Ok(vec![
        measured("census.tools", tools.clone())?, // BORROW: .clone() keeps the frame's list
        measured("census.mcp_servers", servers.clone())?, // BORROW: .clone() keeps the frame's list
        measured(
            "census.init_tools",
            Value::Array(names.into_iter().map(Value::String).collect()),
        )?,
    ])
}
