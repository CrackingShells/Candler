//! The catalogue of the refusals an author can cause, and the walk that finds the one an error
//! carries.
//!
//! [`catalog`] lists one [`CatalogRow`] per variant of every error type that implements
//! [`Refusal`]: the schemas' types, the core's and the harness interface's. [`find`] walks an
//! error's chain of causes to the first of those types, so a caller that holds a boxed error
//! learns its code, side and path without knowing which verb raised it.

use std::error::Error;

use candler_schemas::refusal::{CatalogRow, Refusal};

/// The rows of every coded error, in the order of the modules: the schemas', then the core's by
/// module, then the harness interface's.
#[must_use]
pub fn catalog() -> Vec<CatalogRow> {
    let mut rows = candler_schemas::refusal::catalog();
    rows.extend(crate::evaluation::refusal::catalog());
    rows.extend(crate::experiment::refusal::catalog());
    rows.extend(crate::frames::refusal::catalog());
    #[cfg(unix)]
    rows.extend(crate::isolation::refusal::catalog());
    #[cfg(unix)]
    rows.extend(crate::jobs::refusal::catalog());
    rows.extend(crate::resolve::refusal::catalog());
    #[cfg(unix)]
    rows.extend(crate::runner::refusal::catalog());
    rows.extend(crate::store::refusal::catalog());
    #[cfg(unix)]
    rows.extend(crate::supervise::refusal::catalog());
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

/// The coded type of the catalogue that `error` is, or `None` when it is none of them.
fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    if let Some(found) = schemas_coded(error) {
        return Some(found);
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
    #[cfg(unix)]
    if let Some(found) = crate::isolation::refusal::coded(error) {
        return Some(found);
    }
    #[cfg(unix)]
    if let Some(found) = crate::jobs::refusal::coded(error) {
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

    error
        .downcast_ref::<ExecutionError>()
        .map(as_refusal)
        .or_else(|| error.downcast_ref::<EvaluationError>().map(as_refusal))
        .or_else(|| error.downcast_ref::<OutcomeError>().map(as_refusal))
        .or_else(|| error.downcast_ref::<FormatError>().map(as_refusal))
        .or_else(|| error.downcast_ref::<CanonicalError>().map(as_refusal))
        .or_else(|| error.downcast_ref::<CatalogError>().map(as_refusal))
        .or_else(|| error.downcast_ref::<ValuesError>().map(as_refusal))
        .or_else(|| error.downcast_ref::<VersionMismatch>().map(as_refusal))
}

/// Views a coded error as its [`Refusal`].
fn as_refusal<'e, E: Refusal>(found: &'e E) -> &'e dyn Refusal {
    found
}
