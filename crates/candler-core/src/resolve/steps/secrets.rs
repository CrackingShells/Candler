//! The secrets step: whether each secret the experiment declares is set.
//!
//! The step derives no value. It reports one check per declared secret, named `secret_<name>`,
//! which passes when the secret is present and fails when it is not. An explanation names the
//! secret and never holds a value: the step is handed the names that are present, not what they
//! hold.

use std::collections::BTreeSet;
use std::str::FromStr;

use candler_schemas::primitives::{FormatError, NameToken, SecretName};
use futures::future::BoxFuture;

use crate::resolve::preflight::{
    CheckResult, PreflightError, PreflightStep, ResolveContext, ResolveError, Resolver, StepReport,
};
use std::sync::Arc;

/// Checks that every declared secret is present.
pub struct SecretsStep {
    name: NameToken,
    declared: Vec<SecretName>,
    present: BTreeSet<SecretName>,
}

impl SecretsStep {
    /// A step over the secrets `declared`, of which `present` are set and not empty.
    ///
    /// # Errors
    /// [`FormatError`] when the step name is not a name token.
    pub fn new(
        declared: Vec<SecretName>,
        present: BTreeSet<SecretName>,
    ) -> Result<Self, FormatError> {
        Ok(Self {
            name: NameToken::from_str("secrets@1")?,
            declared,
            present,
        })
    }

    /// The report of the step.
    fn report(&self) -> Result<StepReport, PreflightError> {
        // BORROW: `.clone()` gives the report its own step name
        let mut report = StepReport::new(self.name.clone());
        for secret in &self.declared {
            let check = NameToken::from_str(&format!("secret_{secret}@1")).map_err(|e| {
                PreflightError::Failed {
                    detail: e.to_string(),
                }
            })?;
            let present = self.present.contains(secret);
            let explanation = if present {
                format!("the secret {secret} is set")
            } else {
                format!("the secret {secret} is not set, or is empty")
            };
            report = report.check(CheckResult::new(check, present, explanation));
        }
        Ok(report)
    }
}

impl PreflightStep for SecretsStep {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        _ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        let report = self.report();
        Box::pin(async move { report })
    }
}

/// Registers the secrets step on `resolver`, after the steps already there.
///
/// # Errors
/// [`ResolveError`] when a step of that name is registered already or the name is malformed.
pub fn register_secret_step(
    resolver: Resolver,
    declared: Vec<SecretName>,
    present: BTreeSet<SecretName>,
) -> Result<Resolver, ResolveError> {
    let step = SecretsStep::new(declared, present)?;
    resolver.register(Box::new(step)) // TRAIT_OBJECT: steps of different types are registered side by side
}
