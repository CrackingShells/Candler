//! The preflight steps that resolve the identity of the harness and the treatment, the image and
//! the command catalog.

pub mod catalog;
pub mod gateway;
pub mod identity;
pub mod image;
pub mod secrets;

use std::sync::Arc;

use self::catalog::CatalogStep;
use self::identity::{HarnessIdentityStep, TreatmentIdentityStep};
use self::image::ImageIdStep;
use super::preflight::{ResolveError, Resolver};
use crate::isolation::image::ImageService;
use crate::runner::evidence::catalog::CatalogProvider;
use crate::supervise::Supervisor;

/// What the image, harness, treatment and catalog steps need from the outside.
pub struct M2bServices {
    /// The supervisor that runs the version probes.
    pub supervisor: Arc<Supervisor>,
    // TRAIT_OBJECT: the service is docker, or a fake in tests
    /// Provides the image.
    pub images: Arc<dyn ImageService>,
    // TRAIT_OBJECT: the provider lists commands in the image, or is a fake in tests
    /// Captures the command catalog.
    pub catalog: Arc<dyn CatalogProvider>,
    /// The harness versions the driver supports.
    pub harness_supported: semver::VersionReq,
}

/// Registers the preflight steps on `resolver`, in the order their values depend on one another:
/// the treatment first (its pinned commit is where requirement sources are read), then the image,
/// then the harness (it reads the image), then the catalog.
///
/// Each step clones the services it holds, so `services` stays with the caller.
///
/// # Errors
/// [`ResolveError`] when a step name is already registered, or a step name is malformed.
pub fn register_m2b_steps(
    resolver: Resolver,
    services: &M2bServices,
) -> Result<Resolver, ResolveError> {
    let image = ImageIdStep::new(
        Arc::clone(&services.images),
        Arc::clone(&services.supervisor),
    )?;
    let harness = HarnessIdentityStep::new(
        Arc::clone(&services.images),
        services.harness_supported.clone(), // BORROW: the step owns its requirement
    )?;
    let treatment = TreatmentIdentityStep::new(Arc::clone(&services.supervisor))?;
    let catalog = CatalogStep::new(Arc::clone(&services.catalog))?;
    resolver
        .register(Box::new(treatment))? // TRAIT_OBJECT: steps of different types are registered side by side
        .register(Box::new(image))? // TRAIT_OBJECT: steps of different types are registered side by side
        .register(Box::new(harness))? // TRAIT_OBJECT: steps of different types are registered side by side
        .register(Box::new(catalog)) // TRAIT_OBJECT: steps of different types are registered side by side
}
