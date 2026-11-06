//! The docker command line behind the driver traits.

/// Why a docker call failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DockerError {
    /// The call is not implemented by this driver.
    #[error("docker call is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
    /// Docker exited with an error.
    #[error("docker exited with an error: {detail}")]
    Failed {
        /// What docker said; it never holds a secret.
        detail: String,
    },
    /// Docker's answer could not be read.
    #[error("docker's answer could not be read: {detail}")]
    Unreadable {
        /// What is wrong with it.
        detail: String,
    },
}
