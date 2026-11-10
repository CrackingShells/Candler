//! Preflight steps, their reports and the resolver that runs them.
//!
//! A step is registered with a [`Resolver`], which runs the steps in registration order. Each step
//! sees the configuration and the values resolved so far, including what the steps before it
//! derived. A step that returns an error, or a report with a failed check, has its derived values
//! dropped; the later steps still run, and the resolution fails once every step has run.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use candler_schemas::primitives::{FieldPath, FormatError, NameToken};
use candler_schemas::values::{JsonValue, ResolvedValue, SourceEntry, ValuesError};
use futures::future::BoxFuture;

use super::config::Config;
use super::layers::{Layered, Resolution, ResolveInput};

/// One check a step ran.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct CheckResult {
    /// The check's name.
    pub name: NameToken,
    /// Whether the check passed.
    pub passed: bool,
    /// What was checked and what was found.
    pub explanation: String,
}

impl CheckResult {
    /// A check with its outcome and explanation.
    #[must_use]
    pub const fn new(name: NameToken, passed: bool, explanation: String) -> Self {
        Self {
            name,
            passed,
            explanation,
        }
    }
}

/// What one preflight step reports.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct StepReport {
    /// The step that produced the report.
    pub step: NameToken,
    /// The values the step derived, by path.
    pub derived: Vec<(FieldPath, JsonValue)>,
    /// The location each derived value was read from, by path, for the values that have one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub origins: Vec<(FieldPath, String)>,
    /// The checks the step ran.
    pub checks: Vec<CheckResult>,
}

impl StepReport {
    /// A report of `step` with no derived value and no check.
    #[must_use]
    pub const fn new(step: NameToken) -> Self {
        Self {
            step,
            derived: Vec::new(),
            origins: Vec::new(),
            checks: Vec::new(),
        }
    }

    /// Adds a derived value.
    #[must_use]
    pub fn derive(mut self, path: FieldPath, value: JsonValue) -> Self {
        self.derived.push((path, value));
        self
    }

    /// Adds a derived value together with the location `from` it was read from, as the
    /// experiment file wrote it.
    #[must_use]
    pub fn derive_from(mut self, path: FieldPath, value: JsonValue, from: String) -> Self {
        // BORROW: `.clone()` gives the origin its own path
        self.origins.push((path.clone(), from));
        self.derived.push((path, value));
        self
    }

    /// Adds a check result.
    #[must_use]
    pub fn check(mut self, check: CheckResult) -> Self {
        self.checks.push(check);
        self
    }

    /// Reports whether every check passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|check| check.passed)
    }
}

/// What a step sees: the configuration and the values resolved so far.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ResolveContext {
    /// The configuration, with the caller's working directory.
    pub config: Arc<Config>,
    /// The resolved values, sorted by path.
    pub values: Vec<ResolvedValue>,
    /// The resolved operational settings, sorted by path.
    pub settings: Vec<ResolvedValue>,
    /// The source of every resolved path, sorted by path.
    pub sources: Vec<SourceEntry>,
}

impl ResolveContext {
    /// The value resolved for `path`, among the values and the settings.
    #[must_use]
    pub fn get<'a>(&'a self, path: &FieldPath) -> Option<&'a JsonValue> {
        self.values
            .iter()
            .chain(self.settings.iter())
            .find(|resolved| resolved.path() == path)
            .and_then(ResolvedValue::value)
    }
}

/// Why a step could not run its checks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PreflightError {
    /// The step failed before it could report.
    #[error("{detail}")]
    Failed {
        /// What went wrong.
        detail: String,
    },
}

/// A step that returned an error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct StepError {
    /// The step.
    pub step: NameToken,
    /// The error it returned.
    pub error: PreflightError,
}

/// A check that runs during resolution, derives values and reports.
pub trait PreflightStep: Send + Sync {
    /// The step's name, `name@token`; it is the `step` of the source of every value it derives.
    fn name<'a>(&'a self) -> &'a NameToken;

    /// Runs the step against the values resolved so far.
    fn run(
        &self,
        ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>>;
}

/// Why a resolution could not complete.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ResolveError {
    /// A path that has to be a JSON string is not UTF-8.
    #[error("the path {} is not UTF-8", path.display())]
    NonUtf8Path {
        /// The path.
        path: PathBuf,
    },
    /// A resolved value is not a hashable JSON value.
    #[error(transparent)]
    Value(#[from] ValuesError),
    /// A field path built by the resolver is malformed.
    #[error(transparent)]
    Format(#[from] FormatError),
    /// A step with the same name is already registered.
    #[error("a preflight step named `{name}` is already registered")]
    DuplicateStep {
        /// The name of the step that was refused.
        name: NameToken,
    },
    /// Two steps derive the same path.
    #[error("the preflight steps `{first}` and `{second}` both derive `{path}`")]
    DerivedTwice {
        /// The path both steps derive.
        path: FieldPath,
        /// The step that derived it first, as `name@token`.
        first: String,
        /// The step that derived it second, as `name@token`.
        second: String,
    },
    /// At least one step failed; every step ran.
    #[error(
        "preflight failed: {} failed step report(s), {} step error(s){}",
        reports.len(),
        errors.len(),
        failures(reports, errors)
    )]
    Preflight {
        /// The reports of the steps with a failed check, in registration order.
        reports: Vec<StepReport>,
        /// The steps that returned an error, in registration order.
        errors: Vec<StepError>,
    },
}

/// How many failed steps the refusal names before it counts the rest.
const NAMED: usize = 5;

/// The failed steps of a refusal, each with its failing checks or its error: `: a@1 (check: why),
/// b@2 (error: why)`, the first five and then `and N more`. Empty when no step failed.
fn failures(reports: &[StepReport], errors: &[StepError]) -> String {
    let failed = reports.iter().map(|report| {
        let checks: Vec<String> = report
            .checks
            .iter()
            .filter(|check| !check.passed)
            .map(|check| format!("{}: {}", check.name, check.explanation))
            .collect();
        format!("{} ({})", report.step, checks.join("; "))
    });
    let broken = errors
        .iter()
        .map(|failure| format!("{} (error: {})", failure.step, failure.error));
    let all: Vec<String> = failed.chain(broken).collect();
    if all.is_empty() {
        return String::new();
    }
    let more = all.len().saturating_sub(NAMED);
    let named = all
        .iter()
        .take(NAMED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if more == 0 {
        format!(": {named}")
    } else {
        format!(": {named}, and {more} more")
    }
}

/// Runs preflight steps in registration order over layered values.
pub struct Resolver {
    config: Arc<Config>,
    steps: Vec<Box<dyn PreflightStep>>, // TRAIT_OBJECT: steps of different types are registered side by side
}

impl Resolver {
    /// A resolver with no step.
    #[must_use]
    pub const fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            steps: Vec::new(),
        }
    }

    /// Registers a step after the ones already registered.
    ///
    /// # Errors
    /// Returns [`ResolveError::DuplicateStep`] if a registered step has the same name, whatever
    /// its version token, so `a@1` and `a@2` cannot both be registered.
    pub fn register(mut self, step: Box<dyn PreflightStep>) -> Result<Self, ResolveError> {
        // TRAIT_OBJECT: steps of different types are registered side by side
        let name = step.name();
        if self
            .steps
            .iter()
            .any(|known| known.name().name() == name.name())
        {
            return Err(ResolveError::DuplicateStep {
                // BORROW: `.clone()` makes the owned name the error carries
                name: name.clone(),
            });
        }
        self.steps.push(step);
        Ok(self)
    }

    /// Layers the values of `input` with the configuration, runs every step in order and returns
    /// the resolution.
    ///
    /// # Errors
    /// Returns [`ResolveError::DerivedTwice`] as soon as a passing step derives a path that an
    /// earlier passing step derived, or that it derives twice itself. Returns
    /// [`ResolveError::Preflight`] listing every failed step once all steps ran, and the errors of
    /// [`Layered::new`].
    pub async fn resolve(&self, input: ResolveInput) -> Result<Resolution, ResolveError> {
        // BORROW: `.as_ref()` views the shared config as a `&Config`
        let mut layered = Layered::new(self.config.as_ref(), input)?;
        let mut owners: BTreeMap<FieldPath, NameToken> = BTreeMap::new();
        let mut passed = Vec::new();
        let mut reports = Vec::new();
        let mut errors = Vec::new();
        for step in &self.steps {
            let so_far = layered.resolve();
            let ctx = Arc::new(ResolveContext {
                config: Arc::clone(&self.config),
                values: so_far.values,
                settings: so_far.settings,
                sources: so_far.sources,
            });
            match step.run(ctx).await {
                Ok(report) if report.passed() => {
                    for (path, _) in &report.derived {
                        // BORROW: `.clone()` makes the owned name the owner map holds
                        let earlier = owners.insert(path.clone(), step.name().clone());
                        if let Some(first) = earlier {
                            return Err(ResolveError::DerivedTwice {
                                // BORROW: `.clone()` makes the owned path the error carries
                                path: path.clone(),
                                first: first.to_string(),
                                second: step.name().to_string(),
                            });
                        }
                    }
                    layered.derive(step.name(), &report.derived);
                    layered.origin(&report.origins);
                    passed.push(report);
                }
                Ok(report) => reports.push(report),
                Err(error) => errors.push(StepError {
                    // BORROW: `.clone()` makes the owned name the error carries
                    step: step.name().clone(),
                    error,
                }),
            }
        }
        if !reports.is_empty() || !errors.is_empty() {
            return Err(ResolveError::Preflight { reports, errors });
        }
        let mut resolution = layered.resolve();
        resolution.preflight = passed;
        Ok(resolution)
    }
}
