//! The start check of a job's declared MCP servers: before the job's first run, each stdio
//! server's start command is started in the container a run of its cell would get, and must still
//! be running after a short wait. Remote servers are not started, and no MCP client is involved.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use candler_harness::driver::Plan;
use candler_harness::mcp::servers;
use candler_harness::spec::{Cell, CommandSpec};
use candler_schemas::primitives::RunKey;
use candler_schemas::servers::McpServer;

use super::session::image_of;
use crate::isolation::container::agent_home;
use crate::isolation::model::ContainerPlan;
use crate::isolation::{Isolation, IsolationError, StartProbe};
use crate::runner::Runtime;
use crate::runner::exec::{ExecError, compose_container, mounted_inputs, stage_cell};
use crate::runner::plan::RunPlan;

/// How long a server's start command is given before it must still be running.
///
/// The wait catches a server that crashes as it starts: an import error, a missing file, a missing
/// variable. It does not catch a server that starts and never answers MCP.
pub const SERVER_START_WAIT: Duration = Duration::from_secs(3);

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
type Groups<'a> = BTreeMap<(Option<String>, String), (&'a RunPlan, Vec<(String, McpServer)>)>;

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
        let signature = (
            image_of(comparable),
            serde_json::to_string(&declared).map_err(|e| failed(&e))?,
        );
        let stdio: Vec<(String, McpServer)> = declared
            .into_iter()
            .filter(|(_, server)| server.command.is_some())
            .collect();
        if stdio.is_empty() {
            continue;
        }
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

/// Stages what a run of `plan`'s cell starts with under `root`, a scratch workspace and home, and
/// composes the container such a run gets: the driver's plan for the cell and the container over
/// it, with the cell's inputs and the plan's read-only workspace files mounted.
///
/// # Errors
/// [`ExecError`] when the cell is unknown, a directory cannot be made, the driver refuses the cell
/// or a file cannot be written or mounted.
pub(super) async fn stage_container(
    rt: &Runtime,
    isolation: &dyn Isolation, // TRAIT_OBJECT: the runtime's isolation, or a fake in tests
    plan: &RunPlan,
    root: &Path,
) -> Result<(Plan, ContainerPlan), ExecError> {
    let cell = rt
        .cells
        .get(&plan.cell)
        .ok_or_else(|| IsolationError::Plan {
            detail: String::from("the job holds no record of the cell"),
        })?;
    let comparable = plan.stored.as_ref().unwrap_or(&cell.comparable);
    let workspace = root.join("workspace");
    let home = agent_home(root);
    for dir in [&workspace, &home] {
        fs::create_dir_all(dir).map_err(|error| IsolationError::Plan {
            detail: format!("cannot make a scratch directory: {error}"),
        })?;
    }
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
    .await?;
    let mounted = mounted_inputs(&rt.store, &cell.inputs, &staged.workspace_files)?;
    let container = compose_container(rt, isolation, &view, &workspace, &home, &mounted)?;
    Ok((staged, container))
}

/// Starts each server of one group in the container of `plan`'s cell, over a scratch workspace
/// under `root`.
async fn probe_group(
    rt: &Runtime,
    isolation: &dyn Isolation, // TRAIT_OBJECT: the runtime's isolation, or a fake in tests
    plan: &RunPlan,
    stdio: &[(String, McpServer)],
    root: &Path,
) -> Result<(), ServerCheckError> {
    let (staged, mut container) = stage_container(rt, isolation, plan, root)
        .await
        .map_err(|e| failed(&e))?;
    // Standard input stays open, as a server's own would while the agent runs.
    container.interactive = true;
    for (name, server) in stdio {
        // The server's own variables reach the server alone, as in its configuration, and never
        // the container's environment, where a gateway or plan name would be refused.
        let mut words = Vec::new();
        if let Some(env) = server.env.as_ref().filter(|env| !env.is_empty()) {
            words.push(String::from("env"));
            words.extend(env.iter().map(|(name, value)| format!("{name}={value}")));
        }
        words.extend(server.command.iter().cloned()); // BORROW: the command owns its words
        words.extend(server.args.iter().flatten().cloned()); // BORROW: the command owns its words
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
                key: name.clone(), // BORROW: the error owns the key
                output,
            });
        }
    }
    Ok(())
}
