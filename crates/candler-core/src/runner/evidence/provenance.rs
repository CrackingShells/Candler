//! Where a run's measurement came from: the evaluation infrastructure that ran it, the sandbox
//! contract and the packages of the env image that the experiment names. Each value is a
//! `provenance.<dotted>` entry of the manifest's measured values.

use candler_schemas::values::MeasuredValue;
use serde_json::{Number, Value};

use super::measure::measured;
use crate::isolation::image::PackageReadings;
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
    /// The full 40-hex commit the build was made from.
    pub commit: Option<String>,
    /// Whether the tree had uncommitted changes at build time.
    pub dirty: Option<bool>,
}

/// The `provenance.*` values of a run: the evaluation infrastructure, the sandbox contract and,
/// for each import package the experiment names, the distribution and version its env image
/// holds, as `provenance.packages.<import name>.distribution` and `.version`. A value that is
/// unknown is left out.
#[must_use]
pub fn provenance_values(infra: &EvalInfra, packages: &PackageReadings) -> Vec<MeasuredValue> {
    let mut entries = vec![
        (
            String::from("provenance.eval_infra.tag"),
            // BORROW: .clone() gives the entry its own text
            infra.tag.clone().map(Value::String),
        ),
        (
            String::from("provenance.eval_infra.commits_ahead"),
            infra.commits_ahead.map(|n| Value::Number(Number::from(n))),
        ),
        (
            String::from("provenance.eval_infra.commit"),
            // BORROW: .clone() gives the entry its own text
            infra.commit.clone().map(Value::String),
        ),
        (
            String::from("provenance.eval_infra.dirty"),
            infra.dirty.map(Value::Bool),
        ),
        (
            String::from("provenance.sandbox_contract"),
            // BORROW: .to_owned() gives the entry its own text
            Some(Value::String(SANDBOX_CONTRACT.to_owned())),
        ),
    ];
    for (name, reading) in packages {
        entries.push((
            format!("provenance.packages.{name}.distribution"),
            // BORROW: .clone() gives the entry its own value
            Some(reading.distribution.clone()),
        ));
        entries.push((
            format!("provenance.packages.{name}.version"),
            // BORROW: .clone() gives the entry its own value
            Some(reading.version.clone()),
        ));
    }
    entries
        .into_iter()
        .filter_map(|(path, value)| measured(STEP, &path, value?))
        .collect()
}
