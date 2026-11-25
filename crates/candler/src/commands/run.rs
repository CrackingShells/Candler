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
//! Jobs are registered only through [`candler_core::resolve::preflight::Resolver::register`]; no `environments[]` is written.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use candler_core::experiment::inputs::{ResolvedInputs, resolve_input_groups};
use candler_core::isolation::docker::DockerCli;
use candler_core::isolation::driver::DockerDriver as _;
use candler_core::isolation::image::sidecar_tag;
use candler_core::isolation::model::{BuildRequest, Labels};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::model::JobSpec;
use candler_core::jobs::orchestrate::session::EnvSecrets;
use candler_core::jobs::runtime::detach::{DetachArgs, detach};
use candler_core::jobs::runtime::launch::{mint_job_id, own_identity, spec_digest};
use candler_core::resolve::config::{Config, ConfigEnv};
use candler_core::resolve::layers::{FileValues, Layered, Resolution, ResolveInput};
use candler_core::resolve::steps::ground_truth::GroundTruth;
use candler_core::resolve::steps::plugin_environment::Fragment;
use candler_core::runner::plan::{
    Force, PlanCtx, PlanMode, experiment_of, plan_runs, refuse_instruction_collision,
    treatment_levels,
};
use candler_core::store::Store;
use candler_harness::driver::HarnessDriver;
use candler_schemas::execution::paths;
use candler_schemas::execution::{ExperimentFile, JobRecord, JobState};
use candler_schemas::primitives::FieldPath;

use super::preflight::{
    check_factors, failed, foreground, input_for_level, json_of, path_of, refuse_plan, refused,
    request_settings, requested_harness,
};
use super::refusal::CliRefusal;
use super::{CliError, Exit, common};
use crate::drivers::Registry;

/// The environment variable that tells the supervisor which store the launcher used. The
/// supervisor is started with a cleared environment, and the launcher found the store from its
/// working directory, so the root travels in this variable rather than being found again.
pub const STORE_ENV: &str = "CANDLER_STORE_ROOT";

/// The variables of the launcher the supervisor is given, besides the secrets the experiment
/// declares and [`STORE_ENV`].
const ENV_ALLOWLIST: [&str; 6] = [
    "PATH",
    "HOME",
    "XDG_CONFIG_HOME",
    "XDG_STATE_HOME",
    "DOCKER_HOST",
    "DOCKER_CONTEXT",
];

/// The arguments of `candler run`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The experiment file.
    pub experiment: PathBuf,
    /// A value that replaces the file's: `PATH=VALUE`, the value read as JSON when it is JSON
    /// and as text otherwise. Repeatable. cannot be derived: an override is what the caller types.
    #[arg(long = "set", value_name = "PATH=VALUE", value_parser = parse_field)]
    pub fields: Vec<(FieldPath, String)>,
    /// Accept the differences the plan finds against the stored runs. cannot be derived: whether
    /// to accept differences is the caller's decision, taken after the plan has shown them.
    #[arg(long)]
    pub force: bool,
    #[command(flatten)]
    pub output: super::OutputArgs,
}

/// Reads `PATH=VALUE` from the command line.
fn parse_field(text: &str) -> Result<(FieldPath, String), String> {
    let Some((path, value)) = text.split_once('=') else {
        return Err("expected PATH=VALUE".to_owned()); // BORROW: the parser owns its message
    };
    let path = FieldPath::from_str(path).map_err(|e| e.to_string())?;
    Ok((path, value.to_owned())) // BORROW: the argument owns its value
}

/// Builds the gateway's sidecar image from the gateway's assets, through the supervised docker
/// client, under the tag of the assets' digest, unless docker already holds that tag: an image
/// built from other sources is never the one a job uses.
async fn ensure_sidecar(cli: &DockerCli, context: PathBuf) -> Result<(), CliError> {
    let tag = sidecar_tag(&context).map_err(failed)?;
    if cli.image_inspect(&tag).await.map_err(failed)?.is_some() {
        return Ok(());
    }
    let request = BuildRequest {
        context,
        tag,
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
    let text = std::fs::read_to_string(path).map_err(|e| {
        CliError::refused_by(CliRefusal::ExperimentUnreadable {
            path: path.display().to_string(),
            detail: e.to_string(),
        })
        .with_file(path)
    })?;
    ExperimentFile::from_toml_str(text.as_str()) // BORROW: .as_str() views the text as a `&str`
        .map_err(|e| {
            CliError::refused_by(e)
                .led_by(format!("{} is not an experiment: ", path.display()))
                .with_file(path)
        })
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
        ..
    } = args;
    let cwd = std::env::current_dir().map_err(|e| refused(format!("no working directory: {e}")))?;
    let file = read_experiment(&experiment)?;
    check_factors(&file)?;
    let experiment_dir = cwd
        .join(&experiment)
        .parent()
        .map_or_else(|| cwd.clone(), Path::to_path_buf); // BORROW: the directory is owned
    let root = common::store_root("run", store).await?;
    let store = Store::init(&root).map_err(failed)?;
    let config = Arc::new(Config::load(&cwd, &ConfigEnv::from_process()).map_err(failed)?);
    let settings = request_settings(&file, experiment_dir.as_path(), &fields, &cwd)?;
    let fragment = selected_fragment(&config, &settings)?;
    let declared = declare_inputs(&store, &file, experiment_dir.as_path(), fragment.as_ref())?;
    let input = with_declared_inputs(settings, &declared)?;
    let driver = Registry::builtin().driver(&requested_harness(&input)?)?;
    let own_kinds: Vec<String> = file.inputs().iter().map(|i| i.kind().to_string()).collect();
    let truth = GroundTruth::from_inputs(declared.files.as_slice(), own_kinds.as_slice()); // BORROW: explicit .as_slice()
    let services = foreground(Arc::clone(&config), &root, &driver, truth)?;
    ensure_sidecar(&services.cli, services.gateway_context).await?;
    let declared_levels = treatment_levels(&file);
    let mut resolutions = Vec::new();
    if declared_levels.is_empty() {
        resolutions.push(
            services
                .resolver
                .resolve(input)
                .await
                .map_err(CliError::refused_by)?,
        );
    } else {
        for level in declared_levels {
            let given = input_for_level(&input, &level, experiment_dir.as_path())?;
            resolutions.push(
                services
                    .resolver
                    .resolve(given)
                    .await
                    .map_err(CliError::refused_by)?,
            );
        }
    }
    let spec = plan_job(&store, &file, resolutions, force, declared, &driver)?;
    start(&store, &file, &spec, &root).await
}

/// The plug-in environment the request selects, read before the inputs are resolved. A selection
/// that cannot be read is left to the resolver's plug-in environment step, which refuses it with
/// its code before any job is written.
fn selected_fragment(config: &Config, input: &ResolveInput) -> Result<Option<Fragment>, CliError> {
    let layered = Layered::new(config, input.clone()).map_err(failed)?; // BORROW: the layering owns its input
    let resolved = layered.resolve();
    Ok(Fragment::from_values(
        resolved.values.as_slice(),   // BORROW: explicit .as_slice()
        resolved.settings.as_slice(), // BORROW: explicit .as_slice()
    )
    .ok() // EXPLICIT: an unreadable selection is refused by the resolver's plug-in environment step
    .flatten())
}

/// The inputs of the experiment, and those of the plug-in environment it selects, which resolve
/// against the plug-in's directory.
fn declare_inputs(
    store: &Store,
    file: &ExperimentFile,
    experiment_dir: &Path,
    fragment: Option<&Fragment>,
) -> Result<ResolvedInputs, CliError> {
    let own = [(file.inputs(), experiment_dir)];
    let plugin = fragment.map(|held| (held.inputs.as_slice(), held.dir.as_path())); // BORROW: explicit views
    let groups: Vec<(&[_], &Path)> = own.into_iter().chain(plugin).collect();
    resolve_input_groups(store, groups.as_slice()) // BORROW: explicit .as_slice()
        .map_err(CliError::refused_by)
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
    resolutions: Vec<Resolution>,
    force: bool,
    declared: ResolvedInputs,
    // TRAIT_OBJECT: the registry selects the driver at runtime
    driver: &Arc<dyn HarnessDriver>,
) -> Result<JobSpec, CliError> {
    let mut levels = resolutions;
    let resolution = levels
        .first()
        .cloned() // BORROW: the spec owns its primary level
        .ok_or_else(|| refused(String::from("the experiment has no resolution")))?;
    let experiment = experiment_of(file, levels.as_slice()).map_err(|e| refuse_plan(e, file))?; // BORROW: explicit .as_slice()
    let ctx = PlanCtx {
        store: store.clone(), // BORROW: the context owns its handle on the store
        experiment_lock: store.layout().experiment_lock(&experiment),
        secrets_present: EnvSecrets::new(file.secrets()).present(),
        driver: Some(Arc::clone(driver)), // BORROW: the context shares the driver the collision check reads
    };
    let mode = if force { Force::Yes } else { Force::No };
    let outcome = plan_runs(&ctx, file, levels.as_slice(), mode, PlanMode::Check) // BORROW: explicit .as_slice()
        .map_err(|e| refuse_plan(e, file))?;
    refuse_instruction_collision(driver.as_ref(), outcome.fresh.values(), &declared.targets)
        .map_err(CliError::refused_by)?;
    if levels.len() < 2 {
        levels.clear();
    }
    Ok(JobSpec {
        experiment: file.clone(), // BORROW: the spec owns its file
        resolution,
        levels,
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

#[cfg(test)]
mod allowlist_tests {
    use super::ENV_ALLOWLIST;

    /// Guards: the supervisor reads the machine configuration (the model upstream's CA) from the
    /// same directory as the launcher, so the allowlist carries the variable that names it.
    ///
    /// Oracle: the variable `ConfigEnv::from_process` reads for the configuration directory.
    ///
    /// Bite: `XDG_CONFIG_HOME` removed from the allowlist (red here).
    #[test]
    fn the_supervisor_is_given_the_launchers_config_home() {
        assert!(
            ENV_ALLOWLIST.contains(&"XDG_CONFIG_HOME"),
            "the supervisor would read another configuration than the launcher's"
        );
    }
}
