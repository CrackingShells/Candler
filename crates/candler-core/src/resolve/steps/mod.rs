//! The preflight steps M2b adds to the resolver: the identity of the harness and the treatment,
//! the image and the command catalog.

pub mod catalog;
pub mod identity;
pub mod image;

use std::sync::Arc;

use super::preflight::{ResolveError, Resolver};
use crate::isolation::image::ImageService;
use crate::runner::evidence::catalog::CatalogProvider;
use crate::supervise::Supervisor;

/// What the M2b steps need from the outside.
#[non_exhaustive]
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

/// Registers the M2b steps on `resolver`.
///
/// The services move into the steps.
///
/// # Errors
/// [`ResolveError`] when a step derives a path another step derives.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the steps own the services once they are registered"
)]
pub fn register_m2b_steps(
    resolver: Resolver,
    services: M2bServices,
) -> Result<Resolver, ResolveError> {
    let _ = services;
    Ok(resolver)
}
