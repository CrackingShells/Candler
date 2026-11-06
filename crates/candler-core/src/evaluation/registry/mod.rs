//! Plug-in manifests and suites, checked without spawning anything, and the environment preflight.
//!
//! [`Registry::load`] reads plug-in manifests and keeps each with its directory.
//! [`Registry::check_suite`] checks a suite against what the evaluators described: it is pure, and
//! the engine obtains the descriptions from the protocol client first. A `snapshots` or `inputs`
//! need is refused as unsupported in this version.

use std::io;
use std::path::PathBuf;

use candler_schemas::evaluation::EvaluationError;
use candler_schemas::primitives::{ExpectationId, Name};

mod environment;
mod manifest;
mod needs;
mod suite;

pub use environment::{
    EnvironmentKey, EnvironmentStep, default_env_root, register_evaluation_steps,
};
pub use manifest::{Plugin, Registry};
pub use needs::NeedError;
pub use suite::{CheckedEntry, CheckedSuite};

/// Why a manifest could not be loaded or a suite could not be accepted.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RegistryError {
    /// A manifest file that cannot be read.
    #[error("cannot read the plug-in manifest {}: {source}", path.display())]
    Read {
        /// The manifest path.
        path: PathBuf,
        /// The operating-system error.
        #[source]
        source: io::Error,
    },
    /// A manifest that is not a valid plug-in manifest.
    #[error("the plug-in manifest {} is not valid: {source}", path.display())]
    Manifest {
        /// The manifest path.
        path: PathBuf,
        /// What the schema reader reported.
        #[source]
        source: EvaluationError,
    },
    /// Two manifests that carry the same plug-in name.
    #[error("two plug-in manifests are named `{name}`")]
    DuplicatePlugin {
        /// The repeated name.
        name: Name,
    },
    /// A `describe` answer that disagrees with its manifest.
    #[error(
        "plug-in `{plugin}` described itself with {field} `{described}`, its manifest says `{manifest}`"
    )]
    DescribeMismatch {
        /// The plug-in, by its manifest name.
        plugin: Name,
        /// The field that differs: `name` or `version`.
        field: &'static str,
        /// The manifest's value.
        manifest: String,
        /// The described value.
        described: String,
    },
    /// An entry whose evaluator has no loaded manifest.
    #[error("entry {entry}: evaluator `{evaluator}` has no loaded plug-in manifest")]
    UnknownEvaluator {
        /// The entry.
        entry: ExpectationId,
        /// The evaluator it names.
        evaluator: Name,
    },
    /// An entry whose evaluator has a manifest but no `describe` answer.
    #[error("entry {entry}: evaluator `{evaluator}` has no `describe` answer")]
    NotDescribed {
        /// The entry.
        entry: ExpectationId,
        /// The evaluator it names.
        evaluator: Name,
    },
    /// An entry whose evaluator described a version outside the range the entry names.
    #[error(
        "entry {entry}: plug-in `{plugin}` described version {described}, outside the range `{range}`"
    )]
    VersionOutsideRange {
        /// The entry.
        entry: ExpectationId,
        /// The plug-in, by evaluator name.
        plugin: Name,
        /// The described version.
        described: String,
        /// The entry's version range.
        range: String,
    },
    /// An entry whose kind its evaluator does not describe.
    #[error("entry {entry}: evaluator `{evaluator}` declares no kind `{kind}`")]
    UnknownKind {
        /// The entry.
        entry: ExpectationId,
        /// The evaluator it names.
        evaluator: Name,
        /// The kind it names.
        kind: Name,
    },
    /// A kind whose params schema does not compile.
    #[error("entry {entry}: the params schema of kind `{kind}` does not compile: {detail}")]
    BadParamsSchema {
        /// The entry.
        entry: ExpectationId,
        /// The kind.
        kind: Name,
        /// What the validator reported.
        detail: String,
    },
    /// Params that fail the kind's params schema.
    #[error("entry {entry}: params of kind `{kind}` fail the schema at \"{pointer}\": {detail}")]
    Params {
        /// The entry.
        entry: ExpectationId,
        /// The kind.
        kind: Name,
        /// The JSON pointer of the first fault; empty for the params object itself.
        pointer: String,
        /// What the validator reported.
        detail: String,
    },
    /// No directory holds the user state, so there is no environment root.
    #[error("no state home for the environments: neither XDG_STATE_HOME nor HOME is set")]
    NoStateHome,
    /// A file of a plug-in environment that cannot be read.
    #[error("cannot read {} for the environment digest: {source}", path.display())]
    EnvironmentFile {
        /// The file.
        path: PathBuf,
        /// The operating-system error.
        #[source]
        source: io::Error,
    },
    /// A need the core cannot meet.
    #[error("entry {entry}: kind `{kind}`: {source}")]
    Need {
        /// The entry.
        entry: ExpectationId,
        /// The kind.
        kind: Name,
        /// Which need, and why.
        #[source]
        source: NeedError,
    },
}

#[cfg(test)]
mod scaffold {
    #![allow(clippy::needless_pass_by_value)] // scaffold only; the last commit deletes it

    use std::collections::BTreeMap;

    use candler_schemas::evaluation::{DescribeResult, SuiteFile};
    use candler_schemas::primitives::Name;
    use serde_json::json;

    use super::{Registry, RegistryError};

    fn registry(dir: &std::path::Path) -> Registry {
        let manifest = dir.join("plugin.toml");
        std::fs::write(
            &manifest,
            "schema = \"plugin-manifest/1\"\nname = \"p\"\nversion = \"1.0.0\"\ncommand = [\"run\"]\n\
             [protocol]\nmajor = 1\nminor = 0\n[environment]\nkind = \"none\"\n",
        )
        .expect("written");
        Registry::load(&[manifest]).expect("loads")
    }

    fn described(needs: serde_json::Value, version: &str) -> BTreeMap<Name, DescribeResult> {
        let result: DescribeResult = serde_json::from_value(json!({
            "protocol": {"major": 1, "minor": 0}, "name": "p", "version": version,
            "instruments": [], "environment": {"packages": {}}, "kind": "deterministic",
            "kinds": [{"kind": "k", "params_schema": {"type": "object", "required": ["n"],
                "properties": {"n": {"type": "integer"}}}, "needs": needs,
                "default_deadline_ms": 1000}]
        }))
        .expect("a describe result");
        BTreeMap::from([("p".parse().expect("a name"), result)])
    }

    fn suite(kind: &str, params: &str) -> SuiteFile {
        SuiteFile::from_toml_str(&format!(
            "schema = \"suite/1\"\n[[expectations]]\nid = \"a\"\nrole = \"gate\"\nkind = \"{kind}\"\n\
             depends_on = []\n[expectations.evaluator]\nname = \"p\"\nrange = \"^1\"\n\
             [expectations.params]\n{params}\n"
        ))
        .expect("a suite")
    }

    fn needs(extra: serde_json::Value) -> serde_json::Value {
        let mut base = json!({"trace": false, "artifacts": ["outputs/**"], "snapshots": [],
            "inputs": [], "metadata": ["plan.prompt_domain_name"]});
        if let (Some(base), Some(extra)) = (base.as_object_mut(), extra.as_object()) {
            base.extend(extra.clone());
        }
        base
    }

    #[test]
    fn valid_suite_passes_and_faults_are_refused() {
        let dir = tempfile::tempdir().expect("a dir");
        let registry = registry(dir.path());
        let ok = described(needs(json!({})), "1.0.0");
        assert_eq!(
            registry
                .check_suite(&suite("k", "n = 1"), &ok)
                .expect("ok")
                .entries()
                .len(),
            1
        );
        assert!(matches!(
            registry.check_suite(&suite("zzz", "n = 1"), &ok),
            Err(RegistryError::UnknownKind { .. })
        ));
        let bad = registry
            .check_suite(&suite("k", "n = \"x\""), &ok)
            .expect_err("bad params");
        assert!(
            matches!(&bad, RegistryError::Params { pointer, .. } if pointer == "/n"),
            "{bad}"
        );
        for (need, text) in [
            ("snapshots", "unsupported in this version: snapshots"),
            ("inputs", "unsupported in this version: inputs"),
        ] {
            let d = described(needs(json!({ need: ["x"] })), "1.0.0");
            let RegistryError::Need { source, .. } = registry
                .check_suite(&suite("k", "n = 1"), &d)
                .expect_err("refused")
            else {
                panic!("not a need error")
            };
            assert_eq!(source.to_string(), text);
        }
        let outside = described(needs(json!({"artifacts": ["src/**"]})), "1.0.0");
        assert!(
            registry
                .check_suite(&suite("k", "n = 1"), &outside)
                .is_err()
        );
        let skew = described(needs(json!({})), "1.0.1");
        assert!(matches!(
            registry.check_suite(&suite("k", "n = 1"), &skew),
            Err(RegistryError::DescribeMismatch {
                field: "version",
                ..
            })
        ));
        assert!(matches!(
            registry.check_suite(&suite("k", "n = 1"), &BTreeMap::new()),
            Err(RegistryError::NotDescribed { .. })
        ));
    }
}
