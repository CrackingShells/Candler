#![cfg(test)]
//! Guards for the split of the budget.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use candler_core::experiment::budget::to_caps;
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::{
    PreflightError, PreflightStep, ResolveContext, Resolver, StepReport,
};
use candler_schemas::execution::{Budget, Comparable};
use candler_schemas::primitives::{FieldPath, NameToken};
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;
use serde_json::{Value, json};

fn fp(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

fn js(value: Value) -> JsonValue {
    JsonValue::new(value).expect("a JSON value")
}

/// Derives the five typed roots, as the preflight steps do; `flip` names the one root whose value
/// differs from the base.
struct Roots {
    name: NameToken,
    flip: Option<&'static str>,
}

impl PreflightStep for Roots {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        _ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        let flip = self.flip;
        let digit = move |root: &str| if flip == Some(root) { 'b' } else { 'a' };
        let digest = move |root: &str| format!("sha256:{}", digit(root).to_string().repeat(64));
        let derived = [
            (
                "harness",
                json!({"name": "claude-code", "version": if flip == Some("harness") {"2.0.1"} else {"2.0.0"}}),
            ),
            (
                "treatment",
                json!({"name": if flip == Some("treatment") {"t-b"} else {"t-a"}}),
            ),
            (
                "inputs",
                json!([{"kind": "build_inputs", "digest": digest("inputs")}]),
            ),
            (
                "snapshots",
                json!([{"kind": "catalog", "digest": digest("snapshots")}]),
            ),
            (
                "environments",
                json!([{"plugin": "p@1.0.0", "lock_digest": digest("environments")}]),
            ),
        ];
        // BORROW: `.clone()` gives the report its own step name
        let mut report = StepReport::new(self.name.clone());
        for (path, value) in derived {
            report = report.derive(fp(path), js(value));
        }
        Box::pin(async move { Ok(report) })
    }
}

fn config(cwd: &Path) -> Arc<Config> {
    Arc::new(Config::new(
        // BORROW: `.to_owned()` makes the owned working directory
        cwd.to_owned(),
        None,
        UserConfig::default(),
        Some(cwd.join("state")),
    ))
}

/// Resolves the file values with the typed roots derived, as the registered steps derive them.
async fn resolve(file: &[(&str, Value)], flip: Option<&'static str>) -> Resolution {
    let cwd = PathBuf::from("/work");
    let step = Roots {
        name: "roots@1".parse().expect("a name@token"),
        flip,
    };
    let values = file
        .iter()
        // BORROW: `.clone()` gives the file its own value
        .map(|(path, value)| (fp(path), js(value.clone())))
        .collect();
    Resolver::new(config(&cwd))
        .register(Box::new(step))
        .expect("registers")
        .resolve(ResolveInput::new(Vec::new()).file(FileValues::new(values)))
        .await
        .expect("resolves")
}

fn comparable(
    resolution: &Resolution,
) -> Result<Comparable, candler_schemas::execution::ExecutionError> {
    let settings: Vec<FieldPath> = resolution
        .settings
        .iter()
        // BORROW: `.clone()` makes the owned setting path
        .map(|setting| setting.path().clone())
        .collect();
    Comparable::from_resolution(&resolution.values, &resolution.sources, &settings)
        .map(|(cell, _)| cell)
}

/// Guards: I20 — the budget split: `budget.per_run.*` moves the cell digest, and the caps of the
/// job (`budget.max_runs`, `max_parallel`, `max_tokens`, `max_cost_usd`, `wall_s`) never do; only
/// the per-run caps become agent flags, and the container's expiry is the wall cap plus 60 s.
/// Oracle: R03 section 10.4 formula, written out as literals below (3600 s + 60 s = 3660 s).
/// Bite: read the job cap `budget.max_cost_usd` in `to_caps` instead of `per_run.max_cost_usd`.
#[tokio::test]
async fn b20_budget_split_and_expiry() {
    for path in [
        "budget.per_run.wall_s",
        "budget.per_run.turns",
        "budget.per_run.max_cost_usd",
    ] {
        let one = comparable(&resolve(&[(path, json!(1))], None).await).expect("a cell");
        let other = comparable(&resolve(&[(path, json!(2))], None).await).expect("a cell");
        assert_ne!(one.digest(), other.digest(), "{path} moves the digest");
    }
    for path in [
        "budget.max_runs",
        "budget.max_parallel",
        "budget.max_tokens",
        "budget.max_cost_usd",
        "budget.wall_s",
    ] {
        let one = comparable(&resolve(&[(path, json!(1))], None).await).expect("a cell");
        let other = comparable(&resolve(&[(path, json!(2))], None).await).expect("a cell");
        assert_eq!(
            one.digest(),
            other.digest(),
            "{path} leaves the digest alone"
        );
    }

    let budget: Budget = serde_json::from_value(json!({
        "per_run": {"wall_s": 3600, "turns": 7, "max_cost_usd": 0.5},
        "max_runs": 3,
        "max_cost_usd": 99.0,
        "wall_s": 86400,
    }))
    .expect("a budget");
    let caps = to_caps(&budget);
    assert_eq!(
        caps.flags(),
        ["--max-turns", "7", "--max-budget-usd", "0.5"]
    );
    // R03 10.4: the supervisor's deadline is wall_s; the container expires 60 s later.
    assert_eq!(caps.expiry().map(|expiry| expiry.as_secs()), Some(3660));
}
