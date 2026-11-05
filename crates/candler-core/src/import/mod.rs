//! Import of a legacy results folder into a Candler store.

mod assemble;
mod assets;
pub mod grading;
mod legacy;
pub mod manifest;
pub mod read;
mod report;
pub mod rules;
mod streams;

use candler_schemas::primitives::FormatError;

/// Why a legacy results folder, or one of its runs, cannot be imported: every variant names the file and, where one applies, the run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ImportError {
    /// A builder was asked to build before a required field was set.
    #[error("import rules field is missing (`{field}` was not set)")]
    MissingField {
        /// The name of the field.
        field: &'static str,
    },
    /// An import rule that cannot be applied, such as a pointer that is not a JSON pointer.
    #[error("import rule is invalid ({detail})")]
    Rules {
        /// What is wrong with the rule.
        detail: String,
    },
    /// A file or directory that cannot be read.
    #[error("cannot read `{path}` ({detail})")]
    Io {
        /// The path, as the reader saw it.
        path: String,
        /// The operating system's explanation.
        detail: String,
    },
    /// A file that is not valid JSON, or a line of a JSONL file that is not.
    #[error("`{file}` is not valid JSON ({detail})")]
    Json {
        /// The path of the file, relative to the folder.
        file: String,
        /// The reader's explanation, with the line number for a JSONL file.
        detail: String,
    },
    /// A path that is not a run-relative path or whose name is not UTF-8.
    #[error("path `{path}` cannot be a run-relative path ({detail})")]
    Path {
        /// The path, as the reader saw it.
        path: String,
        /// What is wrong with it.
        detail: String,
    },
    /// A run that has no `outputs/` directory.
    #[error("run `{run}` has no `outputs/` directory")]
    NoOutputs {
        /// The run, as `<cell>/<config>/run-N`.
        run: String,
    },
    /// A grading file name that two eval directories of one run both hold.
    #[error(
        "grading file `{name}` of run `{run}` is in two eval directories: `{first}` and `{second}`"
    )]
    DuplicateGrading {
        /// The run, as `<cell>/<config>/run-N`.
        run: String,
        /// The file name.
        name: String,
        /// The path of the first file.
        first: String,
        /// The path of the second file.
        second: String,
    },
    /// A file, or part of one, that is not what the legacy format requires.
    #[error("`{file}` of `{run}` is not what the legacy format requires ({detail})")]
    Layout {
        /// The path of the file, relative to the folder.
        file: String,
        /// The run, or the cell or the folder when the file belongs to no run.
        run: String,
        /// What is wrong with it.
        detail: String,
    },
    /// A legacy integrity verdict that the importer does not know.
    #[error(
        "verdict `{verdict}` of run `{run}` in `{file}` is not `clean`, `incomplete` or `contaminated`"
    )]
    Verdict {
        /// The file that holds the verdict.
        file: String,
        /// The run, as `<cell>/<config>/run-N`.
        run: String,
        /// The verdict as written.
        verdict: String,
    },
    /// Two files, or two copies of one record, that state different things about the same run or cell.
    #[error("{what} disagree for `{run}`: `{first}` and `{second}` ({detail})")]
    Disagree {
        /// What the files state, such as `harness versions`.
        what: &'static str,
        /// The run, or the cell, as written in errors.
        run: String,
        /// The first file.
        first: String,
        /// The second file.
        second: String,
        /// The two statements.
        detail: String,
    },
    /// A legacy status that the importer does not know.
    #[error(
        "status `{status}` of `{id}` in `{file}` of run `{run}` is not a legacy status the importer knows"
    )]
    Status {
        /// The file that holds the status.
        file: String,
        /// The run, as `<cell>/<config>/run-N`.
        run: String,
        /// The expectation id.
        id: String,
        /// The status as written.
        status: String,
    },
    /// A value that the Candler records cannot hold, such as a weight that its status refuses.
    #[error("`{file}` of run `{run}` does not map to a valid record ({detail})")]
    Record {
        /// The path of the file, relative to the folder.
        file: String,
        /// The run, as `<cell>/<config>/run-N`.
        run: String,
        /// The record reader's explanation.
        detail: String,
    },
    /// A string format of the shared primitives that does not hold.
    #[error(transparent)]
    Format(#[from] FormatError),
}
