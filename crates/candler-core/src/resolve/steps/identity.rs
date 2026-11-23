//! The identity steps: the harness that drives the agent and the treatment that configures it.
//!
//! Both identities are typed roots. They come from the settings `request.harness` and
//! `request.treatment`, which a cell never holds, and enter the cell only through the values these
//! steps derive at `harness` and `treatment`.

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use candler_harness::skills::SkillTree;
use candler_schemas::execution::{
    GitCommit, HarnessIdentity, HarnessRequest, TreatmentRef, TreatmentSpec,
};
use candler_schemas::primitives::{FieldPath, FormatError, NameToken};
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;

use crate::isolation::image::ImageService;
use crate::isolation::model::{ImageId, ImageRef};
use crate::resolve::preflight::{
    CheckResult, PreflightError, PreflightStep, ResolveContext, StepReport,
};
use crate::supervise::Supervisor;
use crate::supervise::git::{
    GitError, READ_DEADLINE, RecordDir, archive_argv, mark_executable, run_git, tree_at_commit,
};
use candler_schemas::execution::paths;

/// The most bytes read from `git`.
const GIT_OUTPUT_LIMIT: usize = 4096;

/// The most bytes a requirement source read from a commit may hold.
pub(super) const SHOW_LIMIT: usize = 1024 * 1024;

/// The marker of the identity steps' child processes.
const MARKER: &str = "treatment-identity";

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
/// no source is refused, and so is one whose pinned commit holds no skill tree.
pub struct TreatmentIdentityStep {
    name: NameToken,
    supervisor: Arc<Supervisor>,
    skills: SkillTree,
}

impl TreatmentIdentityStep {
    /// A step that resolves tags through `supervisor` and checks the skill tree at the pinned
    /// commit as `skills` lays it out.
    ///
    /// # Errors
    /// [`FormatError`] when the step name is not a name token.
    pub fn new(supervisor: Arc<Supervisor>, skills: SkillTree) -> Result<Self, FormatError> {
        Ok(Self {
            name: NameToken::from_str("treatment_identity@1")?,
            supervisor,
            skills,
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
        let skills = self.skills;
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
            let mut report = StepReport::new(step).derive_from(path, value, source.to_owned()); // BORROW: the report owns the source
            if treatment.commit.is_some() {
                let source = ctx.config.cwd.join(source);
                if let Some(refusal) =
                    skill_tree_refusal(&supervisor, skills, &treatment, &source).await?
                {
                    report = report.check(CheckResult::new(
                        NameToken::from_str("skill_tree@1").map_err(|e| failed(e.to_string()))?, // BORROW: .to_string() gives owned text
                        false,
                        refusal,
                    ));
                }
            }
            Ok(report)
        })
    }
}

/// Why the skill tree of `treatment` is not at its pinned commit in the repository at `source`,
/// naming the subtree; `None` when the commit holds it.
///
/// # Errors
/// [`PreflightError::Failed`] when `git` cannot be run or `treatment` pins no commit.
async fn skill_tree_refusal(
    supervisor: &Supervisor,
    skills: SkillTree,
    treatment: &TreatmentRef,
    source: &Path,
) -> Result<Option<String>, PreflightError> {
    let command = skills
        .archive_command(treatment, source)
        .map_err(|e| failed(e.to_string()))?; // BORROW: .to_string() gives owned text
    let subtree = skills
        .subtree(treatment)
        .map_err(|e| failed(e.to_string()))?; // BORROW: .to_string() gives owned text
    let commit = treatment
        .commit
        .as_ref()
        .ok_or_else(|| failed("the treatment pins no commit".to_owned()))?; // BORROW: owned text
    let missing = format!(
        "the pinned commit {commit} of {} holds no skill tree at `{subtree}`",
        source.display()
    );
    match tree_at_commit(supervisor, command.argv().to_vec(), MARKER, None).await {
        Ok(tar) => Ok(skills
            .files(treatment, &tar)
            .err()
            .map(|error| format!("{missing}: {error}"))),
        Err(GitError::Failed(_)) => Ok(Some(missing)),
        Err(other) => Err(failed(format!("cannot archive `{subtree}`: {other}"))),
    }
}

/// The commit `revision` names in the repository at `source`, read by `git rev-parse`.
async fn resolve_commit(
    supervisor: &Supervisor,
    source: &Path,
    revision: &str,
) -> Result<GitCommit, PreflightError> {
    let args = ["rev-parse", "--verify", "--quiet", revision];
    let out = run_git(
        supervisor,
        source,
        &args,
        MARKER,
        GIT_OUTPUT_LIMIT,
        READ_DEADLINE,
    )
    .await
    .map_err(|e| failed(e.to_string()))?;
    let (code, stdout) = (out.code, out.stdout);
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
/// (the treatment's checkout, the root of its repository: a skill lives at
/// `<source_root>/<name>` below it, as the archive command reads it).
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
            "`{path}` is not a path below the treatment's checkout"
        )));
    }
    let object = format!("{commit}:./{path}");
    let out = run_git(
        supervisor,
        source,
        &["show", &object],
        MARKER,
        SHOW_LIMIT + 1,
        READ_DEADLINE,
    )
    .await
    .map_err(|e| failed(e.to_string()))?;
    let (code, stdout) = (out.code, out.stdout);
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

/// A tree of the treatment's repository written to a scratch directory, removed when it is dropped.
pub(super) struct Extracted {
    _dir: RecordDir,
    root: PathBuf,
}

impl Extracted {
    /// The directory the requested path became.
    pub(super) fn root(&self) -> &Path {
        &self.root
    }
}

/// The directory `path` of the treatment's checkout `source`, as `commit` holds it, written to a
/// scratch directory: the bytes of the pinned commit, never those of the working tree.
///
/// A link, a device or an entry that leaves `path` is refused, as is a tree of
/// [`ARCHIVE_LIMIT`](crate::supervise::git::ARCHIVE_LIMIT) bytes or more.
///
/// # Errors
/// [`PreflightError::Failed`] when `path` is not a path below `source`, names no directory with
/// files in the commit, or the archive cannot be read or written.
pub(super) async fn tree_at(
    supervisor: &Supervisor,
    source: &Path,
    commit: &GitCommit,
    path: &str,
) -> Result<Extracted, PreflightError> {
    let trimmed = path.trim_end_matches('/');
    let below = Path::new(trimmed)
        .components()
        .all(|part| matches!(part, std::path::Component::Normal(_)));
    if trimmed.is_empty() || !below {
        return Err(failed(format!(
            "`{path}` is not a path below the treatment's checkout"
        )));
    }
    let revision = commit.to_string();
    let tar = tree_at_commit(
        supervisor,
        archive_argv(source, &revision, trimmed),
        MARKER,
        None,
    )
    .await
    .map_err(|e| match e {
        GitError::Failed(code) => failed(format!(
            "`{path}` is not in commit {commit} of {} (git exited {code})",
            source.display()
        )),
        GitError::TooLarge(limit) => failed(format!("`{path}` is larger than {limit} bytes")),
        other @ (GitError::Record(_)
        | GitError::Request(_)
        | GitError::Run(_)
        | GitError::Signal(_)
        | GitError::Deadline) => failed(other.to_string()),
    })?;
    let files = candler_harness::skills::unpack(&tar, &format!("{trimmed}/"))
        .map_err(|e| failed(format!("`{path}` cannot be read from commit {commit}: {e}")))?;
    if files.is_empty() {
        return Err(failed(format!(
            "`{path}` is not a directory with files in commit {commit}"
        )));
    }
    let base = RecordDir::temp("treatment-tree")
        .map_err(|e| failed(format!("cannot make the tree directory: {e}")))?;
    let extracted = Extracted {
        root: base.path().join(trimmed),
        _dir: base,
    };
    for (inside, data, executable) in files {
        let plain = Path::new(&inside)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)));
        if !plain {
            return Err(failed(format!("`{inside}` leaves `{path}`")));
        }
        let target = extracted.root.join(&inside);
        let parent = target.parent().unwrap_or(&extracted.root);
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| failed(format!("cannot make the tree directory: {e}")))?;
        tokio::fs::write(&target, data)
            .await
            .map_err(|e| failed(format!("cannot write the tree: {e}")))?;
        if executable {
            mark_executable(&target).map_err(|e| failed(format!("cannot write the tree: {e}")))?;
        }
    }
    Ok(extracted)
}
