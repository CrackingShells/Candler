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

use super::ps::store_root;
use super::{CliError, Exit};

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
    services: M2bServices,
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

/// A path as the text a setting holds.
fn text_of_path(path: &Path) -> Result<JsonValue, CliError> {
    let text = path
        .to_str()
        .ok_or_else(|| refused("a path is not UTF-8 text"))?;
    JsonValue::new(serde_json::Value::String(text.to_owned())) // BORROW: the value owns its text
        .map_err(|e| refused(e.to_string()))
}

/// What the experiment file and the command line ask for, as the input of a resolution: the
/// file's fixed values, the settings `request.harness` and `request.treatment` it names, and the
/// arguments of `--set` as explicit values. A treatment source is a location: a relative one
/// from the file is taken relative to the file's directory, and one from `--set` relative to the
/// working directory.
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
    let source = path_of(paths::REQUEST_TREATMENT_SOURCE)?;
    let named = values
        .get(&source)
        .and_then(|found| found.as_value().as_str())
        .map(|text| experiment_dir.join(text));
    if let Some(absolute) = named {
        values.insert(source, text_of_path(absolute.as_path())?);
    }
    values.insert(path_of(paths::REQUEST_HARNESS)?, json_of(file.harness())?);
    values.insert(
        path_of(paths::REQUEST_TREATMENT)?,
        json_of(file.treatment())?,
    );
    let mut input = ResolveInput::default().file(FileValues::new(values.into_iter().collect()));
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


/// Runs the verb.
///
/// # Errors
/// [`CliError`] when the experiment cannot be started.
#[allow(clippy::unused_async, reason = "the verb awaits once it starts the job")]
pub async fn run(args: Args) -> Result<Exit, CliError> {
    let _ = args;
    Err(CliError::Unavailable { verb: "run" })
}
