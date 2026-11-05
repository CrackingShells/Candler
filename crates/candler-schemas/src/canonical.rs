//! RFC 8785 canonical JSON and the digests computed over it.
//!
//! [`to_jcs`] writes a JSON value as its canonical text: no white space, object keys sorted by
//! UTF-16 code units, strings escaped as RFC 8785 §3.2.2.2 says, and numbers written as
//! ECMAScript's `Number.prototype.toString` writes them. A value that has no canonical text is
//! refused: an integer beyond ±(2^53 − 1) held as an integer, a number that is not finite, and in
//! [`from_text`] a lone surrogate escape or a repeated key.

use std::fmt;

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use crate::generate::GeneratedSchema;

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    Vec::new()
}

/// The largest integer magnitude a canonical number may hold as an integer: 2^53 − 1.
const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

/// Why a JSON value or text has no canonical form.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CanonicalError {
    /// An integer outside ±(2^53 − 1).
    #[error("integer is outside ±(2^53 - 1) (got {value})")]
    IntegerRange {
        /// The integer, in decimal.
        value: String,
    },
    /// A number that is `NaN` or infinite.
    #[error("number is not finite")]
    NotFinite,
    /// A `\u` escape that is half of a surrogate pair without its other half.
    #[error("text holds a lone surrogate escape (at byte {offset})")]
    LoneSurrogate {
        /// The byte offset of the escape's backslash.
        offset: usize,
    },
    /// An object that holds the same key twice.
    #[error("object repeats a key (got {key:?})")]
    DuplicateKey {
        /// The repeated key.
        key: String,
    },
    /// Text that is not JSON.
    #[error("text is not JSON ({detail})")]
    Syntax {
        /// The reader's explanation.
        detail: String,
    },
}

/// Returns the [`CanonicalError::IntegerRange`] of `number` when it is held as an integer outside ±(2^53 − 1), and `None` otherwise.
fn number_problem(number: &Number) -> Option<CanonicalError> {
    let beyond = match (number.as_i64(), number.as_u64()) {
        (Some(signed), _) => signed.unsigned_abs() > MAX_SAFE_INTEGER,
        (None, Some(unsigned)) => unsigned > MAX_SAFE_INTEGER,
        (None, None) => false,
    };
    beyond.then(|| CanonicalError::IntegerRange {
        value: number.to_string(),
    })
}

/// Checks that `value` has a canonical text, without writing it.
///
/// # Errors
/// Returns [`CanonicalError::IntegerRange`] if an integer inside `value` lies outside
/// ±(2^53 − 1).
/// Returns [`CanonicalError::NotFinite`] if a number inside `value` is `NaN` or infinite.
pub fn check(value: &Value) -> Result<(), CanonicalError> {
    match value {
        Value::Number(number) => match number_problem(number) {
            Some(problem) => Err(problem),
            None if number.as_f64().is_some_and(f64::is_finite) => Ok(()),
            None => Err(CanonicalError::NotFinite),
        },
        Value::Array(items) => items.iter().try_for_each(check),
        Value::Object(map) => map.values().try_for_each(check),
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
    }
}

/// Writes `value` as RFC 8785 canonical JSON.
///
/// # Errors
/// Returns [`CanonicalError::IntegerRange`] if an integer inside `value` lies outside
/// ±(2^53 − 1).
/// Returns [`CanonicalError::NotFinite`] if a number inside `value` is `NaN` or infinite.
pub fn to_jcs(value: &Value) -> Result<String, CanonicalError> {
    let mut out = String::new();
    write_value(value, &mut out)?;
    Ok(out)
}

fn write_value(value: &Value, out: &mut String) -> Result<(), CanonicalError> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => write_number(number, out)?,
        Value::String(text) => write_string(text, out),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_value(item, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => write_object(map, out)?,
    }
    Ok(())
}

fn write_object(map: &Map<String, Value>, out: &mut String) -> Result<(), CanonicalError> {
    let mut entries: Vec<(&String, &Value)> = map.iter().collect();
    entries.sort_by(|(left, _), (right, _)| left.encode_utf16().cmp(right.encode_utf16()));
    out.push('{');
    for (index, (key, item)) in entries.into_iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        write_string(key, out);
        out.push(':');
        write_value(item, out)?;
    }
    out.push('}');
    Ok(())
}

/// RFC 8785 §3.2.2.2: `\b \t \n \f \r \" \\` and `\u00xx` below U+0020; everything else as is.
fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            control if control < '\u{20}' => {
                let code = u32::from(control);
                out.push_str("\\u00");
                out.extend(char::from_digit(code >> 4, 16));
                out.extend(char::from_digit(code & 0xf, 16));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// RFC 8785 §3.2.2.3: integers in decimal, other numbers as ECMAScript writes them, `-0` as `0`.
fn write_number(number: &Number, out: &mut String) -> Result<(), CanonicalError> {
    if let Some(problem) = number_problem(number) {
        return Err(problem);
    }
    if let Some(signed) = number.as_i64() {
        out.push_str(&signed.to_string());
        return Ok(());
    }
    if let Some(unsigned) = number.as_u64() {
        out.push_str(&unsigned.to_string());
        return Ok(());
    }
    match number.as_f64() {
        Some(0.0) => out.push('0'),
        Some(float) if float.is_finite() => {
            out.push_str(ryu_js::Buffer::new().format_finite(float));
        }
        Some(_) | None => return Err(CanonicalError::NotFinite),
    }
    Ok(())
}

/// A JSON value read with every object key checked for repetition.
struct Strict(Value);

impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictVisitor)
    }
}

/// The visitor of [`Strict`].
struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Strict;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Strict, E> {
        Ok(Strict(Value::Bool(v)))
    }

    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Strict, E> {
        Ok(Strict(Value::from(v)))
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Strict, E> {
        Ok(Strict(Value::from(v)))
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Strict, E> {
        Number::from_f64(v)
            .map(|number| Strict(Value::Number(number)))
            .ok_or_else(|| E::custom("number is not finite"))
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Strict, E> {
        // BORROW: .to_owned() makes the owned text `Value::String` holds
        Ok(Strict(Value::String(v.to_owned())))
    }

    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Strict, E> {
        Ok(Strict(Value::String(v)))
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Strict, E> {
        Ok(Strict(Value::Null))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Strict, A::Error> {
        let mut items = Vec::new();
        while let Some(Strict(item)) = seq.next_element()? {
            items.push(item);
        }
        Ok(Strict(Value::Array(items)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Strict, A::Error> {
        let mut map = Map::new();
        while let Some(key) = access.next_key::<String>()? {
            if map.contains_key(&key) {
                return Err(serde::de::Error::custom(DuplicateMarker(key)));
            }
            let Strict(item) = access.next_value()?;
            map.insert(key, item);
        }
        Ok(Strict(Value::Object(map)))
    }
}

/// Carries a repeated key through the reader's error text.
struct DuplicateMarker(String);

/// The prefix that tells a reader error caused by [`DuplicateMarker`] from any other.
const DUPLICATE_PREFIX: &str = "duplicate key \u{0}";

impl fmt::Display for DuplicateMarker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{DUPLICATE_PREFIX}{}", self.0)
    }
}

/// Reports whether `byte` can be part of a JSON number token.
const fn is_number_byte(byte: u8) -> bool {
    matches!(byte, b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')
}

/// Reads the four hex digits at `at` as a code unit.
fn hex4(bytes: &[u8], at: usize) -> Option<u16> {
    let digits = bytes.get(at..at.checked_add(4)?)?;
    let text = std::str::from_utf8(digits).ok()?;
    if text.bytes().all(|b| b.is_ascii_hexdigit()) {
        u16::from_str_radix(text, 16).ok()
    } else {
        None
    }
}

/// Scans JSON text for what the reader would turn into something else: a lone surrogate escape,
/// and a decimal integer literal beyond ±(2^53 − 1), which the reader would silently read as a
/// float. Text that is not well formed is left for the reader to refuse.
fn scan(text: &str) -> Result<(), CanonicalError> {
    let bytes = text.as_bytes();
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'"' => at = scan_string(bytes, at.saturating_add(1))?,
            b'-' | b'0'..=b'9' => {
                let end = bytes.get(at..).map_or(at, |rest| {
                    at + rest.iter().take_while(|b| is_number_byte(**b)).count()
                });
                let token = text.get(at..end).unwrap_or_default();
                if !token.contains(['.', 'e', 'E']) {
                    let digits = token.trim_start_matches('-').trim_start_matches('0');
                    let beyond = digits.len() > 16
                        || digits.parse::<u64>().is_ok_and(|n| n > MAX_SAFE_INTEGER);
                    if beyond {
                        return Err(CanonicalError::IntegerRange {
                            // BORROW: .to_owned() keeps the offending token in the error
                            value: token.to_owned(),
                        });
                    }
                }
                at = end.max(at.saturating_add(1));
            }
            _ => at = at.saturating_add(1),
        }
    }
    Ok(())
}

/// Scans the string whose first byte after the opening quote is at `at`; returns the offset after
/// its closing quote.
fn scan_string(bytes: &[u8], mut at: usize) -> Result<usize, CanonicalError> {
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'"' => return Ok(at.saturating_add(1)),
            b'\\' => {
                let start = at;
                if bytes.get(at.saturating_add(1)) == Some(&b'u') {
                    let unit = hex4(bytes, at.saturating_add(2));
                    at = at.saturating_add(6);
                    match unit {
                        Some(0xD800..=0xDBFF) => {
                            let low = (bytes.get(at) == Some(&b'\\')
                                && bytes.get(at.saturating_add(1)) == Some(&b'u'))
                            .then(|| hex4(bytes, at.saturating_add(2)))
                            .flatten();
                            if matches!(low, Some(0xDC00..=0xDFFF)) {
                                at = at.saturating_add(6);
                            } else {
                                return Err(CanonicalError::LoneSurrogate { offset: start });
                            }
                        }
                        Some(0xDC00..=0xDFFF) => {
                            return Err(CanonicalError::LoneSurrogate { offset: start });
                        }
                        Some(_) | None => {}
                    }
                } else {
                    at = at.saturating_add(2);
                }
            }
            _ => at = at.saturating_add(1),
        }
    }
    Ok(at)
}

/// Reads JSON text into a value that has a canonical form.
///
/// # Errors
/// Returns [`CanonicalError::LoneSurrogate`] if a `\u` escape is half of a surrogate pair without
/// the other half.
/// Returns [`CanonicalError::IntegerRange`] if an integer literal lies outside ±(2^53 − 1).
/// Returns [`CanonicalError::DuplicateKey`] if an object repeats a key.
/// Returns [`CanonicalError::Syntax`] if the text is not JSON.
pub fn from_text(text: &str) -> Result<Value, CanonicalError> {
    scan(text)?;
    let mut reader = serde_json::Deserializer::from_str(text);
    let parsed = Strict::deserialize(&mut reader)
        .and_then(|strict| reader.end().map(|()| strict))
        .map_err(|e| {
            let detail = e.to_string();
            match detail.split_once(DUPLICATE_PREFIX) {
                Some((_, rest)) => CanonicalError::DuplicateKey {
                    // The reader appends ` at line L column C` to a custom message.
                    key: rest
                        .rsplit_once(" at line ")
                        .map_or(rest, |(key, _)| key)
                        // BORROW: .to_owned() keeps the repeated key in the error
                        .to_owned(),
                },
                None => CanonicalError::Syntax { detail },
            }
        })?;
    check(&parsed.0)?;
    Ok(parsed.0)
}
