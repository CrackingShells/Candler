//! The census of a run: the tools and MCP servers the agent saw at its start, as the harness's
//! driver reports them.

use candler_harness::spec::InitReport;
use candler_schemas::primitives::{FieldPath, NameToken};
use candler_schemas::values::{JsonValue, MeasuredValue, ValueSource};
use serde_json::Value;

/// The preflight step the census values name as their source.
const STEP: &str = "census@1";

/// Why the census could not be taken.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CensusError {
    /// The harness reported no inventory.
    #[error("the harness reported no inventory")]
    NoInit,
    /// The inventory's tool list is not a list of names, or its server list is not a list.
    #[error("the inventory has no usable `{key}`")]
    Malformed {
        /// The name of the list.
        key: &'static str,
    },
    /// A census value could not be built.
    #[error("a census value is not valid: {detail}")]
    Value {
        /// What is wrong.
        detail: String,
    },
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

/// The census values of a run, from the inventory its harness reported: `census.tools` and, when
/// the source sees servers, `census.mcp_servers`, as the harness lists them, and
/// `census.init_tools`, the tool names sorted and deduplicated. The server list never enters
/// `census.init_tools`.
///
/// # Errors
/// [`CensusError::NoInit`] when the harness reported no inventory, which records nothing, and
/// [`CensusError::Malformed`] when the inventory's `tools` is not a list of names or its
/// `mcp_servers` is not a list.
pub fn census(init: Option<&InitReport>) -> Result<Vec<MeasuredValue>, CensusError> {
    let init = init.ok_or(CensusError::NoInit)?;
    let tools = init
        .tools
        .as_array()
        .ok_or(CensusError::Malformed { key: "tools" })?;
    let mut names: Vec<String> = Vec::new();
    for tool in tools {
        // BORROW: `.as_str()` borrows the text of the JSON string
        let name = tool
            .as_str()
            .ok_or(CensusError::Malformed { key: "tools" })?;
        names.push(name.to_owned()); // BORROW: .to_owned() gives the sorted list its own names
    }
    names.sort();
    names.dedup();
    let mut values = vec![measured("census.tools", init.tools.clone())?]; // BORROW: .clone() keeps the harness's list
    if let Some(servers) = &init.mcp_servers {
        if !servers.is_array() {
            return Err(CensusError::Malformed { key: "mcp_servers" });
        }
        values.push(measured("census.mcp_servers", servers.clone())?); // BORROW: .clone() keeps the harness's list
    }
    values.push(measured(
        "census.init_tools",
        Value::Array(names.into_iter().map(Value::String).collect()),
    )?);
    Ok(values)
}
