//! Refusals: the stable code, the side at fault and the place in the document of every error an
//! author can cause, and the `refusal/1` record that carries them to a caller that parses stderr.
//!
//! A [`Refusal`] answers three questions about an error: which [`RefusalCode`] names it, whose
//! input is at fault ([`Side`]) and, when the error sits in a parsed document, where
//! ([`Refusal::path`], an RFC 6901 JSON pointer). A code is `<surface>.<reason>` in lower case,
//! such as `experiment.path_overlap`; it names the cause and never the implementation, and once a
//! release carries a code it is never renamed. `docs/contract/codes.toml` registers every code.
//!
//! The [`refusal_table!`](crate::refusal_table) macro gives an error enum its impl and its
//! [`CatalogRow`]s from one table, so a variant without a row does not compile and the rows cannot
//! drift from the match. [`catalog`] lists this crate's rows; the core crate adds its own.
//!
//! A [`RefusalRecord`] is the `refusal/1` document: `{schema, code, side, path?, file?, message,
//! causes}`. It carries the pointer of a document or the name of a file, never both.

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

use crate::evaluation::VersionMismatch;
use crate::generate::GeneratedSchema;
use crate::map_only::MapOnly;
use crate::primitives::{SchemaTagError, present, schema_tag, text_schema};

/// The version marker every refusal record carries in its `schema` field.
pub const REFUSAL_SCHEMA: &str = "refusal/1";

/// The pattern of a refusal code: a lower-case surface, then one or more reasons of lower-case
/// letters and underscores, each after a dot.
const CODE_PATTERN: &str = r"^[a-z]+(?:\.[a-z_]+)+(?![\s\S])";

/// The pattern of an RFC 6901 JSON pointer: empty, or segments that each start with a slash and
/// escape `~` as `~0`.
const POINTER_PATTERN: &str = r"^(?:/(?:[^/~]|~[01])*)*(?![\s\S])";

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    vec![GeneratedSchema::of::<RefusalRecord>("refusal-record")]
}

/// Whose input a refusal blames.
///
/// This is not the `Side` of the evaluator protocol (`evaluation::Side`), which names the end of a
/// connection, the core or the plug-in, that has to be upgraded.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Side {
    /// A plug-in: its manifest, its answers to the protocol or its environment.
    Plugin,
    /// A suite file.
    Suite,
    /// An experiment file.
    Experiment,
    /// Candler's configuration, its settings or a value it holds for the author.
    Config,
    /// The machine, the container runtime or what the run needs from outside.
    Environment,
    /// Candler itself: the error is not the author's.
    Candler,
}

impl Side {
    /// The side as it is written on the wire and in `codes.toml`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plugin => "plugin",
            Self::Suite => "suite",
            Self::Experiment => "experiment",
            Self::Config => "config",
            Self::Environment => "environment",
            Self::Candler => "candler",
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a refusal code, a pointer or a record was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RefusalError {
    /// A code that is not `<surface>.<reason>` in lower case.
    #[error("refusal code is not `<surface>.<reason>` in lower case (got {value:?})")]
    Code {
        /// The text that was parsed.
        value: String,
    },
    /// A path that is not an RFC 6901 JSON pointer.
    #[error("refusal path is not a JSON pointer (got {value:?})")]
    Pointer {
        /// The text that was parsed.
        value: String,
    },
    /// A record that carries both a path and a file.
    #[error("refusal names both a path and a file (it names one place)")]
    Place,
    /// A file name that is empty.
    #[error("refusal file is empty")]
    EmptyFile,
}

/// Reports whether `text` is a refusal code: `^[a-z]+(\.[a-z_]+)+$`.
///
/// It is a `const fn` so that a table of codes is checked when it is compiled.
#[must_use]
pub const fn is_valid_code(text: &str) -> bool {
    let mut rest = text.as_bytes();
    let mut length = 0_usize;
    let mut dots = 0_usize;
    while let Some((&byte, tail)) = rest.split_first() {
        rest = tail;
        match byte {
            b'a'..=b'z' => length += 1,
            b'_' if dots > 0 => length += 1,
            b'.' => {
                if length == 0 {
                    return false;
                }
                dots += 1;
                length = 0;
            }
            _ => return false,
        }
    }
    length > 0 && dots > 0
}

/// Reports whether `text` is an RFC 6901 JSON pointer: empty, or a run of `/` segments in which a
/// `~` is always followed by `0` or `1`.
#[must_use]
pub fn is_pointer(text: &str) -> bool {
    if text.is_empty() {
        return true;
    }
    if !text.starts_with('/') {
        return false;
    }
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '~' && !matches!(chars.next(), Some('0' | '1')) {
            return false;
        }
    }
    true
}

/// The RFC 6901 pointer of a path of keys, escaping `~` as `~0` and `/` as `~1`.
///
/// No key gives the empty pointer, the root.
#[must_use]
pub fn pointer_of<'a>(keys: impl IntoIterator<Item = &'a str>) -> String {
    let mut pointer = String::new();
    for key in keys {
        pointer.push('/');
        for c in key.chars() {
            match c {
                '~' => pointer.push_str("~0"),
                '/' => pointer.push_str("~1"),
                other => pointer.push(other),
            }
        }
    }
    pointer
}

/// The stable name of a refusal: `<surface>.<reason>`, in lower case, such as
/// `experiment.path_overlap`.
///
/// The text is written as that text and read back only if it fits `^[a-z]+(\.[a-z_]+)+$`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RefusalCode(Cow<'static, str>);

impl RefusalCode {
    /// Reads a code from its text.
    ///
    /// # Errors
    /// Returns [`RefusalError::Code`] if `text` is not `<surface>.<reason>` in lower case.
    pub fn new(text: &str) -> Result<Self, RefusalError> {
        if is_valid_code(text) {
            Ok(Self(Cow::Owned(text.to_owned())))
        } else {
            Err(RefusalError::Code {
                // BORROW: .to_owned() keeps the offending text in the error
                value: text.to_owned(),
            })
        }
    }

    /// A code written in source, which [`refusal_table!`](crate::refusal_table) has already
    /// checked with [`is_valid_code`] when it compiled the table.
    #[doc(hidden)]
    #[must_use]
    pub const fn from_checked(text: &'static str) -> Self {
        Self(Cow::Borrowed(text))
    }

    /// The text form.
    #[must_use]
    pub fn as_str<'a>(&'a self) -> &'a str {
        // BORROW: explicit .as_ref() instead of Deref coercion
        AsRef::<str>::as_ref(&self.0)
    }
}

impl fmt::Display for RefusalCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for RefusalCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RefusalCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        // BORROW: explicit .as_str() instead of Deref coercion
        Self::new(text.as_str()).map_err(<D::Error as serde::de::Error>::custom)
    }
}

impl JsonSchema for RefusalCode {
    fn schema_name() -> Cow<'static, str> {
        "RefusalCode".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::RefusalCode").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        text_schema(
            "The stable name of a refusal, `<surface>.<reason>` in lower case, such as `experiment.path_overlap`.",
            CODE_PATTERN,
            None,
        )
    }
}

/// An error an author can cause, with its stable code, the side at fault and the place in the
/// document where it sits.
///
/// The impls of this crate come from [`refusal_table!`](crate::refusal_table), whose rows
/// [`catalog`] lists. An error that wraps another and adds nothing returns the inner code and
/// path; its side is the document's it is read from.
pub trait Refusal {
    /// The stable code of the refusal.
    fn code(&self) -> RefusalCode;

    /// Whose input is at fault.
    fn side(&self) -> Side;

    /// The RFC 6901 JSON pointer of the place in the parsed document where the error sits, or
    /// `None` when it names a file, the whole document or nothing.
    fn path(&self) -> Option<String>;
}

/// The rows of an error type's table: one per variant, in the order of the table.
pub trait Catalogued {
    /// The rows of the type.
    fn rows() -> &'static [CatalogRow];
}

/// One row of the catalogue of refusals: a variant, its code and side, when it arises and what its
/// path points at.
///
/// The contract pages' tables of refusals are written from these rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogRow {
    /// The name of the error type.
    pub enum_name: &'static str,
    /// The variant, or `-` for an error that is a struct.
    pub variant: &'static str,
    /// The code; empty when the row `delegates` to the wrapped error's.
    pub code: &'static str,
    /// Whose input is at fault.
    pub side: Side,
    /// One sentence saying when the refusal arises, written for a coding agent.
    pub when: &'static str,
    /// What the path points at, or `-` when it points at nothing.
    pub at: &'static str,
    /// Whether the variant wraps another error and returns its code and path.
    pub delegates: bool,
}

/// Gives an error type its [`Refusal`] impl and its [`CatalogRow`]s from one table.
///
/// ```ignore
/// refusal_table! {
///     MyError, path = my_error_path;
///     Unreadable { .. } => row("surface.unreadable", Side::Config, "when it arises", "what the path is"),
///     Other(inner) => delegate(inner, Side::Config),
/// }
/// ```
///
/// A `row` gives the code, the side, one sentence saying when the refusal arises and what the
/// path points at (`-` for nothing); a `delegate` names the binding of the wrapped error and the
/// side the wrapper blames, and returns the wrapped error's code and path. The variants are
/// matched without a wildcard, so a variant added to the enum without a row does not compile,
/// and each code is checked against `^[a-z]+(\.[a-z_]+)+$` when the table is compiled. The
/// function after `path =` takes `&MyError` and returns the pointer, and is written by hand. A
/// struct takes `struct Name, path = f; row(..)` and gets the variant `-`.
///
/// `Side`, `Refusal` and `Catalogued` come from this module and must be in scope.
#[macro_export]
macro_rules! refusal_table {
    (
        $enum:ident, path = $path:path;
        $( $variant:ident $( $shape:tt )? => $kind:ident $args:tt ),+ $(,)?
    ) => {
        const _: () = { $( $crate::refusal_table!(@check $kind $args); )+ };

        impl $crate::refusal::Refusal for $enum {
            fn code(&self) -> $crate::refusal::RefusalCode {
                match self {
                    $( Self::$variant $( $shape )? => $crate::refusal_table!(@code $kind $args), )+
                }
            }

            fn side(&self) -> $crate::refusal::Side {
                match self {
                    $( Self::$variant $( $shape )? => $crate::refusal_table!(@side $kind $args), )+
                }
            }

            fn path(&self) -> ::core::option::Option<::std::string::String> {
                $path(self)
            }
        }

        impl $crate::refusal::Catalogued for $enum {
            fn rows() -> &'static [$crate::refusal::CatalogRow] {
                &[ $( $crate::refusal_table!(@row $enum, $variant, $kind $args), )+ ]
            }
        }
    };
    (
        struct $ty:ident, path = $path:path; row $args:tt
    ) => {
        const _: () = { $crate::refusal_table!(@check row $args); };

        impl $crate::refusal::Refusal for $ty {
            fn code(&self) -> $crate::refusal::RefusalCode {
                $crate::refusal_table!(@code row $args)
            }

            fn side(&self) -> $crate::refusal::Side {
                $crate::refusal_table!(@side row $args)
            }

            fn path(&self) -> ::core::option::Option<::std::string::String> {
                $path(self)
            }
        }

        impl $crate::refusal::Catalogued for $ty {
            fn rows() -> &'static [$crate::refusal::CatalogRow] {
                &[ $crate::refusal_table!(@row $ty, -, row $args) ]
            }
        }
    };
    (@check row ($code:literal, $side:expr, $when:literal, $at:literal $(,)?)) => {
        assert!($crate::refusal::is_valid_code($code), "a refusal code is `<surface>.<reason>` in lower case");
    };
    (@check delegate ($inner:ident, $side:expr)) => {};
    (@code row ($code:literal, $side:expr, $when:literal, $at:literal $(,)?)) => {
        $crate::refusal::RefusalCode::from_checked($code)
    };
    (@code delegate ($inner:ident, $side:expr)) => {
        $crate::refusal::Refusal::code($inner)
    };
    (@side row ($code:literal, $side:expr, $when:literal, $at:literal $(,)?)) => {
        $side
    };
    (@side delegate ($inner:ident, $side:expr)) => {{
        let _ = $inner;
        $side
    }};
    (@row $enum:ident, $variant:tt, row ($code:literal, $side:expr, $when:literal, $at:literal $(,)?)) => {
        $crate::refusal::CatalogRow {
            enum_name: ::core::stringify!($enum),
            variant: ::core::stringify!($variant),
            code: $code,
            side: $side,
            when: $when,
            at: $at,
            delegates: false,
        }
    };
    (@row $enum:ident, $variant:tt, delegate ($inner:ident, $side:expr)) => {
        $crate::refusal::CatalogRow {
            enum_name: ::core::stringify!($enum),
            variant: ::core::stringify!($variant),
            code: "",
            side: $side,
            when: "Wraps another error and returns its code and path.",
            at: "-",
            delegates: true,
        }
    };
}

/// The rows of every refusal of this crate, in the order of the types.
#[must_use]
pub fn catalog() -> Vec<CatalogRow> {
    let tables: [&'static [CatalogRow]; 9] = [
        <crate::execution::ExecutionError as Catalogued>::rows(),
        <crate::evaluation::EvaluationError as Catalogued>::rows(),
        <crate::outcome::OutcomeError as Catalogued>::rows(),
        <crate::primitives::FormatError as Catalogued>::rows(),
        <crate::canonical::CanonicalError as Catalogued>::rows(),
        <crate::catalog::CatalogError as Catalogued>::rows(),
        <crate::values::ValuesError as Catalogued>::rows(),
        <SchemaTagError as Catalogued>::rows(),
        <VersionMismatch as Catalogued>::rows(),
    ];
    tables
        .iter()
        .flat_map(|rows| rows.iter().copied())
        .collect()
}

schema_tag! {
    /// The constant `schema` field of a refusal record.
    RefusalSchemaTag = REFUSAL_SCHEMA
}

/// One refusal as a caller that parses stderr reads it: a document with the version marker `refusal/1` in `schema`, the stable `code`, the `side` at fault, the `path` into the parsed document or the `file`, never both, the `message` and the `causes` that explain it.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    rename = "RefusalRecord",
    extend("not" = {"required": ["path", "file"]})
)]
struct RawRefusalRecord {
    /// The version marker of the document: always `refusal/1`.
    schema: RefusalSchemaTag,
    /// The stable code of the refusal.
    code: RefusalCode,
    /// Whose input is at fault.
    side: Side,
    /// The RFC 6901 JSON pointer of the place in the parsed document where the error sits; empty for the root. Absent when the refusal names a file or nothing.
    #[serde(default, deserialize_with = "present")]
    #[schemars(regex(pattern = POINTER_PATTERN))]
    path: Option<String>,
    /// The file the refusal is about, when it is not about a place in a parsed document.
    #[serde(default, deserialize_with = "present")]
    #[schemars(length(min = 1))]
    file: Option<String>,
    /// The error's message, as the text form of the CLI prints it.
    message: String,
    /// The messages of the errors that caused this one, outermost first; empty when there are none.
    causes: Vec<String>,
}

impl TryFrom<MapOnly<RawRefusalRecord>> for RefusalRecord {
    type Error = RefusalError;

    fn try_from(raw: MapOnly<RawRefusalRecord>) -> Result<Self, Self::Error> {
        let RawRefusalRecord {
            schema: RefusalSchemaTag,
            code,
            side,
            path,
            file,
            message,
            causes,
        } = raw.0;
        Self::checked(code, side, path, file, message, causes)
    }
}

/// One refusal as a caller that parses stderr reads it: a document with the version marker `refusal/1` in `schema`, the stable `code`, the `side` at fault, the `path` into the parsed document or the `file`, never both, the `message` and the `causes` that explain it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawRefusalRecord>")]
#[non_exhaustive]
pub struct RefusalRecord {
    schema: RefusalSchemaTag,
    code: RefusalCode,
    side: Side,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    message: String,
    causes: Vec<String>,
}

impl RefusalRecord {
    /// Builds a record with no place and no cause.
    #[must_use]
    pub const fn new(code: RefusalCode, side: Side, message: String) -> Self {
        Self {
            schema: RefusalSchemaTag,
            code,
            side,
            path: None,
            file: None,
            message,
            causes: Vec::new(),
        }
    }

    /// Builds the record of an error: its code, side and path from [`Refusal`], its message from
    /// [`fmt::Display`] and its causes from the chain of [`std::error::Error::source`].
    #[must_use]
    pub fn of<E>(error: &E) -> Self
    where
        E: Refusal + std::error::Error,
    {
        let mut causes = Vec::new();
        let mut source = error.source();
        while let Some(cause) = source {
            causes.push(cause.to_string());
            source = cause.source();
        }
        Self {
            schema: RefusalSchemaTag,
            code: error.code(),
            side: error.side(),
            path: error.path(),
            file: None,
            message: error.to_string(),
            causes,
        }
    }

    /// Places the refusal at a pointer into the parsed document.
    ///
    /// # Errors
    /// Returns [`RefusalError::Pointer`] if `path` is not an RFC 6901 pointer and
    /// [`RefusalError::Place`] if the record already names a file.
    pub fn at_path(self, path: String) -> Result<Self, RefusalError> {
        Self::checked(
            self.code,
            self.side,
            Some(path),
            self.file,
            self.message,
            self.causes,
        )
    }

    /// Places the refusal in a file.
    ///
    /// # Errors
    /// Returns [`RefusalError::EmptyFile`] if `file` is empty and [`RefusalError::Place`] if the
    /// record already names a path.
    pub fn in_file(self, file: String) -> Result<Self, RefusalError> {
        Self::checked(
            self.code,
            self.side,
            self.path,
            Some(file),
            self.message,
            self.causes,
        )
    }

    /// Sets the messages of the errors that caused this one.
    #[must_use]
    pub fn with_causes(mut self, causes: Vec<String>) -> Self {
        self.causes = causes;
        self
    }

    /// Checks the place and builds the record.
    fn checked(
        code: RefusalCode,
        side: Side,
        path: Option<String>,
        file: Option<String>,
        message: String,
        causes: Vec<String>,
    ) -> Result<Self, RefusalError> {
        if path.is_some() && file.is_some() {
            return Err(RefusalError::Place);
        }
        if let Some(path) = &path
            && !is_pointer(path.as_str())
        {
            return Err(RefusalError::Pointer {
                // BORROW: .clone() keeps the offending text in the error
                value: path.clone(),
            });
        }
        if file.as_deref().is_some_and(str::is_empty) {
            return Err(RefusalError::EmptyFile);
        }
        Ok(Self {
            schema: RefusalSchemaTag,
            code,
            side,
            path,
            file,
            message,
            causes,
        })
    }

    /// The stable code.
    #[must_use]
    pub const fn code<'a>(&'a self) -> &'a RefusalCode {
        &self.code
    }

    /// Whose input is at fault.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// The pointer into the parsed document, if the refusal has one.
    #[must_use]
    pub fn path<'a>(&'a self) -> Option<&'a str> {
        // BORROW: .as_deref() views the owned pointer
        self.path.as_deref()
    }

    /// The file, if the refusal names one.
    #[must_use]
    pub fn file<'a>(&'a self) -> Option<&'a str> {
        // BORROW: .as_deref() views the owned file name
        self.file.as_deref()
    }

    /// The message, as the text form prints it.
    #[must_use]
    pub fn message<'a>(&'a self) -> &'a str {
        // BORROW: explicit .as_str() instead of Deref coercion
        self.message.as_str()
    }

    /// The messages of the errors that caused this one, outermost first.
    #[must_use]
    pub fn causes<'a>(&'a self) -> &'a [String] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.causes.as_slice()
    }
}

/// A fault found while reading a document, with the pointer of the place it sits at.
pub(crate) struct Located {
    /// The pointer to the field at fault, or to its parent when the field is missing; empty at
    /// the root and for a text that is not a document.
    pub(crate) path: String,
    /// The reader's explanation, exactly as the reader words it.
    pub(crate) detail: String,
    /// Whether the reader refused a key the type does not have.
    pub(crate) unknown_field: bool,
}

/// Reports whether a reader's explanation is serde's refusal of an unknown key: a line that starts
/// with ``unknown field ` ``.
fn is_unknown_field(detail: &str) -> bool {
    detail
        .lines()
        .any(|line| line.starts_with("unknown field `"))
}

/// Wraps a reader's explanation and the pointer it failed at.
///
/// serde's own path already ends with the unknown key. A reader that checks keys itself, as the
/// plug-in manifest's environment does for its internally tagged enum, words its message
/// ``unknown field `key` of this table`` and leaves the pointer at the table, so the key is
/// appended to it.
fn located(path: String, detail: String) -> Located {
    let unknown_field = is_unknown_field(detail.as_str());
    let mut path = path;
    if unknown_field
        && detail.contains("` of this table")
        && let Some(key) = unknown_key(detail.as_str())
    {
        path.push_str(pointer_of([key]).as_str());
    }
    Located {
        path,
        detail,
        unknown_field,
    }
}

/// The key serde names in ``unknown field `key` ``.
fn unknown_key(detail: &str) -> Option<&str> {
    let line = detail
        .lines()
        .find(|line| line.starts_with("unknown field `"))?;
    let rest = line.strip_prefix("unknown field `")?;
    rest.split_once('`').map(|(key, _)| key)
}

/// The pointer of the path `serde_path_to_error` followed to a fault.
fn pointer_of_path(path: &serde_path_to_error::Path) -> String {
    use serde_path_to_error::Segment;
    let mut pointer = String::new();
    for segment in path {
        match segment {
            Segment::Seq { index } => {
                pointer.push('/');
                pointer.push_str(index.to_string().as_str());
            }
            Segment::Map { key } => pointer.push_str(pointer_of([key.as_str()]).as_str()),
            Segment::Enum { variant } => {
                pointer.push_str(pointer_of([variant.as_str()]).as_str());
            }
            Segment::Unknown => {} // EXPLICIT: a step the library could not name adds nothing
        }
    }
    pointer
}

/// Reads `T` from TOML text, giving the pointer of the field at fault.
pub(crate) fn read_toml<T: DeserializeOwned>(text: &str) -> Result<T, Located> {
    let deserializer = toml::Deserializer::parse(text)
        .map_err(|error| located(String::new(), error.to_string()))?;
    serde_path_to_error::deserialize(deserializer)
        .map_err(|error| located(pointer_of_path(error.path()), error.inner().to_string()))
}

/// Reads `T` from JSON text, giving the pointer of the field at fault.
pub(crate) fn read_json<T: DeserializeOwned>(text: &str) -> Result<T, Located> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = serde_path_to_error::deserialize(&mut deserializer)
        .map_err(|error| located(pointer_of_path(error.path()), error.inner().to_string()))?;
    deserializer
        .end()
        .map_err(|error| located(String::new(), error.to_string()))?;
    Ok(value)
}

/// Reads `T` from a JSON value, giving the pointer of the field at fault.
pub(crate) fn read_value<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, Located> {
    serde_path_to_error::deserialize(value)
        .map_err(|error| located(pointer_of_path(error.path()), error.inner().to_string()))
}
