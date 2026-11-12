//! The session step: whether a cell asks to run as a session, and whether it may.
//!
//! `runner.mode` is `headless` or `session`, and absent means headless: the step derives nothing
//! for a cell that does not write it, so every cell that predates session mode keeps its digest.
//! In session mode the step derives `runner.idle_s = 60` when the cell gives none. `runner.idle_s`
//! without session mode is refused, and so is session mode on a harness whose driver lacks the
//! session capability.

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
pub const MODES: [&str; 2] = ["headless", "session"];

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
            name: NameToken::from_str("session_mode@1")?,
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
            if let Some(idle) = idle
                && !idle_is_valid(idle)
            {
                return Ok(report.check(check(
                    "idle_s_valid@1",
                    false,
                    format!(
                        "runner.idle_s is {idle}: write whole seconds, or \"{IDLE_NONE}\" to keep the session open until a cap ends it"
                    ),
                )?));
            }
            let Some(mode) = mode else {
                if idle.is_some() {
                    report = report.check(check(
                        "idle_s_needs_session@1",
                        false,
                        "runner.idle_s is set and runner.mode is not: idle_s applies to session mode only".to_owned(), // BORROW: owned text
                    )?);
                }
                return Ok(report);
            };
            let Some(mode) = mode.as_str().filter(|text| MODES.contains(text)) else {
                return Ok(report.check(check(
                    "mode_valid@1",
                    false,
                    format!("runner.mode is {mode}: write \"headless\" or \"session\""),
                )?));
            };
            if mode == "headless" {
                if idle.is_some() {
                    report = report.check(check(
                        "idle_s_needs_session@1",
                        false,
                        "runner.idle_s is set and runner.mode is headless: idle_s applies to session mode only".to_owned(), // BORROW: owned text
                    )?);
                }
                return Ok(report);
            }
            if !capable {
                return Ok(report.check(check(
                    "session_capable@1",
                    false,
                    "runner.mode is session and the harness's driver cannot run a session"
                        .to_owned(), // BORROW: owned text
                )?));
            }
            if idle.is_none() {
                let path = FieldPath::from_str(paths::RUNNER_IDLE_S).map_err(|error| {
                    PreflightError::Failed {
                        detail: error.to_string(), // BORROW: .to_string() gives owned text
                    }
                })?;
                let value = JsonValue::new(Value::Number(Number::from(DEFAULT_IDLE_S))).map_err(
                    |error| PreflightError::Failed {
                        detail: error.to_string(), // BORROW: .to_string() gives owned text
                    },
                )?;
                report = report.derive(path, value);
            }
            Ok(report)
        })();
        Box::pin(async move { outcome })
    }
}
