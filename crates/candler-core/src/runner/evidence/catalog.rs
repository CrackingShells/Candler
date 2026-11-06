//! The command catalog of an image: which commands the agent can run.

use std::path::PathBuf;
use std::str::FromStr as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use candler_harness::spec::{Cell, CommandSpec};
use candler_schemas::catalog::CatalogSnapshot;
use candler_schemas::primitives::{Digest, Marker};
use candler_schemas::process::ExitStatus;
use futures::future::BoxFuture;

use crate::isolation::container::run::control_argv;
use crate::isolation::docker::{ClientEnv, DockerCli};
use crate::isolation::layout;
use crate::isolation::model::{ImageRef, Limits};
use crate::supervise::Supervisor;
use crate::supervise::spawn::SpawnRequest;
use crate::supervise::stop::{Captured, Ended};

/// The exit code of coreutils `timeout` when the command it runs overran.
const TIMEOUT_EXIT: i32 = 124;

/// The marker the control containers of a catalog capture run under.
const CONTROL_MARKER: &str = "control";

/// How long past the command's own time the docker client may run before the supervisor ends it.
const CLIENT_GRACE: Duration = Duration::from_secs(30);

/// The most bytes of a catalog that are read.
const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

/// The process limit of a control container.
const CONTROL_PIDS: u32 = 64;

/// The memory limit of a control container.
const CONTROL_MEMORY: &str = "512m";

/// The cpu limit of a control container.
const CONTROL_CPUS: f64 = 1.0;

/// Numbers the record directories of catalog captures within one process.
static CONTROL_CALLS: AtomicU64 = AtomicU64::new(0);

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
    cli: DockerCli,
    env: ClientEnv,
    store_root: PathBuf,
    command: CatalogCommand,
}

impl CommandCatalogProvider {
    /// A provider that runs `command` in control containers through the docker client `cli`;
    /// each call keeps its record dir under `<store_root>/control`.
    #[must_use]
    pub const fn new(
        supervisor: Arc<Supervisor>,
        cli: DockerCli,
        env: ClientEnv,
        store_root: PathBuf,
        command: CatalogCommand,
    ) -> Self {
        Self {
            supervisor,
            cli,
            env,
            store_root,
            command,
        }
    }

    /// The command the provider runs.
    #[must_use]
    pub const fn command<'a>(&'a self) -> &'a CatalogCommand {
        &self.command
    }

    /// The `docker run` argv of the control container, led by the client's binary.
    fn argv(&self, image: &ImageRef) -> Result<Vec<String>, CatalogError> {
        let unavailable = |detail: String| CatalogError::Unavailable { detail };
        let spec = CommandSpec::builder()
            // BORROW: .clone() gives the spec its own argv
            .argv(self.command.argv.clone())
            .build()
            .map_err(|error| unavailable(error.to_string()))?;
        let limits = Limits {
            pids_limit: CONTROL_PIDS,
            // BORROW: .to_owned() gives the limits their own text
            memory: CONTROL_MEMORY.to_owned(),
            cpus: CONTROL_CPUS,
        };
        let mut argv = control_argv(&image.id, &limits, self.command.timeout, &spec)
            .map_err(|error| unavailable(error.to_string()))?;
        if let Some(first) = argv.first_mut() {
            // BORROW: .to_string_lossy().into_owned() gives the argv the binary path as text
            *first = self.cli.binary().to_string_lossy().into_owned();
        }
        Ok(argv)
    }

    /// Runs the control container with its record dir `records`.
    async fn run(&self, image: &ImageRef, records: PathBuf) -> Result<Catalog, CatalogError> {
        let unavailable = |detail: String| CatalogError::Unavailable { detail };
        let marker = Marker::from_str(CONTROL_MARKER)
            .map_err(|error| unavailable(format!("the control marker is refused: {error}")))?;
        let request = SpawnRequest::builder()
            .argv(self.argv(image)?)
            // BORROW: .to_vec() gives the request its own pairs
            .env(self.env.pairs().to_vec())
            .marker(marker)
            .record_dir(records)
            .deadline(self.command.timeout + CLIENT_GRACE)
            .build()
            .map_err(|error| unavailable(format!("the command cannot be built: {error}")))?;
        let captured = self
            .supervisor
            .run_captured(request, OUTPUT_LIMIT)
            .await
            .map_err(|error| unavailable(format!("the command cannot run: {error}")))?;
        interpret(&captured)
    }

    async fn capture_one(&self, image: &ImageRef) -> Result<Catalog, CatalogError> {
        let records = layout::control_dir(&self.store_root).join(format!(
            "{}-{}",
            std::process::id(),
            CONTROL_CALLS.fetch_add(1, Ordering::Relaxed)
        ));
        tokio::fs::create_dir_all(&records)
            .await
            .map_err(|error| CatalogError::Unavailable {
                detail: format!("cannot create the record dir: {error}"),
            })?;
        // BORROW: .clone() keeps the record dir for its removal
        let outcome = self.run(image, records.clone()).await;
        // The record dir is this call's own and holds only this call's record.
        let _ = tokio::fs::remove_dir_all(&records).await;
        outcome
    }
}

impl CatalogProvider for CommandCatalogProvider {
    fn capture<'a>(
        &'a self,
        image: &'a ImageRef,
        _cell: &'a Cell<'_>,
    ) -> BoxFuture<'a, Result<Catalog, CatalogError>> {
        Box::pin(self.capture_one(image))
    }
}

#[cfg(test)]
mod scaffold {
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::Path;

    use candler_schemas::execution::EventPayload;
    use candler_schemas::primitives::RunKey;

    use super::*;
    use crate::isolation::model::ImageId;
    use crate::supervise::EventSink;

    struct Silent;

    impl EventSink for Silent {
        fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
    }

    fn rig(dir: &Path, answer: &str) -> CommandCatalogProvider {
        let canned = dir.join("canned");
        let bin = dir.join("bin");
        let records = dir.join("records");
        for sub in [&canned, &bin, &records] {
            std::fs::create_dir_all(sub).expect("a directory");
        }
        std::fs::write(canned.join("run.out"), answer).expect("an answer");
        let fake = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/docker/fake-docker");
        let wrapper = bin.join("docker");
        std::fs::write(
            &wrapper,
            format!(
                "#!/bin/sh\nexport FAKE_DOCKER_LOG='{}' FAKE_DOCKER_DIR='{}'\nexec '{}' \"$@\"\n",
                dir.join("docker.log").display(),
                canned.display(),
                fake.display()
            ),
        )
        .expect("a script");
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).expect("a mode");
        // TRAIT_OBJECT: the supervisor takes its sink as a trait object
        let sink: Arc<dyn EventSink> = Arc::new(Silent);
        let supervisor = Arc::new(Supervisor::new(sink));
        let env = ClientEnv::from_process();
        // BORROW: .clone() gives the client its own environment
        let cli = DockerCli::with_binary(Arc::clone(&supervisor), wrapper, env.clone(), records);
        CommandCatalogProvider::new(
            supervisor,
            cli,
            env,
            dir.join("store"),
            CatalogCommand::new(vec!["list-tools".to_owned()], Duration::from_secs(20)),
        )
    }

    fn image() -> ImageRef {
        ImageRef {
            id: ImageId("sha256:aa".to_owned()),
            cc_version: None,
        }
    }

    const OK: &str = r#"{"schema":"tool-catalog-snapshot/1","captured_at":"2026-01-01T00:00:00Z","provider":"cmd@1","servers":[]}"#;

    #[tokio::test]
    async fn capture_runs_a_labelled_control_container_and_cleans_up() {
        let dir = tempfile::tempdir().expect("a directory");
        let provider = rig(dir.path(), OK);
        let cell = Cell::new(&[]);
        let catalog = provider.capture(&image(), &cell).await.expect("a catalog");
        assert_eq!(catalog.servers().len(), 0);
        let log = std::fs::read_to_string(dir.path().join("docker.log")).expect("a log");
        assert!(log.contains("candler.role=control"), "{log}");
        assert!(!log.contains("candler.job"), "{log}");
        let control = dir.path().join("store/control");
        assert_eq!(std::fs::read_dir(control).expect("control").count(), 0);
        assert!(!dir.path().join("store/children").exists());
    }
}
