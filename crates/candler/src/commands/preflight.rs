//! The foreground steps `run` takes before it writes a job: the paths of the image sources, the
//! settings an experiment file and `--set` ask for, and the resolver composed on the services of
//! a foreground check. The verbs that resolve an experiment share them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::image::{DockerImageService, ImageAssets};
use candler_core::isolation::layout;
use candler_core::resolve::config::Config;
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::{ResolveError, Resolver};
use candler_core::resolve::steps::env::EnvRefusal;
use candler_core::resolve::steps::ground_truth::{GroundTruth, GroundTruthStep};
use candler_core::resolve::steps::{M2bServices, register_m2b_steps};
use candler_core::runner::evidence::catalog::{CatalogCommand, CommandCatalogProvider};
use candler_core::runner::plan::{PlanError, experiment_of};
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::paths;
use candler_schemas::execution::{
    EventPayload, ExecutionError, ExperimentFile, Level, check_replicates,
};
use candler_schemas::primitives::{FieldPath, RunKey};
use candler_schemas::refusal::pointer_of;
use candler_schemas::values::JsonValue;

use super::CliError;
use super::assets;
use super::refusal::{CliRefusal, FieldLocation, REQUEST_KEYS};
use candler_harness::driver::HarnessDriver;

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
    /// The build context of the fake model's image, which the session controls run against.
    pub(crate) fakemodel: PathBuf,
}

/// The paths of the image sources: the embedded assets, written below the store root when they
/// are not there yet. The base image is built from the directory `base_context` of
/// `assets/docker`.
///
/// # Errors
/// [`CliError::Failed`] when the assets cannot be written.
pub(crate) fn asset_paths(store_root: &Path, base_context: &str) -> Result<AssetPaths, CliError> {
    let docker = assets::materialise(store_root)?.join("docker");
    Ok(AssetPaths {
        images: ImageAssets {
            base_context: docker.join(base_context),
            env_context: docker.join("env"),
            store_root: store_root.to_path_buf(), // BORROW: the assets own their store root
        },
        gateway: docker.join("gateway"),
        fakemodel: docker.join("fakemodel"),
    })
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

/// The resolver with the preflight steps of the image, the harness, the treatment, the catalog
/// and the ground truth registered; the services say where the harness reads its skills.
///
/// # Errors
/// [`ResolveError`] when a step name is registered twice or malformed.
pub fn compose_resolver(
    config: Arc<Config>,
    services: &M2bServices,
    truth: GroundTruth,
) -> Result<Resolver, ResolveError> {
    let step = GroundTruthStep::new(truth, Arc::clone(&services.supervisor), services.skills)?;
    register_m2b_steps(Resolver::new(config), services, step)
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
/// built for an experiment that cannot run. The treatment, `request.treatment`, is the one setting
/// admitted as a factor: its levels are resolved one by one before the plan, so a level without a
/// resolution at this point is not a refusal of the file.
///
/// # Errors
/// [`CliError::Coded`] with [`PlanError::SettingAsFactor`] when a factor varies a setting.
pub(crate) fn check_factors(file: &ExperimentFile) -> Result<(), CliError> {
    // Only a setting factor is refused here; every other error the empty resolution gives, such as
    // a treatment level it does not resolve, is the plan's to report once the levels are resolved.
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
    if candler_core::resolve::layers::is_known_path(path) {
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
                    let treatment_field = factor.path.as_str() == paths::REQUEST_TREATMENT
                        && keys.len() == 1
                        && keys.first().is_some_and(|key| {
                            candler_core::runner::plan::TREATMENT_FIELDS.contains(&key.as_str())
                        });
                    if treatment_field {
                        continue;
                    }
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

/// Refuses a `replicates` value that is not a count: an integer is checked by
/// [`check_replicates`], the validator of the experiment file, and anything else is refused as
/// not positive.
fn check_replicate_count(value: &JsonValue) -> Result<(), CliError> {
    let count = value
        .as_value()
        .as_u64()
        .ok_or(ExecutionError::NotPositive {
            field: "replicates",
        })
        .map_err(CliError::refused_by)?;
    check_replicates(count)
        .map(drop)
        .map_err(CliError::refused_by)
}

/// Refuses the first of `paths` that names `env`, which only the `[env]` table sets.
fn check_env_source<'a>(paths: impl IntoIterator<Item = &'a FieldPath>) -> Result<(), CliError> {
    paths
        .into_iter()
        .find(|path| EnvRefusal::is_env_path(path))
        .map_or(Ok(()), |path| {
            Err(CliError::refused_by(ResolveError::Env(
                EnvRefusal::Source {
                    path: path.to_string(),
                },
            )))
        })
}

/// The input with `level` as the setting `request.treatment`, its `source` relative to the
/// experiment file's directory `dir` as the file writes it.
pub(crate) fn input_for_level(
    input: &ResolveInput,
    level: &serde_json::Value,
    dir: &Path,
) -> Result<ResolveInput, CliError> {
    let path = path_of(paths::REQUEST_TREATMENT)?;
    let given = FileValues::new(vec![(path.clone(), json_of(level)?)]) // BORROW: the values own the path
        .relative_to(dir)
        .map_err(CliError::refused_by)?;
    let mut input = input.clone(); // BORROW: each level resolves its own copy
    if let (Some(file), Some((_, value))) = (input.file.as_mut(), given.values.into_iter().next()) {
        file.values.retain(|(held, _)| *held != path);
        file.values.push((path, value));
    }
    Ok(input)
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
    check_env_source(
        file.fixed()
            .keys()
            .chain(file.factors().iter().map(|factor| &factor.path))
            .chain(fields.iter().map(|(path, _)| path)),
    )?;
    let mut values: BTreeMap<FieldPath, JsonValue> = file
        .fixed()
        .iter()
        .map(|(path, value)| (path.clone(), value.clone())) // BORROW: the input owns its paths and values
        .collect();
    if !file.env().is_empty() {
        values.insert(path_of(paths::ENV)?, json_of(file.env())?);
    }
    values.insert(path_of(paths::REQUEST_HARNESS)?, json_of(file.harness())?);
    if let Some(treatment) = file.treatment() {
        values.insert(path_of(paths::REQUEST_TREATMENT)?, json_of(treatment)?);
    }
    if let Some(count) = values.get(&path_of("replicates")?) {
        check_replicate_count(count)?;
    }
    let file_values = FileValues::new(values.into_iter().collect())
        .relative_to(experiment_dir)
        .map_err(CliError::refused_by)?;
    let mut input = ResolveInput::new(file.secrets().iter().map(ToString::to_string).collect())
        .file(file_values);
    check_file_paths(file)?;
    check_set_paths(fields)?;
    for (path, text) in fields {
        let mut value = value_of(text)?;
        if path.as_str() == "replicates" {
            check_replicate_count(&value)?;
        }
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

/// The name of the harness the request asks for: the experiment file's, as `--set
/// request.harness` replaces it (an object with a `name`, or the name itself).
///
/// # Errors
/// [`CliError::Refused`] when the request names no harness or a name that is not text.
pub(crate) fn requested_harness(input: &ResolveInput) -> Result<String, CliError> {
    let whole = path_of(paths::REQUEST_HARNESS)?;
    let from_value = |value: &JsonValue| match value.as_value() {
        serde_json::Value::String(name) => Some(name.clone()), // BORROW: the caller owns the name
        serde_json::Value::Object(map) => map
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned), // BORROW: the caller owns the name
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::Array(_) => None,
    };
    let mut name = input
        .file
        .as_ref()
        .and_then(|file| file.values.iter().rev().find(|(path, _)| *path == whole))
        .and_then(|(_, value)| from_value(value));
    for (path, value) in &input.explicit {
        if *path == whole {
            name = from_value(value);
        }
    }
    name.ok_or_else(|| refused("`request.harness` names no harness"))
}

/// Builds the docker client, the image service and the catalog provider over one supervisor, and
/// composes the resolver on them. The harness is the registry's `driver`: it reads the
/// image's version and says which versions are supported, and its base context builds the base
/// image.
///
/// # Errors
/// [`CliError`] when the assets cannot be written, the control directory cannot be made, or the
/// docker client cannot be built.
pub(crate) fn foreground(
    config: Arc<Config>,
    root: &Path,
    driver: &Arc<dyn HarnessDriver>,
    truth: GroundTruth,
) -> Result<Foreground, CliError> {
    let assets = asset_paths(root, driver.base_image().context)?;
    let control = layout::control_dir(root);
    std::fs::create_dir_all(&control).map_err(failed)?;
    // TRAIT_OBJECT: the foreground checks belong to no job and report no event
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let supervisor = Arc::new(Supervisor::new(sink));
    let env = ClientEnv::from_process();
    let cli = DockerCli::new(Arc::clone(&supervisor), env.clone(), control).map_err(failed)?; // BORROW: the client owns its env
    let driver = Arc::clone(driver);
    let images = DockerImageService::new(
        Arc::clone(&supervisor),
        cli.clone(), // BORROW: the service owns its client
        env.clone(), // BORROW: the service owns its env
        assets.images,
        Arc::clone(&driver),
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
        harness_supported: driver.supported(),
        session_capable: driver.capabilities().session,
        skills: driver.skills(),
    };
    let resolver = compose_resolver(config, &services, truth).map_err(failed)?;
    Ok(Foreground {
        cli,
        resolver,
        gateway_context: assets.gateway,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn file(fixed: &str) -> ExperimentFile {
        let text = format!(
            "schema = \"experiment/1\"\nreplicates = 3\n{fixed}\n[budget.per_run]\nwall_s = 5\n\n[harness]\nname = \"h\"\n\n[treatment]\nname = \"t\"\n"
        );
        ExperimentFile::from_toml_str(&text).expect("an experiment file")
    }

    fn settings(file: &ExperimentFile, set: &[&str]) -> Result<ResolveInput, CliError> {
        let fields: Vec<(FieldPath, String)> = set
            .iter()
            .map(|text| {
                let (path, value) = text.split_once('=').expect("key=value");
                (path.parse().expect("a field path"), value.to_owned())
            })
            .collect();
        request_settings(file, Path::new("/work"), &fields, Path::new("/work"))
    }

    /// Guards: I12 — a replicate count of `--set` or of `[fixed]` goes through the validator of
    /// the experiment file: 0 and a value that is no integer are refused, a count is kept.
    /// Oracle: the values written below.
    /// Bite: skip the check for `--set` (the first rows go red), or for `[fixed]` (the last).
    #[test]
    fn a_replicates_value_that_is_not_a_count_is_refused() {
        let plain = file("");
        assert!(settings(&plain, &["replicates=3"]).is_ok());
        for bad in ["replicates=0", "replicates=abc", "replicates=2.5"] {
            assert!(settings(&plain, &[bad]).is_err(), "{bad}");
        }
        assert!(settings(&file("\n[fixed]\nreplicates = 0\n"), &[]).is_err());
    }

    /// Guards: an experiment file without an `env` table gives the resolution no `env` value, so
    /// it resolves, and hashes, as before; a file with one gives exactly one `env` value, the
    /// table as written.
    /// Oracle: the paths and the table are written by hand.
    /// Bite: an empty `env` value written for a file that has no table.
    #[test]
    fn env_is_a_value_only_when_the_file_has_a_table() {
        let paths_of = |input: &ResolveInput| -> Vec<String> {
            input
                .file
                .iter()
                .flat_map(|values| values.values.iter())
                .map(|(path, _)| path.to_string())
                .collect()
        };
        let plain = settings(&file(""), &[]).expect("an input");
        assert!(!paths_of(&plain).iter().any(|path| path == "env"));
        let with = settings(&file("[env]\nTOOL_MODE = \"fast\"\n"), &[]).expect("an input");
        assert_eq!(
            paths_of(&with).iter().filter(|path| *path == "env").count(),
            1
        );
        let table = with
            .file
            .iter()
            .flat_map(|values| values.values.iter())
            .find(|(path, _)| path.as_str() == "env")
            .map(|(_, value)| value.as_value().clone());
        assert_eq!(table, Some(serde_json::json!({"TOOL_MODE": "fast"})));
    }

    /// Guards: the `[env]` table is the only source of `env`: a `[fixed]` key, a factor and a
    /// `--set` that name the path `env` are each refused, and the refusal names `env`.
    /// Oracle: the three spellings are written by hand; the word `env` is the key's own path.
    /// Bite: dropping any one of the three checks, which lets that channel write the table.
    #[test]
    fn env_has_one_source() {
        let fixed = file("[fixed]\nenv = \"x\"\n");
        let factor = file("[[factors]]\npath = \"env\"\nlevels = [\"a\", \"b\"]\n");
        let plain = file("");
        let cases = [
            ("fixed", settings(&fixed, &[])),
            ("factor", settings(&factor, &[])),
            ("set", settings(&plain, &["env=x"])),
            (
                "dotted",
                settings(
                    &file(
                        "[fixed.env]
x = \"y\"
",
                    ),
                    &[],
                ),
            ),
        ];
        for (channel, outcome) in cases {
            let error = outcome.expect_err(channel);
            assert!(
                error.to_string().contains("outside the `[env]` table"),
                "{channel}: {error}"
            );
        }
    }
}
