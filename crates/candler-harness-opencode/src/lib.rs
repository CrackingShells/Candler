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

#![deny(clippy::allow_attributes)]

pub mod driver;
pub mod export;
pub mod live;
mod prepare;

use candler_harness::reader::ReaderError;
use candler_harness::spec::OPENCODE;
use candler_schemas::primitives::Name;
use candler_schemas::trace::SessionSource;

/// The `OpenCode` version this driver supports: the one the recorded runs came from. No stream
/// names the version of the binary that wrote it, so the readers can only stand for one version,
/// and the driver refuses any other.
pub const SUPPORTED: &str = "=2.2.7";

/// The version the readers are written against, and the only one the driver accepts. A session's
/// `harness_version` is this one, which is true because the driver refuses every other.
pub const VALIDATED: semver::Version = semver::Version::new(2, 2, 7);

/// The reader of the live NDJSON, as the trace header names it, `opencode-ndjson@<reader version>`; the version changes whenever the reader's output does.
pub const LIVE_READER: &str = "opencode-ndjson@0.1.0";

/// Defines [`EXPORT_READER`] and its version from the one spelling of the version, so the two
/// cannot differ.
macro_rules! export_reader {
    ($major:literal, $minor:literal, $patch:literal) => {
        /// The reader of the session export, as the trace header names it,
        /// `opencode-export@<reader version>`; the version changes whenever the reader's output does.
        pub const EXPORT_READER: &str =
            concat!("opencode-export@", $major, ".", $minor, ".", $patch);

        /// The version of [`EXPORT_READER`].
        const EXPORT_READER_VERSION: semver::Version = semver::Version::new($major, $minor, $patch);
    };
}

export_reader!(0, 2, 0);

/// The stream name under which the export is collected.
pub const EXPORT_STREAM: &str = "export";

/// The `OpenCode` version the driver supports as a requirement: [`SUPPORTED`] written out, so no
/// text has to parse.
#[must_use]
pub fn supported() -> semver::VersionReq {
    semver::VersionReq {
        comparators: vec![semver::Comparator {
            op: semver::Op::Exact,
            major: VALIDATED.major,
            minor: Some(VALIDATED.minor),
            patch: Some(VALIDATED.patch),
            pre: semver::Prerelease::EMPTY,
        }],
    }
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
        name(OPENCODE)?,
        VALIDATED,
        name(reader.name())?,
        reader.version().clone(), // BORROW: .clone() copies the reader version
    ))
}
