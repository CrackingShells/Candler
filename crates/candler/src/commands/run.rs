//! `candler run`: resolves an experiment, plans its runs and starts the job's supervisor.
//!
//! The verb works in the foreground up to the point where nothing can be refused any more: it
//! reads the experiment file, resolves it through the preflight steps, plans the runs without
//! taking the experiment lock, and writes the job's directory. It then starts the detached
//! supervisor, prints the job's id and returns. The supervisor outlives it: `detach` waits only
//! until the supervisor holds the job's lock, never for the job, and reaps its handle by
//! `try_wait` when the supervisor exits first, so the launcher leaves no zombie behind. When the
//! supervisor does not start, the launcher reports it, exits with 2 and removes nothing: a
//! supervisor that is only late is left to run, and `candler ps` shows what became of it.
//!
//! The only paths this verb reads relative to the working directory are the experiment file and
//! the treatment source a flag names, which the user typed, and the project configuration
//! `candler.toml` found from the working directory, which `ps` and `kill` find the same way.
//! Jobs are registered only through [`Resolver::register`]; no `environments[]` is written.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use candler_core::experiment::inputs::{ResolvedInputs, resolve_inputs};
use candler_core::experiment::paths;
use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::driver::DockerDriver as _;
use candler_core::isolation::image::{DockerImageService, ImageAssets};
use candler_core::isolation::layout;
use candler_core::isolation::model::{BuildRequest, Labels};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::model::JobSpec;
use candler_core::jobs::orchestrate::session::EnvSecrets;
use candler_core::jobs::runtime::detach::{DetachArgs, detach};
use candler_core::jobs::runtime::launch::{mint_job_id, own_identity, spec_digest};
use candler_core::resolve::config::{Config, ConfigEnv};
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::{ResolveError, Resolver};
use candler_core::resolve::steps::{M2bServices, register_m2b_steps};
use candler_core::runner::evidence::catalog::{CatalogCommand, CommandCatalogProvider};
use candler_core::runner::plan::{Force, PlanCtx, PlanMode, experiment_of, plan_runs};
use candler_core::store::Store;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::driver::HarnessDriver as _;
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::{EventPayload, ExperimentFile, JobRecord, JobState};
use candler_schemas::primitives::{FieldPath, RunKey};
use candler_schemas::values::JsonValue;

use super::{CliError, Exit, common};

/// The environment variable that tells the supervisor which store the launcher used. The
/// supervisor is started with a cleared environment, and the launcher found the store from its
/// working directory, so the root travels in this variable rather than being found again.
pub const STORE_ENV: &str = "CANDLER_STORE_ROOT";

/// The variables of the launcher the supervisor is given, besides the secrets the experiment
/// declares and [`STORE_ENV`].
const ENV_ALLOWLIST: [&str; 5] = [
    "PATH",
    "HOME",
    "XDG_STATE_HOME",
    "DOCKER_HOST",
    "DOCKER_CONTEXT",
];

/// The tag the gateway's sidecar image is looked up by. The tag is not a pin: an image another
/// build left under it is the one the jobs use.
const SIDECAR_TAG: &str = "candler-gateway:latest";

/// The time the catalog command gets inside the image.
const CATALOG_TIMEOUT: Duration = Duration::from_secs(60);

/// The program that prints the catalog of an image that offers no MCP server: a snapshot with no
/// server in it, captured now.
const CATALOG_PROGRAM: &str = "printf '{\"schema\":\"tool-catalog-snapshot/1\",\"captured_at\":\"%s\",\"provider\":\"candler-image@1\",\"servers\":[]}' \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"";

/// The arguments of `candler run`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The experiment file.
    pub experiment: PathBuf,
    /// A value that replaces the file's: `PATH=VALUE`, the value read as JSON when it is JSON
    /// and as text otherwise. Repeatable.
    #[arg(long = "set", value_name = "PATH=VALUE", value_parser = parse_field)]
    pub fields: Vec<(FieldPath, String)>,
    /// Accept the differences the plan finds against the stored runs.
    #[arg(long)]
    pub force: bool,
}

/// Reads `PATH=VALUE` from the command line.
fn parse_field(text: &str) -> Result<(FieldPath, String), String> {
    let Some((path, value)) = text.split_once('=') else {
        return Err("expected PATH=VALUE".to_owned()); // BORROW: the parser owns its message
    };
    let path = FieldPath::from_str(path).map_err(|e| e.to_string())?;
    Ok((path, value.to_owned())) // BORROW: the argument owns its value
}

/// The places the sources of the images are.
struct AssetPaths {
    /// The base and env build contexts and the store root.
    images: ImageAssets,
    /// The build context of the gateway's sidecar image.
    gateway: PathBuf,
}

/// The one place the asset paths are named: `assets/docker` of the workspace this binary was
/// built in, found at compile time from this crate's manifest.
fn asset_paths(store_root: &Path) -> AssetPaths {
    let docker = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker"));
    AssetPaths {
        images: ImageAssets {
            base_context: docker.join("base"),
            env_context: docker.join("env"),
            store_root: store_root.to_path_buf(), // BORROW: the assets own their store root
        },
        gateway: docker.join("gateway"),
    }
}

/// An event sink that drops what it is given: the foreground checks belong to no job.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// Why a step of the verb failed, as the error of the verb.
fn failed(error: impl std::error::Error + Send + Sync + 'static) -> CliError {
    CliError::Failed {
        verb: "run",
        source: Box::new(error),
    }
}

/// A refusal of the request.
fn refused(reason: impl Into<String>) -> CliError {
    CliError::Refused {
        reason: reason.into(),
    }
}

/// The resolver with the preflight steps of the image, the harness, the treatment and the
/// catalog registered.
///
/// # Errors
/// [`ResolveError`] when a step name is registered twice or malformed.
pub fn compose_resolver(
    config: Arc<Config>,
    services: &M2bServices,
) -> Result<Resolver, ResolveError> {
    register_m2b_steps(Resolver::new(config), services)
}

/// The value `text` stands for: JSON when it parses as JSON, else the text itself.
fn value_of(text: &str) -> Result<JsonValue, CliError> {
    let json = serde_json::from_str::<serde_json::Value>(text)
        .unwrap_or_else(|_| serde_json::Value::String(text.to_owned())); // BORROW: the value owns its text
    JsonValue::new(json).map_err(|e| refused(e.to_string()))
}

fn json_of<T: serde::Serialize>(value: &T) -> Result<JsonValue, CliError> {
    let json = serde_json::to_value(value).map_err(failed)?;
    JsonValue::new(json).map_err(|e| refused(e.to_string()))
}

fn path_of(text: &str) -> Result<FieldPath, CliError> {
    FieldPath::from_str(text).map_err(|e| refused(e.to_string()))
}

/// What the experiment file and the command line ask for, as the input of a resolution: the
/// file's fixed values, the settings `request.harness` and `request.treatment` it names, and the
/// arguments of `--set` as explicit values. A location setting (a treatment source, the build
/// requirements, lock, Dockerfile and context) is relative to the file's directory when the file
/// writes it, and to the working directory when `--set` gives it.
///
/// # Errors
/// [`CliError::Refused`] when a value cannot be written as a setting.
pub fn request_settings(
    file: &ExperimentFile,
    experiment_dir: &Path,
    fields: &[(FieldPath, String)],
    config: &Config,
) -> Result<ResolveInput, CliError> {
    let mut values: BTreeMap<FieldPath, JsonValue> = file
        .fixed()
        .iter()
        .map(|(path, value)| (path.clone(), value.clone())) // BORROW: the input owns its paths and values
        .collect();
    values.insert(path_of(paths::REQUEST_HARNESS)?, json_of(file.harness())?);
    values.insert(
        path_of(paths::REQUEST_TREATMENT)?,
        json_of(file.treatment())?,
    );
    let file_values = FileValues::new(values.into_iter().collect())
        .relative_to(experiment_dir)
        .map_err(|e| refused(e.to_string()))?;
    let mut input = ResolveInput::default().file(file_values);
    for (path, text) in fields {
        if path.as_str() == paths::REQUEST_TREATMENT_SOURCE {
            // BORROW: .clone() gives the input its own path
            input = input
                .explicit_path(config, path.clone(), Path::new(text))
                .map_err(|e| refused(e.to_string()))?;
        } else {
            input = input.explicit(path.clone(), value_of(text)?); // BORROW: the input owns its path
        }
    }
    Ok(input)
}

/// The services the foreground preflight runs on.
struct Foreground {
    cli: DockerCli,
    resolver: Resolver,
    gateway_context: PathBuf,
}

/// Builds the docker client, the image service and the catalog provider over one supervisor, and
/// composes the resolver on them.
fn foreground(config: Arc<Config>, root: &Path) -> Result<Foreground, CliError> {
    let assets = asset_paths(root);
    let control = layout::control_dir(root);
    std::fs::create_dir_all(&control).map_err(failed)?;
    // TRAIT_OBJECT: the foreground checks belong to no job and report no event
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let supervisor = Arc::new(Supervisor::new(sink));
    let env = ClientEnv::from_process();
    let cli = DockerCli::new(Arc::clone(&supervisor), env.clone(), control).map_err(failed)?; // BORROW: the client owns its env
    let images = DockerImageService::new(
        Arc::clone(&supervisor),
        cli.clone(), // BORROW: the service owns its client
        env.clone(), // BORROW: the service owns its env
        assets.images,
    );
    // BORROW: .to_owned() gives the command its own words
    let argv = vec!["sh".to_owned(), "-c".to_owned(), CATALOG_PROGRAM.to_owned()];
    let catalog = CommandCatalogProvider::new(
        Arc::clone(&supervisor),
        cli.clone(), // BORROW: the provider owns its client
        env,
        root.to_path_buf(), // BORROW: the provider owns its root
        CatalogCommand::new(argv, CATALOG_TIMEOUT),
    );
    let services = M2bServices {
        supervisor,
        images: Arc::new(images),
        catalog: Arc::new(catalog),
        harness_supported: ClaudeCodeDriver::new().map_err(failed)?.supported(),
    };
    let resolver = compose_resolver(config, &services).map_err(failed)?;
    Ok(Foreground {
        cli,
        resolver,
        gateway_context: assets.gateway,
    })
}

/// Makes sure the gateway's sidecar image is there: looks it up by its tag, and builds it from
/// the gateway's assets, through the supervised docker client, only when it is absent.
async fn ensure_sidecar(cli: &DockerCli, context: PathBuf) -> Result<(), CliError> {
    if cli
        .image_inspect(SIDECAR_TAG)
        .await
        .map_err(failed)?
        .is_some()
    {
        return Ok(());
    }
    let request = BuildRequest {
        context,
        tag: SIDECAR_TAG.to_owned(), // BORROW: the request owns its tag
        build_args: Vec::new(),
        labels: Labels::new(),
    };
    cli.build(&request).await.map(|_| ()).map_err(failed)
}

/// The variables the supervisor is started with: the allowlist as the launcher has it, the
/// secrets the experiment declares, and the store root. A secret's value goes only here.
fn supervisor_env(file: &ExperimentFile, root: &Path) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = ENV_ALLOWLIST
        .iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| ((*name).to_owned(), value)) // BORROW: the pair owns its name
        })
        .collect();
    for secret in file.secrets() {
        if let Ok(value) = std::env::var(secret.as_str()) {
            env.push((secret.to_string(), value));
        }
    }
    env.push((STORE_ENV.to_owned(), root.display().to_string())); // BORROW: the pair owns its text
    env
}

/// Reads the experiment file at `path`.
fn read_experiment(path: &Path) -> Result<ExperimentFile, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| refused(format!("cannot read {}: {e}", path.display())))?;
    ExperimentFile::from_toml_str(text.as_str()) // BORROW: .as_str() views the text as a `&str`
        .map_err(|e| refused(format!("{} is not an experiment: {e}", path.display())))
}

/// Resolves, plans and starts the job.
///
/// # Errors
/// [`CliError::Refused`] when the experiment cannot be resolved or planned, [`CliError::Failed`]
/// when docker, the store or the supervisor fails.
pub async fn run(args: Args, store: Option<&Path>) -> Result<Exit, CliError> {
    let Args {
        experiment,
        fields,
        force,
    } = args;
    let cwd = std::env::current_dir().map_err(|e| refused(format!("no working directory: {e}")))?;
    let file = read_experiment(&experiment)?;
    let experiment_dir = cwd
        .join(&experiment)
        .parent()
        .map_or_else(|| cwd.clone(), Path::to_path_buf); // BORROW: the directory is owned
    let root = common::store_root("run", store).await?;
    let store = Store::init(&root).map_err(failed)?;
    let config = Arc::new(Config::load(&cwd, &ConfigEnv::from_process()).map_err(failed)?);
    let declared = resolve_inputs(&store, file.inputs(), experiment_dir.as_path())
        .map_err(|e| refused(e.to_string()))?;
    let input = with_declared_inputs(
        request_settings(&file, experiment_dir.as_path(), &fields, &config)?,
        &declared,
    )?;
    let services = foreground(Arc::clone(&config), &root)?;
    ensure_sidecar(&services.cli, services.gateway_context).await?;
    let resolution = services
        .resolver
        .resolve(input)
        .await
        .map_err(|e| refused(e.to_string()))?;
    require_source(&resolution)?;
    let spec = plan_job(&store, &file, resolution, force, declared)?;
    start(&store, &file, &spec, &root).await
}

/// Refuses a resolution that names no place for the treatment's skills.
fn require_source(resolution: &Resolution) -> Result<(), CliError> {
    let found = resolution
        .settings
        .iter()
        .any(|value| value.path().as_str() == paths::REQUEST_TREATMENT_SOURCE);
    if found {
        Ok(())
    } else {
        Err(refused(
            "the treatment has no source: set `request.treatment_source` in `fixed` or pass \
             --set request.treatment_source=PATH",
        ))
    }
}

/// The input with the agent's declared inputs as the setting `request.inputs`, which the image
/// step merges into the `inputs` root. No agent input sets nothing.
fn with_declared_inputs(
    mut input: ResolveInput,
    declared: &ResolvedInputs,
) -> Result<ResolveInput, CliError> {
    if declared.agent.is_empty() {
        return Ok(input);
    }
    let value = json_of(&declared.agent)?;
    let path = path_of(paths::REQUEST_INPUTS)?;
    match input.file.as_mut() {
        Some(file) => file.values.push((path, value)),
        None => input = input.file(FileValues::new(vec![(path, value)])),
    }
    Ok(input)
}

/// Plans the runs without the experiment lock and builds the job's spec: the agent's input files
/// are mounted into every run and the evaluator-only inputs are listed for the manifest.
fn plan_job(
    store: &Store,
    file: &ExperimentFile,
    resolution: Resolution,
    force: bool,
    declared: ResolvedInputs,
) -> Result<JobSpec, CliError> {
    let experiment = experiment_of(file, &resolution).map_err(failed)?;
    let ctx = PlanCtx {
        store: store.clone(), // BORROW: the context owns its handle on the store
        experiment_lock: store.layout().experiment_lock(&experiment),
        secrets_present: EnvSecrets::new(file.secrets()).present(),
    };
    let mode = if force { Force::Yes } else { Force::No };
    let outcome = plan_runs(&ctx, file, &resolution, mode, PlanMode::Check)
        .map_err(|e| refused(e.to_string()))?;
    Ok(JobSpec {
        experiment: file.clone(), // BORROW: the spec owns its file
        resolution,
        plans: outcome.plans,
        budget: file.budget().clone(), // BORROW: the spec owns its budget
        force,
        inputs: declared.mounts,
        evaluation_inputs: declared.evaluators,
    })
}

/// Writes the job's directory and starts its supervisor, then prints the job's id.
async fn start(
    store: &Store,
    file: &ExperimentFile,
    spec: &JobSpec,
    root: &Path,
) -> Result<Exit, CliError> {
    let bytes = serde_json::to_vec(spec).map_err(failed)?;
    let digest = spec_digest(bytes.as_slice()).map_err(failed)?; // BORROW: explicit .as_slice()
    let id = mint_job_id().map_err(failed)?;
    let record = JobRecord::builder()
        .id(id.clone()) // BORROW: the record owns its id
        .supervisor(own_identity().map_err(failed)?)
        .spec_digest(digest)
        .state(JobState::Running)
        .build()
        .map_err(failed)?;
    let dir = JobDir::create(store.layout(), id.clone(), &record).map_err(failed)?; // BORROW: the dir owns its id
    let args = DetachArgs {
        exe: std::env::current_exe().map_err(failed)?,
        job_dir: dir.path().to_path_buf(), // BORROW: the args own their path
        spec: bytes,
        env: supervisor_env(file, root),
    };
    tokio::task::spawn_blocking(move || detach(args))
        .await
        .map_err(|e| CliError::Runtime {
            detail: e.to_string(),
        })?
        .map_err(failed)?;
    writeln!(std::io::stdout(), "{id}").map_err(failed)?;
    Ok(Exit::Success)
}
