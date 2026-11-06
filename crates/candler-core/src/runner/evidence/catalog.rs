//! The command catalog of an image: which commands the agent can run.

use std::sync::Arc;
use std::time::Duration;

use candler_harness::spec::Cell;
use candler_schemas::catalog::CatalogSnapshot;
use candler_schemas::primitives::Digest;
use candler_schemas::process::ExitStatus;
use futures::future::BoxFuture;

use crate::isolation::driver::DockerDriver;
use crate::isolation::model::ImageRef;
use crate::supervise::Supervisor;
use crate::supervise::stop::{Captured, Ended};

/// The exit code of coreutils `timeout` when the command it runs overran.
const TIMEOUT_EXIT: i32 = 124;

/// The catalog of one image and cell.
pub type Catalog = CatalogSnapshot;

/// Why a catalog could not be captured.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CatalogError {
    /// Capture is not available.
    #[error("catalog capture is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
    /// The command failed: it exited non-zero or a signal ended it.
    #[error("the catalog command failed: {status:?}")]
    Command {
        /// How the command ended.
        status: ExitStatus,
    },
    /// The command overran its time.
    #[error("the catalog command overran its time")]
    Timeout,
    /// The command's output is not a catalog.
    #[error("the catalog command printed no catalog: {detail}")]
    Parse {
        /// What is wrong with the output.
        detail: String,
    },
    /// The catalog has no canonical form.
    #[error("the catalog has no digest: {detail}")]
    Digest {
        /// Why.
        detail: String,
    },
}

/// The command that lists the catalog inside the image, and the time it gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogCommand {
    /// The program and its arguments, run inside the container.
    pub argv: Vec<String>,
    /// The time the command gets; it is also the inner `timeout`.
    pub timeout: Duration,
}

impl CatalogCommand {
    /// A command with the time it gets.
    #[must_use]
    pub const fn new(argv: Vec<String>, timeout: Duration) -> Self {
        Self { argv, timeout }
    }
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
/// [`CatalogError::Digest`] when the catalog cannot be digested.
pub fn catalog_digest(catalog: &Catalog) -> Result<Digest, CatalogError> {
    catalog
        .content_digest()
        .map_err(|error| CatalogError::Digest {
            // BORROW: .to_string() gives the error its own text
            detail: error.to_string(),
        })
}

/// The catalog a finished command printed.
///
/// # Errors
/// [`CatalogError::Timeout`] for a deadline or exit 124, [`CatalogError::Command`] for another
/// non-zero exit or a signal, and [`CatalogError::Parse`] for output that is not a catalog.
pub fn interpret(captured: &Captured) -> Result<Catalog, CatalogError> {
    let status = captured.exit.status;
    if captured.exit.ended == Ended::Deadline
        || matches!(status, ExitStatus::Code { code: TIMEOUT_EXIT })
    {
        return Err(CatalogError::Timeout);
    }
    if !matches!(status, ExitStatus::Code { code: 0 }) {
        return Err(CatalogError::Command { status });
    }
    serde_json::from_slice::<Catalog>(captured.stdout.as_slice()) // BORROW: explicit .as_slice()
        .map_err(|error| CatalogError::Parse {
            // BORROW: .to_string() gives the error its own text
            detail: error.to_string(),
        })
}

/// The provider that lists the commands inside the image.
pub struct CommandCatalogProvider {
    supervisor: Arc<Supervisor>,
    // TRAIT_OBJECT: the driver is docker, or a recording fake in tests
    docker: Arc<dyn DockerDriver>,
    command: CatalogCommand,
}

impl CommandCatalogProvider {
    /// A provider that runs `command` in control containers.
    #[must_use]
    pub fn new(
        supervisor: Arc<Supervisor>,
        docker: Arc<dyn DockerDriver>,
        command: CatalogCommand,
    ) -> Self {
        Self {
            supervisor,
            docker,
            command,
        }
    }

    /// The command the provider runs.
    #[must_use]
    pub const fn command<'a>(&'a self) -> &'a CatalogCommand {
        &self.command
    }
}

impl CatalogProvider for CommandCatalogProvider {
    fn capture<'a>(
        &'a self,
        image: &'a ImageRef,
        cell: &'a Cell<'_>,
    ) -> BoxFuture<'a, Result<Catalog, CatalogError>> {
        let _ = (&self.supervisor, &self.docker, image, cell);
        Box::pin(async {
            Err(CatalogError::Unavailable {
                detail: "the control container argv (container_run's run_argv) is not landed"
                    .to_owned(), // BORROW: .to_owned() gives the error its own text
            })
        })
    }
}
