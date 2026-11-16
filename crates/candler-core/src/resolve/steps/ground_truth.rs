//! The ground-truth step: an evaluator-only input never sits in what the agent receives.
//!
//! The step holds the digest of every file of every evaluator-only input ([`GroundTruth`]). When
//! the set is empty it does nothing, and reads no file. Otherwise it digests the files that go
//! to the agent and refuses on the first byte-identical copy:
//! - the Dockerfile and the context of the image layer, whether the experiment or a plug-in
//!   environment wrote them;
//! - the agent inputs of the selected plug-in environment;
//! - the files of the treatment's skill tree at its pinned commit.
//!
//! A file with no byte never matches. Digests catch identical bytes only. A renamed file is found; a compressed, re-encoded or
//! otherwise transformed copy is not.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use candler_harness::skills::SkillTree;
use candler_schemas::execution::{ExperimentInput, TreatmentRef, TreatmentSpec, paths};
use candler_schemas::primitives::{Digest, FieldPath, FormatError, Marker, NameToken};
use candler_schemas::process::ExitStatus;
use futures::future::BoxFuture;

use super::image::BuildInputs;
use super::plugin_environment::{Fragment, STEP_BASE};
use crate::experiment::inputs::InputFiles;
use crate::resolve::preflight::{
    CheckResult, PreflightError, PreflightStep, ResolveContext, StepReport,
};
use crate::supervise::Supervisor;
use crate::supervise::spawn::SpawnRequest;
use candler_schemas::values::ValueSource;

/// The name of the step.
pub const STEP_NAME: &str = "ground_truth@1";

/// The check that names an evaluator-only input found in what the agent receives.
pub const CHECK_EXPOSED: &str = "ground_truth_exposed@1";

/// The most bytes read from the archive of the treatment's skill tree.
const ARCHIVE_LIMIT: usize = 64 * 1024 * 1024;

/// Why an evaluator-only input is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GroundTruthRefusal {
    /// The bytes of an evaluator-only input are in what the agent receives.
    #[error("{detail}")]
    Exposed {
        /// The input, the place and what to do, for the author of the experiment.
        detail: String,
    },
}

impl GroundTruthRefusal {
    /// The check that names this refusal, as `name@token`.
    #[must_use]
    pub const fn check(&self) -> &'static str {
        match self {
            Self::Exposed { .. } => CHECK_EXPOSED,
        }
    }

    /// The refusal a failed check of the step stands for, when `check` names one.
    #[must_use]
    pub fn from_check(check: &str, detail: &str) -> Option<Self> {
        match check {
            CHECK_EXPOSED => Some(Self::Exposed {
                detail: detail.to_owned(), // BORROW: the refusal owns the detail
            }),
            // EXHAUSTIVE: any other check belongs to another step, which this refusal does not stand for
            _ => None,
        }
    }
}

/// One file of an evaluator-only input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruthFile {
    /// The kind of the input the file belongs to.
    pub kind: String,
    /// The path of the file relative to the input's root.
    pub path: String,
    /// The digest of the file's bytes.
    pub digest: Digest,
}

/// The files of the evaluator-only inputs of one experiment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroundTruth {
    files: Vec<TruthFile>,
    agent: Vec<TruthFile>,
}

impl GroundTruth {
    /// The ground truth that holds `files`.
    #[must_use]
    pub const fn new(files: Vec<TruthFile>) -> Self {
        Self {
            files,
            agent: Vec::new(),
        }
    }

    /// The ground truth that holds the evaluator-only files of `inputs`, checked against the
    /// files of the agent inputs whose kind is in `own_kinds`: the experiment's own, since a
    /// plug-in environment's agent inputs are walked by the step.
    #[must_use]
    pub fn from_inputs(inputs: &[InputFiles], own_kinds: &[String]) -> Self {
        let files_of = |wanted: &dyn Fn(&InputFiles) -> bool| -> Vec<TruthFile> {
            inputs
                .iter()
                .filter(|input| wanted(input))
                .flat_map(|input| {
                    input.entries.iter().map(|entry| TruthFile {
                        kind: input.kind.clone(), // BORROW: the file owns its kind
                        path: entry.path.to_string(),
                        digest: entry.sha256.clone(), // BORROW: the file owns its digest
                    })
                })
                .collect()
        };
        Self {
            files: files_of(&|input| input.evaluator),
            agent: files_of(&|input| !input.evaluator && own_kinds.contains(&input.kind)),
        }
    }

    /// Whether there is no evaluator-only file.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The evaluator-only file whose bytes have `digest`.
    ///
    /// A file with no byte never matches: every empty file has one digest, and an empty
    /// `__init__.py` is no copy of an empty evaluator-only file.
    fn named(&self, digest: &Digest) -> Option<&TruthFile> {
        if *digest == Digest::of_bytes(&[]) {
            return None;
        }
        self.files.iter().find(|file| file.digest == *digest)
    }
}

/// Refuses a cell whose agent-bound bytes hold the bytes of an evaluator-only input.
pub struct GroundTruthStep {
    name: NameToken,
    truth: Arc<GroundTruth>,
    supervisor: Arc<Supervisor>,
    skills: SkillTree,
}

impl GroundTruthStep {
    /// The step over `truth`; `skills` is where the harness reads the treatment's skills.
    ///
    /// # Errors
    /// [`FormatError`] when the step name is not a name token.
    pub fn new(
        truth: GroundTruth,
        supervisor: Arc<Supervisor>,
        skills: SkillTree,
    ) -> Result<Self, FormatError> {
        Ok(Self {
            name: NameToken::from_str(STEP_NAME)?,
            truth: Arc::new(truth),
            supervisor,
            skills,
        })
    }
}

fn failed(detail: String) -> PreflightError {
    PreflightError::Failed { detail }
}

fn text_at(ctx: &ResolveContext, text: &str) -> Result<Option<String>, PreflightError> {
    let path = FieldPath::from_str(text).map_err(|e| failed(e.to_string()))?;
    Ok(ctx
        .get(&path)
        .and_then(|value| value.as_value().as_str())
        .map(str::to_owned)) // BORROW: the step owns the text
}

/// Whether the plug-in environment derived the value at `text`.
fn from_plugin(ctx: &ResolveContext, text: &str) -> bool {
    FieldPath::from_str(text).is_ok_and(|path| {
        ctx.sources.iter().any(|entry| {
            *entry.path() == path
                && matches!(
                    entry.source(),
                    ValueSource::Derived { step, .. } if step.name() == STEP_BASE
                )
        })
    })
}

/// The digest of every file below `path`, keyed by its path relative to `path`; a file gives its
/// own name. A symbolic link is skipped.
fn walk_files(path: &Path) -> Result<BTreeMap<String, Digest>, io::Error> {
    fn go(dir: &Path, prefix: &str, out: &mut BTreeMap<String, Digest>) -> Result<(), io::Error> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let key = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let kind = entry.file_type()?;
            if kind.is_dir() {
                go(&entry.path(), &key, out)?;
            } else if kind.is_file() {
                out.insert(key, Digest::of_reader(fs::File::open(entry.path())?)?);
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    if fs::metadata(path)?.is_dir() {
        go(path, "", &mut out)?;
    } else {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        out.insert(name, Digest::of_reader(fs::File::open(path)?)?);
    }
    Ok(out)
}

/// What the agent gets from the image layer: where the files are, and who wrote them.
async fn context_exposures(
    ctx: &ResolveContext,
    truth: &GroundTruth,
) -> Result<Vec<String>, PreflightError> {
    let dockerfile = text_at(ctx, paths::ISOLATION_BUILD_DOCKERFILE)?;
    let context = text_at(ctx, paths::ISOLATION_BUILD_CONTEXT)?;
    let (Some(dockerfile), Some(context)) = (dockerfile, context) else {
        return Ok(Vec::new());
    };
    let who = if from_plugin(ctx, paths::ISOLATION_BUILD_CONTEXT) {
        "the plug-in environment's build context"
    } else {
        "the experiment's build context"
    };
    let (dockerfile, context) = (
        ctx.config.cwd.join(dockerfile),
        ctx.config.cwd.join(context),
    );
    let inputs = tokio::task::spawn_blocking(move || BuildInputs::read(&dockerfile, &context))
        .await
        .map_err(|e| failed(format!("the build inputs were not read: {e}")))?
        .map_err(|e| failed(format!("the build inputs cannot be read: {e}")))?;
    let mut found = Vec::new();
    if let Some(file) = truth.named(&inputs.dockerfile) {
        found.push(describe(file, &format!("the Dockerfile of {who}")));
    }
    for (path, digest) in &inputs.context {
        if let Some(file) = truth.named(digest) {
            found.push(describe(file, &format!("`{path}` in {who}")));
        }
    }
    Ok(found)
}

/// The agent inputs of the selected plug-in environment.
async fn input_exposures(
    ctx: &ResolveContext,
    truth: &GroundTruth,
) -> Result<Vec<String>, PreflightError> {
    // BORROW: explicit .as_slice()
    let Some(fragment) = Fragment::from_values(ctx.values.as_slice(), ctx.settings.as_slice())
        .ok() // EXPLICIT: an unreadable environment is refused by the plug-in environment step
        .flatten()
    else {
        return Ok(Vec::new());
    };
    let mut found = Vec::new();
    for input in &fragment.inputs {
        let root: PathBuf = fragment.dir.join(input.path());
        let files = tokio::task::spawn_blocking(move || walk_files(&root))
            .await
            .map_err(|e| failed(format!("an input was not read: {e}")))?
            .map_err(|e| failed(format!("the agent input cannot be read: {e}")))?;
        for (path, digest) in &files {
            if let Some(file) = truth.named(digest) {
                found.push(describe(
                    file,
                    &place_of_input(&fragment.selector, input, path),
                ));
            }
        }
    }
    Ok(found)
}

fn place_of_input(selector: &str, input: &ExperimentInput, path: &str) -> String {
    format!(
        "`{path}` in agent input `{}` of the plug-in environment `{selector}`",
        input.kind()
    )
}

fn describe(file: &TruthFile, place: &str) -> String {
    format!(
        "evaluator-only input `{}` ({}) has the bytes of {place}",
        file.kind, file.path
    )
}

/// A directory of its own for the record of the archive command.
fn record_dir() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "candler-ground-truth-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

/// The files of the treatment's skill tree at its pinned commit.
async fn treatment_exposures(
    ctx: &ResolveContext,
    truth: &GroundTruth,
    supervisor: &Supervisor,
    skills: SkillTree,
) -> Result<Vec<String>, PreflightError> {
    let read = |text: &str| {
        FieldPath::from_str(text)
            .ok()
            .and_then(|path| ctx.get(&path))
            .map(|value| value.as_value().clone()) // BORROW: the step owns its copy
    };
    let (Some(spec), Some(pinned)) = (
        read(paths::REQUEST_TREATMENT)
            .and_then(|value| serde_json::from_value::<TreatmentSpec>(value).ok()),
        read(paths::TREATMENT).and_then(|value| serde_json::from_value::<TreatmentRef>(value).ok()),
    ) else {
        return Ok(Vec::new());
    };
    let Some(source) = spec.source.as_deref() else {
        return Ok(Vec::new());
    };
    if pinned.commit.is_none() {
        return Ok(Vec::new());
    }
    let command = skills
        .archive_command(&pinned, &ctx.config.cwd.join(source))
        .map_err(|e| failed(e.to_string()))?;
    let record = record_dir();
    tokio::fs::create_dir_all(&record)
        .await
        .map_err(|e| failed(format!("cannot make the archive directory: {e}")))?;
    let path = std::env::var("PATH").unwrap_or_default();
    let request = SpawnRequest::builder()
        .argv(command.argv().to_vec()) // BORROW: the request owns its arguments
        .env(vec![
            ("PATH".to_owned(), path), // BORROW: owned pair for the environment
            ("LC_ALL".to_owned(), "C".to_owned()), // BORROW: owned pair for the environment
            ("GIT_TERMINAL_PROMPT".to_owned(), "0".to_owned()), // BORROW: owned pair
        ])
        .marker(Marker::from_str("ground-truth-archive").map_err(|e| failed(e.to_string()))?)
        .record_dir(record.clone()) // BORROW: the directory is removed below
        .build()
        .map_err(|e| failed(e.to_string()))?;
    let captured = supervisor.run_captured(request, ARCHIVE_LIMIT).await;
    // The directory holds only the record of the command; a failed removal leaves it for the temp sweep.
    drop(tokio::fs::remove_dir_all(&record).await);
    let captured = captured.map_err(|e| failed(format!("cannot archive the skill tree: {e}")))?;
    if !matches!(captured.exit.status, ExitStatus::Code { code: 0 })
        || captured.stdout.len() >= ARCHIVE_LIMIT
    {
        return Err(failed("the skill tree could not be archived".to_owned())); // BORROW: owned text
    }
    let files = skills
        .files(&pinned, &captured.stdout)
        .map_err(|e| failed(e.to_string()))?;
    Ok(files
        .iter()
        .filter_map(|prepared| {
            truth
                .named(&Digest::of_bytes(&prepared.contents))
                .map(|file| {
                    describe(
                        file,
                        &format!("`{}` in the treatment's skill tree", prepared.path),
                    )
                })
        })
        .collect())
}

impl PreflightStep for GroundTruthStep {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        let step = self.name.clone(); // BORROW: the future owns the step name
        let truth = Arc::clone(&self.truth);
        let supervisor = Arc::clone(&self.supervisor);
        let skills = self.skills;
        Box::pin(async move {
            let report = StepReport::new(step);
            // No evaluator-only input: nothing is read.
            if truth.is_empty() {
                return Ok(report);
            }
            let mut found = context_exposures(&ctx, &truth).await?;
            found.extend(input_exposures(&ctx, &truth).await?);
            found.extend(truth.agent.iter().filter_map(|copy| {
                truth.named(&copy.digest).map(|file| {
                    describe(
                        file,
                        &format!("`{}` in agent input `{}`", copy.path, copy.kind),
                    )
                })
            }));
            found.extend(treatment_exposures(&ctx, &truth, &supervisor, skills).await?);
            if found.is_empty() {
                return Ok(report);
            }
            let name = NameToken::from_str(CHECK_EXPOSED).map_err(|e| failed(e.to_string()))?;
            let detail = format!(
                "{}: the agent would read the ground truth; remove the copy from what the agent receives",
                found.join("; ")
            );
            Ok(report.check(CheckResult::new(name, false, detail)))
        })
    }
}
