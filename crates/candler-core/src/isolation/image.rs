//! The image the agent runs in: built from the lock, found by id, and read for its Claude Code
//! version.

use std::path::PathBuf;
use std::str::FromStr as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use candler_schemas::primitives::Marker;
use futures::future::BoxFuture;

use super::driver::DockerDriver;
use super::layout;
use super::model::ImageRef;
use crate::supervise::Supervisor;
use crate::supervise::spawn::SpawnRequest;
use crate::supervise::stop::Ended;
use candler_schemas::process::ExitStatus;

/// The marker the control containers of the image service run under.
const CONTROL_MARKER: &str = "control";

/// The seconds the entrypoint `timeout` gives `claude --version` inside the control container.
const PROBE_SECONDS: &str = "10";

/// How long the docker client of a control call may run before the supervisor ends it.
const CONTROL_DEADLINE: Duration = Duration::from_secs(60);

/// The most bytes of the probe's output that are read.
const PROBE_OUTPUT_LIMIT: usize = 4 * 1024;

/// The suffix `claude --version` puts after the version.
const VERSION_SUFFIX: &str = " (Claude Code)";

/// The environment names the docker client keeps from its parent: where the daemon is, and how
/// to find the binary. The agent's environment is built separately and shares none of it.
const CLIENT_ENV: [&str; 4] = ["PATH", "DOCKER_HOST", "DOCKER_CONTEXT", "HOME"];

/// Numbers the record directories of control calls within one process.
static CONTROL_CALLS: AtomicU64 = AtomicU64::new(0);

/// Where the image sources are and where the store keeps the image locks and control records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAssets {
    /// The build context of `candler-base`: `assets/docker/base`.
    pub base_context: PathBuf,
    /// The build context of `candler-env`: `assets/docker/env`.
    pub env_context: PathBuf,
    /// The store root; image locks and control records live under its `images` directory.
    pub store_root: PathBuf,
}

/// Builds and reads the images of a job through the docker CLI.
pub struct DockerImageService {
    sup: Arc<Supervisor>,
    // TRAIT_OBJECT: the driver is the real CLI in production and a fake in tests
    docker: Arc<dyn DockerDriver>,
    assets: ImageAssets,
    program: String,
}

impl DockerImageService {
    /// A service over `docker` and `assets`; control containers are run through `sup`.
    #[must_use]
    pub fn new(sup: Arc<Supervisor>, docker: Arc<dyn DockerDriver>, assets: ImageAssets) -> Self {
        // TRAIT_OBJECT: the driver is the real CLI in production and a fake in tests
        Self {
            sup,
            docker,
            assets,
            program: "docker".to_owned(),
        }
    }

    /// The same service with the docker client binary at `program`.
    #[must_use]
    pub fn with_program(mut self, program: &str) -> Self {
        // BORROW: .clone_into() keeps the program name in the service
        program.clone_into(&mut self.program);
        self
    }

    /// Runs `claude --version` in a control container of `image` and parses its output.
    async fn read_version(&self, image: &ImageRef) -> Result<semver::Version, ImageError> {
        let records = layout::control_dir(&self.assets.store_root).join(format!(
            "{}-{}",
            std::process::id(),
            CONTROL_CALLS.fetch_add(1, Ordering::Relaxed)
        ));
        tokio::fs::create_dir_all(&records)
            .await
            .map_err(|source| unreadable(&format!("cannot create the record dir: {source}")))?;
        let outcome = self.probe(image, &records).await;
        // The record dir is this call's own and holds only this call's record.
        let _ = tokio::fs::remove_dir_all(&records).await;
        outcome
    }

    async fn probe(
        &self,
        image: &ImageRef,
        records: &std::path::Path,
    ) -> Result<semver::Version, ImageError> {
        let marker = Marker::from_str(CONTROL_MARKER)
            .map_err(|error| unreadable(&format!("the control marker is refused: {error}")))?;
        let request = SpawnRequest::builder()
            .argv(control_argv(&self.program, image))
            .env(client_env())
            .marker(marker)
            // BORROW: .to_path_buf() keeps the record dir in the request
            .record_dir(records.to_path_buf())
            .deadline(CONTROL_DEADLINE)
            .build()
            .map_err(|error| unreadable(&format!("the probe cannot be built: {error}")))?;
        let captured = self
            .sup
            .run_captured(request, PROBE_OUTPUT_LIMIT)
            .await
            .map_err(|error| unreadable(&format!("the probe cannot run: {error}")))?;
        let exited = matches!(captured.exit.ended, Ended::Exited);
        if !exited {
            return Err(unreadable("the probe did not end on its own"));
        }
        match captured.exit.status {
            ExitStatus::Code { code: 0 } => {}
            ExitStatus::Code { code: 124 } => return Err(unreadable("the probe timed out")),
            ExitStatus::Code { code } => {
                return Err(unreadable(&format!("the probe exited with status {code}")));
            }
            ExitStatus::Signal { signal } => {
                return Err(unreadable(&format!("a signal ({signal}) ended the probe")));
            }
        }
        let text = String::from_utf8(captured.stdout)
            .map_err(|_| unreadable("the version output is not text"))?;
        // BORROW: explicit .as_str() instead of Deref coercion
        parse_cc_version(text.as_str())
    }
}

impl ImageService for DockerImageService {
    fn lock<'a>(&'a self, request: &'a LockRequest) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        let _ = request;
        Box::pin(async {
            Err(ImageError::Unavailable {
                detail: "lock".to_owned(),
            })
        })
    }

    fn ensure<'a>(
        &'a self,
        lock: &'a LockDigest,
        setting: &'a BuildSetting,
    ) -> BoxFuture<'a, Result<ImageRef, ImageError>> {
        let _ = (lock, setting, &self.docker, &self.assets.base_context);
        Box::pin(async {
            Err(ImageError::Unavailable {
                detail: "ensure".to_owned(),
            })
        })
    }

    fn cc_version<'a>(
        &'a self,
        image: &'a ImageRef,
    ) -> BoxFuture<'a, Result<semver::Version, ImageError>> {
        Box::pin(self.read_version(image))
    }
}

fn unreadable(detail: &str) -> ImageError {
    ImageError::VersionUnreadable {
        // BORROW: .to_owned() keeps the description in the error
        detail: detail.to_owned(),
    }
}

/// The environment of a docker client call: the allowlisted names the parent has.
fn client_env() -> Vec<(String, String)> {
    CLIENT_ENV
        .iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                // BORROW: .to_owned() keeps the name next to its value
                .map(|value| ((*name).to_owned(), value))
        })
        .collect()
}

/// The argv of the control container that prints the version: it removes itself, has no
/// network, no capabilities and a read-only root, and its entrypoint is a coreutils `timeout`.
fn control_argv(program: &str, image: &ImageRef) -> Vec<String> {
    let mut argv: Vec<String> = [
        "run",
        "--rm",
        "--init",
        "--network",
        "none",
        "--read-only",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "--pids-limit",
        "64",
        "--memory",
        "256m",
        "--label",
        "candler.role=control",
        "--label",
        "candler.job=preflight",
        "--entrypoint",
        "timeout",
    ]
    .iter()
    .map(|word| (*word).to_owned())
    .collect();
    // BORROW: .clone() keeps the id in the argv
    argv.push(image.id.0.clone());
    argv.extend(
        [PROBE_SECONDS, "claude", "--version"]
            .iter()
            .map(|word| (*word).to_owned()),
    );
    argv.insert(0, program.to_owned());
    argv
}

/// Reads the version from the output of `claude --version`: one line,
/// `<major>.<minor>.<patch> (Claude Code)`, and nothing else.
///
/// # Errors
/// [`ImageError::VersionUnreadable`] for any other output.
pub fn parse_cc_version(output: &str) -> Result<semver::Version, ImageError> {
    let line = output.trim();
    let bad = || unreadable("the output is not `<major>.<minor>.<patch> (Claude Code)`");
    let number = line.strip_suffix(VERSION_SUFFIX).ok_or_else(bad)?;
    let mut parts = number.split('.').map(|part| {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        part.parse::<u64>().ok()
    });
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(Some(major)), Some(Some(minor)), Some(Some(patch)), None) => {
            Ok(semver::Version::new(major, minor, patch))
        }
        _ => Err(bad()),
    }
}

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
    VersionUnreadable {
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
    /// [`ImageError::VersionUnreadable`] when the output names no version.
    fn cc_version<'a>(
        &'a self,
        image: &'a ImageRef,
    ) -> BoxFuture<'a, Result<semver::Version, ImageError>>;
}

#[cfg(test)]
mod scaffold {
    use std::os::unix::fs::PermissionsExt as _;

    use candler_schemas::execution::EventPayload;
    use candler_schemas::primitives::RunKey;

    use super::*;
    use crate::isolation::model::ImageId;
    use crate::isolation::testing::RecordingDocker;
    use crate::supervise::EventSink;

    struct Quiet;

    impl EventSink for Quiet {
        fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
    }

    #[test]
    fn parser_table() {
        let good = |text: &str| parse_cc_version(text).expect("a version");
        assert_eq!(
            good("2.1.311 (Claude Code)\n"),
            semver::Version::new(2, 1, 311)
        );
        assert_eq!(
            good("  0.0.1 (Claude Code)  "),
            semver::Version::new(0, 0, 1)
        );
        for bad in [
            "",
            "v2.1.311 (Claude Code)\n",
            "2.1.311\n",
            "2.1.311 (Claude Code)\n2.1.311 (Claude Code)\n",
            "2.1.311-beta.1 (Claude Code)\n",
            "2.1 (Claude Code)\n",
            "2.1.3.4 (Claude Code)\n",
            "2.1.x (Claude Code)\n",
            "2.1.311 (Claude Code) extra\n",
        ] {
            assert!(
                matches!(
                    parse_cc_version(bad),
                    Err(ImageError::VersionUnreadable { .. })
                ),
                "{bad:?}"
            );
        }
    }

    fn service(script: &str) -> (DockerImageService, tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("a dir");
        let log = dir.path().join("argv.log");
        let path = dir.path().join("docker");
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" > '{}'\n{script}\n",
                log.display()
            ),
        )
        .expect("a script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let assets = ImageAssets {
            base_context: dir.path().join("base"),
            env_context: dir.path().join("env"),
            store_root: dir.path().join("store"),
        };
        let sink: Arc<dyn EventSink> = Arc::new(Quiet);
        let service = DockerImageService::new(
            Arc::new(Supervisor::new(sink)),
            Arc::new(RecordingDocker::new()),
            assets,
        )
        .with_program(path.to_str().expect("utf-8"));
        (service, dir, log)
    }

    fn image() -> ImageRef {
        ImageRef {
            id: ImageId("sha256:abc".to_owned()),
            cc_version: None,
        }
    }

    #[tokio::test]
    async fn version_through_a_control_container() {
        let (service, dir, log) = service("printf '2.1.311 (Claude Code)\\n'");
        let version = service.cc_version(&image()).await.expect("a version");
        assert_eq!(version, semver::Version::new(2, 1, 311));
        let argv = std::fs::read_to_string(log).expect("a log");
        println!("{argv}");
        for word in [
            "run --rm",
            "--network none",
            "--read-only",
            "--cap-drop ALL",
            "no-new-privileges",
            "--init",
            "candler.role=control",
            "candler.job=preflight",
            "--entrypoint timeout sha256:abc 10 claude --version",
        ] {
            assert!(argv.contains(word), "{word}");
        }
        let records = dir.path().join("store/control");
        assert_eq!(std::fs::read_dir(records).expect("dir").count(), 0);
    }

    #[tokio::test]
    async fn failures_are_unreadable() {
        for script in [
            "exit 124",
            "exit 1",
            "printf 'nope\\n'",
            "printf '2.1.311 (Claude Code)\\n'; exit 3",
        ] {
            let (service, _dir, _log) = service(script);
            let result = service.cc_version(&image()).await;
            assert!(
                matches!(result, Err(ImageError::VersionUnreadable { .. })),
                "{script}: {result:?}"
            );
        }
    }

    #[tokio::test]
    #[ignore = "reads a real image from the daemon"]
    async fn real_image_version_equals_the_label() {
        let id = std::env::var("SCAFFOLD_IMAGE").expect("an image id");
        let label = std::env::var("SCAFFOLD_LABEL").expect("the label");
        let (service, _dir, _log) = service("exit 1");
        let service = service.with_program("docker");
        let version = service
            .cc_version(&ImageRef {
                id: ImageId(id),
                cc_version: None,
            })
            .await
            .expect("a version");
        println!("cc_version={version} label={label}");
        assert_eq!(version.to_string(), label);
    }
}
