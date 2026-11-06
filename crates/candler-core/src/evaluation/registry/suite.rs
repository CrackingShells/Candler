//! Checking a suite against what the evaluators described.

use std::collections::BTreeMap;

use candler_schemas::evaluation::{DescribeResult, KindSpec, SuiteEntry, SuiteFile};
use candler_schemas::primitives::Name;
use serde_json::Value;

use super::manifest::Registry;
use super::{RegistryError, needs};

/// A suite entry that passed the check, with the kind spec it was checked against.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CheckedEntry {
    /// The entry.
    pub entry: SuiteEntry,
    /// The kind spec of the entry's evaluator that describes its kind.
    pub spec: KindSpec,
}

/// A suite every entry of which passed the check, in the order of the file.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CheckedSuite {
    entries: Vec<CheckedEntry>,
}

impl CheckedSuite {
    /// The checked entries, in the order of the suite file.
    #[must_use]
    pub const fn entries<'a>(&'a self) -> &'a [CheckedEntry] {
        // BORROW: .as_slice() views the owned list
        self.entries.as_slice()
    }
}

impl Registry {
    /// Checks every entry of `suite`: its evaluator has a manifest and a `describe` answer that
    /// agrees with it, the answer declares the entry's kind, the params satisfy the kind's params
    /// schema and the kind's needs are ones the core can meet. It spawns nothing; the engine
    /// obtains `described`, keyed by evaluator name, from the protocol client first.
    ///
    /// # Errors
    /// Returns the first fault, as a [`RegistryError`] that names the entry and, for params, the
    /// kind and the JSON pointer of the fault. A need the core cannot meet, `snapshots` and
    /// `inputs` among them, is a [`RegistryError::Need`].
    pub fn check_suite(
        &self,
        suite: &SuiteFile,
        described: &BTreeMap<Name, DescribeResult>,
    ) -> Result<CheckedSuite, RegistryError> {
        for (key, result) in described {
            if let Some(plugin) = self.plugins.get(key) {
                if result.name != *key || result.name != *plugin.manifest.name() {
                    return Err(RegistryError::DescribeMismatch {
                        // BORROW: .clone() keeps the name in the error
                        plugin: key.clone(),
                        field: "name",
                        manifest: plugin.manifest.name().to_string(),
                        described: result.name.to_string(),
                    });
                }
                if result.version != *plugin.manifest.version() {
                    return Err(RegistryError::DescribeMismatch {
                        // BORROW: .clone() keeps the name in the error
                        plugin: key.clone(),
                        field: "version",
                        manifest: plugin.manifest.version().to_string(),
                        described: result.version.to_string(),
                    });
                }
            }
        }
        let mut entries = Vec::with_capacity(suite.expectations().len());
        for entry in suite.expectations() {
            entries.push(self.check_entry(entry, described)?);
        }
        Ok(CheckedSuite { entries })
    }

    /// Checks one entry.
    fn check_entry(
        &self,
        entry: &SuiteEntry,
        described: &BTreeMap<Name, DescribeResult>,
    ) -> Result<CheckedEntry, RegistryError> {
        let evaluator = &entry.evaluator().name;
        if !self.plugins.contains_key(evaluator) {
            return Err(RegistryError::UnknownEvaluator {
                // BORROW: .clone() keeps the id in the error
                entry: entry.id().clone(),
                evaluator: evaluator.clone(),
            });
        }
        let Some(result) = described.get(evaluator) else {
            return Err(RegistryError::NotDescribed {
                // BORROW: .clone() keeps the id in the error
                entry: entry.id().clone(),
                evaluator: evaluator.clone(),
            });
        };
        let range = &entry.evaluator().range;
        if !range.matches(&result.version) {
            return Err(RegistryError::VersionOutsideRange {
                // BORROW: .clone() keeps the id in the error
                entry: entry.id().clone(),
                plugin: evaluator.clone(),
                described: result.version.to_string(),
                range: range.to_string(),
            });
        }
        let Some(spec) = result.kinds.iter().find(|spec| spec.kind == *entry.kind()) else {
            return Err(RegistryError::UnknownKind {
                // BORROW: .clone() keeps the id in the error
                entry: entry.id().clone(),
                evaluator: evaluator.clone(),
                kind: entry.kind().clone(),
            });
        };
        check_params(entry, spec)?;
        needs::check(&spec.needs).map_err(|source| RegistryError::Need {
            // BORROW: .clone() keeps the id in the error
            entry: entry.id().clone(),
            kind: entry.kind().clone(),
            source,
        })?;
        Ok(CheckedEntry {
            // BORROW: .clone() hands the checked entry its own copy
            entry: entry.clone(),
            spec: spec.clone(),
        })
    }
}

/// Validates the params of `entry` against the params schema of `spec`.
fn check_params(entry: &SuiteEntry, spec: &KindSpec) -> Result<(), RegistryError> {
    let validator = jsonschema::validator_for(spec.params_schema.as_value()).map_err(|error| {
        RegistryError::BadParamsSchema {
            // BORROW: .clone() keeps the id in the error
            entry: entry.id().clone(),
            kind: entry.kind().clone(),
            detail: error.to_string(),
        }
    })?;
    // BORROW: .clone() builds the JSON object the validator reads
    let instance = Value::Object(entry.params().clone());
    match validator.iter_errors(&instance).next() {
        None => Ok(()),
        Some(error) => Err(RegistryError::Params {
            // BORROW: .clone() keeps the id in the error
            entry: entry.id().clone(),
            kind: entry.kind().clone(),
            pointer: error.instance_path().to_string(),
            detail: error.to_string(),
        }),
    }
}
