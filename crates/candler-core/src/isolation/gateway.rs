//! The gateway container the agent reaches its provider through. The agent holds a dummy token;
//! the gateway holds the real one and forwards the calls.

use candler_schemas::container::ContainerRecord;
use candler_schemas::primitives::{Digest, JobId, RunKey};
use futures::future::BoxFuture;

use super::driver::DockerDriver;
use super::model::GatewayHandle;
use crate::supervise::Supervisor;

/// Why a secret could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SecretError {
    /// The secret is not set.
    #[error("the secret `{name}` is not set")]
    Missing {
        /// The name of the secret; never its value.
        name: String,
    },
}

/// A secret's value. Its `Debug` never shows the value.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretValue(String);

impl SecretValue {
    /// Wraps a value.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// The value, for the one place that needs it.
    #[must_use]
    pub const fn expose<'a>(&'a self) -> &'a str {
        // BORROW: explicit .as_str() instead of Deref coercion
        self.0.as_str()
    }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretValue(<redacted>)")
    }
}

/// Where the real token comes from.
pub trait SecretSource: Send + Sync {
    /// The value of the secret `name`.
    ///
    /// # Errors
    /// [`SecretError::Missing`] when it is not set.
    fn resolve(&self, name: &str) -> Result<SecretValue, SecretError>;
}

/// Why the gateway could not start or be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GatewayError {
    /// The gateway is not implemented.
    #[error("gateway is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
    /// The gateway container did not start.
    #[error("gateway did not start: {detail}")]
    Start {
        /// What went wrong; it never holds a secret.
        detail: String,
    },
}

/// Whether the gateway can serve.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GatewayStatus {
    /// The gateway answers.
    Ready,
    /// The gateway cannot serve.
    Unavailable {
        /// Why, as a reason code.
        reason: String,
    },
}

/// What the gateway saw of one run's provider calls.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GatewaySignal {
    /// The upstream answered with this status.
    UpstreamStatus(u16),
    /// The upstream could not be reached.
    Unreachable,
    /// The gateway's own probe failed.
    ProbeFailed,
}

/// Writes a job's container record, `children/container-<cid12>.json`. The job directory
/// implements it; the gateway calls it after `docker create` and before the sidecar holds the
/// token, so a teardown of the current job finds the sidecar recorded.
pub trait ChildRecorder: Send + Sync {
    /// Writes `record`.
    ///
    /// # Errors
    /// The I/O error that kept the record from being written.
    fn record_child(&self, record: &ContainerRecord) -> std::io::Result<()>;
}

/// What starting a job's gateway needs.
#[derive(Clone, Copy)]
pub struct GatewayStart<'a> {
    /// The job. Its internal network is `NetworkName::for_job`, its egress network
    /// `NetworkName::egress_for_job`; `start` creates both.
    pub job: &'a JobId,
    /// The experiment digest, the value of the sidecar's `candler.experiment` label. Its
    /// `candler.run` label is the literal `gateway`, which no run's path form equals.
    pub experiment: &'a Digest,
    /// Where the sidecar's container record is written.
    // TRAIT_OBJECT: the job directory, or a recording fake in tests
    pub recorder: &'a dyn ChildRecorder,
    /// Resolves the secret.
    // TRAIT_OBJECT: the secret store, or a fixed fake in tests
    pub token: &'a dyn SecretSource,
    /// The name of the secret, the one `model.token_secret` gives (D9).
    pub secret: &'a str,
}

/// Starts the gateway of a job.
pub trait Gateway: Send + Sync {
    /// Starts the gateway of `request.job`: creates the job's internal and egress networks,
    /// creates the sidecar, records it, then hands it the secret.
    ///
    /// # Errors
    /// [`GatewayError::Start`] when the container does not start.
    fn start<'a>(
        &'a self,
        sup: &'a Supervisor,
        docker: &'a dyn DockerDriver,
        request: GatewayStart<'a>,
    ) -> BoxFuture<'a, Result<GatewayHandle, GatewayError>>;

    /// Asks the gateway whether it can serve.
    fn probe<'a>(&'a self, handle: &'a GatewayHandle) -> BoxFuture<'a, GatewayStatus>;

    /// What the gateway saw of the run `run`'s calls.
    ///
    /// # Errors
    /// [`GatewayError`] when the gateway cannot be asked.
    fn run_signals<'a>(
        &'a self,
        handle: &'a GatewayHandle,
        run: &'a RunKey,
    ) -> BoxFuture<'a, Result<Vec<GatewaySignal>, GatewayError>>;
}
