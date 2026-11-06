//! The catalog step: the commands the agent can run in the image, as the cell configures it.
//!
//! The cell holds only the digest of the catalog, in `snapshots`. The catalog itself belongs to
//! the run evidence.

use std::str::FromStr;
use std::sync::Arc;

use candler_harness::spec::Cell;
use candler_schemas::execution::DigestRef;
use candler_schemas::primitives::{FieldPath, FormatError, Name, NameToken};
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;

use crate::experiment::paths;
use crate::isolation::model::{ImageId, ImageRef};
use crate::resolve::preflight::{PreflightError, PreflightStep, ResolveContext, StepReport};
use crate::runner::evidence::catalog::{CatalogProvider, catalog_digest};

fn failed(detail: String) -> PreflightError {
    PreflightError::Failed { detail }
}

fn path_of(text: &str) -> Result<FieldPath, PreflightError> {
    FieldPath::from_str(text).map_err(|e| failed(e.to_string()))
}

/// Derives `snapshots`: the digest of the catalog of the image the cell runs in.
pub struct CatalogStep {
    name: NameToken,
    // TRAIT_OBJECT: the provider lists commands in the image, or is a fake in tests
    catalog: Arc<dyn CatalogProvider>,
}

impl CatalogStep {
    /// A step that captures the catalog through `catalog`.
    ///
    /// # Errors
    /// [`FormatError`] when the step name is not a name token.
    // TRAIT_OBJECT: the provider lists commands in the image, or is a fake in tests
    pub fn new(catalog: Arc<dyn CatalogProvider>) -> Result<Self, FormatError> {
        Ok(Self {
            name: NameToken::from_str("catalog@1")?,
            catalog,
        })
    }
}

impl PreflightStep for CatalogStep {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        // BORROW: `Arc::clone` moves a share of the provider into the future
        let provider = Arc::clone(&self.catalog);
        // BORROW: `.clone()` moves the step name into the future
        let step = self.name.clone();
        Box::pin(async move {
            let image_path = path_of(paths::ISOLATION_IMAGE)?;
            let id = ctx
                .get(&image_path)
                .and_then(|value| value.as_value().as_str())
                .ok_or_else(|| failed(format!("`{}` has no image id", paths::ISOLATION_IMAGE)))?;
            let image = ImageRef {
                id: ImageId(id.to_owned()), // BORROW: owned id for the image reference
                cc_version: None,
            };
            let catalog = provider
                .capture(&image, &Cell::new(&ctx.values))
                .await
                .map_err(|e| failed(e.to_string()))?;
            let digest = catalog_digest(&catalog).map_err(|e| failed(e.to_string()))?;
            let kind = Name::from_str("catalog").map_err(|e| failed(e.to_string()))?;
            let entry = serde_json::to_value([DigestRef::new(kind, digest)])
                .map_err(|e| failed(e.to_string()))?;
            Ok(StepReport::new(step).derive(
                path_of(paths::SNAPSHOTS)?,
                JsonValue::new(entry).map_err(|e| failed(e.to_string()))?,
            ))
        })
    }
}
