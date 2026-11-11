//! The refusal codes of the errors of importing: a legacy results folder and a raw trace
//! directory.
//!
//! One `refusal_table!` call per error type: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other. The module keeps its name `import` and so do the codes of
//! both enums, whichever verb raises them. The table of `RawIngestError` is in `raw.rs`, beside
//! the enum, and this module lists its rows and finds the type.

use std::error::Error;

use crate::import::ImportError;
use crate::import::raw::RawIngestError;
use candler_schemas::refusal::{CatalogRow, Catalogued, Refusal, Side};
use candler_schemas::refusal_table;

/// The pointer of the place in a document where an [`ImportError`] sits, if it names one.
fn import_error_path(error: &ImportError) -> Option<String> {
    match error {
        ImportError::Format(inner) => Refusal::path(inner),
        ImportError::MissingField { .. }
        | ImportError::Rules { .. }
        | ImportError::Io { .. }
        | ImportError::Json { .. }
        | ImportError::Path { .. }
        | ImportError::NoOutputs { .. }
        | ImportError::DuplicateGrading { .. }
        | ImportError::Layout { .. }
        | ImportError::Verdict { .. }
        | ImportError::Disagree { .. }
        | ImportError::Status { .. }
        | ImportError::Record { .. }
        | ImportError::Store { .. }
        | ImportError::Conflict { .. } => None,
    }
}

refusal_table! {
    ImportError, path = import_error_path;
    MissingField { .. } => row(
        "config.import_field_unset",
        Side::Config,
        "A field the import rules need is not set.",
        "-",
    ),
    Rules { .. } => row(
        "config.import_rule_invalid",
        Side::Config,
        "A rule of the import rules is not valid.",
        "-",
    ),
    Io { .. } => row(
        "import.file_unreadable",
        Side::Environment,
        "A file or directory of the legacy results folder cannot be read.",
        "-",
    ),
    Json { .. } => row(
        "import.json_invalid",
        Side::Environment,
        "A file of the legacy results folder is not valid JSON.",
        "-",
    ),
    Path { .. } => row(
        "import.path_invalid",
        Side::Environment,
        "A path found in the legacy results folder cannot be a path relative to a run.",
        "-",
    ),
    NoOutputs { .. } => row(
        "import.outputs_missing",
        Side::Environment,
        "A run directory of the legacy results folder has no `outputs/` directory.",
        "-",
    ),
    DuplicateGrading { .. } => row(
        "import.grading_repeated",
        Side::Environment,
        "A grading file of a run is in two evaluation directories.",
        "-",
    ),
    Layout { .. } => row(
        "import.layout_invalid",
        Side::Environment,
        "A file of the legacy results folder is not what the legacy format requires.",
        "-",
    ),
    Verdict { .. } => row(
        "import.verdict_unknown",
        Side::Environment,
        "A verdict in the legacy results folder is not `clean`, `incomplete` or `contaminated`.",
        "-",
    ),
    Disagree { .. } => row(
        "import.sources_disagree",
        Side::Environment,
        "Two files of the legacy results folder give different values for one fact of a run.",
        "-",
    ),
    Status { .. } => row(
        "import.status_unknown",
        Side::Environment,
        "A status in the legacy results folder is not one the importer knows.",
        "-",
    ),
    Record { .. } => row(
        "import.record_unmappable",
        Side::Environment,
        "A file of the legacy results folder does not map to a valid record.",
        "-",
    ),
    Store { .. } => row(
        "import.store_failed",
        Side::Environment,
        "The store failed while a run of the legacy results folder was imported.",
        "-",
    ),
    Conflict { .. } => row(
        "import.run_differs",
        Side::Environment,
        "A run that is finished in the store differs from the folder that is imported for it.",
        "-",
    ),
    Format (inner) => delegate(inner, Side::Environment),
}

/// The rows of every coded error of this module, in the order of the types.
#[must_use]
pub(crate) fn catalog() -> Vec<CatalogRow> {
    let tables: [&'static [CatalogRow]; 2] = [
        <ImportError as Catalogued>::rows(),
        <RawIngestError as Catalogued>::rows(),
    ];
    tables
        .iter()
        .flat_map(|rows| rows.iter().copied())
        .collect()
}

/// Views a coded error as its [`Refusal`].
fn as_refusal<'e, E: Refusal>(found: &'e E) -> &'e dyn Refusal {
    found
}

/// The refusal that answers for an [`ImportError`]: a format error it wraps answers with its own
/// code, side and path, and every other variant with its own.
fn import_answer(error: &ImportError) -> &dyn Refusal {
    match error {
        ImportError::Format(inner) => crate::refusal::find(inner).unwrap_or(error),
        ImportError::MissingField { .. }
        | ImportError::Rules { .. }
        | ImportError::Io { .. }
        | ImportError::Json { .. }
        | ImportError::Path { .. }
        | ImportError::NoOutputs { .. }
        | ImportError::DuplicateGrading { .. }
        | ImportError::Layout { .. }
        | ImportError::Verdict { .. }
        | ImportError::Disagree { .. }
        | ImportError::Status { .. }
        | ImportError::Record { .. }
        | ImportError::Store { .. }
        | ImportError::Conflict { .. } => error,
    }
}

/// The coded error of this module that `error` is, or `None` when it is none of them.
#[must_use]
pub(crate) fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    error
        .downcast_ref::<ImportError>()
        .map(import_answer)
        .or_else(|| error.downcast_ref::<RawIngestError>().map(as_refusal))
}
