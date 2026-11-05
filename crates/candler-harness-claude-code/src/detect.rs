//! Recognising a Claude Code stream-json sample by its first line.

use std::sync::LazyLock;

use candler_harness::spec::{Confidence, FormatVersion};
use semver::VersionReq;
use serde_json::{Map, Value};

/// The supported range as a requirement, parsed from [`SUPPORTED`](crate::SUPPORTED); `None` only if that text is not a requirement, which then supports no version.
static RANGE: LazyLock<Option<VersionReq>> =
    LazyLock::new(|| VersionReq::parse(crate::SUPPORTED).ok());

/// Reports whether `version` is in the supported range [`SUPPORTED`](crate::SUPPORTED), `>=2.1.220, <2.2.0`; a pre-release version is outside it.
#[must_use]
pub fn supports(version: &semver::Version) -> bool {
    RANGE.as_ref().is_some_and(|range| range.matches(version))
}

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

/// Recognises a sample whose first non-blank line is an init line of a supported Claude Code version.
///
/// A first line that is cut short, is not JSON, is not an init line, or names a version outside
/// the supported range gives `None`.
#[must_use]
pub fn detect(sample: &[u8]) -> Option<(FormatVersion, Confidence)> {
    let first = sample
        .split(|byte| *byte == b'\n')
        .find(|line| !line.iter().all(u8::is_ascii_whitespace))?;
    let Value::Object(line) = serde_json::from_slice::<Value>(first).ok()? else {
        return None;
    };
    let version = init_version(&line)?;
    supports(&version).then(|| (FormatVersion::new(version), Confidence::Certain))
}
