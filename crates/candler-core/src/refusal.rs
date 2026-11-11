//! The catalogue of the refusals an author can cause, and the walk that finds the one an error
//! carries.
//!
//! [`catalog`] lists one [`CatalogRow`] per variant of every error type that implements
//! [`Refusal`]: the schemas' types, the core's and the harness interface's. [`find`] walks an
//! error's chain of causes to the first of those types, so a caller that holds a boxed error
//! learns its code, side and path without knowing which verb raised it.

use std::error::Error;

use candler_schemas::refusal::{CatalogRow, Refusal, Side};

/// The rows of every coded error, in the order of the modules: the schemas', then the core's by
/// module, then the harness interface's.
#[must_use]
pub fn catalog() -> Vec<CatalogRow> {
    let mut rows = candler_schemas::refusal::catalog();
    rows.extend(document_value_rows());
    rows.extend(
        <crate::analysis::refusal::ReportError as candler_schemas::refusal::Catalogued>::rows()
            .iter()
            .copied(),
    );
    rows.extend(crate::evaluation::refusal::catalog());
    rows.extend(crate::experiment::refusal::catalog());
    rows.extend(crate::frames::refusal::catalog());
    rows.extend(crate::import::refusal::catalog());
    #[cfg(unix)]
    rows.extend(crate::isolation::refusal::catalog());
    #[cfg(unix)]
    rows.extend(crate::jobs::refusal::catalog());
    rows.extend(crate::manifest::refusal::catalog());
    rows.extend(crate::resolve::refusal::catalog());
    #[cfg(unix)]
    rows.extend(crate::runner::refusal::catalog());
    rows.extend(crate::store::refusal::catalog());
    #[cfg(unix)]
    rows.extend(crate::supervise::refusal::catalog());
    rows.extend(crate::trace::refusal::catalog());
    rows.extend(candler_harness::refusal::catalog());
    rows
}

/// The first error in the chain of `error` (`error` itself, then its causes) that is a coded
/// type of the catalogue, or `None` when the chain holds none.
///
/// A type that wraps another and adds nothing hides it from the chain and answers with the
/// code and path of what it wraps, so it is found in place of it.
#[must_use]
pub fn find<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    let mut current: Option<&'e (dyn Error + 'static)> = Some(error);
    while let Some(link) = current {
        if let Some(found) = coded(link) {
            return Some(found);
        }
        current = link.source();
    }
    None
}

/// The refusal that answers for a variant of `wrapper` that only wraps `inner`: what the walk of
/// causes finds from `inner`, with the code, the side and the path of the cause, or `wrapper`
/// when the walk finds nothing. A cause that blames Candler answers as it is, with its own code
/// and the side `candler`.
///
/// Every record then carries a code with the side `codes.toml` registers for it. A wrapper of a
/// `FormatError`, which is a check of a value with no context, is not one of these: it has a
/// row of its own, with the code and the side of the document the value came from.
///
/// Each module's `coded` hands the walk a wrapper's inner error through this function. Asking a
/// wrapper for its `side` directly still gives the side of its table row.
pub(crate) fn through<'e, E: Error + 'static>(
    inner: &'e E,
    wrapper: &'e dyn Refusal,
) -> &'e dyn Refusal {
    find(inner).unwrap_or(wrapper)
}

/// A refusal of a wrapper of the schemas crate that has a code of its own: the schemas' impl of
/// the wrapper answers with the code of the value it wraps, which says nothing of the document
/// the value was read from. It names no place in a document.
struct DocumentValue {
    /// The code, `<surface>.value_invalid`.
    code: &'static str,
    /// The side of the document.
    side: Side,
}

impl Refusal for DocumentValue {
    fn code(&self) -> candler_schemas::refusal::RefusalCode {
        candler_schemas::refusal::RefusalCode::from_checked(self.code)
    }

    fn side(&self) -> Side {
        self.side
    }

    fn path(&self) -> Option<String> {
        None
    }
}

/// The refusal of an experiment record that holds a value of the wrong form.
static EXECUTION_VALUE: DocumentValue = DocumentValue {
    code: "experiment.value_invalid",
    side: Side::Experiment,
};

/// The refusal of an outcome record that holds a value of the wrong form.
static OUTCOME_VALUE: DocumentValue = DocumentValue {
    code: "outcome.value_invalid",
    side: Side::Plugin,
};

/// The rows of the two schemas wrappers that [`EXECUTION_VALUE`] and [`OUTCOME_VALUE`] answer for.
fn document_value_rows() -> [CatalogRow; 2] {
    let row = |enum_name, found: &DocumentValue, when| CatalogRow {
        enum_name,
        variant: "Format",
        code: found.code,
        side: found.side,
        when,
        at: "-",
        delegates: false,
    };
    [
        row(
            "ExecutionError",
            &EXECUTION_VALUE,
            "A value of an experiment record has the wrong form for what it names, such as a name with a space.",
        ),
        row(
            "OutcomeError",
            &OUTCOME_VALUE,
            "A value of an outcome record has the wrong form for what it names, such as a name with a space.",
        ),
    ]
}

/// The coded type of the catalogue that `error` is, or `None` when it is none of them.
fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    if let Some(found) = schemas_coded(error) {
        return Some(found);
    }
    if let Some(found) = error.downcast_ref::<crate::analysis::refusal::ReportError>() {
        return Some(as_refusal(found));
    }
    if let Some(found) = crate::evaluation::refusal::coded(error) {
        return Some(found);
    }
    if let Some(found) = crate::experiment::refusal::coded(error) {
        return Some(found);
    }
    if let Some(found) = crate::frames::refusal::coded(error) {
        return Some(found);
    }
    if let Some(found) = crate::import::refusal::coded(error) {
        return Some(found);
    }
    #[cfg(unix)]
    if let Some(found) = crate::isolation::refusal::coded(error) {
        return Some(found);
    }
    #[cfg(unix)]
    if let Some(found) = crate::jobs::refusal::coded(error) {
        return Some(found);
    }
    if let Some(found) = crate::manifest::refusal::coded(error) {
        return Some(found);
    }
    if let Some(found) = crate::resolve::refusal::coded(error) {
        return Some(found);
    }
    #[cfg(unix)]
    if let Some(found) = crate::runner::refusal::coded(error) {
        return Some(found);
    }
    if let Some(found) = crate::store::refusal::coded(error) {
        return Some(found);
    }
    #[cfg(unix)]
    if let Some(found) = crate::supervise::refusal::coded(error) {
        return Some(found);
    }
    if let Some(found) = crate::trace::refusal::coded(error) {
        return Some(found);
    }
    candler_harness::refusal::coded(error)
}

/// The coded type of the schemas crate that `error` is, or `None`.
fn schemas_coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    use candler_schemas::canonical::CanonicalError;
    use candler_schemas::catalog::CatalogError;
    use candler_schemas::evaluation::{EvaluationError, VersionMismatch};
    use candler_schemas::execution::ExecutionError;
    use candler_schemas::outcome::OutcomeError;
    use candler_schemas::primitives::FormatError;
    use candler_schemas::values::ValuesError;

    if let Some(found) = error.downcast_ref::<ExecutionError>() {
        return Some(execution_answer(found));
    }
    if let Some(found) = error.downcast_ref::<EvaluationError>() {
        return Some(evaluation_answer(found));
    }
    if let Some(found) = error.downcast_ref::<OutcomeError>() {
        return Some(outcome_answer(found));
    }
    if let Some(found) = error.downcast_ref::<ValuesError>() {
        return Some(values_answer(found));
    }
    error
        .downcast_ref::<FormatError>()
        .map(as_refusal)
        .or_else(|| error.downcast_ref::<CanonicalError>().map(as_refusal))
        .or_else(|| error.downcast_ref::<CatalogError>().map(as_refusal))
        .or_else(|| error.downcast_ref::<VersionMismatch>().map(as_refusal))
}

/// The refusal that answers for an [`ExecutionError`]: a format error it wraps answers with the
/// code of the document, and a values or canonical error with its own code, side and path.
fn execution_answer(error: &candler_schemas::execution::ExecutionError) -> &dyn Refusal {
    use candler_schemas::execution::ExecutionError;
    if let ExecutionError::Format(..) = error {
        &EXECUTION_VALUE
    } else if let ExecutionError::Values(inner) = error {
        through(inner, error)
    } else if let ExecutionError::Canonical(inner) = error {
        through(inner, error)
    } else {
        error
    }
}

/// The refusal that answers for an [`EvaluationError`]: a canonical error it wraps answers with
/// its own code, side and path.
fn evaluation_answer(error: &candler_schemas::evaluation::EvaluationError) -> &dyn Refusal {
    use candler_schemas::evaluation::EvaluationError;
    if let EvaluationError::Canonical(inner) = error {
        through(inner, error)
    } else {
        error
    }
}

/// The refusal that answers for an [`OutcomeError`]: a format error it wraps answers with the code
/// of the document.
fn outcome_answer(error: &candler_schemas::outcome::OutcomeError) -> &dyn Refusal {
    use candler_schemas::outcome::OutcomeError;
    if let OutcomeError::Format(..) = error {
        &OUTCOME_VALUE
    } else {
        error
    }
}

/// The refusal that answers for a [`ValuesError`]: a canonical error it wraps answers with its
/// own code, side and path.
fn values_answer(error: &candler_schemas::values::ValuesError) -> &dyn Refusal {
    use candler_schemas::values::ValuesError;
    if let ValuesError::Canonical(inner) = error {
        through(inner, error)
    } else {
        error
    }
}

/// Views a coded error as its [`Refusal`].
fn as_refusal<'e, E: Refusal>(found: &'e E) -> &'e dyn Refusal {
    found
}
