//! The string formats and small value types that wire records share.
//!
//! Each text format is a newtype with a private field. It is built through [`FromStr`] or
//! [`TryFrom<String>`], so a value of the type is valid however it was made. It serialises as a
//! JSON string, and its [`JsonSchema`] carries a `pattern` that accepts the same text as the
//! parser. Rules a `pattern` cannot state are enforced by the parser and named in the schema's
//! `$comment`.

use std::fmt;
use std::num::NonZeroU32;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A text that does not have the shape of the format it was parsed as.
///
/// Every variant names the format, what is wrong with the text, and the offending text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FormatError {
    /// A [`Digest`] that is not `sha256:` followed by 64 lowercase hex digits.
    #[error("digest {problem} (got {value:?})")]
    Digest {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// An [`InstrumentDigest`] that is neither a [`Digest`] nor `git:` and a commit hash.
    #[error("instrument digest {problem} (got {value:?})")]
    InstrumentDigest {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// A [`NameVersion`] that is not `name@version` with a `SemVer` 2.0 version.
    #[error("name@version {problem} (got {value:?})")]
    NameVersion {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// A [`NameToken`] that is not `name@token` with a free version token.
    #[error("name@token {problem} (got {value:?})")]
    NameToken {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// A [`RelPath`] that is not a run-relative POSIX path.
    #[error("relative path {problem} (got {value:?})")]
    RelPath {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// A [`Timestamp`] that is not an RFC 3339 timestamp with an explicit offset.
    #[error("timestamp {problem} (got {value:?})")]
    Timestamp {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// A [`Timestamp`] of the right shape that names no instant.
    #[error("timestamp names no instant ({detail}; got {value:?})")]
    TimestampInstant {
        /// The calendar check's explanation.
        detail: String,
        /// The text that was parsed.
        value: String,
    },
    /// A [`RunKey`] that is not `<cell hex>-r<replicate>-a<attempt>`, or a counter below 1.
    #[error("run key {problem} (got {value:?})")]
    RunKey {
        /// What is wrong with the text or the counters.
        problem: &'static str,
        /// The text that was parsed, or the counters in their path form.
        value: String,
    },
    /// A [`RequestId`] that is not a lowercase hyphenated `UUIDv7`.
    #[error("request id {problem} (got {value:?})")]
    RequestId {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// An [`EvidenceRef`] that is neither an event nor a path reference.
    #[error("evidence reference {problem} (got {value:?})")]
    EvidenceRef {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// An [`ExpectationId`] outside `[A-Za-z0-9][A-Za-z0-9._-]*`.
    #[error("expectation id {problem} (got {value:?})")]
    ExpectationId {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// A [`ReasonCode`] outside `[a-z][a-z0-9_]*`.
    #[error("reason code {problem} (got {value:?})")]
    ReasonCode {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
}

/// The `SemVer` 2.0 grammar as a regular expression, without anchors.
macro_rules! semver_re {
    () => {
        r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-(?:0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*))*)?(?:\+[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*)?"
    };
}

/// One character of a name: neither `@` nor white space.
macro_rules! name_char_re {
    () => {
        r"[^@\s\u0085\uFEFF]"
    };
}

/// One character of a session id: neither `#` nor white space.
macro_rules! session_char_re {
    () => {
        r"[^#\s\u0085\uFEFF]"
    };
}

/// A run-relative POSIX path as a regular expression, without anchors.
///
/// `$seg` is the class of characters a segment may hold and `$end` what may follow the path, so
/// that a `.` or `..` segment is refused wherever the path stops.
macro_rules! rel_path_re {
    ($seg:literal, $end:literal) => {
        concat!(
            r"(?!\.\.?(?:/|",
            $end,
            r"))",
            $seg,
            r"+(?:/(?!\.\.?(?:/|",
            $end,
            r"))",
            $seg,
            r"+)*"
        )
    };
}

const DIGEST_PATTERN: &str = "^sha256:[0-9a-f]{64}$";
const INSTRUMENT_DIGEST_PATTERN: &str =
    "^(?:sha256:[0-9a-f]{64}|git:(?:[0-9a-f]{40}|[0-9a-f]{64}))$";
const NAME_VERSION_PATTERN: &str = concat!("^", name_char_re!(), "+@", semver_re!(), "$");
const NAME_TOKEN_PATTERN: &str = concat!("^", name_char_re!(), "+@[0-9A-Za-z.+-]+$");
const REL_PATH_PATTERN: &str = concat!("^", rel_path_re!(r"[^/\\\x00]", "$"), "$");
const TIMESTAMP_PATTERN: &str = r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]+)?(?:Z|[+-](?:[01][0-9]|2[0-3]):[0-5][0-9])$";
const REQUEST_ID_PATTERN: &str =
    "^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$";
const EVIDENCE_REF_PATTERN: &str = concat!(
    "^(?:event:",
    session_char_re!(),
    "+#(?:0|[1-9][0-9]*)|path:",
    rel_path_re!(r"[^/\\#\x00]", "#|$"),
    r"(?:#L[1-9][0-9]*(?:-[1-9][0-9]*)?)?)$"
);
const EXPECTATION_ID_PATTERN: &str = "^[A-Za-z0-9][A-Za-z0-9._-]*$";
const REASON_CODE_PATTERN: &str = "^[a-z][a-z0-9_]*$";

const DIGEST_PREFIX: &str = "sha256:";
const GIT_PREFIX: &str = "git:";

/// The schema of a text format: a string with a description, a pattern and an optional `$comment`.
fn text_schema(description: &str, pattern: &str, comment: Option<&str>) -> Schema {
    let mut schema = json_schema!({
        "type": "string",
        "description": description,
        "pattern": pattern,
    });
    if let Some(comment) = comment {
        // BORROW: .to_owned() makes the owned key `Schema::insert` takes
        schema.insert("$comment".to_owned(), comment.into());
    }
    schema
}

/// Defines a text-form newtype with the shared surface every text format has.
///
/// The `description` is both the type's rustdoc and the `description` of its schema. The
/// `pattern` and the optional `comment` complete the schema. The optional `fields` are values the
/// parser derives from the text; the type provides [`FromStr`], and the macro adds `as_str`,
/// [`fmt::Display`], [`TryFrom<String>`], `From<Type> for String` and [`JsonSchema`].
macro_rules! text_type {
    (@comment) => {
        None
    };
    (@comment $comment:literal) => {
        Some($comment)
    };
    (
        $ty:ident {
            description: $description:literal,
            pattern: $pattern:expr
            $(, comment: $comment:literal)?
            $(, fields: { $($field:ident: $field_ty:ty),+ $(,)? })?
            $(,)?
        }
    ) => {
        #[doc = $description]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $ty {
            // Derived values come first, so they order values before the text does.
            $($($field: $field_ty,)+)?
            // The text as written.
            text: String,
        }

        impl $ty {
            /// The text form.
            #[must_use]
            pub const fn as_str<'a>(&'a self) -> &'a str {
                // BORROW: explicit .as_str() instead of Deref coercion
                self.text.as_str()
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl TryFrom<String> for $ty {
            type Error = FormatError;

            fn try_from(text: String) -> Result<Self, Self::Error> {
                // BORROW: explicit .as_str() instead of Deref coercion
                text.as_str().parse()
            }
        }

        impl From<$ty> for String {
            fn from(value: $ty) -> Self {
                value.text
            }
        }

        impl JsonSchema for $ty {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($ty).into()
            }

            fn schema_id() -> std::borrow::Cow<'static, str> {
                concat!(module_path!(), "::", stringify!($ty)).into()
            }

            fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
                text_schema($description, $pattern, text_type!(@comment $($comment)?))
            }
        }
    };
}

fn is_lower_hex(text: &str) -> bool {
    text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Parses a decimal without sign, leading zero or separator.
fn canonical_decimal<T: FromStr>(text: &str) -> Option<T> {
    let plain = !text.is_empty()
        && text.bytes().all(|b| b.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'));
    if plain { text.parse().ok() } else { None }
}

/// Reports whether `text` is `template` with every `d` standing for one ASCII digit.
fn fits(text: &str, template: &str) -> bool {
    text.len() == template.len()
        && text.bytes().zip(template.bytes()).all(|(t, p)| {
            if p == b'd' {
                t.is_ascii_digit()
            } else {
                t == p
            }
        })
}

/// Reports whether `c` is white space or the byte-order mark.
const fn is_blank(c: char) -> bool {
    c.is_whitespace() || c == '\u{FEFF}'
}

text_type!(Digest {
    description: "A content digest: `sha256:` followed by 64 lowercase hex digits.",
    pattern: DIGEST_PATTERN,
});

impl Digest {
    /// Builds a digest from its bare hex digits, as blob paths spell them.
    ///
    /// # Errors
    /// Returns [`FormatError::Digest`] if `hex` is not 64 lowercase hex digits.
    pub fn from_hex(hex: &str) -> Result<Self, FormatError> {
        if hex.len() != 64 || !is_lower_hex(hex) {
            return Err(FormatError::Digest {
                problem: "is not 64 lowercase hex digits after `sha256:`",
                value: hex.into(),
            });
        }
        Ok(Self {
            text: format!("{DIGEST_PREFIX}{hex}"),
        })
    }

    /// The 64 hex digits without the `sha256:` prefix.
    #[must_use]
    pub fn hex<'a>(&'a self) -> &'a str {
        self.text.strip_prefix(DIGEST_PREFIX).unwrap_or_default()
    }
}

impl FromStr for Digest {
    type Err = FormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let hex = text
            .strip_prefix(DIGEST_PREFIX)
            .ok_or(FormatError::Digest {
                problem: "lacks the `sha256:` prefix",
                value: text.into(),
            })?;
        Self::from_hex(hex).map_err(|_| FormatError::Digest {
            problem: "is not 64 lowercase hex digits after `sha256:`",
            value: text.into(),
        })
    }
}

text_type!(InstrumentDigest {
    description: "The digest of an instrument: a content digest, or `git:` followed by the 40 or 64 lowercase hex digits of a commit.",
    pattern: INSTRUMENT_DIGEST_PATTERN,
});

impl InstrumentDigest {
    /// Reports whether the digest names a git commit rather than a content digest.
    #[must_use]
    pub fn is_git(&self) -> bool {
        self.text.starts_with(GIT_PREFIX)
    }
}

impl FromStr for InstrumentDigest {
    type Err = FormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if let Some(hash) = text.strip_prefix(GIT_PREFIX) {
            if !matches!(hash.len(), 40 | 64) || !is_lower_hex(hash) {
                return Err(FormatError::InstrumentDigest {
                    problem: "has a git hash that is not 40 or 64 lowercase hex digits",
                    value: text.into(),
                });
            }
            return Ok(Self { text: text.into() });
        }
        let digest = text
            .parse::<Digest>()
            .map_err(|_| FormatError::InstrumentDigest {
                problem: "is neither `sha256:<64 hex>` nor `git:<40 or 64 hex>`",
                value: text.into(),
            })?;
        Ok(Self { text: digest.text })
    }
}

/// Splits `name@version` at its last `@` and checks the name.
fn split_name<'a>(text: &'a str) -> Result<(&'a str, &'a str), &'static str> {
    let (name, version) = text.rsplit_once('@').ok_or("lacks an `@`")?;
    if name.is_empty() {
        return Err("has an empty name");
    }
    if name.chars().any(|c| c == '@' || is_blank(c)) {
        return Err("has a name with `@` or white space");
    }
    Ok((name, version))
}

text_type!(NameVersion {
    description: "An evaluator or instrument reference, `name@version`, split at the last `@`: the name is non-empty and holds neither `@` nor white space, and the version is `SemVer` 2.0.",
    pattern: NAME_VERSION_PATTERN,
    comment: "Each version number fits in 64 bits; the parser enforces it.",
    fields: { name: String, version: semver::Version },
});

impl NameVersion {
    /// The name before the last `@`.
    #[must_use]
    pub const fn name<'a>(&'a self) -> &'a str {
        // BORROW: explicit .as_str() instead of Deref coercion
        self.name.as_str()
    }

    /// The `SemVer` 2.0 version after the last `@`.
    #[must_use]
    pub const fn version<'a>(&'a self) -> &'a semver::Version {
        &self.version
    }
}

impl FromStr for NameVersion {
    type Err = FormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let fail = |problem| FormatError::NameVersion {
            problem,
            value: text.into(),
        };
        let (name, version) = split_name(text).map_err(fail)?;
        let version = semver::Version::parse(version)
            .map_err(|_| fail("has a version that is not SemVer 2.0"))?;
        Ok(Self {
            text: text.into(),
            name: name.into(),
            version,
        })
    }
}

text_type!(NameToken {
    description: "A catalog provider reference, `name@token`, split at the last `@`: the name is non-empty and holds neither `@` nor white space, and the token matches `[0-9A-Za-z.+-]+`.",
    pattern: NAME_TOKEN_PATTERN,
    fields: { name: String, version: String },
});

impl NameToken {
    /// The name before the last `@`.
    #[must_use]
    pub const fn name<'a>(&'a self) -> &'a str {
        // BORROW: explicit .as_str() instead of Deref coercion
        self.name.as_str()
    }

    /// The version token after the last `@`.
    #[must_use]
    pub const fn version<'a>(&'a self) -> &'a str {
        // BORROW: explicit .as_str() instead of Deref coercion
        self.version.as_str()
    }
}

impl FromStr for NameToken {
    type Err = FormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let fail = |problem| FormatError::NameToken {
            problem,
            value: text.into(),
        };
        let (name, version) = split_name(text).map_err(fail)?;
        let token = !version.is_empty()
            && version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-'));
        if !token {
            return Err(fail("has a version token outside `[0-9A-Za-z.+-]+`"));
        }
        Ok(Self {
            text: text.into(),
            name: name.into(),
            version: version.into(),
        })
    }
}

/// Returns what is wrong with `text` as a run-relative POSIX path, or `None` if nothing is.
fn rel_path_problem(text: &str) -> Option<&'static str> {
    if text.is_empty() {
        Some("is empty")
    } else if text.starts_with('/') {
        Some("is absolute")
    } else if text.contains('\\') {
        Some("contains a backslash")
    } else if text.contains('\0') {
        Some("contains a NUL character")
    } else {
        text.split('/').find_map(|segment| match segment {
            "" => Some("contains an empty segment"),
            "." | ".." => Some("contains a `.` or `..` segment"),
            _ => None,
        })
    }
}

text_type!(RelPath {
    description: "A run-relative POSIX path: non-empty, with no leading `/`, no empty segment, no `.` or `..` segment, no NUL and no `\\`.",
    pattern: REL_PATH_PATTERN,
});

impl FromStr for RelPath {
    type Err = FormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match rel_path_problem(text) {
            Some(problem) => Err(FormatError::RelPath {
                problem,
                value: text.into(),
            }),
            None => Ok(Self { text: text.into() }),
        }
    }
}

/// Reports whether `digits` is `HH:MM` with hours 00 to 23 and minutes 00 to 59.
fn offset_in_range(digits: &str) -> bool {
    // Two-digit ASCII fields compare as numbers when compared as text.
    fits(digits, "dd:dd")
        && digits.get(..2).is_some_and(|hours| hours <= "23")
        && digits.get(3..).is_some_and(|minutes| minutes <= "59")
}

/// Reports whether `text` has the lexical shape `YYYY-MM-DDTHH:MM:SS[.f+](Z|±HH:MM)`.
fn has_rfc3339_shape(text: &str) -> bool {
    let Some((date, rest)) = text.split_once('T') else {
        return false;
    };
    let (clock, offset_fits) = if let Some(clock) = rest.strip_suffix('Z') {
        (clock, true)
    } else if let Some((clock, offset)) = rest
        .rfind(['+', '-'])
        .and_then(|at| rest.split_at_checked(at))
    {
        (clock, offset.get(1..).is_some_and(offset_in_range))
    } else {
        return false;
    };
    let (seconds, fraction) = match clock.split_once('.') {
        Some((seconds, fraction)) => (seconds, Some(fraction)),
        None => (clock, None),
    };
    offset_fits
        && fits(date, "dddd-dd-dd")
        && fits(seconds, "dd:dd:dd")
        && fraction
            .is_none_or(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

text_type!(Timestamp {
    description: "An RFC 3339 timestamp with an uppercase `T` and an explicit offset, `Z` or `±HH:MM`, kept as written.",
    pattern: TIMESTAMP_PATTERN,
    comment: "The pattern states the lexical form; a date or time that names no instant, such as month 13, and a fraction of more than nine digits are refused by the parser.",
    fields: { instant: jiff::Timestamp },
});

impl Timestamp {
    /// The instant the text names.
    #[must_use]
    pub const fn instant(&self) -> jiff::Timestamp {
        self.instant
    }
}

impl FromStr for Timestamp {
    type Err = FormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if !has_rfc3339_shape(text) {
            return Err(FormatError::Timestamp {
                problem: "is not `YYYY-MM-DDTHH:MM:SS[.fraction]` followed by `Z` or `±HH:MM`",
                value: text.into(),
            });
        }
        let instant =
            text.parse::<jiff::Timestamp>()
                .map_err(|e| FormatError::TimestampInstant {
                    detail: e.to_string(),
                    value: text.into(),
                })?;
        Ok(Self {
            text: text.into(),
            instant,
        })
    }
}

/// The identity of one run: the digest of its cell, and its replicate and attempt numbers, both counted from 1.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[schemars(deny_unknown_fields)]
pub struct RunKey {
    /// The digest of the cell the run belongs to.
    cell: Digest,
    /// Replicate number, from 1.
    replicate: NonZeroU32,
    /// Attempt number, from 1.
    attempt: NonZeroU32,
}

impl RunKey {
    /// Builds a run key.
    ///
    /// # Errors
    /// Returns [`FormatError::RunKey`] if `replicate` or `attempt` is 0.
    pub fn new(cell: Digest, replicate: u32, attempt: u32) -> Result<Self, FormatError> {
        match (NonZeroU32::new(replicate), NonZeroU32::new(attempt)) {
            (Some(replicate), Some(attempt)) => Ok(Self {
                cell,
                replicate,
                attempt,
            }),
            (None, _) | (_, None) => Err(FormatError::RunKey {
                problem: "has a counter below 1",
                value: format!("{}-r{replicate}-a{attempt}", cell.hex()),
            }),
        }
    }

    /// The digest of the cell the run belongs to.
    #[must_use]
    pub const fn cell<'a>(&'a self) -> &'a Digest {
        &self.cell
    }

    /// The replicate number, from 1.
    #[must_use]
    pub const fn replicate(&self) -> u32 {
        self.replicate.get()
    }

    /// The attempt number, from 1.
    #[must_use]
    pub const fn attempt(&self) -> u32 {
        self.attempt.get()
    }
}

/// Writes the path form `<cell hex>-r<replicate>-a<attempt>`, which paths and `CANDLER_RUN` use.
impl fmt::Display for RunKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}-r{}-a{}",
            self.cell.hex(),
            self.replicate,
            self.attempt
        )
    }
}

/// Reads the path form `<cell hex>-r<replicate>-a<attempt>`, with both counters from 1 and
/// written without sign or leading zero.
impl FromStr for RunKey {
    type Err = FormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let fail = |problem| FormatError::RunKey {
            problem,
            value: text.into(),
        };
        let (hex, rest) = text
            .split_once("-r")
            .ok_or_else(|| fail("lacks `-r<replicate>`"))?;
        let (replicate, attempt) = rest
            .split_once("-a")
            .ok_or_else(|| fail("lacks `-a<attempt>`"))?;
        let cell = Digest::from_hex(hex)
            .map_err(|_| fail("has a cell that is not 64 lowercase hex digits"))?;
        let replicate = canonical_decimal::<u32>(replicate)
            .ok_or_else(|| fail("has a replicate that is not a plain decimal"))?;
        let attempt = canonical_decimal::<u32>(attempt)
            .ok_or_else(|| fail("has an attempt that is not a plain decimal"))?;
        Self::new(cell, replicate, attempt).map_err(|_| fail("has a counter below 1"))
    }
}

text_type!(RequestId {
    description: "A request id: a `UUIDv7` in lowercase hyphenated form.",
    pattern: REQUEST_ID_PATTERN,
});

impl FromStr for RequestId {
    type Err = FormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let fail = |problem| FormatError::RequestId {
            problem,
            value: text.into(),
        };
        let uuid = Uuid::try_parse(text).map_err(|_| fail("is not a UUID"))?;
        if uuid.hyphenated().to_string() != text {
            return Err(fail("is not in lowercase hyphenated form"));
        }
        if uuid.get_version() != Some(uuid::Version::SortRand) {
            return Err(fail("is not version 7"));
        }
        if uuid.get_variant() != uuid::Variant::RFC4122 {
            return Err(fail("is not of the RFC 4122 variant"));
        }
        Ok(Self { text: text.into() })
    }
}

/// Checks the line anchor after the `#` of a path reference: `L<n>` or `L<n>-<m>`.
fn line_anchor_problem(anchor: &str) -> Option<&'static str> {
    let Some(lines) = anchor.strip_prefix('L') else {
        return Some(
            "has a path followed by something other than a line anchor `#L<n>` or `#L<n>-<m>`",
        );
    };
    let (first, last) = match lines.split_once('-') {
        Some((first, last)) => (first, Some(last)),
        None => (lines, None),
    };
    let line = |digits: &str| canonical_decimal::<u64>(digits).filter(|n| *n >= 1);
    match (line(first), last.map(line)) {
        (None, _) | (_, Some(None)) => Some("has a line number that is not a decimal from 1"),
        (Some(first), Some(Some(last))) if first > last => {
            Some("has a line range that ends before it starts")
        }
        (Some(_), Some(Some(_)) | None) => None,
    }
}

text_type!(EvidenceRef {
    description: "A pointer to the evidence behind an outcome: `event:<session-id>#<seq>`, or `path:<run-relative path>` optionally followed by `#L<n>` or `#L<n>-<m>`. The session id is non-empty and holds neither `#` nor white space, `seq` is a decimal without leading zeros, line numbers count from 1, and a path in a reference holds no `#`.",
    pattern: EVIDENCE_REF_PATTERN,
    comment: "In a line range the first line is at most the last line, and the sequence and line numbers fit in 64 bits; the parser enforces both.",
});

impl FromStr for EvidenceRef {
    type Err = FormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let fail = |problem| FormatError::EvidenceRef {
            problem,
            value: text.into(),
        };
        let problem = if let Some(event) = text.strip_prefix("event:") {
            match event.split_once('#') {
                None => Some("has an event without `#<seq>`"),
                Some((session, _)) if session.is_empty() || session.chars().any(is_blank) => {
                    Some("has a session id that is empty or holds white space")
                }
                Some((_, seq)) if canonical_decimal::<u64>(seq).is_none() => {
                    Some("has a sequence number that is not a plain decimal")
                }
                Some(_) => None,
            }
        } else if let Some(target) = text.strip_prefix("path:") {
            let (path, anchor) = match target.split_once('#') {
                Some((path, anchor)) => (path, Some(anchor)),
                None => (target, None),
            };
            rel_path_problem(path).or_else(|| anchor.and_then(line_anchor_problem))
        } else {
            Some("starts with neither `event:` nor `path:`")
        };
        match problem {
            Some(problem) => Err(fail(problem)),
            None => Ok(Self { text: text.into() }),
        }
    }
}

text_type!(ExpectationId {
    description: "An expectation identifier: a letter or digit, then letters, digits, `.`, `_` or `-`.",
    pattern: EXPECTATION_ID_PATTERN,
});

impl FromStr for ExpectationId {
    type Err = FormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut bytes = text.bytes();
        let first = bytes.next().is_some_and(|b| b.is_ascii_alphanumeric());
        if first && bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')) {
            Ok(Self { text: text.into() })
        } else {
            Err(FormatError::ExpectationId {
                problem: "does not match `[A-Za-z0-9][A-Za-z0-9._-]*`",
                value: text.into(),
            })
        }
    }
}

text_type!(ReasonCode {
    description: "A reason code: a lowercase letter, then lowercase letters, digits or `_`.",
    pattern: REASON_CODE_PATTERN,
});

impl FromStr for ReasonCode {
    type Err = FormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut bytes = text.bytes();
        let first = bytes.next().is_some_and(|b| b.is_ascii_lowercase());
        if first && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
            Ok(Self { text: text.into() })
        } else {
            Err(FormatError::ReasonCode {
                problem: "does not match `[a-z][a-z0-9_]*`",
                value: text.into(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde::de::DeserializeOwned;
    use serde_json::json;

    use super::*;

    const ZERO: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    const HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn validator<T: JsonSchema>() -> jsonschema::Validator {
        let root = SchemaGenerator::default().into_root_schema_for::<T>();
        jsonschema::validator_for(root.as_value()).expect("the generated schema compiles")
    }

    /// Every text parses, round-trips through serde and satisfies the generated schema.
    fn assert_accepts<T>(texts: &[&str])
    where
        T: FromStr<Err = FormatError>
            + JsonSchema
            + Serialize
            + DeserializeOwned
            + fmt::Debug
            + fmt::Display
            + PartialEq,
    {
        let schema = validator::<T>();
        for text in texts {
            let parsed: T = text
                .parse()
                .unwrap_or_else(|e| panic!("{text:?} should parse: {e}"));
            assert_eq!(parsed.to_string(), *text);
            assert!(
                schema.is_valid(&json!(text)),
                "schema should accept {text:?}"
            );
            let read: T = serde_json::from_value(json!(text)).expect("serde reads the text");
            assert_eq!(read, parsed);
            assert_eq!(
                serde_json::to_value(&parsed).expect("serde writes"),
                json!(text)
            );
        }
    }

    /// The parser and serde reject every text.
    fn assert_parser_rejects<T>(texts: &[&str])
    where
        T: FromStr<Err = FormatError> + DeserializeOwned + fmt::Debug,
    {
        for text in texts {
            assert!(text.parse::<T>().is_err(), "{text:?} should not parse");
            assert!(
                serde_json::from_value::<T>(json!(text)).is_err(),
                "serde should refuse {text:?}"
            );
        }
        assert!(serde_json::from_value::<T>(json!(7)).is_err());
        assert!(serde_json::from_value::<T>(json!(null)).is_err());
    }

    /// The parser, serde and the generated schema reject every text.
    fn assert_rejects<T>(texts: &[&str])
    where
        T: FromStr<Err = FormatError> + JsonSchema + DeserializeOwned + fmt::Debug,
    {
        assert_parser_rejects::<T>(texts);
        let schema = validator::<T>();
        for text in texts {
            assert!(
                !schema.is_valid(&json!(text)),
                "schema should refuse {text:?}"
            );
        }
    }

    /// The parser refuses texts whose fault the schema's `pattern` cannot state.
    fn assert_rejects_beyond_pattern<T>(texts: &[&str])
    where
        T: FromStr<Err = FormatError> + JsonSchema + DeserializeOwned + fmt::Debug,
    {
        assert_parser_rejects::<T>(texts);
        let schema = validator::<T>();
        for text in texts {
            assert!(
                schema.is_valid(&json!(text)),
                "pattern should accept {text:?} and leave it to the parser"
            );
        }
    }

    #[test]
    fn digest_accepts_sha256_with_lowercase_hex() {
        assert_accepts::<Digest>(&[ZERO, &format!("sha256:{HEX}")]);
        let digest = Digest::from_hex(HEX).expect("64 lowercase hex digits");
        assert_eq!(digest.hex(), HEX);
        // BORROW: explicit .as_str() on the text form under test
        assert_eq!(digest.as_str(), format!("sha256:{HEX}"));
        assert_eq!(Digest::try_from(format!("sha256:{HEX}")), Ok(digest));
    }

    #[test]
    fn digest_rejects_other_prefixes_lengths_and_alphabets() {
        let upper = format!("sha256:{}", HEX.to_uppercase());
        // INDEX: HEX has 64 bytes, so a prefix of at most 63 is in range
        let short = format!("sha256:{}", &HEX[..63]);
        let long = format!("sha256:{HEX}0");
        // INDEX: HEX has 64 bytes, so a prefix of at most 63 is in range
        let non_hex = format!("sha256:{}g", &HEX[..63]);
        let spaced = format!("sha256:{HEX} ");
        let newline = format!("sha256:{HEX}\n");
        let upper_prefix = format!("SHA256:{HEX}");
        let sha1 = format!("sha1:{HEX}");
        assert_rejects::<Digest>(&[
            "",
            "sha256:",
            HEX,
            &upper,
            &short,
            &long,
            &non_hex,
            &spaced,
            &newline,
            &upper_prefix,
            &sha1,
        ]);
        assert!(Digest::from_hex("abc").is_err());
        assert!(Digest::from_hex(&format!("sha256:{HEX}")).is_err());
    }

    #[test]
    fn digest_errors_name_the_format_the_problem_and_the_text() {
        let error = "abc".parse::<Digest>().expect_err("no prefix");
        assert_eq!(
            error.to_string(),
            "digest lacks the `sha256:` prefix (got \"abc\")"
        );
    }

    #[test]
    fn instrument_digest_accepts_content_digests_and_git_commits() {
        // INDEX: HEX has 64 bytes, so a prefix of at most 63 is in range
        let git40 = format!("git:{}", &HEX[..40]);
        let git64 = format!("git:{HEX}");
        assert_accepts::<InstrumentDigest>(&[ZERO, &git40, &git64]);
        assert!(
            git40
                .parse::<InstrumentDigest>()
                .expect("git commit")
                .is_git()
        );
        assert!(!ZERO.parse::<InstrumentDigest>().expect("digest").is_git());
    }

    #[test]
    fn instrument_digest_rejects_other_lengths_prefixes_and_alphabets() {
        // INDEX: HEX has 64 bytes, so a prefix of at most 63 is in range
        let git41 = format!("git:{}0", &HEX[..40]);
        // INDEX: HEX has 64 bytes, so a prefix of at most 63 is in range
        let git39 = format!("git:{}", &HEX[..39]);
        // INDEX: HEX has 64 bytes, so a prefix of at most 63 is in range
        let git_upper = format!("git:{}", HEX[..40].to_uppercase());
        // INDEX: HEX has 64 bytes, so a prefix of at most 63 is in range
        let git_non_hex = format!("git:{}g", &HEX[..39]);
        // INDEX: HEX has 64 bytes, so a prefix of at most 63 is in range
        let md5 = format!("md5:{}", &HEX[..32]);
        assert_rejects::<InstrumentDigest>(&[
            "",
            "git:",
            "sha256:",
            "sha256:abc",
            &git41,
            &git39,
            &git_upper,
            &git_non_hex,
            &md5,
            HEX,
        ]);
    }

    #[test]
    fn name_version_accepts_semver_after_the_last_at() {
        assert_accepts::<NameVersion>(&[
            "evaluator@1.2.3",
            "a@0.0.0-rc.1+build.5",
            "x@1.0.0-0A.is.legal",
            "vendor/pkg_name.v2@10.20.30",
            "évaluateur@1.0.0",
        ]);
        let parsed: NameVersion = "tier-a@2.0.1-beta".parse().expect("valid");
        assert_eq!(parsed.name(), "tier-a");
        assert_eq!(parsed.version().to_string(), "2.0.1-beta");
    }

    #[test]
    fn name_version_rejects_missing_parts_bad_names_and_non_semver() {
        assert_rejects::<NameVersion>(&[
            "",
            "noat",
            "@1.0.0",
            "a@",
            "a@1.0",
            "a@1",
            "a@01.0.0",
            "a@1.0.0-",
            "a@1.0.0+",
            "a@v1.0.0",
            "a@1.0.0 ",
            "a@1.0.0\n",
            "a b@1.0.0",
            "a@b@1.0.0",
            "a\u{a0}b@1.0.0",
            "a\u{85}b@1.0.0",
            "a\u{feff}b@1.0.0",
        ]);
    }

    #[test]
    fn name_token_accepts_free_version_tokens() {
        assert_accepts::<NameToken>(&["rikyu@2026.10", "provider@v1+x-y.z", "a@1", "a@latest"]);
        let parsed: NameToken = "bio-opentrons@0.4.2+g1".parse().expect("valid");
        assert_eq!(parsed.name(), "bio-opentrons");
        assert_eq!(parsed.version(), "0.4.2+g1");
    }

    #[test]
    fn name_token_rejects_empty_parts_and_characters_outside_the_token_class() {
        assert_rejects::<NameToken>(&[
            "", "a", "@1", "a@", "a@v 1", "a@v/1", "a@v_1", "a@é", "a@1\n", "a b@1", "a@b@c",
        ]);
    }

    #[test]
    fn rel_path_accepts_posix_paths_without_dot_segments() {
        assert_accepts::<RelPath>(&[
            "a",
            "a/b.txt",
            "outputs/.hidden",
            "a/.../b",
            "dir with space/f",
            "a#b/c",
            "..a",
            "a..",
            "é/ü",
        ]);
    }

    #[test]
    fn rel_path_rejects_absolute_dotted_backslashed_and_empty_segments() {
        assert_rejects::<RelPath>(&[
            "", "/", "/a", "a/", "a//b", ".", "..", "./a", "a/./b", "a/..", "../a", "a/../b",
            "a\\b", "a\n/../b", "a\0b", "\0",
        ]);
    }

    #[test]
    fn timestamp_accepts_rfc3339_with_an_explicit_offset_and_keeps_its_form() {
        assert_accepts::<Timestamp>(&[
            "2026-10-29T12:34:56Z",
            "2026-10-29T12:34:56.5Z",
            "2026-10-29T12:34:56.123456789+09:00",
            "2026-10-29T12:34:56-05:30",
            "2026-10-29T21:34:56+09:00",
            "2026-10-29T12:34:56+23:59",
            "2026-10-29T12:34:56-23:59",
            "2026-10-29T12:34:56+00:00",
            "2026-10-29T23:59:60Z",
        ]);
        let local: Timestamp = "2026-10-29T21:34:56+09:00".parse().expect("valid");
        let utc: Timestamp = "2026-10-29T12:34:56Z".parse().expect("valid");
        assert_eq!(local.instant(), utc.instant());
        assert_ne!(local, utc);
        // BORROW: explicit .as_str() on the text form under test
        assert_eq!(local.as_str(), "2026-10-29T21:34:56+09:00");
    }

    #[test]
    fn timestamp_rejects_a_missing_offset_and_other_lexical_forms() {
        assert_rejects::<Timestamp>(&[
            "",
            "2026-10-29",
            "2026-10-29T12:34:56",
            "2026-10-29 12:34:56Z",
            "2026-10-29t12:34:56Z",
            "2026-10-29T12:34:56z",
            "2026-10-29T12:34:56+0900",
            "2026-10-29T12:34:56+09",
            "2026-10-29T12:34:56.Z",
            "2026-10-29T12:34Z",
            "26-10-29T12:34:56Z",
            "2026-10-29T12:34:56Z ",
            "2026-10-29T12:34:56Z\n",
            "2026-10-29T12:34:56+24:00",
            "2026-10-29T12:34:56-24:00",
            "2026-10-29T12:34:56+25:59",
            "2026-10-29T12:34:56+26:00",
            "2026-10-29T12:34:56+99:99",
            "2026-10-29T12:34:56+09:60",
        ]);
    }

    #[test]
    fn timestamp_rejects_dates_and_times_that_name_no_instant() {
        assert_rejects_beyond_pattern::<Timestamp>(&[
            "2026-13-01T00:00:00Z",
            "2026-02-30T00:00:00Z",
            "2026-10-29T25:00:00Z",
            "2026-10-29T12:61:00Z",
            "2026-10-29T12:34:56.1234567890Z",
        ]);
        assert!(matches!(
            "2026-13-01T00:00:00Z".parse::<Timestamp>(),
            Err(FormatError::TimestampInstant { .. })
        ));
    }

    fn run_key(replicate: u32, attempt: u32) -> RunKey {
        RunKey::new(HEX_DIGEST.parse().expect("digest"), replicate, attempt).expect("counters")
    }

    const HEX_DIGEST: &str =
        "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn run_key_round_trips_through_its_object_and_path_forms() {
        let key = run_key(3, 12);
        assert_eq!(key.to_string(), format!("{HEX}-r3-a12"));
        assert_eq!(key.to_string().parse::<RunKey>(), Ok(key.clone()));
        let object = json!({"cell": HEX_DIGEST, "replicate": 3, "attempt": 12});
        assert_eq!(serde_json::to_value(&key).expect("serde writes"), object);
        assert_eq!(
            serde_json::from_value::<RunKey>(object.clone()).expect("serde reads"),
            key
        );
        assert!(validator::<RunKey>().is_valid(&object));
        assert_eq!((key.replicate(), key.attempt()), (3, 12));
        assert_eq!(key.cell().hex(), HEX);
    }

    #[test]
    fn run_key_accepts_counters_from_one() {
        assert_eq!(run_key(1, 1).to_string(), format!("{HEX}-r1-a1"));
        let largest: RunKey = format!("{HEX}-r4294967295-a4294967295")
            .parse()
            .expect("u32::MAX counters");
        assert_eq!(largest.replicate(), u32::MAX);
        let schema = validator::<RunKey>();
        assert!(schema.is_valid(&json!({"cell": HEX_DIGEST, "replicate": 1, "attempt": 1})));
    }

    #[test]
    fn run_key_rejects_counters_below_one_and_malformed_forms() {
        assert!(RunKey::new(HEX_DIGEST.parse().expect("digest"), 0, 1).is_err());
        assert!(RunKey::new(HEX_DIGEST.parse().expect("digest"), 1, 0).is_err());
        let upper = HEX.to_uppercase();
        // INDEX: HEX has 64 bytes, so a prefix of at most 63 is in range
        let short = &HEX[..63];
        for text in [
            String::new(),
            // BORROW: .to_owned() makes the owned String the array holds
            HEX.to_owned(),
            format!("{HEX}-r1"),
            format!("{HEX}-a1"),
            format!("{HEX}-r0-a1"),
            format!("{HEX}-r1-a0"),
            format!("{HEX}-r01-a1"),
            format!("{HEX}-r1-a+1"),
            format!("{HEX}-r-1-a1"),
            format!("{HEX}-r1-a"),
            format!("{HEX}-r-a1"),
            format!("{HEX}-r4294967296-a1"),
            format!("{upper}-r1-a1"),
            format!("{short}-r1-a1"),
            format!("sha256:{HEX}-r1-a1"),
            format!("{HEX}-r1-a1 "),
        ] {
            assert!(text.parse::<RunKey>().is_err(), "{text:?} should not parse");
        }
        let schema = validator::<RunKey>();
        for object in [
            json!({"cell": HEX_DIGEST, "replicate": 0, "attempt": 1}),
            json!({"cell": HEX_DIGEST, "replicate": 1, "attempt": 0}),
            json!({"cell": HEX_DIGEST, "replicate": 1}),
            json!({"replicate": 1, "attempt": 1}),
            json!({"cell": HEX, "replicate": 1, "attempt": 1}),
            json!({"cell": HEX_DIGEST, "replicate": -1, "attempt": 1}),
            json!({"cell": HEX_DIGEST, "replicate": 1, "attempt": 1, "extra": true}),
        ] {
            assert!(!schema.is_valid(&object), "schema should refuse {object}");
            if object.get("extra").is_none() {
                assert!(
                    serde_json::from_value::<RunKey>(object.clone()).is_err(),
                    "serde should refuse {object}"
                );
            }
        }
    }

    #[test]
    fn run_key_readers_ignore_unknown_fields() {
        let object = json!({"cell": HEX_DIGEST, "replicate": 1, "attempt": 1, "extra": true});
        assert_eq!(
            serde_json::from_value::<RunKey>(object).expect("serde reads"),
            run_key(1, 1)
        );
    }

    proptest! {
        #[test]
        fn run_key_forms_round_trip(
            hex in "[0-9a-f]{64}",
            replicate in 1..=u32::MAX,
            attempt in 1..=u32::MAX,
        ) {
            let key = RunKey::new(Digest::from_hex(&hex).expect("digest"), replicate, attempt)
                .expect("counters");
            prop_assert_eq!(key.to_string().parse::<RunKey>(), Ok(key.clone()));
            let value = serde_json::to_value(&key).expect("serde writes");
            prop_assert_eq!(serde_json::from_value::<RunKey>(value).expect("serde reads"), key);
        }
    }

    #[test]
    fn request_id_accepts_lowercase_hyphenated_uuid_v7() {
        assert_accepts::<RequestId>(&[
            "018f3c1e-7b2a-7c3d-8e4f-0123456789ab",
            "01900000-0000-7000-b000-000000000000",
        ]);
    }

    #[test]
    fn request_id_rejects_other_versions_variants_and_spellings() {
        assert_rejects::<RequestId>(&[
            "",
            "550e8400-e29b-41d4-a716-446655440000",
            "018F3C1E-7B2A-7C3D-8E4F-0123456789AB",
            "018f3c1e7b2a7c3d8e4f0123456789ab",
            "{018f3c1e-7b2a-7c3d-8e4f-0123456789ab}",
            "urn:uuid:018f3c1e-7b2a-7c3d-8e4f-0123456789ab",
            "018f3c1e-7b2a-7c3d-ce4f-0123456789ab",
            "018f3c1e-7b2a-7c3d-8e4f-0123456789a",
            "018f3c1e-7b2a-7c3d-8e4f-0123456789ab ",
            "018f3c1e-7b2a-8c3d-8e4f-0123456789ab",
        ]);
    }

    #[test]
    fn evidence_ref_accepts_event_and_path_references() {
        assert_accepts::<EvidenceRef>(&[
            "event:sess-1#0",
            "event:7f9c2b1e-aaaa-4bbb-8ccc-0123456789ab#42",
            "path:outputs/domain.py",
            "path:outputs/domain.py#L12",
            "path:a/b#L3-9",
            "path:a#L5-5",
            "path:a/.../b",
        ]);
    }

    #[test]
    fn evidence_ref_rejects_malformed_events_paths_and_anchors() {
        assert_rejects::<EvidenceRef>(&[
            "",
            "event:",
            "event:s",
            "event:#1",
            "event:s#",
            "event:s#01",
            "event:s#-1",
            "event:s#1#2",
            "event: s#1",
            "event:s #1",
            "path:",
            "path:/a",
            "path:a/",
            "path:a/../b",
            "path:..#L3",
            "path:a/.#L1",
            "path:a#",
            "path:a#L",
            "path:a#L0",
            "path:a#L01",
            "path:a#L3-",
            "path:a#L-3",
            "path:a#L3-0",
            "path:a#x",
            "path:a#L3#L4",
            "path:a\\b",
            "path:a\0b",
            "file:a",
            "event:s#1\n",
        ]);
    }

    #[test]
    fn evidence_ref_rejects_a_line_range_that_ends_before_it_starts() {
        assert_rejects_beyond_pattern::<EvidenceRef>(&["path:a#L9-3", "path:a#L2-1"]);
    }

    #[test]
    fn expectation_id_accepts_alphanumeric_start_then_dot_underscore_dash() {
        assert_accepts::<ExpectationId>(&["a", "E1", "tier-a.domain_structure", "0x", "a-"]);
    }

    #[test]
    fn expectation_id_rejects_empty_leading_punctuation_and_other_characters() {
        assert_rejects::<ExpectationId>(&["", "-a", ".a", "_a", "a b", "a/b", "é", "a\n", "a@b"]);
    }

    #[test]
    fn reason_code_accepts_lowercase_snake_case() {
        assert_accepts::<ReasonCode>(&["deadline", "not_applicable_to_arm", "a1", "a_", "x"]);
    }

    #[test]
    fn reason_code_rejects_uppercase_leading_digits_and_dashes() {
        assert_rejects::<ReasonCode>(&["", "1a", "_a", "A", "aB", "a-b", "a b", "é", "a\n"]);
    }

    #[test]
    fn name_version_rejects_version_numbers_beyond_64_bits() {
        assert_rejects_beyond_pattern::<NameVersion>(&[
            "a@18446744073709551616.0.0",
            "a@1.18446744073709551616.0",
            "a@1.0.18446744073709551616",
        ]);
        assert!("a@18446744073709551615.0.0".parse::<NameVersion>().is_ok());
    }

    #[test]
    fn evidence_ref_rejects_numbers_beyond_64_bits() {
        assert_rejects_beyond_pattern::<EvidenceRef>(&[
            "event:s#18446744073709551616",
            "path:a#L18446744073709551616",
            "path:a#L1-18446744073709551616",
        ]);
        assert!(
            "event:s#18446744073709551615"
                .parse::<EvidenceRef>()
                .is_ok()
        );
    }

    #[test]
    fn timestamps_order_by_instant_then_by_text() {
        let parse = |text: &str| text.parse::<Timestamp>().expect("valid");
        let tokyo_noon = parse("2026-10-29T12:00:00+09:00");
        let utc_four = parse("2026-10-29T04:00:00Z");
        let utc_three = parse("2026-10-29T03:00:00Z");
        // The instant decides before the text does: the earlier instant has the later text.
        assert!(utc_four.as_str() < tokyo_noon.as_str());
        assert!(tokyo_noon.instant() < utc_four.instant());
        assert!(tokyo_noon < utc_four);
        // The same instant written two ways orders by text, and the two are not equal.
        assert_eq!(tokyo_noon.instant(), utc_three.instant());
        assert!(utc_three.as_str() < tokyo_noon.as_str());
        assert!(utc_three < tokyo_noon);
        assert_ne!(utc_three, tokyo_noon);
    }

    #[test]
    fn name_versions_order_by_name_then_semver() {
        let parse = |text: &str| text.parse::<NameVersion>().expect("valid");
        assert!(parse("a@1.2.0") < parse("a@1.10.0"));
        assert!(parse("a@9.0.0") < parse("b@1.0.0"));
        assert!(parse("a@1.0.0-rc.1") < parse("a@1.0.0"));
    }
}
