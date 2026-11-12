//! The identity steps: the harness that drives the agent and the treatment that configures it.
//!
//! Both identities are typed roots. They come from the settings `request.harness` and
//! `request.treatment`, which a cell never holds, and enter the cell only through the values these
//! steps derive at `harness` and `treatment`.

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use candler_schemas::execution::{GitCommit, HarnessIdentity, HarnessRequest, TreatmentSpec};
use candler_schemas::primitives::{FieldPath, FormatError, Marker, NameToken};
use candler_schemas::process::ExitStatus;
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;

use crate::experiment::paths;
use crate::isolation::image::ImageService;
use crate::isolation::model::{ImageId, ImageRef};
use crate::resolve::preflight::{PreflightError, PreflightStep, ResolveContext, StepReport};
use crate::supervise::Supervisor;
use crate::supervise::spawn::SpawnRequest;

/// The most bytes read from `git`.
const GIT_OUTPUT_LIMIT: usize = 4096;

/// The most bytes a requirement source read from a commit may hold.
pub(super) const SHOW_LIMIT: usize = 1024 * 1024;

/// Counts the record directories this process made, so two probes never share one.
static PROBES: AtomicU64 = AtomicU64::new(0);

fn failed(detail: String) -> PreflightError {
    PreflightError::Failed { detail }
}

fn path_of(text: &str) -> Result<FieldPath, PreflightError> {
    FieldPath::from_str(text).map_err(|e| failed(e.to_string()))
}

/// Reads the setting or value at `text` as a `T`.
fn read<T: serde::de::DeserializeOwned>(
    ctx: &ResolveContext,
    text: &str,
) -> Result<T, PreflightError> {
    let path = path_of(text)?;
    let found = ctx
        .get(&path)
        .ok_or_else(|| failed(format!("`{text}` has no value")))?;
    // BORROW: `.clone()` hands the JSON to the deserializer, which takes it by value
    serde_json::from_value(found.as_value().clone())
        .map_err(|e| failed(format!("`{text}` is not valid: {e}")))
}

fn json_of<T: serde::Serialize>(value: &T) -> Result<JsonValue, PreflightError> {
    let json = serde_json::to_value(value).map_err(|e| failed(e.to_string()))?;
    JsonValue::new(json).map_err(|e| failed(e.to_string()))
}

/// Derives `harness`: the name the request asks for and the version of that harness in the image.
pub struct HarnessIdentityStep {
    name: NameToken,
    // TRAIT_OBJECT: the service is docker, or a fake in tests
    images: Arc<dyn ImageService>,
    supported: semver::VersionReq,
}

impl HarnessIdentityStep {
    /// A step that reads the version through `images` and accepts the versions `supported` names.
    ///
    /// # Errors
    /// [`FormatError`] when the step name is not a name token.
    // TRAIT_OBJECT: the service is docker, or a fake in tests
    pub fn new(
        images: Arc<dyn ImageService>,
        supported: semver::VersionReq,
    ) -> Result<Self, FormatError> {
        Ok(Self {
            name: NameToken::from_str("harness_identity@1")?,
            images,
            supported,
        })
    }
}

impl PreflightStep for HarnessIdentityStep {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        // BORROW: `Arc::clone` moves a share of the service into the future
        let images = Arc::clone(&self.images);
        // BORROW: `.clone()` moves the requirement into the future
        let supported = self.supported.clone();
        // BORROW: `.clone()` moves the step name into the future
        let step = self.name.clone();
        Box::pin(async move {
            let request: HarnessRequest = read(&ctx, paths::REQUEST_HARNESS)?;
            let image: String = read(&ctx, paths::ISOLATION_IMAGE)?;
            let image = ImageRef {
                id: ImageId(image),
                harness_version: None,
            };
            let version = images
                .harness_version(&image)
                .await
                .map_err(|e| failed(e.to_string()))?;
            if !supported.matches(&version) {
                return Err(failed(format!(
                    "{name} {version} is outside the versions the driver supports ({supported})",
                    name = request.name
                )));
            }
            if let Some(wanted) = &request.version
                && !wanted.matches(&version)
            {
                return Err(failed(format!(
                    "{name} {version} is outside the version the request asks for ({wanted})",
                    name = request.name
                )));
            }
            let identity = HarnessIdentity::new(request.name, version);
            Ok(StepReport::new(step).derive(path_of(paths::HARNESS)?, json_of(&identity)?))
        })
    }
}

/// Derives `treatment`: the name, and the commit a tag resolves to in the checkout the treatment's
/// `source` names, recording that source as where the value was read from. A treatment that names
/// no source is refused.
pub struct TreatmentIdentityStep {
    name: NameToken,
    supervisor: Arc<Supervisor>,
}

impl TreatmentIdentityStep {
    /// A step that resolves tags through `supervisor`.
    ///
    /// # Errors
    /// [`FormatError`] when the step name is not a name token.
    pub fn new(supervisor: Arc<Supervisor>) -> Result<Self, FormatError> {
        Ok(Self {
            name: NameToken::from_str("treatment_identity@1")?,
            supervisor,
        })
    }
}

impl PreflightStep for TreatmentIdentityStep {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        // BORROW: `Arc::clone` moves a share of the supervisor into the future
        let supervisor = Arc::clone(&self.supervisor);
        // BORROW: `.clone()` moves the step name into the future
        let step = self.name.clone();
        Box::pin(async move {
            let spec: TreatmentSpec = read(&ctx, paths::REQUEST_TREATMENT)?;
            let mut treatment = spec.reference();
            let source = spec.source.as_deref().ok_or_else(|| {
                failed(
                    "the treatment names no source: set `treatment.source` in the experiment \
                     file's `[treatment]`"
                        .to_owned(), // BORROW: the error owns its text
                )
            })?;
            let revision = match (&treatment.tag, &treatment.commit) {
                (Some(tag), _) => Some(format!("refs/tags/{tag}^{{commit}}")),
                (None, Some(commit)) => Some(format!("{commit}^{{commit}}")),
                (None, None) => None,
            };
            if let Some(revision) = revision {
                let source = ctx.config.cwd.join(source);
                let found = resolve_commit(&supervisor, &source, &revision).await?;
                if let Some(wanted) = &treatment.commit
                    && *wanted != found
                {
                    return Err(failed(format!(
                        "the treatment asks for commit {wanted} but its tag is at {found}"
                    )));
                }
                treatment.commit = Some(found);
            }
            let path = path_of(paths::TREATMENT)?;
            let value = json_of(&treatment)?;
            Ok(StepReport::new(step).derive_from(path, value, source.to_owned())) // BORROW: the report owns the source
        })
    }
}

/// The commit `revision` names in the repository at `source`, read by `git rev-parse`.
async fn resolve_commit(
    supervisor: &Supervisor,
    source: &Path,
    revision: &str,
) -> Result<GitCommit, PreflightError> {
    let args = ["rev-parse", "--verify", "--quiet", revision];
    let (code, stdout) = git_output(supervisor, source, &args, GIT_OUTPUT_LIMIT).await?;
    if code != 0 {
        return Err(failed(format!(
            "`{revision}` does not name a commit in {} (git exited {code})",
            source.display()
        )));
    }
    let text = String::from_utf8_lossy(&stdout);
    GitCommit::from_str(text.trim()).map_err(|e| failed(format!("git printed no commit: {e}")))
}

/// The bytes of `path` as `commit` holds them, with `path` read relative to the directory `source`
/// (so a skill directory inside a larger repository is the root of its own paths).
///
/// `commit` is a full hex id, so the revision argument cannot start with `-`; a path that is
/// absolute or climbs out of `source` is refused.
///
/// # Errors
/// [`PreflightError::Failed`] when the path is not below `source`, the file is not in the commit,
/// or it is larger than [`SHOW_LIMIT`] bytes.
pub(super) async fn show_file(
    supervisor: &Supervisor,
    source: &Path,
    commit: &GitCommit,
    path: &str,
) -> Result<Vec<u8>, PreflightError> {
    let below = Path::new(path)
        .components()
        .all(|part| matches!(part, std::path::Component::Normal(_)));
    if path.is_empty() || !below {
        return Err(failed(format!(
            "`{path}` is not a path below the skill directory"
        )));
    }
    let object = format!("{commit}:./{path}");
    let (code, stdout) = git_output(supervisor, source, &["show", &object], SHOW_LIMIT + 1).await?;
    if code != 0 {
        return Err(failed(format!(
            "`{path}` is not in commit {commit} of {} (git exited {code})",
            source.display()
        )));
    }
    if stdout.len() > SHOW_LIMIT {
        return Err(failed(format!(
            "`{path}` is larger than {SHOW_LIMIT} bytes"
        )));
    }
    Ok(stdout)
}

/// Runs `git -C source args…` through the supervisor and returns its exit code and the first
/// `limit` bytes of standard output.
async fn git_output(
    supervisor: &Supervisor,
    source: &Path,
    args: &[&str],
    limit: usize,
) -> Result<(i32, Vec<u8>), PreflightError> {
    let record_dir = record_dir();
    tokio::fs::create_dir_all(&record_dir)
        .await
        .map_err(|e| failed(format!("cannot make the probe directory: {e}")))?;
    // BORROW: `.clone()` gives the probe its own copy; this function removes the directory
    let outcome = probe(supervisor, source, args, limit, record_dir.clone()).await;
    // The directory holds only the probe's record; a failed removal leaves it for the temp sweep.
    drop(tokio::fs::remove_dir_all(&record_dir).await);
    outcome
}

async fn probe(
    supervisor: &Supervisor,
    source: &Path,
    subcommand: &[&str],
    limit: usize,
    record_dir: PathBuf,
) -> Result<(i32, Vec<u8>), PreflightError> {
    let marker = Marker::from_str("treatment-identity").map_err(|e| failed(e.to_string()))?;
    let mut argv = vec![
        "git".to_owned(), // BORROW: owned argument for the request
        "-C".to_owned(),  // BORROW: owned argument for the request
        source.to_string_lossy().into_owned(),
    ];
    argv.extend(subcommand.iter().map(|arg| (*arg).to_owned())); // BORROW: owned arguments for the request
    let path = std::env::var("PATH").unwrap_or_default();
    let request = SpawnRequest::builder()
        .argv(argv)
        .env(vec![
            ("PATH".to_owned(), path), // BORROW: owned name for the child's environment
            ("LC_ALL".to_owned(), "C".to_owned()), // BORROW: owned pair for the environment
            ("GIT_TERMINAL_PROMPT".to_owned(), "0".to_owned()), // BORROW: owned pair for the environment
        ])
        .marker(marker)
        .record_dir(record_dir)
        .build()
        .map_err(|e| failed(e.to_string()))?;
    let captured = supervisor
        .run_captured(request, limit)
        .await
        .map_err(|e| failed(format!("cannot run git: {e}")))?;
    match captured.exit.status {
        ExitStatus::Code { code } => Ok((code, captured.stdout)),
        ExitStatus::Signal { signal } => Err(failed(format!("git ended on signal {signal}"))),
    }
}

/// A directory no other probe of this process uses.
fn record_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    std::env::temp_dir().join(format!(
        "candler-treatment-{}-{nanos}-{}",
        std::process::id(),
        PROBES.fetch_add(1, Ordering::Relaxed)
    ))
}
