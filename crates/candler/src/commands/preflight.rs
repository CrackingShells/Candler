//! The foreground steps `run` takes before it writes a job: the paths of the image sources, the
//! settings an experiment file and `--set` ask for, and the resolver composed on the services of
//! a foreground check. The verbs that resolve an experiment share them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use candler_core::experiment::paths;
use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::image::{DockerImageService, ImageAssets};
use candler_core::isolation::layout;
use candler_core::resolve::config::Config;
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::{ResolveError, Resolver};
use candler_core::resolve::steps::{M2bServices, register_m2b_steps};
use candler_core::runner::evidence::catalog::{CatalogCommand, CommandCatalogProvider};
use candler_core::runner::plan::{PlanError, experiment_of};
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::driver::HarnessDriver as _;
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::{EventPayload, ExperimentFile, Level};
use candler_schemas::primitives::{FieldPath, RunKey};
use candler_schemas::refusal::pointer_of;
use candler_schemas::values::JsonValue;

use super::CliError;
use super::refusal::{CliRefusal, FieldLocation, REQUEST_KEYS};

/// The path of the setting `[treatment] source` replaces; `--set` refuses it by name.
const RETIRED_TREATMENT_SOURCE: &str = concat!("request.", "treatment_source");

/// The time the catalog command gets inside the image.
const CATALOG_TIMEOUT: Duration = Duration::from_secs(60);

/// The program that prints the catalog of an image that offers no MCP server: a snapshot with no
/// server in it, captured now.
const CATALOG_PROGRAM: &str = "printf '{\"schema\":\"tool-catalog-snapshot/1\",\"captured_at\":\"%s\",\"provider\":\"candler-image@1\",\"servers\":[]}' \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"";

/// The places the sources of the images are.
pub(crate) struct AssetPaths {
    /// The base and env build contexts and the store root.
    pub(crate) images: ImageAssets,
    /// The build context of the gateway's sidecar image.
    pub(crate) gateway: PathBuf,
}

/// The one place the asset paths are named: `assets/docker` of the workspace this binary was
/// built in, found at compile time from this crate's manifest.
pub(crate) fn asset_paths(store_root: &Path) -> AssetPaths {
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
pub(crate) fn failed(error: impl std::error::Error + Send + Sync + 'static) -> CliError {
    CliError::Failed {
        verb: "run",
        source: Box::new(error),
    }
}

/// A refusal of the request.
pub(crate) fn refused(reason: impl Into<String>) -> CliError {
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
    JsonValue::new(json).map_err(CliError::refused_by)
}

pub(crate) fn json_of<T: serde::Serialize>(value: &T) -> Result<JsonValue, CliError> {
    let json = serde_json::to_value(value).map_err(failed)?;
    JsonValue::new(json).map_err(CliError::refused_by)
}

pub(crate) fn path_of(text: &str) -> Result<FieldPath, CliError> {
    FieldPath::from_str(text).map_err(CliError::refused_by)
}

/// The refusal of a plan, placed where the experiment file writes the fault when it can be told:
/// a setting a factor varies is placed at the `path` of that factor.
pub(crate) fn refuse_plan(error: PlanError, file: &ExperimentFile) -> CliError {
    let pointer = if let PlanError::SettingAsFactor { path } = &error {
        factor_pointer(file, path.as_str())
    } else {
        None
    };
    let refused = CliError::refused_by(error);
    match pointer {
        Some(pointer) => refused.with_pointer(pointer),
        None => refused,
    }
}

/// The pointer to the `path` of the factor that holds the setting `setting`: the factor whose path
/// is the setting's or one of its ancestors.
fn factor_pointer(file: &ExperimentFile, setting: &str) -> Option<String> {
    let setting = setting.parse::<FieldPath>().ok()?;
    let index = file
        .factors()
        .iter()
        .position(|factor| setting.starts_with(&factor.path))?;
    Some(pointer_of(["factors", index.to_string().as_str(), "path"]))
}

/// Refuses an experiment that has a factor over a setting, before anything is resolved: the same
/// refusal the plan makes, reached without the resolution it would need, so that nothing has been
/// built for an experiment that cannot run.
///
/// # Errors
/// [`CliError::Coded`] with [`PlanError::SettingAsFactor`] when a factor varies a setting.
pub(crate) fn check_factors(file: &ExperimentFile) -> Result<(), CliError> {
    if let Err(error @ PlanError::SettingAsFactor { .. }) =
        experiment_of(file, &Resolution::default())
    {
        return Err(refuse_plan(error, file));
    }
    Ok(())
}

/// Refuses a `--set` path under `request` that is not one of the request settings, or below one.
fn check_request_key(path: &FieldPath) -> Result<(), CliError> {
    let mut segments = path.segments();
    let under_request = segments.next() == Some("request") && segments.next().is_some();
    let known = REQUEST_KEYS.iter().any(|key| {
        key.parse::<FieldPath>()
            .is_ok_and(|known| path.starts_with(&known))
    });
    if under_request && !known {
        return Err(CliError::refused_by(CliRefusal::UnknownRequestKey {
            path: path.to_string(),
        }));
    }
    Ok(())
}

/// Refuses a path that no path class of the surface knows, naming it and where it was written.
fn check_known(path: &FieldPath, location: FieldLocation) -> Result<(), CliError> {
    if paths::is_known_path(path) {
        return Ok(());
    }
    Err(CliError::refused_by(CliRefusal::UnknownFieldPath {
        path: path.to_string(),
        location,
    }))
}

/// The leaves of a table level, each as the keys that lead to it from the table: a leaf is any
/// value that is not a table.
fn leaves(
    table: &serde_json::Map<String, serde_json::Value>,
    keys: &mut Vec<String>,
    out: &mut Vec<Vec<String>>,
) {
    for (key, value) in table {
        keys.push(key.clone()); // BORROW: the walk owns the keys it has passed
        if let serde_json::Value::Object(inner) = value {
            leaves(inner, keys, out);
        } else {
            out.push(keys.clone()); // BORROW: the list owns its keys
        }
        keys.pop();
    }
}

/// Refuses an experiment file that writes a path no path class knows: a key of `[fixed]`, the
/// `path` of a factor, or a leaf of a table level.
fn check_file_paths(file: &ExperimentFile) -> Result<(), CliError> {
    for path in file.fixed().keys() {
        check_known(path, FieldLocation::Fixed)?;
    }
    for (index, factor) in file.factors().iter().enumerate() {
        for (level_index, level) in factor.levels.iter().enumerate() {
            if let Level::Fill(table) = level {
                let mut found = Vec::new();
                leaves(table, &mut Vec::new(), &mut found);
                for keys in found {
                    let text = format!("{}.{}", factor.path, keys.join("."));
                    if let Ok(path) = text.parse::<FieldPath>() {
                        let location = FieldLocation::Factor {
                            index,
                            leaf: Some((level_index, keys)),
                        };
                        check_known(&path, location)?;
                    }
                }
            } else {
                check_known(&factor.path, FieldLocation::Factor { index, leaf: None })?;
            }
        }
    }
    Ok(())
}

/// Refuses the `--set` paths that name the retired setting, a key under `request` that no
/// request setting has, or a path no path class knows.
fn check_set_paths(fields: &[(FieldPath, String)]) -> Result<(), CliError> {
    for (path, _) in fields {
        if path.as_str() == RETIRED_TREATMENT_SOURCE {
            return Err(CliError::refused_by(CliRefusal::RetiredSetting {
                path: RETIRED_TREATMENT_SOURCE.to_owned(), // BORROW: the refusal owns its path
            }));
        }
        check_request_key(path)?;
        check_known(path, FieldLocation::Set)?;
    }
    Ok(())
}

/// What the experiment file and the command line ask for, as the input of a resolution: the
/// file's fixed values, the settings `request.harness` and `request.treatment` it names, and the
/// arguments of `--set` as explicit values. A location setting (the treatment's source, the build
/// requirements, lock, Dockerfile and context) is relative to the file's directory when the file
/// writes it, and to the working directory `cwd` when `--set` gives it; so is the `source` of a
/// treatment that `--set request.treatment` gives.
///
/// # Errors
/// [`CliError::Coded`] when a value cannot be written as a setting, when `--set` names the
/// retired setting of the treatment's checkout, which `[treatment] source` replaces, or when it
/// names a key under `request` that no request setting has.
pub fn request_settings(
    file: &ExperimentFile,
    experiment_dir: &Path,
    fields: &[(FieldPath, String)],
    cwd: &Path,
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
        .map_err(CliError::refused_by)?;
    let mut input = ResolveInput::default().file(file_values);
    check_file_paths(file)?;
    check_set_paths(fields)?;
    for (path, text) in fields {
        let mut value = value_of(text)?;
        if path.as_str() == paths::REQUEST_TREATMENT {
            // BORROW: .clone() gives the file values their own path
            let given = FileValues::new(vec![(path.clone(), value.clone())])
                .relative_to(cwd)
                .map_err(CliError::refused_by)?;
            if let Some((_, resolved)) = given.values.into_iter().next() {
                value = resolved;
            }
        }
        input = input.explicit(path.clone(), value); // BORROW: the input owns its path
    }
    Ok(input)
}

/// The services the foreground preflight runs on.
pub(crate) struct Foreground {
    pub(crate) cli: DockerCli,
    pub(crate) resolver: Resolver,
    pub(crate) gateway_context: PathBuf,
}

/// Builds the docker client, the image service and the catalog provider over one supervisor, and
/// composes the resolver on them.
pub(crate) fn foreground(config: Arc<Config>, root: &Path) -> Result<Foreground, CliError> {
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
