#![cfg(test)]
//! Guard: input that Candler once accepted and then ignored or misread is refused with a code and a
//! pointer. One table, one row per case: an input, the refusal code and the pointer it must give.
//!
//! The codes and pointers are written by hand from the leaf `silent_acceptance`, never read back
//! from the code under test. `ResolveInput::secret_names` being required is a compile-time fact,
//! so it has no row: a caller that does not give the names does not build.

use std::error::Error;

use candler_core::jobs::orchestrate::servers::stdio_servers;
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, Layered, Resolution, ResolveInput};
use candler_core::runner::plan::{Force, PlanMode, plan_runs};
use candler_schemas::primitives::FieldPath;
use candler_schemas::values::JsonValue;
use serde_json::{Value, json};

use crate::support::run_plan::{Facts, experiment, fresh_store, plan_ctx, resolution};

/// What a refusal says: its code and its pointer.
type Refused = (String, Option<String>);

fn refused_by(error: &(dyn Error + 'static)) -> Refused {
    let found = candler_core::refusal::find(error).expect("a coded refusal");
    (found.code().as_str().to_owned(), found.path())
}

fn fp(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

fn js(value: Value) -> JsonValue {
    JsonValue::new(value).expect("a JSON value")
}

fn config(cwd: &std::path::Path) -> Config {
    Config::new(
        cwd.to_owned(), // BORROW: the config owns its working directory
        None,
        UserConfig::default(),
        Some(cwd.join("state")),
    )
}

/// Lays out `entries` as the experiment file's values, with no declared secret.
fn layered(entries: &[(&str, Value)]) -> Result<Layered, Refused> {
    let root = tempfile::tempdir().expect("a temp dir");
    let values = entries
        .iter()
        .map(|(path, value)| (fp(path), js(value.clone())));
    let input = ResolveInput::default().file(FileValues::new(values.collect()));
    Layered::new(&config(root.path()), input).map_err(|error| refused_by(&error))
}

/// The refusal of the resolve layer for `entries`, if any.
fn at_resolve(entries: &[(&str, Value)]) -> Option<Refused> {
    layered(entries).err()
}

/// The refusal of planning an experiment that writes `replicates = 3` when the file's values carry
/// `replicates = 5` under `[fixed]`.
fn at_plan_fixed_five() -> Option<Refused> {
    let layers = layered(&[("replicates", json!(5))])
        .expect("layers")
        .resolve();
    let facts = Facts::base();
    let mut resolved = serde_json::to_value(resolution(&facts)).expect("JSON");
    resolved["settings"] = serde_json::to_value(&layers.settings).expect("JSON");
    resolved["sources"] = serde_json::to_value(&layers.sources).expect("JSON");
    let resolved: Resolution = serde_json::from_value(resolved).expect("a resolution");
    let (_dir, store, lock) = fresh_store();
    let ctx = plan_ctx(&store, &lock, &facts);
    plan_runs(
        &ctx,
        &experiment(&["m1", "m2"], 3),
        &resolved,
        Force::No,
        PlanMode::Check,
    )
    .err()
    .map(|error| refused_by(&error))
}

/// The refusal of the start check's reading of `declared`, if any.
fn at_servers(declared: Value) -> Option<Refused> {
    let Value::Object(table) = declared else {
        panic!("a table of servers");
    };
    drop(stdio_servers(&table));
    None
}

struct Row {
    name: &'static str,
    code: &'static str,
    pointer: &'static str,
    run: fn() -> Option<Refused>,
}

const ROWS: [Row; 7] = [
    Row {
        name: "an env name equal to model.token_secret",
        code: "experiment.env_token_secret",
        pointer: "/env/GATEWAY_TOKEN",
        run: || {
            at_resolve(&[
                ("model.token_secret", json!("GATEWAY_TOKEN")),
                ("env", json!({"GATEWAY_TOKEN": "x"})),
            ])
        },
    },
    Row {
        name: "a [fixed] replicates that differs from the file's own count",
        code: "experiment.replicates_conflict",
        pointer: "/replicates",
        run: at_plan_fixed_five,
    },
    Row {
        name: "a replicates.<x> path",
        code: "experiment.replicates_path",
        pointer: "/replicates/x",
        run: || at_resolve(&[("replicates.x", json!(2))]),
    },
    Row {
        name: "a replicate count that is a string",
        code: "experiment.replicates_not_integer",
        pointer: "/replicates",
        run: || at_resolve(&[("replicates", json!("three"))]),
    },
    Row {
        name: "a replicate count that is a fraction",
        code: "experiment.replicates_not_integer",
        pointer: "/replicates",
        run: || at_resolve(&[("replicates", json!(2.5))]),
    },
    Row {
        name: "an MCP server command that is not a string",
        code: "jobs.server_command_not_string",
        pointer: "/mcp/servers/fs/command",
        run: || at_servers(json!({"fs": {"command": 7}})),
    },
    Row {
        name: "MCP server args that are not all strings",
        code: "jobs.server_args_not_strings",
        pointer: "/mcp/servers/fs/args",
        run: || at_servers(json!({"fs": {"command": "mcp-fs", "args": ["--root", 3]}})),
    },
];

/// Guards: every silent case is refused with its own code and pointer.
/// Oracle: the codes and pointers of the leaf, written by hand in `ROWS`.
/// Bite: dropping one check turns its row to `None`; reading a code from the error under test.
#[test]
fn silent_cases_are_refused_with_a_code_and_a_pointer() {
    let mut wrong = Vec::new();
    for row in ROWS {
        let want = Some((row.code.to_owned(), Some(row.pointer.to_owned())));
        let got = (row.run)();
        if got != want {
            wrong.push(format!("{}: got {got:?}, want {want:?}", row.name));
        }
    }
    assert!(
        wrong.is_empty(),
        "rows not refused as written:\n{}",
        wrong.join("\n")
    );
}
