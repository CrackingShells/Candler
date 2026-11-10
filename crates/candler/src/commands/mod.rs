//! The verbs of the `candler` binary, one file each, and what they share in [`common`].

pub mod common;
pub mod diff;
pub mod evaluate;
pub mod import;
pub mod kill;
pub mod ps;
pub mod run;
pub mod show;
pub mod supervise;
pub mod watch;

use std::error::Error;

/// How a verb that did not fail ended; `main` maps it to the exit code. A change found by `diff`
/// is an `Exit`, never a [`CliError`], so that it never travels through `?` to stderr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Exit {
    /// The verb did its work and found nothing to report: exit 0.
    Success,
    /// `diff` found `count` changed rows: exit 1.
    Changed {
        /// How many differences it found.
        count: usize,
    },
}

/// Why a verb did not run to an answer: exit 2.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CliError {
    /// The request was refused before any work began.
    #[error("refused: {reason}")]
    Refused {
        /// Why it was refused.
        reason: String,
    },
    /// The verb started and failed.
    #[error("`{verb}` failed: {source}")]
    Failed {
        /// The verb.
        verb: &'static str,
        /// The cause.
        // TRAIT_OBJECT: the failures of different crates travel as one type
        source: Box<dyn Error + Send + Sync>,
    },
    /// The process could not run the verb at all.
    #[error("{detail}")]
    Runtime {
        /// What went wrong.
        detail: String,
    },
}

impl CliError {
    /// A [`CliError::Failed`] for `verb` caused by `source`.
    #[must_use]
    pub fn failed<E: Error + Send + Sync + 'static>(verb: &'static str, source: E) -> Self {
        // TRAIT_OBJECT: the failures of different crates travel as one type
        Self::Failed {
            verb,
            source: Box::new(source),
        }
    }

    /// A [`CliError::Refused`] for `reason`.
    #[must_use]
    pub fn refused(reason: impl Into<String>) -> Self {
        Self::Refused {
            reason: reason.into(),
        }
    }
}
