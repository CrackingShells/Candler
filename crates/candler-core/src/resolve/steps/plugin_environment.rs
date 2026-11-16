//! The plug-in environment step: the values and inputs of a plug-in's agent environment, as a
//! layer below the experiment file.
//!
//! An experiment selects an environment with the setting `isolation.build.plugin = "<plugin>/<env>"`.
//! [`Fragment`] reads the plug-in's manifest from the directories of `plugins.dirs`, makes every
//! path the environment writes absolute against the plug-in directory and refuses a path that
//! leaves it. The step derives each value of the fragment, recording `<plugin>/<env>` as its
//! origin, so a value the experiment writes wins and one it leaves out is the plug-in's. It never
//! derives `request.inputs`: the image step owns that root, and the plug-in's inputs reach it
//! through the experiment's declared inputs.
//!

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::Arc;

use candler_schemas::execution::{ExperimentInput, InputAudience, paths};
use candler_schemas::primitives::{FieldPath, FormatError, Name, NameToken};
use candler_schemas::values::{JsonValue, ResolvedValue};
use futures::future::BoxFuture;
use serde_json::Value;

use crate::evaluation::registry::{Registry, RegistryError, discover_manifests};
use crate::resolve::layers::FileValues;
use crate::resolve::preflight::{
    CheckResult, PreflightError, PreflightStep, ResolveContext, StepReport,
};

/// The name of the step.
pub const STEP_NAME: &str = "plugin_environment@1";

/// The step's name without its version token, which names its reports whatever the version.
pub const STEP_BASE: &str = "plugin_environment";

/// The check that names a plug-in without the environment the experiment selects.
pub const CHECK_MISSING: &str = "plugin_environment_missing@1";

/// The paths of an environment that name a file or a directory of the plug-in.
const LOCATIONS: [&str; 4] = [
    paths::ISOLATION_BUILD_DOCKERFILE,
    paths::ISOLATION_BUILD_CONTEXT,
    paths::ISOLATION_BUILD_LOCK,
    paths::ISOLATION_BUILD_REQUIREMENTS,
];

/// Why the selected environment is refused by the step.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PluginEnvironmentRefusal {
    /// The plug-in has no environment of the selected name.
    #[error("{detail}")]
    Missing {
        /// The plug-in, the environment and the environments the plug-in offers.
        detail: String,
    },
}

impl PluginEnvironmentRefusal {
    /// The check that names this refusal, as `name@token`.
    #[must_use]
    pub const fn check(&self) -> &'static str {
        match self {
            Self::Missing { .. } => CHECK_MISSING,
        }
    }

    /// What is wrong, for the author of the experiment.
    #[must_use]
    pub fn detail<'a>(&'a self) -> &'a str {
        match self {
            Self::Missing { detail } => detail.as_str(),
        }
    }

    /// The refusal a failed check of the step stands for, when `check` names one.
    #[must_use]
    pub fn from_check(check: &str, detail: &str) -> Option<Self> {
        match check {
            CHECK_MISSING => Some(Self::Missing {
                detail: detail.to_owned(), // BORROW: the refusal owns the detail
            }),
            // EXHAUSTIVE: any other check belongs to another step, which this refusal does not stand for
            _ => None,
        }
    }
}

/// Why a fragment cannot be loaded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FragmentError {
    /// The selection is not `<plugin>/<environment>`.
    #[error(
        "`{}` must be `<plugin>/<environment>` of plug-in and environment names, not {text}",
        paths::ISOLATION_BUILD_PLUGIN
    )]
    Selector {
        /// What the experiment wrote.
        text: String,
    },
    /// The plug-in manifests cannot be read.
    #[error(transparent)]
    Registry(#[from] RegistryError),
    /// No manifest of `plugins.dirs` names the plug-in.
    #[error("no plug-in named `{plugin}` is in `plugins.dirs`")]
    UnknownPlugin {
        /// The plug-in the experiment selected.
        plugin: String,
    },
    /// The plug-in has no environment of that name.
    #[error("plug-in `{plugin}` has no agent environment `{environment}`; it offers: {offered}")]
    Missing {
        /// The plug-in.
        plugin: String,
        /// The environment the experiment selected.
        environment: String,
        /// The environments the plug-in offers, or `none`.
        offered: String,
    },
    /// A path of the environment leaves the plug-in directory.
    #[error("agent environment `{selector}`: `{path}` {why}")]
    Escapes {
        /// The selection.
        selector: String,
        /// The path as the manifest wrote it.
        path: String,
        /// What is wrong with it.
        why: String,
    },
    /// An input or a value of the environment is not valid.
    #[error("agent environment `{selector}`: {detail}")]
    Invalid {
        /// The selection.
        selector: String,
        /// What is wrong.
        detail: String,
    },
}

/// The agent environment a plug-in offers, as layered values and inputs.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Fragment {
    /// The selection, `<plugin>/<environment>`, as the experiment wrote it.
    pub selector: String,
    /// The plug-in's directory.
    pub dir: PathBuf,
    /// The environment's values by path, with every location absolute below `dir`.
    pub values: Vec<(FieldPath, JsonValue)>,
    /// The environment's inputs, for the agent, each relative to `dir`.
    pub inputs: Vec<ExperimentInput>,
}

/// The value of `path` among the resolved values and settings.
fn resolved<'a>(
    values: &'a [ResolvedValue],
    settings: &'a [ResolvedValue],
    path: &str,
) -> Option<&'a Value> {
    values
        .iter()
        .chain(settings.iter())
        .find(|found| found.path().as_str() == path)
        .and_then(ResolvedValue::value)
        .map(JsonValue::as_value)
}

impl Fragment {
    /// The fragment the resolved values select with `isolation.build.plugin`, read from the
    /// directories of `plugins.dirs`; `None` when no environment is selected.
    ///
    /// # Errors
    /// The [`FragmentError`] of [`Fragment::load`], and [`FragmentError::Selector`] for a
    /// selection that is not text.
    pub fn from_values(
        values: &[ResolvedValue],
        settings: &[ResolvedValue],
    ) -> Result<Option<Self>, FragmentError> {
        let Some(selected) = resolved(values, settings, paths::ISOLATION_BUILD_PLUGIN) else {
            return Ok(None);
        };
        let selector = selected.as_str().ok_or_else(|| FragmentError::Selector {
            text: selected.to_string(),
        })?;
        let dirs: Vec<PathBuf> = resolved(values, settings, paths::PLUGINS_DIRS)
            .and_then(Value::as_array)
            .map(|dirs| {
                dirs.iter()
                    .filter_map(Value::as_str)
                    .map(PathBuf::from)
                    .collect()
            })
            .unwrap_or_default();
        Self::load(dirs.as_slice(), selector).map(Some) // BORROW: explicit .as_slice()
    }

    /// Reads the environment `selector` (`<plugin>/<environment>`) from the manifests of `dirs`.
    ///
    /// # Errors
    /// [`FragmentError::Selector`] when `selector` is not two names, [`FragmentError::Registry`]
    /// when a manifest cannot be read, [`FragmentError::UnknownPlugin`] and
    /// [`FragmentError::Missing`] when the plug-in or its environment does not exist,
    /// [`FragmentError::Escapes`] when a location or an input leaves the plug-in directory, and
    /// [`FragmentError::Invalid`] when a value or an input is not valid.
    pub fn load(dirs: &[PathBuf], selector: &str) -> Result<Self, FragmentError> {
        let bad = || FragmentError::Selector {
            text: selector.to_owned(), // BORROW: the error owns the text
        };
        let (plugin, environment) = selector.split_once('/').ok_or_else(bad)?;
        let plugin_name = Name::from_str(plugin).map_err(|_| bad())?;
        let environment_name = Name::from_str(environment).map_err(|_| bad())?;
        let manifests = discover_manifests(dirs)?;
        let registry = Registry::load(manifests.as_slice())?; // BORROW: explicit .as_slice()
        let found = registry
            .plugin(&plugin_name)
            .ok_or_else(|| FragmentError::UnknownPlugin {
                plugin: plugin.to_owned(), // BORROW: the error owns the name
            })?;
        let offered = found.manifest.agent_environments();
        let environment_found =
            offered
                .get(&environment_name)
                .ok_or_else(|| FragmentError::Missing {
                    plugin: plugin.to_owned(),           // BORROW: the error owns the name
                    environment: environment.to_owned(), // BORROW: the error owns the name
                    offered: if offered.is_empty() {
                        "none".to_owned() // BORROW: the error owns the text
                    } else {
                        offered
                            .keys()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    },
                })?;
        let escapes = |path: &str, why: String| FragmentError::Escapes {
            selector: selector.to_owned(), // BORROW: the error owns the text
            path: path.to_owned(),         // BORROW: the error owns the text
            why,
        };
        let invalid = |detail: String| FragmentError::Invalid {
            selector: selector.to_owned(), // BORROW: the error owns the text
            detail,
        };
        let root = fs::canonicalize(&found.dir)
            .map_err(|e| invalid(format!("the plug-in directory cannot be read: {e}")))?;
        let mut values: Vec<(FieldPath, JsonValue)> = Vec::new();
        for (path, value) in &environment_found.values {
            if LOCATIONS.contains(&path.as_str()) {
                for written in location_texts(value.as_value()) {
                    inside(root.as_path(), written).map_err(|why| escapes(written, why))?;
                }
            }
            values.push((path.clone(), value.clone())); // BORROW: the fragment owns its values
        }
        let values = FileValues::new(values)
            .relative_to(found.dir.as_path())
            .map_err(|e| invalid(e.to_string()))?
            .values;
        let mut inputs = Vec::new();
        for input in &environment_found.inputs {
            inside(root.as_path(), input.path()).map_err(|why| escapes(input.path(), why))?;
            inputs.push(
                ExperimentInput::new(
                    input.kind().clone(),          // BORROW: the input owns its kind
                    input.path().to_owned(),       // BORROW: the input owns its path
                    input.at().map(str::to_owned), // BORROW: the input owns its text
                    InputAudience::Agent,
                )
                .map_err(|e| invalid(e.to_string()))?,
            );
        }
        Ok(Self {
            selector: selector.to_owned(), // BORROW: the fragment owns its selection
            dir: found.dir.clone(),        // BORROW: the fragment owns its directory
            values,
            inputs,
        })
    }

    /// The entries of the list the fragment sets at `path`, in order; none when it sets none.
    #[must_use]
    pub fn list(&self, path: &str) -> Vec<String> {
        self.values
            .iter()
            .find(|(own, _)| own.as_str() == path)
            .and_then(|(_, value)| value.as_value().as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned) // BORROW: the caller owns the entries
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Whether the fragment sets `path`.
    #[must_use]
    pub fn sets(&self, path: &str) -> bool {
        self.values.iter().any(|(own, _)| own.as_str() == path)
    }
}

/// The path texts of a location value: a string, or the strings of a list that are not
/// `treatment:` sources.
fn location_texts<'a>(value: &'a Value) -> Vec<&'a str> {
    match value {
        Value::String(text) => vec![text.as_str()],
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .filter(|text| !text.starts_with(paths::TREATMENT_SOURCE_PREFIX))
            .collect(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_) => Vec::new(),
    }
}

/// Checks that `written`, a path relative to the plug-in directory `root` (canonical), stays
/// inside it: no absolute path, no `..` segment, and no symbolic link on the way out.
fn inside(root: &Path, written: &str) -> Result<(), String> {
    if written.starts_with(['/', '\\']) || Path::new(written).is_absolute() {
        return Err("is an absolute path".to_owned());
    }
    if written.split(['/', '\\']).any(|segment| segment == "..") {
        return Err("leaves the plug-in's directory".to_owned());
    }
    let mut probe = root.join(written);
    while fs::symlink_metadata(&probe).is_err() {
        if !probe.pop() {
            return Ok(());
        }
    }
    let real = fs::canonicalize(&probe).map_err(|e| format!("cannot be resolved: {e}"))?;
    if real.starts_with(root) {
        Ok(())
    } else {
        Err("leads out of the plug-in's directory through a symbolic link".to_owned())
    }
}

/// Derives the values of the plug-in environment the experiment selects.
pub struct PluginEnvironmentStep {
    name: NameToken,
}

impl PluginEnvironmentStep {
    /// The step.
    ///
    /// # Errors
    /// [`FormatError`] when the step name is not a name token.
    pub fn new() -> Result<Self, FormatError> {
        Ok(Self {
            name: NameToken::from_str(STEP_NAME)?,
        })
    }
}

impl PreflightStep for PluginEnvironmentStep {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        let step = self.name.clone(); // BORROW: the future owns the step name
        let outcome = (|| {
            let report = StepReport::new(step);
            // BORROW: explicit .as_slice()
            let fragment =
                match Fragment::from_values(ctx.values.as_slice(), ctx.settings.as_slice()) {
                    Ok(Some(fragment)) => fragment,
                    Ok(None) => return Ok(report),
                    Err(error @ FragmentError::Missing { .. }) => {
                        let name = NameToken::from_str(CHECK_MISSING).map_err(|e| {
                            PreflightError::Failed {
                                detail: e.to_string(), // BORROW: owned text
                            }
                        })?;
                        return Ok(report.check(CheckResult::new(name, false, error.to_string())));
                    }
                    Err(error) => {
                        return Err(PreflightError::Failed {
                            detail: error.to_string(), // BORROW: owned text
                        });
                    }
                };
            let mut report = report;
            for (path, value) in &fragment.values {
                if path.as_str() == paths::ISOLATION_BUILD_PLUGIN {
                    continue;
                }
                // BORROW: the report owns its path, value and origin
                report = report.derive_from(path.clone(), value.clone(), fragment.selector.clone());
            }
            Ok(report)
        })();
        Box::pin(async move { outcome })
    }
}
