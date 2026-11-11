//! The evaluation engine: plans the requests for a selection of runs and records their outcomes.

use std::collections::BTreeMap;
use std::io;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use candler_schemas::evaluation::{EvaluationError, PluginEnvironment, SuiteFile};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{Marker, Name, NameVersion, RequestId, RunKey};
use futures::future::join_all;
use globset::Glob;
use tokio::sync::watch;

use crate::evaluation::client::{Client, ClientError};
use crate::evaluation::registry::{EnvironmentKey, Registry, RegistryError};
use crate::jobs::slots::{Holder, SlotError, Slots};
use crate::store::{Store, StoreError};
use crate::supervise::{EventSink, Supervisor};

mod pending;
mod plan;
mod run;
mod write;

pub use pending::pending_runs;

pub use plan::{Batch, PlanError, RunPlan, applies, plan};
pub use run::REQUEST_DEADLINE_MARGIN;

/// The machine-wide pool an evaluation's requests take their slot from: one slot per evaluator
/// process in flight, whoever holds the others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatorSlots {
    /// The pool's directory, `jobs::slots::evaluator_slots_dir` for a job.
    pub dir: PathBuf,
    /// How many slots the pool has.
    pub count: NonZeroU32,
    /// The job the slots are held for, as the holder files name it.
    pub job: String,
}

/// Which runs of the store an evaluation grades. Only sealed runs are ever graded.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum RunSelection {
    /// Every run.
    All,
    /// The runs whose key matches the glob.
    Glob(String),
    /// The runs whose key is in the list.
    Keys(Vec<RunKey>),
    /// The runs with no complete outcome set for the suite's evaluator versions, as
    /// [`pending_runs`] decides.
    Pending,
}

/// What one `evaluate` call grades: which runs of which store, by which plug-ins and suite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluateRequest {
    /// The store root.
    pub store: PathBuf,
    /// The plug-in manifest paths.
    pub plugins: Vec<PathBuf>,
    /// The suite file.
    pub suite: PathBuf,
    /// Which runs to grade.
    pub selection: RunSelection,
    /// The pool the requests take their evaluator slot from.
    pub slots: EvaluatorSlots,
    /// The root under which the plug-in environments are built, outside the store.
    pub env_root: PathBuf,
    /// A flat deadline for every request; `None` sets each request's deadline to the sum of its
    /// items' deadlines plus a margin.
    pub request_deadline: Option<Duration>,
    /// Where the evaluator processes' child records go; `None` keeps them in the call's own
    /// scratch directory, which the call removes.
    pub record_dir: Option<PathBuf>,
}

/// What one `evaluate` call did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EvaluateReport {
    /// The id of every request sent.
    pub request_ids: Vec<RequestId>,
    /// The deadline each request was given, in the order of `request_ids`: the flat override when
    /// one is set, otherwise the sum of the items' deadlines plus `REQUEST_DEADLINE_MARGIN`. A
    /// request that never reached the evaluator has no entry.
    pub request_deadlines: Vec<(RequestId, Duration)>,
    /// The number of outcome records per status.
    pub counts: BTreeMap<String, u64>,
    /// The outcome files written.
    pub outcome_files: Vec<PathBuf>,
    /// The requests that failed, each with why.
    pub failed_requests: Vec<(RequestId, String)>,
}

impl EvaluateReport {
    /// The report as the document `candler evaluate` prints: `request_ids`, `counts`,
    /// `outcome_files` and `failed_requests`, each failure as `{request, reason}`.
    #[must_use]
    pub fn to_document(&self) -> serde_json::Value {
        serde_json::json!({
            "request_ids": self.request_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "counts": self.counts,
            "outcome_files": self
                .outcome_files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>(),
            "failed_requests": self
                .failed_requests
                .iter()
                .map(|(id, why)| serde_json::json!({"request": id.to_string(), "reason": why}))
                .collect::<Vec<_>>(),
        })
    }
}

/// Why an `evaluate` call could not run.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EvaluateError {
    /// A plug-in's environment has not been built: its `ready` marker is missing.
    #[error("the environment of plug-in `{plugin}` is not ready: {} has no `ready` marker", dir.display())]
    EnvironmentNotReady {
        /// The plug-in.
        plugin: Name,
        /// The environment directory that lacks the marker.
        dir: PathBuf,
    },
    /// The plug-in manifests or the suite check failed.
    #[error("the plug-in registry: {0}")]
    Registry(#[from] RegistryError),
    /// The suite file is not a suite.
    #[error("the suite {} is not valid: {source}", path.display())]
    Suite {
        /// The suite file.
        path: PathBuf,
        /// What the schema reader reported.
        #[source]
        source: EvaluationError,
    },
    /// A file system call failed.
    #[error("{op} failed on {}: {source}", path.display())]
    Io {
        /// What the engine was doing.
        op: &'static str,
        /// The path the call named.
        path: PathBuf,
        /// The error of the call.
        #[source]
        source: io::Error,
    },
    /// The store could not be opened or read.
    #[error("the store: {0}")]
    Store(#[from] StoreError),
    /// The run selection is not a glob.
    #[error("the run selection `{glob}` is not a glob: {detail}")]
    Glob {
        /// The selection as given.
        glob: String,
        /// What the glob compiler said.
        detail: String,
    },
    /// The suite cannot be planned.
    #[error("the suite cannot be planned: {0}")]
    Plan(#[from] PlanError),
    /// A plug-in could not describe itself.
    #[error("plug-in `{plugin}`: {source}")]
    Describe {
        /// The plug-in.
        plugin: Name,
        /// Why the client could not get a description.
        #[source]
        source: ClientError,
    },
    /// The evaluator slot pool could not be opened.
    #[error("the evaluator slots: {0}")]
    Slots(#[from] SlotError),
    /// A blocking task of the engine panicked or was lost.
    #[error("an engine task failed: {0}")]
    Task(String),
}

/// The variable that tells a job's supervisor which store the launcher used. It is the launcher's
/// own business, so no plug-in process is given it.
const STORE_ROOT_VARIABLE: &str = "CANDLER_STORE_ROOT";

/// The size of the evaluator pool `regrade` opens.
const REGRADE_SLOTS: u32 = 2;

/// The directory under the environment root where `regrade`'s evaluator pool lives.
const REGRADE_POOL: &str = "slots-regrade";

/// Drops what the engine reports: the sink of a call that does not ask for events.
struct Quiet;

impl EventSink for Quiet {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// Grades the runs `request` selects and records their outcomes, as [`evaluate_reporting`] does,
/// and reports nothing to anyone but the supervisor's sink.
///
/// # Errors
///
/// Returns the errors of [`evaluate_reporting`].
pub async fn evaluate(
    request: EvaluateRequest,
    supervisor: Arc<Supervisor>,
    cancel: watch::Receiver<bool>,
) -> Result<EvaluateReport, EvaluateError> {
    // TRAIT_OBJECT: the call reports through a sink that drops what it is given
    evaluate_reporting(request, supervisor, cancel, Arc::new(Quiet)).await
}

/// Grades the runs `request` selects and records their outcomes. Every evaluator process is
/// spawned and stopped through `supervisor`; when `cancel` turns `true`, the requests in flight are
/// cancelled through the client and commit no outcome file.
///
/// Each request waits for a slot of `request.slots` before its evaluator process starts, and holds
/// it until the request's outcome is settled. Once the runs are selected and planned, `events`
/// gets one `evaluation.planned` with the number of outcome records the call will try to commit;
/// then one `evaluation.outcome` for each record of an outcome file, right after that file is
/// published, in the order of the file. A cancelled or failed request reports no outcome.
///
/// Each run's entries are planned by [`plan()`], and each batch of the plan is one request with one
/// outcome file. A run that is not sealed is skipped, since its context can still change. A
/// failing request leaves no file and is listed in [`EvaluateReport::failed_requests`]; the call
/// still returns `Ok` for it. A watch whose sender was dropped is read as "not cancelled".
///
/// # Errors
///
/// Returns [`EvaluateError::EnvironmentNotReady`] when a plug-in the suite names has no built
/// environment, and the other [`EvaluateError`] variants when the manifests, the suite or the store
/// cannot be read, a plug-in cannot describe itself, the suite fails its check or cannot be planned.
pub async fn evaluate_reporting(
    request: EvaluateRequest,
    supervisor: Arc<Supervisor>,
    cancel: watch::Receiver<bool>,
    // TRAIT_OBJECT: the sink is the job's event log, or a recording fake in tests
    events: Arc<dyn EventSink>,
) -> Result<EvaluateReport, EvaluateError> {
    let env_root = std::path::absolute(&request.env_root).map_err(|source| EvaluateError::Io {
        op: "resolve the environment root",
        path: request.env_root.clone(), // BORROW: the error owns the path
        source,
    })?;

    let work = env_root.join(format!("work-{}", RequestId::new_v7()));
    let result = evaluate_in(request, &env_root, &work, supervisor, cancel, events).await;
    // A failed removal leaves a scratch directory outside the store; the next call has its own.
    let removal = work; // BORROW: the blocking task owns the path
    let _ = blocking(move || std::fs::remove_dir_all(removal)).await;
    result
}

/// The body of [`evaluate`]; the caller removes `work` when it returns.
async fn evaluate_in(
    request: EvaluateRequest,
    env_root: &Path,
    work: &Path,
    supervisor: Arc<Supervisor>,
    cancel: watch::Receiver<bool>,
    // TRAIT_OBJECT: the sink is the job's event log, or a recording fake in tests
    events: Arc<dyn EventSink>,
) -> Result<EvaluateReport, EvaluateError> {
    let record_dir = request
        .record_dir
        .clone() // BORROW: the batches own the path
        .unwrap_or_else(|| work.join("records"));
    let plugins = request.plugins.clone(); // BORROW: the blocking task owns the paths
    let suite_path = request.suite.clone(); // BORROW: the blocking task owns the path
    let (registry, suite) = blocking(move || {
        let registry = Registry::load(&plugins)?;
        let text = std::fs::read_to_string(&suite_path).map_err(|source| EvaluateError::Io {
            op: "read the suite",
            path: suite_path.clone(), // BORROW: the error owns the path
            source,
        })?;
        // The reader follows the extension: `.json` is JSON, anything else is TOML.
        let is_json = suite_path.extension().is_some_and(|ext| ext == "json");
        // BORROW: explicit .as_str() instead of Deref coercion
        let parsed = if is_json {
            SuiteFile::from_json_str(text.as_str())
        } else {
            SuiteFile::from_toml_str(text.as_str())
        };
        let suite = parsed.map_err(|source| EvaluateError::Suite {
            path: suite_path,
            source,
        })?;
        Ok::<_, EvaluateError>((registry, suite))
    })
    .await??;
    // Preflight: every plug-in the suite names must have a built environment.
    let mut names: Vec<Name> = Vec::new();
    for entry in suite.expectations() {
        let name = &entry.evaluator().name;
        if registry.plugin(name).is_some() && !names.contains(name) {
            names.push(name.clone()); // BORROW: the list owns the names
        }
    }
    let mut env_dirs: BTreeMap<Name, Option<PathBuf>> = BTreeMap::new();
    for name in &names {
        let Some(plugin) = registry.plugin(name) else {
            continue;
        };
        let dir = if let PluginEnvironment::Uv { project, lock, .. } = plugin.manifest.environment()
        {
            {
                let project_dir = plugin.dir.join(project.as_str());
                let key = EnvironmentKey::of_files(
                    &plugin.dir.join(lock.as_str()),
                    &project_dir.join("pyproject.toml"),
                )
                .await?;
                let dir = key.dir(env_root);
                let ready = dir.join("ready");
                let present = tokio::fs::try_exists(&ready).await.map_err(|source| {
                    EvaluateError::Io {
                        op: "look for the ready marker",
                        path: ready.clone(), // BORROW: the error owns the path
                        source,
                    }
                })?;
                if !present {
                    return Err(EvaluateError::EnvironmentNotReady {
                        plugin: name.clone(), // BORROW: the error owns the name
                        dir,
                    });
                }
                Some(dir)
            }
        } else {
            // EXPLICIT: a plug-in with no `uv` environment has nothing to preflight
            None
        };
        env_dirs.insert(name.clone(), dir); // BORROW: the map owns the names
    }

    // Describe each plug-in once, then check the suite against the answers.
    let mut evaluators: BTreeMap<Name, run::Evaluator> = BTreeMap::new();
    let mut described = BTreeMap::new();
    for name in &names {
        if *cancel.borrow() {
            return Ok(EvaluateReport::default());
        }
        let Some(plugin) = registry.plugin(name) else {
            continue;
        };
        let (cwd, env) = process_of(plugin, env_dirs.get(name).and_then(Option::as_ref));
        let marker: Marker = format!("evaluation-describe-{}", RequestId::new_v7())
            .parse()
            .map_err(|_| EvaluateError::Task("the evaluation marker is invalid".to_owned()))?;
        let command = crate::evaluation::client::EvaluatorCommand::new(
            // BORROW: .to_vec() gives the command its own argument list
            plugin.manifest.command().to_vec(),
            marker,
            record_dir.clone(), // BORROW: the command owns the path
        )
        .env(env.clone()) // BORROW: the command owns its environment
        .cwd(cwd.clone()); // BORROW: the command owns its directory
        tokio::fs::create_dir_all(&record_dir)
            .await
            .map_err(|source| EvaluateError::Io {
                op: "create the record directory",
                path: record_dir.clone(), // BORROW: the error owns the path
                source,
            })?;
        let result = Client::new(&supervisor)
            .describe(&command)
            .await
            .map_err(|source| EvaluateError::Describe {
                plugin: name.clone(), // BORROW: the error owns the name
                source,
            })?;
        let identity: NameVersion = format!("{}@{}", result.name, result.version)
            .parse()
            .map_err(|_| {
                EvaluateError::Task(format!("plug-in `{name}` described an invalid identity"))
            })?;
        evaluators.insert(
            name.clone(), // BORROW: the map owns the names
            run::Evaluator {
                identity,
                instruments: result.instruments.clone(), // BORROW: the engine owns the instruments
                argv: plugin.manifest.command().to_vec(),
                env,
                cwd,
            },
        );
        described.insert(name.clone(), result); // BORROW: the map owns the names
    }
    let checked = registry.check_suite(&suite, &described)?;

    // Select and plan the runs.
    let store = Store::open(&request.store)?;
    let matcher = match &request.selection {
        RunSelection::Glob(glob) => Some(
            Glob::new(glob.as_str()) // BORROW: explicit .as_str()
                .map(|compiled| compiled.compile_matcher())
                .map_err(|error| EvaluateError::Glob {
                    glob: glob.clone(), // BORROW: the error owns the text
                    detail: error.to_string(),
                })?,
        ),
        RunSelection::All | RunSelection::Keys(_) | RunSelection::Pending => None,
    };
    let mut plans = Vec::new();
    for entry in store.runs()? {
        if !entry.done {
            continue;
        }
        let selected = match &request.selection {
            RunSelection::Glob(_) => matcher
                .as_ref()
                .is_some_and(|matcher| matcher.is_match(entry.key.to_string())),
            RunSelection::Keys(keys) => keys.contains(&entry.key),
            RunSelection::All | RunSelection::Pending => true,
        };
        if !selected {
            continue;
        }
        let manifest = store.read_manifest(&entry.key)?;
        plans.push(plan::plan(&manifest, &checked)?);
    }
    if request.selection == RunSelection::Pending {
        let identities: BTreeMap<Name, NameVersion> = evaluators
            .iter()
            .map(|(name, evaluator)| (name.clone(), evaluator.identity.clone())) // BORROW: the map owns its keys and identities
            .collect();
        plans = pending_runs(&store, plans, &identities)?;
    }
    let planned: usize = plans
        .iter()
        .map(|plan| {
            plan.not_applicable
                .len()
                .saturating_add(plan.batches.iter().map(|batch| batch.entries.len()).sum())
        })
        .sum();
    events.emit(
        None,
        EventPayload::evaluation_planned(u32::try_from(planned).unwrap_or(u32::MAX)),
    );

    let pool = Slots::open(&request.slots.dir, request.slots.count.get())?;
    let shared = run::Shared {
        store,
        evaluators,
        supervisor,
        pool,
        holder: Holder {
            job: request.slots.job.clone(), // BORROW: the batches own the holder
            run: None,
        },
        events,
        cancel,
        work: work.to_path_buf(), // BORROW: the batches own the path
        record_dir,
        deadline: request.request_deadline,
    };
    let outputs = join_all(plans.iter().map(|plan| run::run_plan(&shared, plan))).await;

    let mut report = EvaluateReport::default();
    for output in outputs {
        report.request_ids.extend(output.request_ids);
        report.request_deadlines.extend(output.request_deadlines);
        report.outcome_files.extend(output.files);
        report.failed_requests.extend(output.failed);
        for (status, count) in output.counts {
            *report.counts.entry(status).or_default() += count;
        }
    }
    Ok(report)
}

/// The environment of a plug-in process whose `uv` environment is `venv`: the forwarded variables,
/// with `VIRTUAL_ENV` set to `venv`, `<venv>/bin` first on `PATH`, and `UV_PROJECT_ENVIRONMENT`
/// set to `venv`. The command then finds the environment's own interpreter and packages, and
/// never re-syncs it. Every spawn of the evaluator (`describe` and each request) takes its
/// environment from here.
fn activated_env(
    forwarded: Vec<(String, String)>,
    venv: &std::path::Path,
) -> Vec<(String, String)> {
    let bin = venv.join("bin");
    let mut env: Vec<(String, String)> = Vec::new();
    let mut path_seen = false;
    for (name, value) in forwarded {
        if name == "PATH" {
            path_seen = true;
            let joined = std::env::join_paths(
                std::iter::once(bin.clone()).chain(std::env::split_paths(value.as_str())), // BORROW: explicit .as_str()
            )
            .map_or_else(|_| value.clone(), |p| p.to_string_lossy().into_owned()); // BORROW: .clone() keeps the original on failure
            env.push((name, joined));
        } else {
            env.push((name, value));
        }
    }
    if !path_seen {
        env.push(("PATH".to_owned(), bin.display().to_string()));
    }
    env.push(("VIRTUAL_ENV".to_owned(), venv.display().to_string()));
    env.push((
        "UV_PROJECT_ENVIRONMENT".to_owned(),
        venv.display().to_string(),
    ));
    env
}

/// The directory and environment a plug-in process starts with: the engine process's own
/// environment, with the environment's `.venv` first on `PATH` for a `uv` plug-in.
fn process_of(
    plugin: &crate::evaluation::registry::Plugin,
    env_dir: Option<&PathBuf>,
) -> (PathBuf, Vec<(String, String)>) {
    // The evaluators inherit the environment of the process that runs the engine: for an evaluate
    // job, the caller's. It is passed on as the process environment and recorded nowhere.
    let forwarded: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .filter(|(name, _)| name != STORE_ROOT_VARIABLE)
        .collect();
    if let PluginEnvironment::Uv { project, .. } = plugin.manifest.environment() {
        let env = match env_dir {
            Some(dir) => activated_env(forwarded, dir.join(".venv").as_path()), // BORROW: explicit .as_path()
            None => forwarded,
        };
        (plugin.dir.join(project.as_str()), env)
    } else {
        (plugin.dir.clone(), forwarded) // BORROW: the process owns its directory
    }
}

/// Runs `work` on a blocking thread.
async fn blocking<T, F>(work: F) -> Result<T, EvaluateError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| EvaluateError::Task(error.to_string()))
}

/// Grades the runs `runs` selects (every run for `None`) of the store at `store` with the plug-in
/// whose manifest is `plugin` and the suite at `suite`, as [`evaluate`] does, with the defaults the
/// golden-parity comparison uses: two requests at once, from an evaluator pool of its own under
/// `env_root/slots-regrade`, each with the deadline its items add up to, and no cancel. It keeps
/// the sender of its cancel watch alive until [`evaluate`] returns.
///
/// # Errors
///
/// Returns the errors of [`evaluate`].
pub async fn regrade(
    store: &Path,
    plugin: &Path,
    suite: &Path,
    runs: Option<&str>,
    env_root: &Path,
    supervisor: Arc<Supervisor>,
) -> Result<EvaluateReport, EvaluateError> {
    let (sender, receiver) = watch::channel(false);
    let request = EvaluateRequest {
        store: store.to_path_buf(),
        plugins: vec![plugin.to_path_buf()],
        suite: suite.to_path_buf(),
        selection: runs.map_or(RunSelection::All, |glob| {
            RunSelection::Glob(glob.to_owned())
        }),
        slots: EvaluatorSlots {
            dir: env_root.join(REGRADE_POOL),
            count: NonZeroU32::new(REGRADE_SLOTS).unwrap_or(NonZeroU32::MIN),
            job: format!("regrade-{}", RequestId::new_v7()),
        },
        env_root: env_root.to_path_buf(),
        request_deadline: None,
        record_dir: None,
    };
    let report = evaluate(request, supervisor, receiver).await;
    drop(sender);
    report
}
