//! The report of an import: what was imported, and every difference between the legacy folder and the Candler records, each with its explanation.

use candler_schemas::execution::InvalidReason;
use candler_schemas::primitives::{RelPath, RunKey};
use serde::Serialize;

use super::ImportError;
use super::assets::TreeDiff;
use super::read::SkipReason;
use super::streams::{TraceFailure, UsageDiff};

/// A run that does not count, with the reason and what the importer did about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct InvalidRun {
    /// The run.
    pub run: RunKey,
    /// Why the run is invalid.
    pub reason: InvalidReason,
    /// The legacy verdict behind the reason, and what the import keeps of the run.
    pub explanation: String,
}

impl InvalidRun {
    /// An invalid run.
    #[must_use]
    pub const fn new(run: RunKey, reason: InvalidReason, explanation: String) -> Self {
        Self {
            run,
            reason,
            explanation,
        }
    }
}

/// A file or directory of the folder that the import leaves out, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[non_exhaustive]
pub struct SkippedEntry {
    /// The path relative to the folder.
    pub path: RelPath,
    /// The reason; its JSON spelling is `apple_double`, `finder_metadata`, `archive_metadata` or `shared_inputs`.
    pub reason: SkipReason,
    /// What the entry is and why it is not imported.
    pub explanation: String,
}

impl SkippedEntry {
    /// A skipped entry.
    #[must_use]
    pub const fn new(path: RelPath, reason: SkipReason, explanation: String) -> Self {
        Self {
            path,
            reason,
            explanation,
        }
    }
}

/// What an import did: the number of runs and outcome records, and every difference with its explanation, each list sorted.
///
/// The report is computed from the folder and the stored files, so a re-import of a finished store
/// returns an equal report.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct ImportReport {
    /// The number of runs of the folder.
    pub runs: u64,
    /// The number of outcome records the folder's valid runs have.
    pub outcomes: u64,
    /// The runs that do not count, sorted by run.
    pub invalid_runs: Vec<InvalidRun>,
    /// The runs whose stream could not be normalised, sorted by run.
    pub trace_failures: Vec<TraceFailure>,
    /// The token counters whose sums differ between the trace and the timing file, sorted by run and counter.
    pub usage_diffs: Vec<UsageDiff>,
    /// The outputs that `outputs/` and the timing file do not share, sorted by run and path.
    pub tree_diffs: Vec<TreeDiff>,
    /// The entries of the folder that the import leaves out, sorted by path.
    pub skipped: Vec<SkippedEntry>,
}

impl ImportReport {
    /// An empty report.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            runs: 0,
            outcomes: 0,
            invalid_runs: Vec::new(),
            trace_failures: Vec::new(),
            usage_diffs: Vec::new(),
            tree_diffs: Vec::new(),
            skipped: Vec::new(),
        }
    }

    /// Sorts every list.
    pub(super) fn sort(&mut self) {
        self.invalid_runs.sort_by(|a, b| a.run.cmp(&b.run));
        self.trace_failures.sort();
        self.usage_diffs.sort();
        self.tree_diffs.sort();
        self.skipped.sort();
    }

    /// The report as pretty-printed JSON with a trailing line feed.
    ///
    /// # Errors
    /// Returns [`ImportError::Json`] if the report cannot be serialised, which no report's content causes.
    pub fn to_json(&self) -> Result<String, ImportError> {
        let mut text = serde_json::to_string_pretty(self).map_err(|e| ImportError::Json {
            file: "import report".into(),
            detail: e.to_string(),
        })?;
        text.push('\n');
        Ok(text)
    }
}

impl Default for ImportReport {
    fn default() -> Self {
        Self::new()
    }
}
