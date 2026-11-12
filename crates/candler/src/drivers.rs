//! The harness drivers this binary ships, selected by the name an experiment's
//! `request.harness` gives.
//!
//! This is the one place the binary names a driver crate: everything else asks the registry for
//! the entry of a harness by name and uses its parts.

use std::collections::BTreeMap;
use std::str::FromStr as _;
use std::sync::Arc;

use candler_core::import::raw::Recognised;
use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::TraceReader;
use candler_harness::skills::SkillTree;
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_harness_claude_code::skills::ClaudeSkills;
use candler_harness_claude_code::{ClaudeCodeError, ClaudeCodeReader, detect};
use candler_harness_opencode::driver::OpenCodeDriver;
use candler_harness_opencode::live::OpenCodeLiveReader;
use candler_harness_opencode::skills::OpenCodeSkills;
use candler_harness_opencode::{OpenCodeError, SUPPORTED as OPENCODE_SUPPORTED};
use candler_schemas::primitives::Name;
use candler_schemas::raw::StreamName;
use serde_json::Value;

use crate::commands::CliError;
use crate::commands::refusal::CliRefusal;

/// Why a harness's parts could not be built.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BuildError {
    /// The driver could not be built.
    #[error(transparent)]
    Driver(#[from] DriverError),
    /// The reader of Claude Code could not be built.
    #[error(transparent)]
    Reader(#[from] ClaudeCodeError),
    /// The driver or a reader of `OpenCode` could not be built.
    #[error(transparent)]
    OpenCode(#[from] OpenCodeError),
}

/// Builds a new reader of one harness's output.
// TRAIT_OBJECT: a reader is chosen by the harness's name at runtime
pub type ReaderBuilder = fn() -> Result<Box<dyn TraceReader>, BuildError>;

/// Recognises the start of a raw stream of one harness: the version the stream names and whether
/// the harness's reader supports it, or `None` when the stream is not the harness's. The name of
/// the harness is the registry's, the one its driver gives.
pub type Detector = fn(&[u8]) -> Option<(semver::Version, bool)>;

/// What the binary holds of one harness: its driver, its skill layout, the directory under
/// `assets/docker` that builds its base image, the builder of its reader and the recogniser of
/// its raw streams.
#[derive(Clone)]
pub struct DriverEntry {
    /// What the runner needs from the harness, as data.
    // TRAIT_OBJECT: the driver is chosen by the harness's name at runtime
    pub driver: Arc<dyn HarnessDriver>,
    /// Where a treatment's skill tree comes from for this harness.
    // TRAIT_OBJECT: the skill layout is chosen by the harness's name at runtime
    pub skills: Arc<dyn SkillTree>,
    /// The directory under `assets/docker` that holds the build context of the base image.
    pub base_context: &'static str,
    /// The streams the harness collects after a run, each read offline by its driver's reader.
    pub offline_streams: &'static [&'static str],
    /// Builds a reader of the harness's output.
    pub reader: ReaderBuilder,
    /// Recognises a raw stream of the harness from its first line.
    pub detect: Detector,
}

/// The harnesses this binary ships, by the name of their driver.
pub struct Registry {
    entries: BTreeMap<String, DriverEntry>,
}

/// A reader of Claude Code's stream-json output.
fn claude_code_reader() -> Result<Box<dyn TraceReader>, BuildError> {
    // TRAIT_OBJECT: the reader is handed out as the trait it implements
    Ok(Box::new(ClaudeCodeReader::new()?))
}

/// Recognises a Claude Code stream: its first non-blank line is an init line that names a version.
fn claude_code_detect(sample: &[u8]) -> Option<(semver::Version, bool)> {
    let line = sample
        .split(|byte| *byte == b'\n')
        .find(|line| !line.iter().all(u8::is_ascii_whitespace))?;
    let Value::Object(map) = serde_json::from_slice::<Value>(line).ok()? else {
        return None;
    };
    let version = detect::init_version(&map)?;
    let supported = detect::supports(&version);
    Some((version, supported))
}

/// A reader of `OpenCode`'s live NDJSON.
fn opencode_reader() -> Result<Box<dyn TraceReader>, BuildError> {
    // TRAIT_OBJECT: the reader is handed out as the trait it implements
    Ok(Box::new(OpenCodeLiveReader::new()?))
}

/// Recognises an `OpenCode` stream: its first non-blank line is an event of a kind the reader
/// knows that names a session. No stream names the version that wrote it, so the version is the
/// one the reader is written against, which the driver accepts.
fn opencode_detect(sample: &[u8]) -> Option<(semver::Version, bool)> {
    let reader = OpenCodeLiveReader::new().ok()?;
    let (format, _) = candler_harness::reader::TraceReader::detect(&reader, sample)?;
    let version = format.as_version().clone(); // BORROW: .clone() copies the version
    let supported = semver::VersionReq::parse(OPENCODE_SUPPORTED)
        .is_ok_and(|requirement| requirement.matches(&version));
    Some((version, supported))
}

impl Registry {
    /// The registry of the harnesses this binary ships.
    ///
    /// # Errors
    /// [`BuildError`] when a shipped driver cannot be built, which no constant of the binary
    /// causes.
    pub fn builtin() -> Result<Self, BuildError> {
        let driver = ClaudeCodeDriver::new()?;
        let name = driver.id().to_string();
        let mut entries = BTreeMap::new();
        entries.insert(
            name,
            DriverEntry {
                driver: Arc::new(driver),
                skills: Arc::new(ClaudeSkills),
                base_context: "base",
                offline_streams: &[],
                reader: claude_code_reader,
                detect: claude_code_detect,
            },
        );
        let opencode = OpenCodeDriver::new()?;
        entries.insert(
            opencode.id().to_string(),
            DriverEntry {
                driver: Arc::new(opencode),
                skills: Arc::new(OpenCodeSkills),
                base_context: "base-opencode",
                offline_streams: &[candler_harness_opencode::EXPORT_STREAM],
                reader: opencode_reader,
                detect: opencode_detect,
            },
        );
        Ok(Self { entries })
    }

    /// The names of the registered harnesses, in order.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.entries.keys().cloned().collect() // BORROW: the names are owned by the caller
    }

    /// The streams the harness `name` collects after a run, each with the offline reader its
    /// driver gives it. A harness that is not registered, or that collects none, gives none.
    // TRAIT_OBJECT: the readers are the drivers', chosen by the harness's name at runtime
    #[must_use]
    pub fn offline(&self, name: &str) -> Vec<(StreamName, Box<dyn TraceReader>)> {
        let Some(entry) = self.entries.get(name) else {
            return Vec::new();
        };
        entry
            .offline_streams
            .iter()
            .filter_map(|stream| {
                let stream: StreamName = stream.parse().ok()?;
                let reader = entry.driver.offline_reader(&stream)?;
                Some((stream, reader))
            })
            .collect()
    }

    /// What the first harness that recognises `sample`, the start of a raw stream, says of it. The
    /// harness is named as its driver names itself.
    #[must_use]
    pub fn recognise(&self, sample: &[u8]) -> Option<Recognised> {
        self.entries.iter().find_map(|(name, entry)| {
            let (version, supported) = (entry.detect)(sample)?;
            Some(Recognised::new(
                Name::from_str(name).ok()?,
                version,
                supported,
            ))
        })
    }

    /// The entry of the harness `name`.
    ///
    /// # Errors
    /// [`CliError::Coded`] carrying [`CliRefusal::UnknownHarness`], naming the harness and the
    /// registered ones, when no harness has that name.
    pub fn entry(&self, name: &str) -> Result<DriverEntry, CliError> {
        self.entries.get(name).cloned().ok_or_else(|| {
            CliError::refused_by(CliRefusal::UnknownHarness {
                name: name.to_owned(), // BORROW: the refusal owns the name
                registered: self.names().join(", "),
            })
        })
    }
}
