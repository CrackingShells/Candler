//! The command catalog of an image: which commands the agent can run.

use candler_harness::spec::Cell;
use candler_schemas::catalog::CatalogSnapshot;
use candler_schemas::primitives::Digest;
use futures::future::BoxFuture;

use crate::isolation::model::ImageRef;

/// The catalog of one image and cell.
pub type Catalog = CatalogSnapshot;

/// Why a catalog could not be captured.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CatalogError {
    /// Capture is not implemented.
    #[error("catalog capture is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
}

/// Captures the catalog.
pub trait CatalogProvider: Send + Sync {
    /// The catalog of `image` as `cell` configures it.
    ///
    /// # Errors
    /// [`CatalogError`] when the image cannot be listed.
    fn capture<'a>(
        &'a self,
        image: &'a ImageRef,
        cell: &'a Cell<'_>,
    ) -> BoxFuture<'a, Result<Catalog, CatalogError>>;
}

/// The digest of a catalog.
///
/// # Errors
/// [`CatalogError`] when the catalog cannot be digested.
pub fn catalog_digest(catalog: &Catalog) -> Result<Digest, CatalogError> {
    let _ = catalog;
    Err(CatalogError::Unavailable {
        detail: "catalog_digest".to_owned(),
    })
}

/// The provider that lists the commands inside the image.
#[derive(Debug, Default)]
pub struct CommandCatalogProvider;
