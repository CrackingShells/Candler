//! A job's session: the cells it runs, the secrets it reads and the gateway it owns.
//!
//! The session opens before any run starts and closes after the last run is sealed. Everything it
//! creates is destroyed through the container guard, and the token is read only inside
//! [`Gateway::start`].

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::str::FromStr as _;
use std::sync::Arc;
use std::time::Duration;

use candler_schemas::container::ContainerRole;
use candler_schemas::execution::{Comparable, EventPayload, SecretPresence, TreatmentSpec};
use candler_schemas::primitives::{Digest, FieldPath, Marker, NameToken, SecretName};
use candler_schemas::values::{ResolvedValue, SourceEntry, ValueSource};
use futures::future::BoxFuture;

use super::controls::{ControlContexts, ControlRunner, DockerControlRunner};
use super::schedule::{Clock, wall_clock};
use crate::isolation::Isolation;
use crate::isolation::container::guard::ContainerGuard;
use crate::isolation::container::sweeper::LabelSweeper;
use crate::isolation::container::{DockerDirs, DockerIsolation};
use crate::isolation::docker::{ClientEnv, DockerCli, DockerError};
use crate::isolation::driver::DockerDriver;
use crate::isolation::gateway::ChildRecorder;
use crate::isolation::gateway::DockerCli as GatewayCli;
use crate::isolation::gateway::{
    DockerGateway, Gateway, GatewayError, GatewayStart, GatewayStatus, SecretError, SecretSource,
    SecretValue, UpstreamConfig,
};
use crate::isolation::image::{PackageProbe, PackageReadings};
use crate::isolation::model::{ContainerRef, ImageRef, NetworkName, SweepScope};
use crate::jobs::dir::JobDir;
use crate::jobs::model::JobSpec;
use crate::jobs::view::JobView;
use crate::resolve::layers::Resolution;
use crate::resolve::steps::session;
use crate::runner::evidence::Inputs;
use crate::runner::exec::CellRun;
use crate::runner::plan::{CellId, PlanError, PlanOutcome};
use crate::runner::{JobGateway, Runtime};
use crate::supervise::{EventSink, Supervisor};
use candler_schemas::execution::paths::{
    ENVIRONMENTS, HARNESS, INPUTS, ISOLATION_IMAGE, ISOLATION_RECORD_PACKAGES, MODEL_ENDPOINT,
    MODEL_TOKEN_SECRET, REQUEST_TREATMENT, RUNNER_IDLE_S, SNAPSHOTS, TREATMENT,
};

/// The roots of a cell that hold typed records; their sources are not value paths.
const TYPED_ROOTS: [&str; 5] = [HARNESS, TREATMENT, SNAPSHOTS, INPUTS, ENVIRONMENTS];

/// The tag the gateway's sidecar image is looked up by.
pub(super) const SIDECAR_TAG: &str = "candler-gateway:latest";
/// The marker the gateway's docker commands carry.
const GATEWAY_MARKER: &str = "gateway";
/// How long a sweep of the job's containers may take.
pub(crate) const SWEEP_LIMIT: Duration = Duration::from_secs(60);

/// Why a job could not be set up or run.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OrchestrateError {
    /// The secret that holds the gateway's token cannot be read.
    #[error("the secret `{name}` cannot be read")]
    TokenSource {
        /// The name of the secret; never its value.
        name: String,
    },
    /// The gateway does not serve.
    #[error("the job's gateway does not serve: {reason}")]
    Gateway {
        /// Why, as a reason code or a message.
        reason: String,
    },
    /// The setting that holds the treatment is not set, or does not hold a treatment.
    #[error("the setting `request.treatment` is not set to a treatment")]
    Treatment,
    /// The setting that names the gateway's secret is not set.
    #[error("the setting `model.token_secret` is not set")]
    TokenSetting,
    /// Planning refused or failed.
    #[error(transparent)]
    Plan(#[from] PlanError),
    /// Docker cannot be reached.
    #[error("docker is not available: {detail}")]
    Docker {
        /// What went wrong.
        detail: String,
    },
    /// The session controls of the harness version in the base image did not pass, or could not
    /// run: session mode is refused for that harness version.
    #[error(
        "session mode is refused for {harness}: the control `{control}` did not pass: {reason}"
    )]
    Controls {
        /// The harness and its version, as `<name> <version>`.
        harness: String,
        /// The control that failed (`c1` or `c2`), or `setup` when none could run.
        control: String,
        /// Why it failed.
        reason: String,
    },
    /// The job's directory cannot be read.
    #[error("the job's directory cannot be read: {detail}")]
    Job {
        /// What went wrong.
        detail: String,
    },
}

/// Reads secrets from the environment of this process, only the ones the experiment declares.
#[derive(Debug, Clone)]
pub struct EnvSecrets {
    declared: BTreeSet<SecretName>,
}

impl EnvSecrets {
    /// A source of the secrets `declared`.
    #[must_use]
    pub fn new(declared: &[SecretName]) -> Self {
        Self {
            // BORROW: .iter().cloned() gives the source its own names
            declared: declared.iter().cloned().collect(),
        }
    }

    /// The declared secrets that are set and not empty now. It tests for presence and never
    /// keeps a value.
    #[must_use]
    pub fn present(&self) -> BTreeSet<SecretName> {
        self.declared
            .iter()
            // BORROW: .as_str() hands the name to the environment
            .filter(|name| std::env::var_os(name.as_str()).is_some_and(|value| !value.is_empty()))
            .cloned() // BORROW: .cloned() gives the set its own names
            .collect()
    }
}

impl SecretSource for EnvSecrets {
    fn resolve(&self, name: &str) -> Result<SecretValue, SecretError> {
        let missing = || SecretError::Missing {
            // BORROW: .to_owned() gives the error the name, never a value
            name: name.to_owned(),
        };
        // BORROW: .as_str() compares the declared name with the one asked for
        if !self.declared.iter().any(|own| own.as_str() == name) {
            return Err(missing());
        }
        match std::env::var(name) {
            Ok(value) if !value.is_empty() => Ok(SecretValue::new(value)),
            Ok(_) | Err(_) => Err(missing()),
        }
    }
}

/// What a job needs from the machine around it: the docker client and the gateway's service. The
/// binary builds the production host; a test builds one over fakes.
pub trait JobHost: Send + Sync {
    /// The docker client the job's guard destroys through.
    ///
    /// # Errors
    /// [`DockerError::Unavailable`] when there is no docker client.
    fn docker_cli(
        &self,
        supervisor: Arc<Supervisor>,
        record_dir: PathBuf,
    ) -> Result<DockerCli, DockerError>;

    /// The service that starts the job's gateway.
    ///
    /// # Errors
    /// [`OrchestrateError`] when the gateway cannot be configured.
    fn gateway<'a>(
        &'a self,
        rt: &'a Runtime,
        spec: &'a JobSpec,
        // TRAIT_OBJECT: the gateway is the docker sidecar, or a fake in tests
    ) -> BoxFuture<'a, Result<Arc<dyn Gateway>, OrchestrateError>>;

    /// The isolation the job's runs are placed through, from the docker one the session built.
    /// The default is `isolation` itself.
    // TRAIT_OBJECT: the runtime holds the isolation as a trait object
    fn isolation(&self, isolation: DockerIsolation) -> Arc<dyn Isolation> {
        Arc::new(isolation)
    }

    /// The clock the job's wall cap reads. The default counts real time from the call.
    fn clock(&self) -> Clock {
        wall_clock()
    }

    /// The runner of the session controls, or why this host has none: a job with a session cell
    /// is then refused for that reason.
    ///
    /// # Errors
    /// The reason the host cannot run session controls.
    // TRAIT_OBJECT: the runner is docker, or a fake in tests
    fn controls(&self) -> Result<Arc<dyn ControlRunner>, String> {
        Err("this host has no runner for the session controls".to_owned()) // BORROW: the error owns its text
    }

    /// The distribution and version of the import packages `names` inside the env image `image`,
    /// read once for the job. The default reads nothing.
    fn packages<'a>(
        &'a self,
        rt: &'a Runtime,
        image: &'a ImageRef,
        names: &'a [String],
    ) -> BoxFuture<'a, Option<PackageReadings>> {
        let _ = (rt, image, names);
        Box::pin(async { None })
    }
}

/// The host of a real job: the `docker` on `PATH` and a [`DockerGateway`] from the sidecar image
/// docker holds under the tag `candler-gateway:latest`.
#[derive(Debug, Clone)]
pub struct DockerHost {
    controls: Result<ControlContexts, String>,
}

impl Default for DockerHost {
    fn default() -> Self {
        Self::without_controls("this host has no runner for the session controls")
    }
}

impl DockerHost {
    /// The host that runs session controls from the build contexts `contexts`.
    #[must_use]
    pub fn with_controls(contexts: ControlContexts) -> Self {
        Self {
            controls: Ok(contexts),
        }
    }

    /// The host that cannot run session controls, for the reason `why`.
    #[must_use]
    pub fn without_controls(why: &str) -> Self {
        Self {
            controls: Err(why.to_owned()), // BORROW: the host owns its reason
        }
    }
}

/// The text a resolution holds at `path`, among its values and its settings.
fn text_at(resolution: &Resolution, path: &str) -> Option<String> {
    resolution
        .values
        .iter()
        .chain(resolution.settings.iter())
        // BORROW: .as_str() compares the path with the one asked for
        .find(|value| value.path().as_str() == path)
        .and_then(ResolvedValue::value)
        .and_then(|value| value.as_value().as_str())
        .map(str::to_owned) // BORROW: the caller owns the text
}

/// The treatment a resolution's `request.treatment` setting holds.
fn treatment_of(resolution: &Resolution) -> Result<TreatmentSpec, OrchestrateError> {
    resolution
        .settings
        .iter()
        // BORROW: .as_str() compares the path with the one asked for
        .find(|value| value.path().as_str() == REQUEST_TREATMENT)
        .and_then(ResolvedValue::value)
        // BORROW: .clone() hands the JSON to the deserializer, which takes it by value
        .and_then(|value| serde_json::from_value(value.as_value().clone()).ok())
        .ok_or(OrchestrateError::Treatment)
}

/// The names a resolution lists at `path`, among its values and its settings; none when the path
/// is unset or does not hold a list of strings.
fn strings_at(resolution: &Resolution, path: &str) -> Vec<String> {
    resolution
        .values
        .iter()
        .chain(resolution.settings.iter())
        // BORROW: .as_str() compares the path with the one asked for
        .find(|value| value.path().as_str() == path)
        .and_then(ResolvedValue::value)
        .and_then(|value| value.as_value().as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_owned)) // BORROW: the caller owns the names
                .collect()
        })
        .unwrap_or_default()
}

/// The import packages the experiment records for each env image: the names of
/// `isolation.record_packages`.
#[must_use]
pub fn record_packages(resolution: &Resolution) -> Vec<String> {
    strings_at(resolution, ISOLATION_RECORD_PACKAGES)
}

/// The upstream a `model.endpoint` URL names.
fn upstream_of(endpoint: &str) -> Result<UpstreamConfig, GatewayError> {
    let url = url::Url::parse(endpoint).map_err(|error| GatewayError::Config {
        detail: error.to_string(),
    })?;
    let host = url.host_str().ok_or_else(|| GatewayError::Config {
        // BORROW: .to_owned() gives the error its own text
        detail: "the model endpoint names no host".to_owned(),
    })?;
    let builder = UpstreamConfig::builder().host(host);
    match url.port() {
        Some(port) => builder.port(port).build(),
        None => builder.build(),
    }
}

/// The job's gateway service over the sidecar image docker holds under [`SIDECAR_TAG`],
/// forwarding to `upstream` with at most `max_parallel` calls at once; its docker commands carry
/// `marker` and record under the job directory of `rt`.
///
/// # Errors
/// [`OrchestrateError::Gateway`] when the sidecar image is missing or the job directory cannot
/// be opened.
pub(super) async fn sidecar_gateway(
    rt: &Runtime,
    upstream: UpstreamConfig,
    max_parallel: u32,
    marker: Marker,
) -> Result<Arc<dyn Gateway>, OrchestrateError> {
    let failed = |reason: String| OrchestrateError::Gateway { reason };
    let info = rt
        .docker
        .image_inspect(SIDECAR_TAG)
        .await
        .map_err(|e| failed(e.to_string()))?
        // BORROW: .to_owned() gives the error its own text
        .ok_or_else(|| failed("sidecar_image_missing".to_owned()))?;
    let image = ImageRef {
        id: info.id,
        harness_version: None,
    };
    let mut env: Vec<(String, String)> = ClientEnv::from_process().pairs().to_vec();
    if let Some(path) = std::env::var_os("PATH") {
        // BORROW: the child's environment owns its pair
        env.push(("PATH".to_owned(), path.to_string_lossy().into_owned()));
    }
    let job_dir = JobDir::open(rt.store.layout(), &rt.job).map_err(|e| failed(e.to_string()))?;
    let cli = GatewayCli::new(env, marker, job_dir.control_dir(), job_dir.children_dir());
    let service = DockerGateway::new(
        image,
        upstream,
        max_parallel,
        cli,
        Arc::clone(&rt.supervisor),
    );
    // TRAIT_OBJECT: the job starts its gateway through the service trait
    let service: Arc<dyn Gateway> = Arc::new(service);
    Ok(service)
}

impl JobHost for DockerHost {
    fn controls(&self) -> Result<Arc<dyn ControlRunner>, String> {
        let contexts = self.controls.clone()?; // BORROW: the runner owns its contexts
        let runner: Arc<dyn ControlRunner> = Arc::new(DockerControlRunner::new(contexts));
        Ok(runner)
    }

    fn packages<'a>(
        &'a self,
        rt: &'a Runtime,
        image: &'a ImageRef,
        names: &'a [String],
    ) -> BoxFuture<'a, Option<PackageReadings>> {
        Box::pin(async move {
            let job_dir = JobDir::open(rt.store.layout(), &rt.job).ok()?;
            let env = ClientEnv::from_process();
            let cli = DockerCli::new(
                Arc::clone(&rt.supervisor),
                env.clone(), // BORROW: the client owns its environment
                job_dir.control_dir(),
            )
            .ok()?;
            let probe = PackageProbe::new(
                Arc::clone(&rt.supervisor),
                cli.binary().to_path_buf(), // BORROW: the probe owns the client's binary path
                env,
                rt.store.layout().root().to_path_buf(), // BORROW: the probe owns the store root
                names.to_vec(),                         // BORROW: the probe owns the names
            );
            probe.read(image).await.ok()
        })
    }

    fn docker_cli(
        &self,
        supervisor: Arc<Supervisor>,
        record_dir: PathBuf,
    ) -> Result<DockerCli, DockerError> {
        DockerCli::new(supervisor, ClientEnv::from_process(), record_dir)
    }

    fn gateway<'a>(
        &'a self,
        rt: &'a Runtime,
        spec: &'a JobSpec,
        // TRAIT_OBJECT: the gateway is the docker sidecar, or a fake in tests
    ) -> BoxFuture<'a, Result<Arc<dyn Gateway>, OrchestrateError>> {
        Box::pin(async move {
            let failed = |reason: String| OrchestrateError::Gateway { reason };
            let endpoint = text_at(&spec.resolution, MODEL_ENDPOINT)
                // BORROW: .to_owned() gives the error its own text
                .ok_or_else(|| failed("model_endpoint_unset".to_owned()))?;
            // BORROW: .as_str() hands the endpoint to the parser
            let upstream = upstream_of(endpoint.as_str()).map_err(|e| failed(e.to_string()))?;
            let marker: Marker = GATEWAY_MARKER
                .parse()
                // BORROW: .to_owned() gives the error its own text
                .map_err(|_| failed("marker_invalid".to_owned()))?;
            sidecar_gateway(rt, upstream, spec.budget.max_parallel, marker).await
        })
    }
}

/// What a job owns while it runs: the guard it destroys through and the names of what it made.
pub struct JobSession {
    pub(super) guard: Arc<ContainerGuard>,
    pub(super) sweeper: Arc<LabelSweeper>,
    // TRAIT_OBJECT: the sink is the job's event log, or a recorder in tests
    pub(super) events: Arc<dyn EventSink>,
    pub(super) networks: [NetworkName; 2],
    pub(super) gateway: Option<ContainerRef>,
}

/// The sources a cell's manifest carries: one for each value of the cell, and the ones of the
/// typed records. A value the resolution gives no source is one the experiment file sets for the
/// cell: a factor level or a cap. The sources of settings are not the cell's.
fn sources_of(
    comparable: &Comparable,
    resolution: &Resolution,
    idle_derived: bool,
) -> Vec<SourceEntry> {
    let held: BTreeSet<&FieldPath> = comparable
        .values()
        .iter()
        .map(ResolvedValue::path)
        .collect();
    let mut sources: Vec<SourceEntry> = resolution
        .sources
        .iter()
        .filter(|entry| {
            held.contains(entry.path())
                || entry
                    .path()
                    .segments()
                    .next()
                    .is_some_and(|root| TYPED_ROOTS.contains(&root))
        })
        .cloned() // BORROW: the cell owns its sources
        .collect();
    for path in &held {
        if !sources.iter().any(|entry| entry.path() == *path) {
            // A session whose mode is a factor level gets its idle default per cell, from the
            // session rule: planning says which cells it derived it for.
            let derived = path.as_str() == RUNNER_IDLE_S && idle_derived;
            let source = match NameToken::from_str(session::STEP_NAME) {
                Ok(step) if derived => ValueSource::derived(step),
                _ => ValueSource::ExperimentFile,
            };
            // BORROW: the entry owns its path
            sources.push(SourceEntry::new((*path).clone(), source));
        }
    }
    sources.sort_by(|a, b| a.path().cmp(b.path()));
    sources
}

/// The id of the env image a cell runs.
#[must_use]
pub fn image_of(comparable: &Comparable) -> Option<String> {
    comparable
        .values()
        .iter()
        // BORROW: .as_str() compares the path with the one asked for
        .find(|value| value.path().as_str() == ISOLATION_IMAGE)
        .and_then(ResolvedValue::value)
        .and_then(|value| value.as_value().as_str())
        .map(str::to_owned) // BORROW: the caller owns the id
}

/// The cells of a job: what each one is, with the secrets it needs and where its treatment is.
///
/// # Errors
/// [`OrchestrateError::Treatment`] when the setting that holds the treatment is not set to a
/// treatment.
pub fn cells_of(
    spec: &JobSpec,
    outcome: &PlanOutcome,
    present: &BTreeSet<SecretName>,
    packages: &BTreeMap<String, PackageReadings>,
) -> Result<BTreeMap<CellId, Arc<CellRun>>, OrchestrateError> {
    let source = treatment_of(&spec.resolution)?.source.map(PathBuf::from);
    let factors: Vec<_> = spec
        .experiment
        .factors()
        .iter()
        .map(|factor| factor.path.clone()) // BORROW: each cell owns its factor paths
        .collect();
    let secrets: Vec<SecretPresence> = spec
        .experiment
        .secrets()
        .iter()
        .map(|name| {
            // BORROW: each cell owns its secret names
            SecretPresence::new(name.clone(), present.contains(name))
        })
        .collect();
    let inputs = Inputs {
        // BORROW: .clone() gives the cells their own input digests
        digests: spec.inputs.clone(),
    };
    let mut cells = BTreeMap::new();
    for (digest, comparable) in &outcome.fresh {
        // a pinned cell is written as it was stored, so its sources are the stored cell's
        let written = outcome
            .plans
            .iter()
            .find(|plan| plan.cell == *digest)
            .and_then(|plan| plan.stored.as_ref())
            .unwrap_or(comparable);
        let cell = CellRun::new(
            comparable.clone(),         // BORROW: the cell owns its comparable
            outcome.experiment.clone(), // BORROW: the cell owns its experiment digest
            factors.clone(),            // BORROW: the cell owns its factors
            sources_of(
                written,
                &spec.resolution,
                outcome.idle_derived.contains(digest),
            ),
            secrets.clone(), // BORROW: the cell owns its secrets
            source.clone(),  // BORROW: the cell owns its treatment source
            inputs.clone(),  // BORROW: the cell owns its inputs
        )
        // BORROW: the cell owns the evaluator-only inputs
        .with_evaluation_inputs(spec.evaluation_inputs.clone())
        // BORROW: the cell owns the package readings of its image
        .with_packages(
            image_of(comparable)
                .and_then(|id| packages.get(&id).cloned())
                .unwrap_or_default(),
        );
        cells.insert(digest.clone(), Arc::new(cell)); // BORROW: the map owns the cell digest
    }
    Ok(cells)
}

impl JobSession {
    /// Opens the session: builds the guard and the sweeper, starts the gateway on the job's own
    /// networks, asks it once whether it serves, then sets the runtime's isolation and gateway.
    /// A failure after a network may exist destroys what exists, through the guard, first.
    ///
    /// # Errors
    /// [`OrchestrateError::TokenSetting`] when `model.token_secret` names nothing,
    /// [`OrchestrateError::TokenSource`] when the secret cannot be read,
    /// [`OrchestrateError::Docker`] and [`OrchestrateError::Job`] when the client or the job's
    /// directory cannot be had, and [`OrchestrateError::Gateway`] when the gateway does not
    /// start or does not serve.
    pub async fn open(
        rt: &mut Runtime,
        spec: &JobSpec,
        experiment: &Digest,
        // TRAIT_OBJECT: the host is the process's own host, or a fake in tests
        host: &dyn JobHost,
        // TRAIT_OBJECT: the secrets are the environment's, or a fake in tests
        secrets: &dyn SecretSource,
    ) -> Result<Self, OrchestrateError> {
        let name =
            text_at(&spec.resolution, MODEL_TOKEN_SECRET).ok_or(OrchestrateError::TokenSetting)?;
        let service = host.gateway(rt, spec).await?;
        Self::open_service(rt, &name, service, experiment, host, secrets).await
    }

    /// Opens the session over the gateway `service`, whose token is the secret `name`: the
    /// part of [`JobSession::open`] that does not depend on the job's spec.
    ///
    /// # Errors
    /// The errors of [`JobSession::open`] that follow the gateway's configuration.
    pub(super) async fn open_service(
        rt: &mut Runtime,
        name: &str,
        // TRAIT_OBJECT: the gateway is the docker sidecar, or a fake in tests
        service: Arc<dyn Gateway>,
        experiment: &Digest,
        // TRAIT_OBJECT: the host is the process's own host, or a fake in tests
        host: &dyn JobHost,
        // TRAIT_OBJECT: the secrets are the environment's, or a fake in tests
        secrets: &dyn SecretSource,
    ) -> Result<Self, OrchestrateError> {
        let job_dir =
            JobDir::open(rt.store.layout(), &rt.job).map_err(|error| OrchestrateError::Job {
                detail: error.to_string(),
            })?;
        let cli = host
            .docker_cli(Arc::clone(&rt.supervisor), job_dir.control_dir())
            .map_err(|error| OrchestrateError::Docker {
                detail: error.to_string(),
            })?;
        // TRAIT_OBJECT: the guard and the sweeper read the job through its directory
        let view: Arc<dyn JobView> = Arc::new(job_dir.clone()); // BORROW: the view owns its directory
        let guard = Arc::new(
            ContainerGuard::over_docker(&cli, Arc::clone(&view)).with_events(Arc::clone(&rt.sink)),
        );
        let sweeper = Arc::new(
            LabelSweeper::new(
                Arc::clone(&rt.docker),
                Arc::clone(&guard),
                view,
                SWEEP_LIMIT,
            )
            .with_events(Arc::clone(&rt.sink)),
        );
        let mut session = Self {
            guard: Arc::clone(&guard),
            sweeper: Arc::clone(&sweeper),
            events: Arc::clone(&rt.sink),
            networks: [
                NetworkName::for_job(&rt.job),
                NetworkName::egress_for_job(&rt.job),
            ],
            gateway: None,
        };
        let docker: &dyn DockerDriver = &*rt.docker; // TRAIT_OBJECT: the driver is docker, or a recording fake in tests
        let started = service
            .start(
                &rt.supervisor,
                docker,
                GatewayStart {
                    job: &rt.job,
                    experiment,
                    recorder: &job_dir,
                    token: secrets,
                    secret: name,
                },
            )
            .await;
        let handle = match started {
            Ok(handle) => handle,
            Err(error) => {
                session.close_unstarted().await;
                return Err(match error {
                    GatewayError::Secret(SecretError::Missing { name }) => {
                        OrchestrateError::TokenSource { name }
                    }
                    GatewayError::Unavailable { .. }
                    | GatewayError::Start { .. }
                    | GatewayError::SecretFormat { .. }
                    | GatewayError::Config { .. }
                    | GatewayError::NotReady { .. } => OrchestrateError::Gateway {
                        reason: error.to_string(),
                    },
                });
            }
        };
        // the gateway's record is written; its event names it
        rt.sink.emit(
            None,
            EventPayload::container_created(handle.container.cid.clone(), ContainerRole::Gateway), // BORROW: the event owns the id
        );
        session.gateway = Some(handle.container.clone()); // BORROW: the session owns the sidecar's reference
        match service.probe(&handle).await {
            // EXPLICIT: a serving gateway is the wanted state; the session goes on
            GatewayStatus::Ready => {}
            GatewayStatus::Unavailable { reason } => {
                session.close(&SweepScope::CurrentJob).await;
                return Err(OrchestrateError::Gateway { reason });
            }
        }
        let store_root = rt.store.layout().root().to_path_buf();
        // TRAIT_OBJECT: the isolation records through the job directory as a trait object
        let recorder: Arc<dyn ChildRecorder> = Arc::new(job_dir.clone()); // BORROW: the recorder owns its directory handle
        let isolation = DockerIsolation::new(
            handle.clone(), // BORROW: the isolation owns the handle it plans against
            Arc::clone(&rt.docker),
            guard,
        )
        .with_sweeper(sweeper)
        .with_recorder(recorder)
        .with_dirs(DockerDirs {
            children: job_dir.children_dir(),
            project_root: store_root.clone(), // BORROW: the dirs own their roots
            store_root,
        });
        rt.isolation = Some(host.isolation(isolation));
        rt.gateway = Some(JobGateway { service, handle });
        Ok(session)
    }
}
