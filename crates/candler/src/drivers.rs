//! The harness drivers this binary ships, selected by the name an experiment's
//! `request.harness` gives.
//!
//! This is the one place the binary names a driver crate: everything else asks the registry for
//! the entry of a harness by name and uses its parts.

use std::collections::BTreeMap;
use std::sync::Arc;

use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::TraceReader;
use candler_harness::skills::SkillTree;
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_harness_claude_code::skills::ClaudeSkills;
use candler_harness_claude_code::{ClaudeCodeError, ClaudeCodeReader};

use crate::commands::CliError;
use crate::commands::refusal::CliRefusal;

/// Why a harness's parts could not be built.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BuildError {
    /// The driver could not be built.
    #[error(transparent)]
    Driver(#[from] DriverError),
    /// The reader could not be built.
    #[error(transparent)]
    Reader(#[from] ClaudeCodeError),
}

/// Builds a new reader of one harness's output.
// TRAIT_OBJECT: a reader is chosen by the harness's name at runtime
pub type ReaderBuilder = fn() -> Result<Box<dyn TraceReader>, BuildError>;

/// What the binary holds of one harness: its driver, its skill layout, the directory under
/// `assets/docker` that builds its base image, and the builder of its reader.
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
    /// Builds a reader of the harness's output.
    pub reader: ReaderBuilder,
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
                reader: claude_code_reader,
            },
        );
        Ok(Self { entries })
    }

    /// The names of the registered harnesses, in order.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.entries.keys().cloned().collect() // BORROW: the names are owned by the caller
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
