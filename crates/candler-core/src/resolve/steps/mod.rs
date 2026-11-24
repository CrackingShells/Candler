//! The preflight steps that resolve the identity of the harness and the treatment and the image.

pub mod egress;
pub mod env;
pub mod gateway;
pub mod ground_truth;
pub mod identity;
pub mod image;
pub mod plugin_environment;
pub mod secrets;
pub mod session;

use std::sync::Arc;

use self::egress::EgressStep;
use self::identity::{HarnessIdentityStep, TreatmentIdentityStep};
use self::image::ImageIdStep;
use self::plugin_environment::PluginEnvironmentStep;
use self::session::SessionModeStep;
use super::preflight::{ResolveError, Resolver};
use crate::isolation::image::ImageService;
use crate::supervise::Supervisor;
use candler_harness::skills::SkillTree;

/// What the image, harness and treatment steps need from the outside.
pub struct M2bServices {
    /// The supervisor that runs the version probes.
    pub supervisor: Arc<Supervisor>,
    // TRAIT_OBJECT: the service is docker, or a fake in tests
    /// Provides the image.
    pub images: Arc<dyn ImageService>,
    /// The harness versions the driver supports.
    pub harness_supported: semver::VersionReq,
    /// Whether the harness's driver runs sessions.
    pub session_capable: bool,
    /// Where the harness reads the treatment's skills.
    pub skills: SkillTree,
}

/// Registers the preflight steps on `resolver`, in the order their values depend on one another:
/// the treatment first (its pinned commit is where requirement sources are read), then the plug-in
/// environment (it supplies the image step's values), then the ground truth (a gate: an image
/// that would hold evaluator-only bytes is never built), then the image,
/// then the harness (it reads the image), then the session mode, then the egress allowlist.
///
/// Each step clones the services it holds, so `services` stays with the caller.
///
/// # Errors
/// [`ResolveError`] when a step name is already registered, or a step name is malformed.
pub fn register_m2b_steps(
    resolver: Resolver,
    services: &M2bServices,
    ground_truth: ground_truth::GroundTruthStep,
) -> Result<Resolver, ResolveError> {
    let image = ImageIdStep::new(
        Arc::clone(&services.images),
        Arc::clone(&services.supervisor),
    )?;
    let harness = HarnessIdentityStep::new(
        Arc::clone(&services.images),
        services.harness_supported.clone(), // BORROW: the step owns its requirement
    )?;
    let treatment = TreatmentIdentityStep::new(Arc::clone(&services.supervisor), services.skills)?;
    let session = SessionModeStep::new(services.session_capable)?;
    let plugin_environment = PluginEnvironmentStep::new()?;
    let egress = EgressStep::new()?;
    resolver
        .register(Box::new(treatment))? // TRAIT_OBJECT: steps of different types are registered side by side
        .register(Box::new(plugin_environment))? // TRAIT_OBJECT: steps of different types are registered side by side
        .register(Box::new(ground_truth))? // TRAIT_OBJECT: steps of different types are registered side by side
        .register(Box::new(image))? // TRAIT_OBJECT: steps of different types are registered side by side
        .register(Box::new(harness))? // TRAIT_OBJECT: steps of different types are registered side by side
        .register(Box::new(session))? // TRAIT_OBJECT: steps of different types are registered side by side
        .register(Box::new(egress)) // TRAIT_OBJECT: steps of different types are registered side by side
}
