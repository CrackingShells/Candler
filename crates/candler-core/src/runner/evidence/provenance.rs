//! Where a run's measurement came from: the version of the skill, the evaluation
//! infrastructure that ran it, the sandbox contract and the planner library of the env image.
//! Each value is a `provenance.<dotted>` entry of the manifest's measured values.

use candler_schemas::values::MeasuredValue;
use serde_json::{Number, Value};

use super::measure::measured;
use crate::isolation::image::PlannerAxis;
use crate::isolation::layout::SANDBOX_CONTRACT;

/// The preflight step the provenance values name as their source.
const STEP: &str = "provenance@1";

/// Which build of the evaluation infrastructure runs the job, as the binary was built from git.
/// A field the build could not read is none, and is left out of the manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvalInfra {
    /// The nearest tag at or before the commit.
    pub tag: Option<String>,
    /// The commits between the tag and the build's commit.
    pub commits_ahead: Option<u64>,
    /// The abbreviated commit the build was made from.
    pub commit: Option<String>,
    /// Whether the tree had uncommitted changes at build time.
    pub dirty: Option<bool>,
}

/// The version of a skill as the reference names it: the tag without its `<name>@` prefix and
/// without a leading `v`, then one `v` in front.
#[must_use]
pub fn skill_version(name: &str, tag: &str) -> String {
    let prefix = format!("{name}@");
    let bare = tag.strip_prefix(prefix.as_str()).unwrap_or(tag);
    let bare = bare.strip_prefix('v').unwrap_or(bare);
    format!("v{bare}")
}

/// The `provenance.*` values of a run: the skill version from the treatment's tag, the
/// evaluation infrastructure, the sandbox contract and, when the probe read it, the planner
/// library of the env image. A value that is unknown is left out.
#[must_use]
pub fn provenance_values(
    skill: (&str, Option<&str>),
    infra: &EvalInfra,
    planner: Option<&PlannerAxis>,
) -> Vec<MeasuredValue> {
    let (name, tag) = skill;
    let entries = [
        (
            "provenance.skill_version",
            tag.map(|tag| Value::String(skill_version(name, tag))),
        ),
        (
            "provenance.eval_infra.tag",
            // BORROW: .clone() gives the entry its own text
            infra.tag.clone().map(Value::String),
        ),
        (
            "provenance.eval_infra.commits_ahead",
            infra.commits_ahead.map(|n| Value::Number(Number::from(n))),
        ),
        (
            "provenance.eval_infra.commit",
            // BORROW: .clone() gives the entry its own text
            infra.commit.clone().map(Value::String),
        ),
        ("provenance.eval_infra.dirty", infra.dirty.map(Value::Bool)),
        (
            "provenance.sandbox_contract",
            // BORROW: .to_owned() gives the entry its own text
            Some(Value::String(SANDBOX_CONTRACT.to_owned())),
        ),
        (
            "provenance.planner.sandbox.distribution",
            // BORROW: .clone() gives the entry its own value
            planner.map(|axis| axis.distribution.clone()),
        ),
        (
            "provenance.planner.sandbox.version",
            // BORROW: .clone() gives the entry its own value
            planner.map(|axis| axis.version.clone()),
        ),
    ];
    entries
        .into_iter()
        .filter_map(|(path, value)| measured(STEP, path, value?))
        .collect()
}
