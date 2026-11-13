//! Recognising a Claude Code stream-json sample by its first line.

use candler_harness::spec::{Confidence, FormatVersion};
use serde_json::{Map, Value};

/// The Claude Code version an init line names, or `None` when the line is not an init line or names no version.
#[must_use]
pub fn init_version(line: &Map<String, Value>) -> Option<semver::Version> {
    let is_init = line.get("type").and_then(Value::as_str) == Some("system")
        && line.get("subtype").and_then(Value::as_str) == Some("init");
    if !is_init {
        return None;
    }
    line.get("claude_code_version")
        .and_then(Value::as_str)
        .and_then(|text| semver::Version::parse(text).ok())
}

/// The Claude Code version the first non-blank line of `sample` names, when that line is an init
/// line; the version may be outside the supported range.
#[must_use]
pub fn sample_version(sample: &[u8]) -> Option<semver::Version> {
    let first = sample
        .split(|byte| *byte == b'\n')
        .find(|line| !line.iter().all(u8::is_ascii_whitespace))?;
    let Value::Object(line) = serde_json::from_slice::<Value>(first).ok()? else {
        return None;
    };
    init_version(&line)
}

/// Recognises a sample whose first non-blank line is an init line of a supported Claude Code version.
///
/// A first line that is cut short, is not JSON, is not an init line, or names a version outside
/// the supported range gives `None`.
#[must_use]
pub fn detect(sample: &[u8]) -> Option<(FormatVersion, Confidence)> {
    let version = sample_version(sample)?;
    crate::supported()
        .matches(&version)
        .then(|| (FormatVersion::new(version), Confidence::Certain))
}
