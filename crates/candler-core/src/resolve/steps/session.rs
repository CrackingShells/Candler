//! The session step: whether a cell asks to run as a session, and whether it may.
//!
//! `runner.mode` is `headless` or `session`, and absent means headless: the step derives nothing
//! for a cell that does not write it, so every cell that predates session mode keeps its digest.
//! In session mode the step derives `runner.idle_s = 60` when the cell gives none. `runner.idle_s`
//! without session mode is refused, and so is session mode on a harness whose driver lacks the
//! session capability. [`settle`] is that rule as one pure function: the step applies it to the
//! experiment-wide values and planning to each cell's own, so a session cell is the same cell
//! whether `runner.mode` is fixed or a factor level.

use std::str::FromStr as _;
use std::sync::Arc;

use candler_schemas::primitives::{FieldPath, FormatError, NameToken};
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;
use serde_json::{Number, Value};

use crate::experiment::paths;
use crate::resolve::preflight::{
    CheckResult, PreflightError, PreflightStep, ResolveContext, StepReport,
};

/// The seconds a session stays open after the agent yields, when the cell gives none.
pub const DEFAULT_IDLE_S: u64 = 60;

/// The text of `runner.idle_s` that keeps a session open until a cap ends it.
pub const IDLE_NONE: &str = "none";

/// The two modes a cell can run in.
pub const MODES: [&str; 2] = ["headless", MODE_SESSION];

/// The `runner.mode` that keeps the harness's input open.
pub const MODE_SESSION: &str = "session";

/// The name of the step.
pub const STEP_NAME: &str = "session_mode@1";

/// The check that names a refusal.
fn check(name: &str, passed: bool, explanation: String) -> Result<CheckResult, PreflightError> {
    let name = NameToken::from_str(name).map_err(|error| PreflightError::Failed {
        detail: error.to_string(), // BORROW: .to_string() gives owned text
    })?;
    Ok(CheckResult::new(name, passed, explanation))
}

/// Whether `value` is a whole number of seconds or the text `none`.
fn idle_is_valid(value: &Value) -> bool {
    match value {
        Value::Number(number) => number.is_u64(),
        Value::String(text) => text == IDLE_NONE,
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => false,
    }
}

/// Why a cell's session settings are refused: the name of the check that failed, and why.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SessionRefusal {
    /// The settings of a cell fail the session rule.
    #[error("{detail}")]
    Refused {
        /// The check that failed, as `name@token`.
        check: String,
        /// What is wrong, for the author of the experiment.
        detail: String,
    },
}

impl SessionRefusal {
    fn new(check: &str, detail: String) -> Self {
        Self::Refused {
            check: check.to_owned(), // BORROW: the refusal owns the check name
            detail,
        }
    }

    /// The check that failed, as `name@token`.
    #[must_use]
    pub fn check(&self) -> &str {
        match self {
            Self::Refused { check, .. } => check.as_str(),
        }
    }

    /// What is wrong, for the author of the experiment.
    #[must_use]
    pub fn detail(&self) -> &str {
        match self {
            Self::Refused { detail, .. } => detail.as_str(),
        }
    }
}

/// What the session rule asks of a cell that passes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settled {
    /// Whether the cell is a session that gave no `runner.idle_s`, so the default belongs in it.
    pub derive_idle: bool,
}

/// The session rule of one cell, from its `runner.mode` and `runner.idle_s`.
///
/// `runner.mode` is `headless` or `session`; `runner.idle_s` is whole seconds or `none` and
/// belongs to session mode only; a session needs a driver that can run one (`capable`, unknown
/// when `None`: the check is then left to the caller that knows the driver). A session without
/// `runner.idle_s` takes [`DEFAULT_IDLE_S`], which the caller derives.
///
/// # Errors
/// [`SessionRefusal`] naming the check that failed.
pub fn settle(
    mode: Option<&Value>,
    idle: Option<&Value>,
    capable: Option<bool>,
) -> Result<Settled, SessionRefusal> {
    if let Some(idle) = idle
        && !idle_is_valid(idle)
    {
        return Err(SessionRefusal::new(
            "idle_s_valid@1",
            format!(
                "runner.idle_s is {idle}: write whole seconds, or \"{IDLE_NONE}\" to keep the session open until a cap ends it"
            ),
        ));
    }
    let Some(mode) = mode else {
        return if idle.is_some() {
            Err(SessionRefusal::new(
                "idle_s_needs_session@1",
                "runner.idle_s is set and runner.mode is not: idle_s applies to session mode only"
                    .to_owned(), // BORROW: owned text
            ))
        } else {
            Ok(Settled { derive_idle: false })
        };
    };
    // BORROW: explicit .as_str()
    let Some(mode) = mode.as_str().filter(|text| MODES.contains(text)) else {
        return Err(SessionRefusal::new(
            "mode_valid@1",
            format!("runner.mode is {mode}: write \"headless\" or \"session\""),
        ));
    };
    if mode != MODE_SESSION {
        return if idle.is_some() {
            Err(SessionRefusal::new(
                "idle_s_needs_session@1",
                "runner.idle_s is set and runner.mode is headless: idle_s applies to session mode only"
                    .to_owned(), // BORROW: owned text
            ))
        } else {
            Ok(Settled { derive_idle: false })
        };
    }
    if capable == Some(false) {
        return Err(SessionRefusal::new(
            "session_capable@1",
            "runner.mode is session and the harness's driver cannot run a session".to_owned(), // BORROW: owned text
        ));
    }
    Ok(Settled {
        derive_idle: idle.is_none(),
    })
}

/// Validates `runner.mode` and `runner.idle_s`, and derives the idle default of a session.
pub struct SessionModeStep {
    name: NameToken,
    session_capable: bool,
}

impl SessionModeStep {
    /// A step for a harness whose driver does, or does not, run sessions.
    ///
    /// # Errors
    /// [`FormatError`] when the step name is not a name token.
    pub fn new(session_capable: bool) -> Result<Self, FormatError> {
        Ok(Self {
            name: NameToken::from_str(STEP_NAME)?,
            session_capable,
        })
    }
}

/// The value the context resolved at `path`.
fn value_at<'a>(ctx: &'a ResolveContext, path: &str) -> Result<Option<&'a Value>, PreflightError> {
    let path = FieldPath::from_str(path).map_err(|error| PreflightError::Failed {
        detail: error.to_string(), // BORROW: .to_string() gives owned text
    })?;
    Ok(ctx.get(&path).map(JsonValue::as_value))
}

/// The derived default of `runner.idle_s`, as a path and a value.
///
/// # Errors
/// [`PreflightError::Failed`] when the path or the value cannot be built.
pub fn default_idle() -> Result<(FieldPath, JsonValue), PreflightError> {
    let path =
        FieldPath::from_str(paths::RUNNER_IDLE_S).map_err(|error| PreflightError::Failed {
            detail: error.to_string(), // BORROW: .to_string() gives owned text
        })?;
    let value = JsonValue::new(Value::Number(Number::from(DEFAULT_IDLE_S))).map_err(|error| {
        PreflightError::Failed {
            detail: error.to_string(), // BORROW: .to_string() gives owned text
        }
    })?;
    Ok((path, value))
}

impl PreflightStep for SessionModeStep {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        let step = self.name.clone(); // BORROW: the future owns the step name
        let capable = self.session_capable;
        let outcome = (|| {
            let mut report = StepReport::new(step);
            let mode = value_at(&ctx, paths::RUNNER_MODE)?;
            let idle = value_at(&ctx, paths::RUNNER_IDLE_S)?;
            match settle(mode, idle, Some(capable)) {
                Err(refusal) => {
                    report =
                        report.check(check(refusal.check(), false, refusal.detail().to_owned())?); // BORROW: owned text
                }
                Ok(Settled { derive_idle: true }) => {
                    let (path, value) = default_idle()?;
                    report = report.derive(path, value);
                }
                Ok(Settled { derive_idle: false }) => {}
            }
            Ok(report)
        })();
        Box::pin(async move { outcome })
    }
}
