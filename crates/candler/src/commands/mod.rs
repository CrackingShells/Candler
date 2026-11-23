//! The verbs of the `candler` binary, one file each, and what they share in [`common`].

pub mod analyze;
pub mod assets;
pub mod common;
pub mod diff;
pub mod evaluate;
pub mod ingest;
pub mod inspect;
pub mod kill;
pub mod ls;
pub mod preflight;
pub mod ps;
pub mod refusal;
pub mod resolve;
pub mod run;
pub mod show;
pub mod supervise;
pub mod term;
pub mod watch;

use std::error::Error;
use std::path::PathBuf;

use candler_schemas::refusal::Refusal;

/// The rendering a data verb prints when asked, in place of its JSON record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Rendering {
    /// The same record as lines for a person.
    Text,
}

/// The option every data verb shares: JSON is what a verb prints without it.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
pub struct OutputArgs {
    /// Print the output as text instead of its JSON record; a refusal is written as text too (on
    /// a verb with no data to print, only the refusal changes). not an experiment setting: it is
    /// how the output is read.
    #[arg(long, value_enum)]
    pub format: Option<Rendering>,
}

impl OutputArgs {
    /// Whether the text rendering was asked for.
    #[must_use]
    pub const fn text(self) -> bool {
        self.format.is_some()
    }
}

/// Writes `record` as one JSON document and a line feed.
///
/// # Errors
/// [`CliError::Failed`] when the record cannot be serialised or `out` cannot be written.
pub fn write_record<T: serde::Serialize>(
    verb: &'static str,
    // TRAIT_OBJECT: the output is stdout, or a buffer in tests
    out: &mut dyn std::io::Write,
    record: &T,
) -> Result<(), CliError> {
    let text = serde_json::to_string(record).map_err(|e| CliError::failed(verb, e))?;
    writeln!(out, "{text}").map_err(|e| CliError::failed(verb, e))
}

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

/// The exit codes of the binary, each with what it means. `--help` prints them and the contract
/// index lists them.
pub const EXIT_CODES: [(u8, &str); 3] = [
    (0, "the verb did its work and found nothing to report"),
    (1, "`diff` found changed rows"),
    (2, "the verb was refused or failed"),
];

/// The `--help` text that closes the command line's help: one line per exit code.
#[must_use]
pub fn exit_help() -> String {
    let lines: Vec<String> = EXIT_CODES
        .iter()
        .map(|(code, meaning)| format!("  {code}  {meaning}"))
        .collect();
    format!("Exit codes:\n{}", lines.join("\n"))
}

#[cfg(test)]
mod exit_codes_tests {
    use super::{CliError, EXIT_CODES, Exit};

    /// Guards: the binary's one exit mapping is the `EXIT_CODES` table, so `--help` and the
    /// contract index never say another code than the process returns.
    /// Oracle: the table's own rows, read by position.
    #[test]
    fn the_status_mapping_reads_the_exit_code_table() {
        let failure: Result<Exit, CliError> = Err(CliError::Runtime {
            detail: "trouble".to_owned(),
        });
        let seen = [
            crate::status(&Ok(Exit::Success)),
            crate::status(&Ok(Exit::Changed { count: 1 })),
            crate::status(&failure),
        ];
        for (row, status) in EXIT_CODES.iter().zip(seen) {
            assert_eq!(row.0, status, "{}", row.1);
        }
    }
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
    /// The request was refused before any work began, by an error that has a code.
    #[error("refused: {lead}{refusal}")]
    Coded {
        /// The refusal, which gives the record its code, side and path.
        // TRAIT_OBJECT: the refusals of different crates travel as one type
        refusal: Box<dyn Carried>,
        /// The words the text form puts before the refusal's own, or nothing.
        lead: String,
        /// The file the verb read, named in the record when the refusal has no pointer.
        file: Option<PathBuf>,
        /// The pointer into the parsed document, when the verb knows a narrower place than the
        /// refusal does.
        pointer: Option<String>,
    },
}

/// An error that carries a refusal code: what a verb may hand to [`CliError::refused_by`].
pub trait Carried: Refusal + Error + Send + Sync + 'static {}

impl<E: Refusal + Error + Send + Sync + 'static> Carried for E {}

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

    /// A [`CliError::Coded`] for `refusal`: it prints as `refused: <refusal>`, and its record
    /// carries the refusal's code, side and path.
    #[must_use]
    pub fn refused_by<E: Carried>(refusal: E) -> Self {
        // TRAIT_OBJECT: the refusals of different crates travel as one type
        Self::Coded {
            refusal: Box::new(refusal),
            lead: String::new(),
            file: None,
            pointer: None,
        }
    }

    /// Names the file the verb read. The record names it when the refusal has no pointer into the
    /// document; an error that carries no coded refusal is returned as it is.
    #[must_use]
    pub fn with_file(self, path: impl Into<PathBuf>) -> Self {
        match self {
            Self::Coded {
                refusal,
                lead,
                pointer,
                ..
            } => Self::Coded {
                refusal,
                lead,
                file: Some(path.into()),
                pointer,
            },
            other @ (Self::Refused { .. } | Self::Failed { .. } | Self::Runtime { .. }) => other,
        }
    }

    /// Narrows the place of the refusal to `pointer`, an RFC 6901 pointer into the document the
    /// verb read; an error that carries no coded refusal is returned as it is.
    #[must_use]
    pub fn with_pointer(self, pointer: String) -> Self {
        match self {
            Self::Coded {
                refusal,
                lead,
                file,
                ..
            } => Self::Coded {
                refusal,
                lead,
                file,
                pointer: Some(pointer),
            },
            other @ (Self::Refused { .. } | Self::Failed { .. } | Self::Runtime { .. }) => other,
        }
    }

    /// Puts `lead` before the refusal's own words in the text form; an error that carries no
    /// coded refusal is returned as it is.
    #[must_use]
    pub fn led_by(self, lead: String) -> Self {
        match self {
            Self::Coded {
                refusal,
                file,
                pointer,
                ..
            } => Self::Coded {
                refusal,
                lead,
                file,
                pointer,
            },
            other @ (Self::Refused { .. } | Self::Failed { .. } | Self::Runtime { .. }) => other,
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
