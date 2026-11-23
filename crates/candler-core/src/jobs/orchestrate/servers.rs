//! The start check of a job's declared MCP servers: before the job's first run, each stdio
//! server's start command is started in the container a run of its cell would get, and must still
//! be running after a short wait. Remote servers are not started, and no MCP client is involved.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use candler_harness::mcp::servers;
use candler_harness::spec::{Cell, CommandSpec};
use candler_schemas::primitives::RunKey;
use serde_json::{Map, Value};

use super::session::image_of;
use crate::isolation::container::agent_home;
use crate::isolation::{Isolation, StartProbe};
use crate::runner::Runtime;
use crate::runner::exec::{compose_container, stage_cell};
use crate::runner::plan::RunPlan;

/// How long a server's start command is given before it must still be running.
///
/// The wait catches a server that crashes as it starts: an import error, a missing file, a missing
/// variable. It does not catch a server that starts and never answers MCP; the census of each run
/// records which servers the harness saw.
pub const SERVER_START_WAIT: Duration = Duration::from_secs(3);

/// One stdio server to start: its key, its command words and its literal environment.
struct Stdio {
    key: String,
    argv: Vec<String>,
    env: Vec<(String, String)>,
}

/// The stdio servers of `declared`; a server with a `url`, or without a `command`, is skipped.
fn stdio_servers(declared: &Map<String, Value>) -> Vec<Stdio> {
    let mut found = Vec::new();
    for (key, server) in declared {
        if server.get("url").is_some() {
            continue;
        }
        let Some(command) = server.get("command").and_then(Value::as_str) else {
            continue;
        };
        let mut argv = vec![command.to_owned()]; // BORROW: the probe owns its words
        if let Some(args) = server.get("args").and_then(Value::as_array) {
            argv.extend(args.iter().filter_map(Value::as_str).map(str::to_owned)); // BORROW: the probe owns its words
        }
        let env = server
            .get("env")
            .and_then(Value::as_object)
            .map(|table| {
                table
                    .iter()
                    .filter_map(|(name, value)| {
                        value.as_str().map(|text| (name.clone(), text.to_owned())) // BORROW: the probe owns its entries
                    })
                    .collect()
            })
            .unwrap_or_default();
        found.push(Stdio {
            key: key.clone(), // BORROW: the probe owns its key
            argv,
            env,
        });
    }
    found
}

/// Why the start check refused the job.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ServerCheckError {
    /// A declared server exited when it was started.
    #[error("the server `{key}` exited at start: {output}")]
    Server {
        /// The key of the server under `mcp.servers`.
        key: String,
        /// The tail of what the server printed.
        output: String,
    },
    /// The check could not run.
    #[error("the start check of the declared servers could not run: {detail}")]
    ServerCheck {
        /// What went wrong.
        detail: String,
    },
}

/// The distinct pairs of image and declared servers, each with the first plan of such a cell.
type Groups<'a> = BTreeMap<(Option<String>, String), (&'a RunPlan, Vec<Stdio>)>;

fn failed(detail: &impl ToString) -> ServerCheckError {
    ServerCheckError::ServerCheck {
        detail: detail.to_string(), // BORROW: .to_string() gives owned text
    }
}

/// Starts the declared stdio servers of every distinct pair of image and servers among the cells
/// of `plans`, each in the container a run of that cell gets, and refuses when one has exited
/// after [`SERVER_START_WAIT`].
///
/// # Errors
/// [`ServerCheckError::Server`] with the server's key and its output when one exited, and
/// [`ServerCheckError::ServerCheck`] when the check itself could not run.
pub async fn check_servers(rt: &Runtime, plans: &[RunPlan]) -> Result<(), ServerCheckError> {
    let isolation = rt
        .isolation
        .as_ref()
        .ok_or_else(|| failed(&"no isolation"))?;
    let mut groups: Groups<'_> = BTreeMap::new();
    for plan in plans {
        let Some(cell) = rt.cells.get(&plan.cell) else {
            continue;
        };
        let comparable = plan.stored.as_ref().unwrap_or(&cell.comparable);
        let declared = servers(&Cell::new(comparable.values())).map_err(|e| failed(&e))?;
        let Some(declared) = declared else {
            continue;
        };
        let stdio = stdio_servers(&declared);
        if stdio.is_empty() {
            continue;
        }
        let signature = (image_of(comparable), Value::Object(declared).to_string());
        groups.entry(signature).or_insert((plan, stdio));
    }
    if groups.is_empty() {
        return Ok(());
    }
    let scratch = rt
        .store
        .layout()
        .job_dir(&rt.job)
        .join("scratch")
        .join("server-check");
    let mut verdict = Ok(());
    for (index, (plan, stdio)) in groups.values().enumerate() {
        let root = scratch.join(index.to_string());
        verdict = probe_group(rt, isolation.as_ref(), plan, stdio, &root).await;
        let _ = fs::remove_dir_all(&root); // EXPLICIT: the scratch is removed whether or not the probe passed; a leftover is swept with the job directory
        if verdict.is_err() {
            break;
        }
    }
    let _ = fs::remove_dir_all(&scratch); // EXPLICIT: the scratch root holds nothing once the groups are done
    verdict
}

/// Starts each server of one group in the container of `plan`'s cell, over a scratch workspace
/// under `root`.
async fn probe_group(
    rt: &Runtime,
    isolation: &dyn Isolation, // TRAIT_OBJECT: the runtime's isolation, or a fake in tests
    plan: &RunPlan,
    stdio: &[Stdio],
    root: &Path,
) -> Result<(), ServerCheckError> {
    let cell = rt.cells.get(&plan.cell).ok_or_else(|| failed(&"no cell"))?;
    let comparable = plan.stored.as_ref().unwrap_or(&cell.comparable);
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).map_err(|e| failed(&e))?;
    let home = agent_home(root);
    fs::create_dir_all(&home).map_err(|e| failed(&e))?;
    let view = Cell::new(comparable.values());
    let (staged, _written) = stage_cell(
        rt,
        cell,
        comparable,
        &view,
        &workspace,
        &home,
        &root.join("scratch"),
    )
    .await
    .map_err(|e| failed(&e))?;
    let mut container = compose_container(rt, isolation, &view, &workspace, &home, &cell.inputs)
        .map_err(|e| failed(&e))?;
    // Standard input stays open, as a server's own would while the agent runs.
    container.interactive = true;
    for server in stdio {
        // The server's own variables reach the server alone, as in its configuration, and never
        // the container's environment, where a gateway or plan name would be refused.
        let mut words = Vec::new();
        if !server.env.is_empty() {
            words.push(String::from("env"));
            words.extend(
                server
                    .env
                    .iter()
                    .map(|(name, value)| format!("{name}={value}")),
            );
        }
        words.extend(server.argv.iter().cloned()); // BORROW: the command owns its words
        let mut builder = CommandSpec::builder().argv(words);
        for entry in staged
            .prepared
            .env_overlay
            .iter()
            .chain(staged.command.env_overlay())
        {
            builder = builder.env(entry.clone()); // BORROW: the command owns its entries
        }
        let command = builder.build().map_err(|e| failed(&e))?;
        let key: RunKey = plan.key.clone(); // BORROW: the container is labelled as the run it stands in for
        let seen = isolation
            .start_probe(&container, &command, key, SERVER_START_WAIT)
            .await
            .map_err(|e| failed(&e))?;
        if let StartProbe::Exited { output } = seen {
            return Err(ServerCheckError::Server {
                key: server.key.clone(), // BORROW: the error owns the key
                output,
            });
        }
    }
    Ok(())
}
