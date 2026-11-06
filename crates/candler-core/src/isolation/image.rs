//! The image the agent runs in: built from the lock, found by id, and read for its Claude Code
//! version.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use candler_schemas::primitives::Marker;
use futures::future::BoxFuture;
use rustix::fs::{FlockOperation, flock};
use sha2::Digest as _;

use super::docker::{ClientEnv, DockerCli, DockerError};
use super::driver::DockerDriver;
use super::layout;
use super::model::{BuildRequest, ImageId, ImageInfo, ImageRef, Labels, LayerDigest};
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

/// The repository of the base image.
const BASE_REPO: &str = "candler-base";

/// The repository of the env images.
const ENV_REPO: &str = "candler-env";

/// The label of an env image that holds the hex digest of its lock.
const LOCK_LABEL: &str = "candler.lock";

/// The label of the base image that holds the Claude Code version it carries.
const CC_LABEL: &str = "candler.cc_version";

/// The repository of the images that carry an experiment's own layer.
const EXT_REPO: &str = "candler-ext";

/// The stable dependencies every lock is compiled with, in the env assets.
const STABLE_SOURCE: &str = "stable.in";

/// Where the lock sources are mounted inside the compile container.
const SOURCE_MOUNT: &str = "/lock-src";

/// The seconds the entrypoint `timeout` gives the lock compile.
const COMPILE_SECONDS: &str = "300";

/// How long the docker client of the compile may run before the supervisor ends it.
const LOCK_DEADLINE: Duration = Duration::from_secs(360);

/// The most bytes of the compile's output that are read.
const LOCK_OUTPUT_LIMIT: usize = 4 * 1024 * 1024;

/// The most bytes of the compile's error output that an error carries.
const STDERR_TAIL: usize = 2048;

/// The build argument of the env Dockerfile that names its base.
const BASE_ARG: &str = "BASE";

/// How many hex digits of the source digest tag the base image.
const SOURCE_DIGITS: usize = 12;

/// The files of the env build context that come from the env assets.
const ENV_SOURCES: [&str; 2] = ["Dockerfile", ".dockerignore"];

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
    binary: PathBuf,
    env: ClientEnv,
}

impl DockerImageService {
    /// A service over the docker client `cli`; control containers are run through `sup` with the
    /// client's binary and `env`.
    #[must_use]
    pub fn new(sup: Arc<Supervisor>, cli: DockerCli, env: ClientEnv, assets: ImageAssets) -> Self {
        // BORROW: .to_path_buf() keeps the client's binary for the control calls
        let binary = cli.binary().to_path_buf();
        Self {
            sup,
            docker: Arc::new(cli),
            assets,
            binary,
            env,
        }
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

    /// Takes the exclusive lock of the image `tag`, waiting for its holder. The lock is held for
    /// as long as the returned file lives.
    async fn lock_tag(&self, tag: &str) -> Result<File, ImageError> {
        let path = layout::image_lock_path(&self.assets.store_root, tag);
        let held = tokio::task::spawn_blocking(move || -> Result<File, std::io::Error> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path)?;
            flock(&file, FlockOperation::LockExclusive).map_err(std::io::Error::from)?;
            Ok(file)
        })
        .await
        .map_err(|error| build_failed(&format!("the lock task ended: {error}")))?;
        held.map_err(|error| build_failed(&format!("cannot lock the image: {error}")))
    }

    /// The base image: built once per digest of its sources, under the lock of its tag.
    async fn ensure_base(&self) -> Result<BaseImage, ImageError> {
        // BORROW: .clone() moves the context into the blocking task
        let context = self.assets.base_context.clone();
        let source = tokio::task::spawn_blocking(move || source_digest(&context))
            .await
            .map_err(|error| build_failed(&format!("the digest task ended: {error}")))?
            .map_err(|error| build_failed(&format!("cannot read the base sources: {error}")))?;
        let tag = format!("{BASE_REPO}:src-{source}");
        let _guard = self.lock_tag(&tag).await?;
        let found = self
            .docker
            .image_inspect(&tag)
            .await
            .map_err(|error| docker_failed(&error))?;
        let info = if let Some(info) = found {
            info
        } else {
            let request = BuildRequest {
                // BORROW: .clone() keeps the context in the service
                context: self.assets.base_context.clone(),
                // BORROW: .clone() keeps the tag for the pin
                tag: tag.clone(),
                build_args: Vec::new(),
                labels: Labels::new(),
            };
            self.docker
                .build(&request)
                .await
                .map_err(|error| docker_failed(&error))?
        };
        Ok(BaseImage {
            tag,
            id: info.id,
            layers: info.layers,
        })
    }

    /// The image `tag` over `parent`, built once under the lock of its tag: an image already
    /// there that starts with the parent's layers is used, any other is built again. The flag says
    /// whether a build ran.
    async fn ensure_layer(
        &self,
        tag: &str,
        key: &str,
        parent: &BaseImage,
        layer: Layer<'_>,
        labels: &Labels,
    ) -> Result<(ImageInfo, bool), ImageError> {
        let _guard = self.lock_tag(tag).await?;
        let found = self
            .docker
            .image_inspect(tag)
            .await
            .map_err(|error| docker_failed(&error))?;
        if let Some(info) = found.filter(|info| rootfs_prefix(&parent.layers, &info.layers)) {
            return Ok((info, false));
        }
        let context = self
            .assets
            .store_root
            .join("images")
            .join(format!("ctx-{key}"));
        let staged = self.stage_context(&context, layer).await;
        let built = match staged {
            Ok(()) => self.build_layer(&context, tag, labels, parent).await,
            Err(error) => Err(error),
        };
        // The context is this build's own, under the lock of its tag.
        let _ = tokio::fs::remove_dir_all(&context).await;
        Ok((built?, true))
    }

    /// Writes the build context of `layer` into `context`.
    async fn stage_context(&self, context: &Path, layer: Layer<'_>) -> Result<(), ImageError> {
        let failed = |error: std::io::Error| {
            build_failed(&format!("cannot stage the build context: {error}"))
        };
        let _ = tokio::fs::remove_dir_all(context).await;
        tokio::fs::create_dir_all(context).await.map_err(failed)?;
        match layer {
            Layer::Env(setting) => {
                for name in ENV_SOURCES {
                    tokio::fs::copy(self.assets.env_context.join(name), context.join(name))
                        .await
                        .map_err(failed)?;
                }
                tokio::fs::write(context.join("requirements.lock"), &setting.lock)
                    .await
                    .map_err(failed)
            }
            Layer::Extension(extension) => {
                // BORROW: .clone() moves the paths into the blocking task
                let from = extension.context.clone();
                // BORROW: .clone() moves the paths into the blocking task
                let dockerfile = extension.dockerfile.clone();
                // BORROW: .to_path_buf() moves the path into the blocking task
                let to = context.to_path_buf();
                tokio::task::spawn_blocking(move || -> Result<(), std::io::Error> {
                    copy_tree(&from, &to)?;
                    std::fs::copy(&dockerfile, to.join("Dockerfile")).map(|_| ())
                })
                .await
                .map_err(|error| build_failed(&format!("the staging task ended: {error}")))?
                .map_err(failed)
            }
        }
    }

    /// Builds `tag` over `parent`: pinned by id when docker takes the pinned form, by tag
    /// otherwise; either way the result must start with the parent's layers.
    async fn build_layer(
        &self,
        context: &Path,
        tag: &str,
        labels: &Labels,
        parent: &BaseImage,
    ) -> Result<ImageInfo, ImageError> {
        let request = |from: String| BuildRequest {
            // BORROW: .to_path_buf() keeps the context in the request
            context: context.to_path_buf(),
            // BORROW: .to_owned() keeps the tag in the request
            tag: tag.to_owned(),
            // BORROW: .to_owned() keeps the argument name in the request
            build_args: vec![(BASE_ARG.to_owned(), from)],
            // BORROW: .clone() keeps the labels in the request
            labels: labels.clone(),
        };
        let pinned = request(format!("{}@{}", parent.tag, parent.id.0));
        let info = if let Ok(info) = self.docker.build(&pinned).await {
            info
        } else {
            // BORROW: .clone() keeps the tag in the parent
            let by_tag = request(parent.tag.clone());
            self.docker
                .build(&by_tag)
                .await
                .map_err(|error| docker_failed(&error))?
        };
        if !rootfs_prefix(&parent.layers, &info.layers) {
            return Err(build_failed(
                "the image does not start with the layers of its parent",
            ));
        }
        Ok(info)
    }

    async fn ensure_image(
        &self,
        lock: &LockDigest,
        setting: &BuildSetting,
    ) -> Result<ImageRef, ImageError> {
        let hex = hex_of(&sha2::Sha256::digest(&setting.lock));
        // BORROW: explicit .as_str() instead of Deref coercion
        let given = lock.0.strip_prefix("sha256:").unwrap_or(lock.0.as_str());
        if !given.eq_ignore_ascii_case(&hex) {
            return Err(ImageError::LockMismatch);
        }
        let base = self.ensure_base().await?;
        let labels = Labels::new().with(LOCK_LABEL, &hex);
        let env_tag = format!("{ENV_REPO}:{hex}");
        let (mut info, mut built) = self
            .ensure_layer(&env_tag, &hex, &base, Layer::Env(setting), &labels)
            .await?;
        if let Some(extension) = &setting.extension {
            let env = BaseImage {
                tag: env_tag,
                // BORROW: .clone() keeps the env id for the extension's digest
                id: info.id.clone(),
                layers: info.layers,
            };
            // BORROW: .clone() moves the extension into the blocking task
            let sources = extension.clone();
            // BORROW: .clone() moves the id into the blocking task
            let env_id = env.id.clone();
            let digest = tokio::task::spawn_blocking(move || extension_digest(&env_id, &sources))
                .await
                .map_err(|error| build_failed(&format!("the digest task ended: {error}")))?
                .map_err(|error| {
                    build_failed(&format!("cannot read the extension sources: {error}"))
                })?;
            let tag = format!("{EXT_REPO}:{digest}");
            let (made, ran) = self
                .ensure_layer(&tag, &digest, &env, Layer::Extension(extension), &labels)
                .await?;
            info = made;
            built = built || ran;
        }
        let label = label_version(&info)?;
        let image = ImageRef {
            id: info.id,
            // BORROW: .clone() keeps the label version to compare with the binary
            cc_version: Some(label.clone()),
        };
        if built {
            let binary = self.read_version(&image).await?;
            if binary != label {
                return Err(ImageError::VersionMismatch {
                    label: label.to_string(),
                    binary: binary.to_string(),
                });
            }
        }
        Ok(image)
    }

    /// Compiles the lock of `request` and the stable dependencies in a control container of the
    /// base image, the one control call that has a network.
    async fn compile_lock(&self, request: &LockRequest) -> Result<Vec<u8>, ImageError> {
        // BORROW: .to_owned() keeps the name in the argv
        let mut names = vec![STABLE_SOURCE.to_owned()];
        for source in &request.sources {
            if !source_name_ok(&source.name) {
                return Err(build_failed(
                    "a lock source has a name the compiler refuses",
                ));
            }
            // BORROW: .clone() keeps the name for the compiler's argv
            names.push(source.name.clone());
        }
        let base = self.ensure_base().await?;
        let control = layout::control_dir(&self.assets.store_root);
        let call = format!(
            "{}-{}",
            std::process::id(),
            CONTROL_CALLS.fetch_add(1, Ordering::Relaxed)
        );
        let records = control.join(&call);
        let sources = control.join(format!("{call}-src"));
        let staged = self.stage_sources(&records, &sources, request).await;
        let outcome = match staged {
            Ok(()) => self.compile(&base.id, &records, &sources, &names).await,
            Err(error) => Err(error),
        };
        // Both directories are this call's own.
        let _ = tokio::fs::remove_dir_all(&records).await;
        let _ = tokio::fs::remove_dir_all(&sources).await;
        outcome
    }

    async fn stage_sources(
        &self,
        records: &Path,
        sources: &Path,
        request: &LockRequest,
    ) -> Result<(), ImageError> {
        let failed = |error: std::io::Error| {
            build_failed(&format!("cannot stage the lock sources: {error}"))
        };
        tokio::fs::create_dir_all(records).await.map_err(failed)?;
        tokio::fs::create_dir_all(sources).await.map_err(failed)?;
        tokio::fs::copy(
            self.assets.env_context.join(STABLE_SOURCE),
            sources.join(STABLE_SOURCE),
        )
        .await
        .map_err(failed)?;
        for source in &request.sources {
            tokio::fs::write(sources.join(&source.name), &source.bytes)
                .await
                .map_err(failed)?;
        }
        Ok(())
    }

    async fn compile(
        &self,
        image: &ImageId,
        records: &Path,
        sources: &Path,
        names: &[String],
    ) -> Result<Vec<u8>, ImageError> {
        let marker = Marker::from_str(CONTROL_MARKER)
            .map_err(|error| build_failed(&format!("the control marker is refused: {error}")))?;
        let spawn = SpawnRequest::builder()
            .argv(compile_argv(&self.binary, image, sources, names))
            // BORROW: .to_vec() gives the request its own pairs
            .env(self.env.pairs().to_vec())
            .marker(marker)
            // BORROW: .to_path_buf() keeps the record dir in the request
            .record_dir(records.to_path_buf())
            .deadline(LOCK_DEADLINE)
            .build()
            .map_err(|error| build_failed(&format!("the compile cannot be built: {error}")))?;
        let captured = self
            .sup
            .run_captured(spawn, LOCK_OUTPUT_LIMIT)
            .await
            .map_err(|error| build_failed(&format!("the compile cannot run: {error}")))?;
        if !matches!(captured.exit.ended, Ended::Exited) {
            return Err(build_failed("the compile did not end on its own"));
        }
        match captured.exit.status {
            ExitStatus::Code { code: 0 } => Ok(captured.stdout),
            ExitStatus::Code { code: 124 } => Err(build_failed("the compile timed out")),
            ExitStatus::Code { code } => Err(build_failed(&format!(
                "the compile exited with status {code}: {}",
                tail_of(&captured.stderr)
            ))),
            ExitStatus::Signal { signal } => Err(build_failed(&format!(
                "a signal ({signal}) ended the compile"
            ))),
        }
    }

    async fn probe(
        &self,
        image: &ImageRef,
        records: &std::path::Path,
    ) -> Result<semver::Version, ImageError> {
        let marker = Marker::from_str(CONTROL_MARKER)
            .map_err(|error| unreadable(&format!("the control marker is refused: {error}")))?;
        let request = SpawnRequest::builder()
            .argv(control_argv(&self.binary, image))
            // BORROW: .to_vec() gives the request its own pairs
            .env(self.env.pairs().to_vec())
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
        Box::pin(self.compile_lock(request))
    }

    fn ensure<'a>(
        &'a self,
        lock: &'a LockDigest,
        setting: &'a BuildSetting,
    ) -> BoxFuture<'a, Result<ImageRef, ImageError>> {
        Box::pin(self.ensure_image(lock, setting))
    }

    fn cc_version<'a>(
        &'a self,
        image: &'a ImageRef,
    ) -> BoxFuture<'a, Result<semver::Version, ImageError>> {
        Box::pin(self.read_version(image))
    }
}

/// What a layer image is built from.
enum Layer<'a> {
    /// The env layer: the env Dockerfile and the lock.
    Env(&'a BuildSetting),
    /// The experiment's own Dockerfile and context.
    Extension(&'a Extension),
}

/// The image a layer is built on.
struct BaseImage {
    tag: String,
    id: ImageId,
    layers: Vec<LayerDigest>,
}

/// Whether `env` starts with every layer of `base`: the env image is built on that base.
#[must_use]
pub fn rootfs_prefix(base: &[LayerDigest], env: &[LayerDigest]) -> bool {
    !base.is_empty() && env.starts_with(base)
}

fn build_failed(detail: &str) -> ImageError {
    ImageError::Build {
        // BORROW: .to_owned() keeps the description in the error
        detail: detail.to_owned(),
    }
}

fn docker_failed(error: &DockerError) -> ImageError {
    build_failed(&error.to_string())
}

/// The Claude Code version the image's label names.
fn label_version(info: &ImageInfo) -> Result<semver::Version, ImageError> {
    let text = info
        .labels
        .get(CC_LABEL)
        .ok_or_else(|| unreadable("the image has no Claude Code version label"))?;
    semver::Version::from_str(text)
        .map_err(|_| unreadable("the Claude Code version label is not a version"))
}

/// Lower-case hex of `bytes`.
fn hex_of(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        hex.extend(char::from_digit(u32::from(byte >> 4), 16));
        hex.extend(char::from_digit(u32::from(byte & 0xf), 16));
    }
    hex
}

/// The digest of every file under `dir`: paths in order, each with its bytes.
fn tree_digest(dir: &Path) -> Result<String, std::io::Error> {
    let mut files = Vec::new();
    collect_files(dir, dir, &mut files)?;
    files.sort();
    let mut hasher = sha2::Sha256::new();
    for relative in &files {
        hasher.update(relative.to_string_lossy().as_bytes());
        hasher.update([0_u8]);
        hasher.update(std::fs::read(dir.join(relative))?);
        hasher.update([0_u8]);
    }
    Ok(hex_of(&hasher.finalize()))
}

/// The first digits of the digest of the sources under `dir`.
fn source_digest(dir: &Path) -> Result<String, std::io::Error> {
    let mut hex = tree_digest(dir)?;
    hex.truncate(SOURCE_DIGITS);
    Ok(hex)
}

/// The digest of an extension over the env image it builds on: the env id, the Dockerfile and
/// every file of the context.
fn extension_digest(env: &ImageId, extension: &Extension) -> Result<String, std::io::Error> {
    let mut hasher = sha2::Sha256::new();
    hasher.update(env.0.as_bytes());
    hasher.update([0_u8]);
    hasher.update(std::fs::read(&extension.dockerfile)?);
    hasher.update([0_u8]);
    hasher.update(tree_digest(&extension.context)?.as_bytes());
    Ok(hex_of(&hasher.finalize()))
}

/// Copies the files under `from` to `to`, keeping the layout.
fn copy_tree(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Whether the compiler can take `name` as a file of the source mount.
fn source_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name != STABLE_SOURCE
        && !name.starts_with(['-', '.'])
        && !name.contains(['/', '\\', '\0'])
}

/// The last bytes of `bytes` as text.
fn tail_of(bytes: &[u8]) -> String {
    let start = bytes.len().saturating_sub(STDERR_TAIL);
    String::from_utf8_lossy(bytes.get(start..).unwrap_or_default())
        .trim()
        // BORROW: .to_owned() keeps the text in the error
        .to_owned()
}

/// The argv of the compile container: it removes itself, has a writable `/tmp` only, reaches the
/// package index over the default bridge network, and mounts the sources read-only.
fn compile_argv(program: &Path, image: &ImageId, sources: &Path, names: &[String]) -> Vec<String> {
    let mut argv: Vec<String> = [
        "run",
        "--rm",
        "--init",
        "--read-only",
        "--tmpfs",
        "/tmp",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "--pids-limit",
        "256",
        "--memory",
        "2g",
        "--network",
        "bridge",
        "--env",
        "UV_NO_CACHE=1",
        "--label",
        "candler.role=control",
        "--mount",
    ]
    .iter()
    // BORROW: `.to_owned()` makes each word an owned argument
    .map(|word| (*word).to_owned())
    .collect();
    // BORROW: .to_string_lossy().into_owned() gives the argv the binary path as text
    argv.insert(0, program.to_string_lossy().into_owned());
    argv.push(format!(
        "type=bind,src={},dst={SOURCE_MOUNT},readonly",
        sources.display()
    ));
    argv.extend(
        ["--entrypoint", "timeout"]
            .iter()
            // BORROW: `.to_owned()` makes each word an owned argument
            .map(|word| (*word).to_owned()),
    );
    // BORROW: .clone() keeps the id in the argv
    argv.push(image.0.clone());
    argv.extend(
        [COMPILE_SECONDS, "sh", "-c", COMPILE_SCRIPT, "sh"]
            .iter()
            // BORROW: `.to_owned()` makes each word an owned argument
            .map(|word| (*word).to_owned()),
    );
    argv.extend(names.iter().map(|name| format!("{SOURCE_MOUNT}/{name}")));
    argv
}

/// The compile, for the platform the image runs on.
const COMPILE_SCRIPT: &str = "exec uv pip compile --python-version 3.12 \
--python-platform \"$(uname -m)-unknown-linux-gnu\" --no-header \"$@\"";

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(root, &path, out)?;
        } else if let Ok(relative) = path.strip_prefix(root) {
            // BORROW: .to_path_buf() keeps the relative path in the list
            out.push(relative.to_path_buf());
        }
    }
    Ok(())
}

fn unreadable(detail: &str) -> ImageError {
    ImageError::VersionUnreadable {
        // BORROW: .to_owned() keeps the description in the error
        detail: detail.to_owned(),
    }
}

/// The argv of the control container that prints the version: it removes itself, has no
/// network, no capabilities and a read-only root, and its entrypoint is a coreutils `timeout`.
fn control_argv(program: &Path, image: &ImageRef) -> Vec<String> {
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
        "--entrypoint",
        "timeout",
    ]
    .iter()
    // BORROW: `.to_owned()` makes each word an owned argument
    .map(|word| (*word).to_owned())
    .collect();
    // BORROW: .clone() keeps the id in the argv
    argv.push(image.id.0.clone());
    argv.extend(
        [PROBE_SECONDS, "claude", "--version"]
            .iter()
            // BORROW: `.to_owned()` makes each word an owned argument
            .map(|word| (*word).to_owned()),
    );
    // BORROW: .to_string_lossy().into_owned() gives the argv the binary path as text
    argv.insert(0, program.to_string_lossy().into_owned());
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
    /// The lock bytes do not have the digest the caller named.
    #[error("the lock does not match its digest")]
    LockMismatch,
    /// The binary in the image and its label name different Claude Code versions.
    #[error("the image label says {label} but the binary says {binary}")]
    VersionMismatch {
        /// The version the label names.
        label: String,
        /// The version the binary prints.
        binary: String,
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
    /// [`ImageError::LockMismatch`] when the lock bytes are not the lock the digest names,
    /// [`ImageError::Build`] when a build fails, [`ImageError::VersionMismatch`] when the built
    /// image's label and binary disagree.
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
