//! Job ids, field paths, and resolved values with their sources.
//!
//! A [`JsonValue`] is a JSON value that can be hashed: it is not `null` at the top, its integers
//! lie within ±(2^53 − 1) and its numbers are finite. A [`ResolvedValue`] pairs a [`FieldPath`]
//! with such a value, or with none when the source system did not record one; a [`SourceEntry`]
//! pairs a path with the [`ValueSource`] it came from, and a [`MeasuredValue`] carries both.
//!
//! An optional field is omitted when absent and never written as `null`; the reader refuses an
//! explicit `null`. Unknown fields are ignored on input.

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::canonical::{self, CanonicalError};
use crate::generate::GeneratedSchema;
use crate::map_only::MapOnly;
use crate::primitives::{FieldPath, NameToken, RelPath, object_only, present};

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    Vec::new()
}

/// Why a value could not be built or read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ValuesError {
    /// A value that is `null` at the top.
    #[error("value is null (an absent value is omitted)")]
    Null,
    /// A value that has no canonical form, such as an integer outside ±(2^53 − 1).
    #[error(transparent)]
    Canonical(#[from] CanonicalError),
}

/// Where a resolved value came from: an object whose `kind` names the source, with `step` for `derived` and `file` for `imported`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum ValueSource {
    /// An argument given on the command line or in a request.
    ExplicitArgument,
    /// The experiment or suite file.
    ExperimentFile,
    /// The project configuration file.
    ProjectConfig,
    /// The user configuration file.
    UserConfig,
    /// A value a preflight step derived, under the key `step`.
    #[non_exhaustive]
    Derived {
        /// The preflight step that derived the value, as `name@token`.
        step: NameToken,
    },
    /// The default the core ships.
    CoreDefault,
    /// A value read from a legacy run, under the key `file`.
    #[non_exhaustive]
    Imported {
        /// The run-relative file the value was read from.
        file: RelPath,
    },
    /// A value the source system did not record; it carries no value and nothing guesses one.
    Unrecorded,
}

object_only!(ValueSource);

impl ValueSource {
    /// A value derived by the preflight step `step`.
    #[must_use]
    pub const fn derived(step: NameToken) -> Self {
        Self::Derived { step }
    }

    /// A value imported from the run-relative file `file`.
    #[must_use]
    pub const fn imported(file: RelPath) -> Self {
        Self::Imported { file }
    }
}

/// A JSON value that can be hashed: not `null` at the top, with integers within ±(2^53 − 1) and finite numbers. A `null` nested inside an array or object is data and is kept.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct JsonValue(Value);

impl JsonValue {
    /// Builds a value.
    ///
    /// # Errors
    /// Returns [`ValuesError::Null`] if `value` is `null`.
    /// Returns [`ValuesError::Canonical`] if `value` has no canonical form, such as an integer
    /// outside ±(2^53 − 1).
    pub fn new(value: Value) -> Result<Self, ValuesError> {
        if value.is_null() {
            return Err(ValuesError::Null);
        }
        canonical::check(&value)?;
        Ok(Self(value))
    }

    /// The JSON value.
    #[must_use]
    pub const fn as_value<'a>(&'a self) -> &'a Value {
        &self.0
    }

    /// The JSON value, by value.
    #[must_use]
    pub fn into_value(self) -> Value {
        self.0
    }
}

impl TryFrom<Value> for JsonValue {
    type Error = ValuesError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for JsonValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(Value::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for JsonValue {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "JsonValue".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": ["string", "number", "boolean", "array", "object"],
            "description": "Any JSON value except null at the top, with finite numbers.",
            "$comment": "Integers lie within ±(2^53 - 1); JSON Schema cannot state this, and the Rust reader refuses a larger one.",
        })
    }
}

/// A value resolved for a field, written as an object with the string `path` and, unless the source system did not record it, the JSON `value`.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "ResolvedValue")]
struct RawResolvedValue {
    /// The dotted path of the field the value belongs to.
    path: FieldPath,
    /// The resolved value; absent when the source system did not record one.
    #[serde(default, deserialize_with = "present")]
    value: Option<JsonValue>,
}

impl From<MapOnly<RawResolvedValue>> for ResolvedValue {
    fn from(raw: MapOnly<RawResolvedValue>) -> Self {
        Self {
            path: raw.0.path,
            value: raw.0.value,
        }
    }
}

/// A value resolved for a field, written as an object with the string `path` and, unless the source system did not record it, the JSON `value`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(from = "MapOnly<RawResolvedValue>")]
#[non_exhaustive]
pub struct ResolvedValue {
    path: FieldPath,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<JsonValue>,
}

impl ResolvedValue {
    /// A field with a recorded value.
    #[must_use]
    pub const fn recorded(path: FieldPath, value: JsonValue) -> Self {
        Self {
            path,
            value: Some(value),
        }
    }

    /// A field whose value the source system did not record.
    #[must_use]
    pub const fn unrecorded(path: FieldPath) -> Self {
        Self { path, value: None }
    }

    /// The path of the field.
    #[must_use]
    pub const fn path<'a>(&'a self) -> &'a FieldPath {
        &self.path
    }

    /// The value, or `None` when it was not recorded.
    #[must_use]
    pub const fn value<'a>(&'a self) -> Option<&'a JsonValue> {
        self.value.as_ref()
    }
}

/// The source of the value of one field, written as an object with the string `path` and the object `source`.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "SourceEntry")]
struct RawSourceEntry {
    /// The dotted path of the field.
    path: FieldPath,
    /// Where the field's value came from.
    source: ValueSource,
}

impl From<MapOnly<RawSourceEntry>> for SourceEntry {
    fn from(raw: MapOnly<RawSourceEntry>) -> Self {
        Self {
            path: raw.0.path,
            source: raw.0.source,
        }
    }
}

/// The source of the value of one field, written as an object with the string `path` and the object `source`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(from = "MapOnly<RawSourceEntry>")]
#[non_exhaustive]
pub struct SourceEntry {
    path: FieldPath,
    source: ValueSource,
}

impl SourceEntry {
    /// Records that the field at `path` took its value from `source`.
    #[must_use]
    pub const fn new(path: FieldPath, source: ValueSource) -> Self {
        Self { path, source }
    }

    /// The path of the field.
    #[must_use]
    pub const fn path<'a>(&'a self) -> &'a FieldPath {
        &self.path
    }

    /// Where the field's value came from.
    #[must_use]
    pub const fn source<'a>(&'a self) -> &'a ValueSource {
        &self.source
    }
}

/// A value together with its source, written as an object with the string `path`, the JSON `value` and the object `source`.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "MeasuredValue")]
struct RawMeasuredValue {
    /// The dotted path of the field.
    path: FieldPath,
    /// The value.
    value: JsonValue,
    /// Where the value came from.
    source: ValueSource,
}

impl From<MapOnly<RawMeasuredValue>> for MeasuredValue {
    fn from(raw: MapOnly<RawMeasuredValue>) -> Self {
        Self {
            path: raw.0.path,
            value: raw.0.value,
            source: raw.0.source,
        }
    }
}

/// A value together with its source, written as an object with the string `path`, the JSON `value` and the object `source`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(from = "MapOnly<RawMeasuredValue>")]
#[non_exhaustive]
pub struct MeasuredValue {
    path: FieldPath,
    value: JsonValue,
    source: ValueSource,
}

impl MeasuredValue {
    /// A value at `path` that came from `source`.
    #[must_use]
    pub const fn new(path: FieldPath, value: JsonValue, source: ValueSource) -> Self {
        Self {
            path,
            value,
            source,
        }
    }

    /// The path of the field.
    #[must_use]
    pub const fn path<'a>(&'a self) -> &'a FieldPath {
        &self.path
    }

    /// The value.
    #[must_use]
    pub const fn value<'a>(&'a self) -> &'a JsonValue {
        &self.value
    }

    /// Where the value came from.
    #[must_use]
    pub const fn source<'a>(&'a self) -> &'a ValueSource {
        &self.source
    }
}
