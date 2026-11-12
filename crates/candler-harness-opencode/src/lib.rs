//! Candler harness for `OpenCode` V2.
//!
//! `OpenCode` runs as `opencode run --standalone --format json`, stores its sessions in SQLite and
//! hands them out as JSON through `opencode session export <id>`. The recorded runs that the
//! driver is written against are under `tests/fixtures/streams/`, and
//! `tests/fixtures/PROVENANCE.toml` names the binary they came from.
//!
//! - [`driver::OpenCodeDriver`] describes the run: the command, the configuration file, the state
//!   paths, the in-container interrupt, the export collect and how a run ended.
//! - [`live::OpenCodeLiveReader`] reads the NDJSON on stdout for the session id and the last word.
//! - [`export::OpenCodeExportReader`] reads the export, which is the authoritative trace.
//! - [`skills::OpenCodeSkills`] places a treatment's skill tree where `OpenCode` reads it.

#![deny(clippy::allow_attributes)]

pub mod driver;
pub mod export;
pub mod live;
mod prepare;
pub mod skills;

use candler_harness::reader::ReaderError;
use candler_schemas::primitives::Name;
use candler_schemas::trace::SessionSource;

/// The `OpenCode` version this driver supports: the one the recorded runs came from. No stream
/// names the version of the binary that wrote it, so the readers can only stand for one version,
/// and the driver refuses any other.
pub const SUPPORTED: &str = "=2.2.7";

/// The version the readers are written against, and the only one the driver accepts. A session's
/// `harness_version` is this one, which is true because the driver refuses every other.
pub const VALIDATED: semver::Version = semver::Version::new(2, 2, 7);

/// The reader of the live NDJSON, as the trace header names it.
pub const LIVE_READER: &str = "opencode-ndjson@0.1.0";

/// The reader of the session export, as the trace header names it.
pub const EXPORT_READER: &str = "opencode-export@0.1.0";

/// The stream name under which the gateway's census is stored.
pub const CENSUS_STREAM: &str = "gateway_census";

/// The name of the harness.
pub const HARNESS: &str = "opencode";

/// The stream name under which the export is collected.
pub const EXPORT_STREAM: &str = "export";

/// Why a reader or the driver could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OpenCodeError {
    /// A reader identity or harness name that is not a valid name.
    #[error("the identity is not a valid name ({detail})")]
    Identity {
        /// What is wrong with it.
        detail: String,
    },
}

/// The source every session of this crate carries.
pub(crate) fn session_source(
    reader: &candler_schemas::primitives::NameVersion,
) -> Result<SessionSource, ReaderError> {
    let name = |text: &str| {
        text.parse::<Name>()
            .map_err(|error| ReaderError::Incomplete {
                detail: error.to_string(), // BORROW: .to_string() gives owned text
            })
    };
    Ok(SessionSource::new(
        name(HARNESS)?,
        VALIDATED,
        name(reader.name())?,
        reader.version().clone(), // BORROW: .clone() copies the reader version
    ))
}
