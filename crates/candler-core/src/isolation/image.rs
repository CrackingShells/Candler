//! The image the agent runs in: built from the lock, found by id, and read for its Claude Code
//! version.

use futures::future::BoxFuture;

use super::model::ImageRef;

/// The digest of the image lock the build starts from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockDigest(pub String);

/// The `isolation.build.*` settings and the lock they build from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSetting {
    /// The bytes of the lock file.
    pub lock: Vec<u8>,
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
