//! The surface of an experiment: the values a cell is made of.
//!
//! The surface is the part of a cell the agent's run is configured from: the model, the tools, the
//! MCP servers, the permission mode, the prompt and the isolation limits. A setting is not part of
//! it: `request.*`, `model.token_secret` and `isolation.build.*` are never read here.

use std::str::FromStr;

use candler_harness::spec::Cell;
use candler_schemas::primitives::{FieldPath, FormatError};
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::{Map, Value};

use candler_schemas::execution::paths;

/// The roots that preflight steps derive into typed fields; no value sits below one.
const RESERVED_ROOTS: [&str; 5] = [
    "harness",
    "treatment",
    "snapshots",
    "inputs",
    "environments",
];

/// Why a cell has no surface.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SurfaceError {
    /// A required path has no value.
    #[error("the cell has no value at `{0}`")]
    Missing(FieldPath),
    /// A value is not of the JSON type its path takes.
    #[error("the value at `{path}` must be {expected}")]
    Type {
        /// The path.
        path: FieldPath,
        /// What the path takes.
        expected: &'static str,
    },
    /// A value sits below a reserved root, which takes one value.
    #[error("the value at `{path}` sits below a reserved root")]
    ReservedRoot {
        /// The path.
        path: FieldPath,
    },
    /// A value holds an absolute path as a string.
    #[error("the value at `{path}` holds an absolute path")]
    AbsolutePath {
        /// The path.
        path: FieldPath,
    },
    /// A path of the surface is malformed.
    #[error(transparent)]
    Format(#[from] FormatError),
}

/// The configuration surface of one cell.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Surface {
    /// `model.id`.
    pub model_id: String,
    /// `model.endpoint`: the gateway's upstream, never the agent's.
    pub model_endpoint: String,
    /// `model.effort`, when set.
    pub model_effort: Option<String>,
    /// `tools.allowed`; empty when unset.
    pub tools_allowed: Vec<String>,
    /// `tools.disallowed`; empty when unset.
    pub tools_disallowed: Vec<String>,
    /// `mcp`: the server configuration, when set.
    pub mcp: Option<Map<String, Value>>,
    /// `permission_mode`.
    pub permission_mode: String,
    /// `prompt.template`.
    pub prompt_template: String,
    /// `prompt.values`; empty when unset.
    pub prompt_values: Map<String, Value>,
    /// `isolation.network`, when set.
    pub network: Option<String>,
    /// `isolation.limits.pids`, when set.
    pub pids_limit: Option<u32>,
    /// `isolation.limits.memory`, when set.
    pub memory: Option<String>,
    /// `isolation.limits.cpus`, when set.
    pub cpus: Option<f64>,
}

impl Surface {
    /// Reads the surface of `cell`.
    ///
    /// A cell is read by path, so a value below a reserved root cannot be found here; use
    /// [`Surface::from_values`] on the resolved values to refuse one.
    ///
    /// # Errors
    /// [`SurfaceError::Missing`] when `model.id`, `model.endpoint`, `permission_mode` or
    /// `prompt.template` has no value, [`SurfaceError::Type`] for a value of the wrong JSON type
    /// and [`SurfaceError::AbsolutePath`] for a value that holds an absolute path.
    pub fn from_cell(cell: &Cell<'_>) -> Result<Self, SurfaceError> {
        Self::read(|path| cell.get(path))
    }

    /// Reads the surface of resolved values, after refusing any value below a reserved root.
    ///
    /// # Errors
    /// [`SurfaceError::ReservedRoot`] for a value below `harness`, `treatment`, `snapshots`,
    /// `inputs` or `environments`, and the errors of [`Surface::from_cell`].
    pub fn from_values(values: &[ResolvedValue]) -> Result<Self, SurfaceError> {
        for value in values {
            let mut segments = value.path().segments();
            let below = segments
                .next()
                .is_some_and(|root| RESERVED_ROOTS.contains(&root))
                && segments.next().is_some();
            if below {
                return Err(SurfaceError::ReservedRoot {
                    // BORROW: `.clone()` makes the owned path the error carries
                    path: value.path().clone(),
                });
            }
        }
        Self::read(|path| {
            values
                .iter()
                .find(|resolved| resolved.path() == path)
                .and_then(ResolvedValue::value)
        })
    }

    fn read<'a, F>(get: F) -> Result<Self, SurfaceError>
    where
        F: Fn(&FieldPath) -> Option<&'a JsonValue>,
    {
        let reader = Reader { get };
        let pids_limit = match reader.value(paths::ISOLATION_LIMITS_PIDS)? {
            Some((path, value)) => Some(value.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or(
                SurfaceError::Type {
                    path,
                    expected: "an integer that fits in 32 bits",
                },
            )?),
            None => None,
        };
        let cpus = match reader.value(paths::ISOLATION_LIMITS_CPUS)? {
            Some((path, value)) => Some(value.as_f64().ok_or(SurfaceError::Type {
                path,
                expected: "a number",
            })?),
            None => None,
        };
        Ok(Self {
            model_id: reader.required_text(paths::MODEL_ID)?,
            model_endpoint: reader.required_text(paths::MODEL_ENDPOINT)?,
            model_effort: reader.text(paths::MODEL_EFFORT)?,
            tools_allowed: reader.texts(paths::TOOLS_ALLOWED)?,
            tools_disallowed: reader.texts(paths::TOOLS_DISALLOWED)?,
            mcp: reader.object(paths::MCP)?,
            permission_mode: reader.required_text(paths::PERMISSION_MODE)?,
            prompt_template: reader.required_text(paths::PROMPT_TEMPLATE)?,
            prompt_values: reader.object(paths::PROMPT_VALUES)?.unwrap_or_default(),
            network: reader.text(paths::ISOLATION_NETWORK)?,
            pids_limit,
            memory: reader.text(paths::ISOLATION_LIMITS_MEMORY)?,
            cpus,
        })
    }
}

/// Reads typed values by path from a lookup, refusing absolute paths.
struct Reader<F> {
    get: F,
}

impl<'a, F> Reader<F>
where
    F: Fn(&FieldPath) -> Option<&'a JsonValue>,
{
    /// The value at `text`, with its path, when there is one.
    fn value(&self, text: &str) -> Result<Option<(FieldPath, &'a Value)>, SurfaceError> {
        let path = FieldPath::from_str(text)?;
        let Some(json) = (self.get)(&path) else {
            return Ok(None);
        };
        if holds_absolute_path(json.as_value()) {
            return Err(SurfaceError::AbsolutePath { path });
        }
        Ok(Some((path, json.as_value())))
    }

    fn text(&self, text: &str) -> Result<Option<String>, SurfaceError> {
        match self.value(text)? {
            Some((path, value)) => value
                .as_str()
                // BORROW: `.to_owned()` makes the owned text the surface holds
                .map(|s| Some(s.to_owned()))
                .ok_or(SurfaceError::Type {
                    path,
                    expected: "a string",
                }),
            None => Ok(None),
        }
    }

    fn required_text(&self, text: &str) -> Result<String, SurfaceError> {
        match self.text(text)? {
            Some(found) => Ok(found),
            None => Err(SurfaceError::Missing(FieldPath::from_str(text)?)),
        }
    }

    fn texts(&self, text: &str) -> Result<Vec<String>, SurfaceError> {
        let Some((path, value)) = self.value(text)? else {
            return Ok(Vec::new());
        };
        let wrong = || SurfaceError::Type {
            // BORROW: `.clone()` makes the owned path the error carries
            path: path.clone(),
            expected: "an array of strings",
        };
        value
            .as_array()
            .ok_or_else(wrong)?
            .iter()
            // BORROW: `.to_owned()` makes the owned text the surface holds
            .map(|item| item.as_str().map(str::to_owned).ok_or_else(wrong))
            .collect()
    }

    fn object(&self, text: &str) -> Result<Option<Map<String, Value>>, SurfaceError> {
        match self.value(text)? {
            Some((path, value)) => value
                .as_object()
                // BORROW: `.clone()` makes the owned map the surface holds
                .map(|map| Some(map.clone()))
                .ok_or(SurfaceError::Type {
                    path,
                    expected: "an object",
                }),
            None => Ok(None),
        }
    }
}

/// Reports whether `value`, or a string nested in it at any depth, is an absolute path.
fn holds_absolute_path(value: &Value) -> bool {
    match value {
        Value::String(text) => is_absolute_path(text),
        Value::Array(items) => items.iter().any(holds_absolute_path),
        Value::Object(fields) => fields.values().any(holds_absolute_path),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

/// Reports whether `text` starts with a separator, or with a drive letter and a separator.
fn is_absolute_path(text: &str) -> bool {
    let mut bytes = text.bytes();
    match (bytes.next(), bytes.next(), bytes.next()) {
        (Some(b'/' | b'\\'), _, _) => true,
        (Some(drive), Some(b':'), Some(b'/' | b'\\')) => drive.is_ascii_alphabetic(),
        _ => false,
    }
}
