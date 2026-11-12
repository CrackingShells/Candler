//! Values from six layers, resolved to one value per path with the source it came from.
//!
//! The order, highest first: an explicit argument, the experiment or suite file, the project
//! configuration, the user configuration, a value a preflight step derived, and the core default.
//! Within one layer a later entry replaces an earlier one for the same path; among derived values,
//! the step that ran last wins. A path that is an operational setting resolves into
//! [`Resolution::settings`]; every other path resolves into [`Resolution::values`]. [`is_setting`]
//! decides, by path alone, which paths are settings.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use candler_schemas::primitives::{FieldPath, NameToken};
use candler_schemas::values::{JsonValue, ResolvedValue, SourceEntry, ValueSource};
use serde_json::Value;

use super::config::Config;
use super::preflight::{ResolveError, StepReport};

/// The path of the store root.
const STORE_ROOT: &str = "store.root";
/// The path of the plug-in directories.
const PLUGINS_DIRS: &str = "plugins.dirs";
/// The path of the slot count.
const SLOTS_COUNT: &str = "slots.count";
/// The path of the evaluator slot count.
const SLOTS_EVALUATORS: &str = crate::experiment::paths::SLOTS_EVALUATORS;

/// The setting that names the harness a request asks for.
const REQUEST_HARNESS: &str = "request.harness";
/// The setting that names the treatment a request asks for.
const REQUEST_TREATMENT: &str = "request.treatment";
/// The setting that carries the agent's declared inputs as `{kind, digest}`.
const REQUEST_INPUTS: &str = "request.inputs";
/// The setting that names the Dockerfile of the experiment's image layer.
const ISOLATION_BUILD_DOCKERFILE: &str = "isolation.build.dockerfile";
/// The setting that names the build context directory.
const ISOLATION_BUILD_CONTEXT: &str = "isolation.build.context";
/// The setting that names a ready env lock file.
const ISOLATION_BUILD_LOCK: &str = "isolation.build.lock";
/// The setting that lists the requirement sources of the env lock.
const ISOLATION_BUILD_REQUIREMENTS: &str = "isolation.build.requirements";
/// The setting that lists the import packages each run records from its env image.
const ISOLATION_RECORD_PACKAGES: &str = "isolation.record_packages";
/// The prefix of a requirement source that is read from the treatment's repository, not a path.
const TREATMENT_SOURCE_PREFIX: &str = "treatment:";
/// The setting that names the secret the gateway presents to the model endpoint.
const MODEL_TOKEN_SECRET: &str = "model.token_secret";

/// Reports whether `path` is an operational setting, by its path alone: a configuration key
/// (`store.root`, `plugins.dirs`, `slots.count`, `slots.evaluators`), a job cap (every `budget.*` path except
/// `budget.per_run.*`), `replicates`, a request key (`request.harness`, `request.treatment`,
/// `request.inputs`), the gateway's secret (`model.token_secret`), the packages a run records
/// (`isolation.record_packages`) or an image build request (every `isolation.build.*` path). A
/// setting never reaches a cell digest. The paths an
/// explicit argument marks are settings too; [`Layered::is_setting`] adds them.
///
/// The match follows path segments, so `isolation.image` and `isolation.builder` are not
/// settings and `model.id` is not one either.
#[must_use]
pub fn is_setting(path: &FieldPath) -> bool {
    let mut segments = path.segments();
    match (segments.next(), segments.next()) {
        (Some("budget"), Some("per_run")) => false,
        (Some("budget" | "replicates"), _) => true,
        (Some("isolation"), Some("build")) => segments.next().is_some(),
        // BORROW: the path's text, compared with the setting keys
        _ => matches!(
            path.as_str(),
            STORE_ROOT
                | PLUGINS_DIRS
                | SLOTS_COUNT
                | SLOTS_EVALUATORS
                | REQUEST_HARNESS
                | REQUEST_TREATMENT
                | ISOLATION_RECORD_PACKAGES
                | REQUEST_INPUTS
                | MODEL_TOKEN_SECRET
        ),
    }
}

/// The values an experiment file sets.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct FileValues {
    /// The values, by path.
    pub values: Vec<(FieldPath, JsonValue)>,
}

impl FileValues {
    /// The values of a file.
    #[must_use]
    pub const fn new(values: Vec<(FieldPath, JsonValue)>) -> Self {
        Self { values }
    }

    /// Makes every path the file writes absolute against `dir`, the file's own directory: a
    /// location setting that holds a string, or a list of strings (a requirement source that
    /// starts with `treatment:` is not a path and stays as written), and the `source` of the
    /// treatment `request.treatment` holds. A path that is already
    /// absolute stays as it is, and a value of another shape is left for the step that reads it
    /// to refuse.
    ///
    /// # Errors
    /// Returns [`ResolveError::NonUtf8Path`] if a resulting path is not UTF-8.
    pub fn relative_to(mut self, dir: &Path) -> Result<Self, ResolveError> {
        // EXPLICIT: the loop rewrites each location value of `self.values` in place; .map() would hide the update
        for (path, value) in &mut self.values {
            if path.as_str() == REQUEST_TREATMENT {
                *value = JsonValue::new(treatment_source_under(dir, value.as_value())?)?;
                continue;
            }
            if !is_location(path) {
                continue;
            }
            let joined = match value.as_value() {
                Value::String(text) => Value::String(join_text(dir, text)?),
                Value::Array(items) => Value::Array(
                    items
                        .iter()
                        .map(|item| match item {
                            Value::String(text) if !text.starts_with(TREATMENT_SOURCE_PREFIX) => {
                                join_text(dir, text).map(Value::String)
                            }
                            // BORROW: `.clone()` keeps an entry that is not a path as written
                            other @ (Value::String(_)
                            | Value::Null
                            | Value::Bool(_)
                            | Value::Number(_)
                            | Value::Array(_)
                            | Value::Object(_)) => Ok(other.clone()),
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                // BORROW: `.clone()` keeps a value of another shape as written
                other @ (Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_)) => {
                    other.clone()
                }
            };
            *value = JsonValue::new(joined)?;
        }
        Ok(self)
    }
}

/// What the caller supplies to a resolution, besides the configuration.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct ResolveInput {
    /// Values given as arguments, by path.
    pub explicit: Vec<(FieldPath, JsonValue)>,
    /// The paths among `explicit` that are operational settings, besides those [`is_setting`]
    /// names.
    pub explicit_settings: Vec<FieldPath>,
    /// The experiment file's values.
    pub file: Option<FileValues>,
    /// The core's defaults, by path; they replace the built-in `store.root` default.
    pub core_defaults: Vec<(FieldPath, JsonValue)>,
}

impl ResolveInput {
    /// Adds an explicit argument that is a value.
    #[must_use]
    pub fn explicit(mut self, path: FieldPath, value: JsonValue) -> Self {
        self.explicit.push((path, value));
        self
    }

    /// Adds an explicit argument that is a path. A relative `value` is made absolute against the
    /// working directory of `config`. A path is a location, so it resolves into
    /// [`Resolution::settings`].
    ///
    /// # Errors
    /// Returns [`ResolveError::NonUtf8Path`] if the resulting path is not UTF-8.
    pub fn explicit_path<'a>(
        mut self,
        config: &'a Config,
        path: FieldPath,
        value: &'a Path,
    ) -> Result<Self, ResolveError> {
        let absolute = config.cwd.join(value);
        // BORROW: `.clone()` keeps the path for the explicit entry below
        self.explicit_settings.push(path.clone());
        // BORROW: `.as_path()` views the owned absolute path as a `&Path`
        self.explicit.push((path, path_json(absolute.as_path())?));
        Ok(self)
    }

    /// Sets the values of an experiment file.
    #[must_use]
    pub fn file(mut self, file: FileValues) -> Self {
        self.file = Some(file);
        self
    }
}

/// The result of a resolution.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct Resolution {
    /// The resolved values, sorted by path. They can reach a cell digest.
    pub values: Vec<ResolvedValue>,
    /// The resolved operational settings, sorted by path and disjoint from `values`. They never
    /// reach a cell digest.
    pub settings: Vec<ResolvedValue>,
    /// The source of every path of `values` and `settings`, sorted by path.
    pub sources: Vec<SourceEntry>,
    /// The report of every preflight step that ran, in registration order.
    pub preflight: Vec<StepReport>,
}

/// The six layers of values, each holding the entries that set a path.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct Layered {
    /// Explicit arguments.
    pub explicit: Vec<(FieldPath, JsonValue)>,
    /// The experiment file's values.
    pub file: Vec<(FieldPath, JsonValue)>,
    /// Values from the project configuration.
    pub project: Vec<(FieldPath, JsonValue)>,
    /// Values from the user configuration.
    pub user: Vec<(FieldPath, JsonValue)>,
    /// Values preflight steps derived, with the step, in the order the steps ran.
    pub derived: Vec<(NameToken, FieldPath, JsonValue)>,
    /// The core defaults.
    pub core_defaults: Vec<(FieldPath, JsonValue)>,
    /// The paths marked as settings besides those [`is_setting`] names.
    pub settings: BTreeSet<FieldPath>,
    /// The location a derived value was read from, by path.
    pub origins: BTreeMap<FieldPath, String>,
}

impl Layered {
    /// Lays out the layers: the configuration values enter as `store.root` and `plugins.dirs`
    /// (project) and `slots.count` and `slots.evaluators` (user). The core default of `store.root` is
    /// `<project root>/.candler/store` with a project, and `<state home>/candler/store` without
    /// one (`$XDG_STATE_HOME`, else `~/.local/state`); with neither, there is no default.
    ///
    /// # Errors
    /// Returns [`ResolveError::NonUtf8Path`] if a configured path is not UTF-8.
    pub fn new(config: &Config, input: ResolveInput) -> Result<Self, ResolveError> {
        let store_root: FieldPath = STORE_ROOT.parse()?;
        let plugins_dirs: FieldPath = PLUGINS_DIRS.parse()?;
        let slots_count: FieldPath = SLOTS_COUNT.parse()?;
        let slots_evaluators: FieldPath = SLOTS_EVALUATORS.parse()?;

        let mut project = Vec::new();
        if let Some(found) = config.project.as_ref() {
            if let Some(root) = found.store_root.as_deref() {
                // BORROW: `.clone()` makes the owned path the layer entry holds
                project.push((store_root.clone(), path_json(root)?));
            }
            if !found.plugin_dirs.is_empty() {
                let dirs = found
                    .plugin_dirs
                    .iter()
                    // BORROW: `.to_owned()` makes the owned text the JSON string holds
                    .map(|dir| path_text(dir).map(|text| Value::String(text.to_owned())))
                    .collect::<Result<Vec<_>, _>>()?;
                project.push((plugins_dirs, JsonValue::new(Value::Array(dirs))?));
            }
        }
        let mut user = Vec::new();
        if let Some(count) = config.user.slots_count {
            user.push((slots_count, JsonValue::new(Value::from(count))?));
        }
        if let Some(count) = config.user.slots_evaluators {
            user.push((slots_evaluators, JsonValue::new(Value::from(count))?));
        }
        let default_root = match config.project.as_ref() {
            Some(found) => Some(found.root.join(".candler").join("store")),
            None => config
                .state_home
                .as_deref()
                .map(|state| state.join("candler").join("store")),
        };
        let mut core_defaults = Vec::new();
        if let Some(default_root) = default_root {
            // BORROW: `.as_path()` views the owned default root as a `&Path`
            core_defaults.push((store_root, path_json(default_root.as_path())?));
        }
        core_defaults.extend(input.core_defaults);

        Ok(Self {
            explicit: input.explicit,
            file: input.file.map(|f| f.values).unwrap_or_default(),
            project,
            user,
            derived: Vec::new(),
            core_defaults,
            settings: input.explicit_settings.into_iter().collect(),
            origins: BTreeMap::new(),
        })
    }

    /// Reports whether `path` resolves into settings: [`is_setting`] names it, or an explicit
    /// argument marked it.
    #[must_use]
    pub fn is_setting(&self, path: &FieldPath) -> bool {
        is_setting(path) || self.settings.contains(path)
    }

    /// Layers in the values a step derived.
    pub fn derive(&mut self, step: &NameToken, derived: &[(FieldPath, JsonValue)]) {
        for (path, value) in derived {
            // BORROW: `.clone()` makes the owned step, path and value the layer entry holds
            self.derived
                .push((step.clone(), path.clone(), value.clone()));
        }
    }

    /// Records the location each derived value at a path was read from.
    pub fn origin(&mut self, origins: &[(FieldPath, String)]) {
        for (path, from) in origins {
            // BORROW: `.clone()` makes the owned path and text the map holds
            self.origins.insert(path.clone(), from.clone());
        }
    }

    /// Picks, for every path, the value of the highest layer that sets it. The `preflight` of the
    /// result is empty.
    #[must_use]
    pub fn resolve<'a>(&'a self) -> Resolution {
        let layer = |entries: &'a [(FieldPath, JsonValue)], source: ValueSource| {
            entries
                .iter()
                .rev()
                // BORROW: `.clone()` makes the owned source each entry holds
                .map(move |(path, value)| (path, value, source.clone()))
        };
        let candidates = layer(&self.explicit, ValueSource::ExplicitArgument)
            .chain(layer(&self.file, ValueSource::ExperimentFile))
            .chain(layer(&self.project, ValueSource::ProjectConfig))
            .chain(layer(&self.user, ValueSource::UserConfig))
            .chain(self.derived.iter().rev().map(|(step, path, value)| {
                let source = self.origins.get(path).map_or_else(
                    // BORROW: `.clone()` makes the owned step the source holds
                    || ValueSource::derived(step.clone()),
                    // BORROW: `.clone()` makes the text the source holds
                    |from| ValueSource::derived_from(step.clone(), from.clone()),
                );
                (path, value, source)
            }))
            .chain(layer(&self.core_defaults, ValueSource::CoreDefault));
        let mut winners: BTreeMap<&FieldPath, (&JsonValue, ValueSource)> = BTreeMap::new();
        for (path, value, source) in candidates {
            winners.entry(path).or_insert((value, source));
        }

        let mut resolution = Resolution::default();
        for (path, (value, source)) in winners {
            // BORROW: `.clone()` makes the owned entries the resolution holds
            let resolved = ResolvedValue::recorded(path.clone(), value.clone());
            if self.is_setting(path) {
                resolution.settings.push(resolved);
            } else {
                resolution.values.push(resolved);
            }
            // BORROW: `.clone()` makes the owned path the entry holds
            resolution
                .sources
                .push(SourceEntry::new(path.clone(), source));
        }
        resolution
    }
}

fn path_text<'a>(path: &'a Path) -> Result<&'a str, ResolveError> {
    path.to_str().ok_or_else(|| ResolveError::NonUtf8Path {
        // BORROW: `.to_owned()` makes the owned path the error carries
        path: path.to_owned(),
    })
}

/// Reports whether `path` is a setting whose value names a location.
fn is_location(path: &FieldPath) -> bool {
    // BORROW: the path's text, compared with the location settings
    matches!(
        path.as_str(),
        STORE_ROOT
            | PLUGINS_DIRS
            | ISOLATION_BUILD_DOCKERFILE
            | ISOLATION_BUILD_CONTEXT
            | ISOLATION_BUILD_LOCK
            | ISOLATION_BUILD_REQUIREMENTS
    )
}

/// The treatment `request` with its `source`, when it holds a string, made a path under `dir`;
/// anything else stays as written.
fn treatment_source_under(dir: &Path, request: &Value) -> Result<Value, ResolveError> {
    let mut request = request.clone(); // BORROW: the rewritten request is a copy
    if let Some(Value::String(text)) = request.get_mut("source") {
        let joined = dir.join(text.as_str()); // BORROW: .as_str() views the text as a `&str`
        path_text(lexical(joined.as_path()).as_path())?.clone_into(text); // BORROW: the request owns its text
    }
    Ok(request)
}

/// `path` without its `.` segments and with each `..` applied to the segment before it, as text
/// would read; no symbolic link is followed and no file is read.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() && !out.has_root() {
                    out.push(part);
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => out.push(part),
        }
    }
    out
}

/// `text` as a path under `dir`, as text.
fn join_text(dir: &Path, text: &str) -> Result<String, ResolveError> {
    // BORROW: `.to_owned()` makes the owned text the joined path is written as
    Ok(path_text(dir.join(text).as_path())?.to_owned())
}

fn path_json(path: &Path) -> Result<JsonValue, ResolveError> {
    // BORROW: `.to_owned()` makes the owned text the JSON string holds
    Ok(JsonValue::new(Value::String(path_text(path)?.to_owned()))?)
}
