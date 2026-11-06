//! The image the agent runs in: built from the lock, found by id, and read for its Claude Code
//! version.

use std::path::PathBuf;

use futures::future::BoxFuture;

use super::model::ImageRef;

/// The digest of the image lock the build starts from: the bare sha256 hex of the lock bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockDigest(pub String);

/// The `isolation.build.*` settings and the lock they build from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSetting {
    /// The bytes of the resolved requirement lock the env layer installs.
    pub lock: Vec<u8>,
    /// The experiment's own layer on top of the env image, when it names one.
    pub extension: Option<Extension>,
}

/// An experiment's own Dockerfile (`isolation.build.{dockerfile, context}`), built `FROM` the env
/// image pinned by its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extension {
    /// The Dockerfile.
    pub dockerfile: PathBuf,
    /// The build context directory.
    pub context: PathBuf,
}

/// One requirement source the env lock is compiled from, such as the skill's
/// `scripts/pyproject.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockSource {
    /// The file name the compiler sees.
    pub name: String,
    /// The file's bytes.
    pub bytes: Vec<u8>,
}

/// What [`ImageService::lock`] compiles: the experiment's sources, on top of the stable
/// dependencies the env assets declare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockRequest {
    /// The sources, in the order the experiment lists them.
    pub sources: Vec<LockSource>,
}

/// Why an image could not be provided.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ImageError {
    /// The service is not implemented.
    #[error("image service is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
    /// The build failed.
    #[error("image build failed: {detail}")]
    Build {
        /// What the build reported.
        detail: String,
    },
    /// The version output named no version.
    #[error("the Claude Code version cannot be read: {detail}")]
    Version {
        /// What is wrong with the output.
        detail: String,
    },
}

/// Provides the image of a job.
pub trait ImageService: Send + Sync {
    /// Compiles the requirement lock of the env layer from `request` and the stable dependencies,
    /// for the base image's interpreter and platform.
    ///
    /// # Errors
    /// [`ImageError::Build`] when the compile fails.
    fn lock<'a>(&'a self, request: &'a LockRequest) -> BoxFuture<'a, Result<Vec<u8>, ImageError>>;

    /// Finds the image the lock names, building it when docker does not have it.
    ///
    /// # Errors
    /// [`ImageError::Build`] when the build fails.
    fn ensure<'a>(
        &'a self,
        lock: &'a LockDigest,
        setting: &'a BuildSetting,
    ) -> BoxFuture<'a, Result<ImageRef, ImageError>>;

    /// Reads the Claude Code version inside the image.
    ///
    /// # Errors
    /// [`ImageError::Version`] when the output names no version.
    fn cc_version<'a>(
        &'a self,
        image: &'a ImageRef,
    ) -> BoxFuture<'a, Result<semver::Version, ImageError>>;
}
