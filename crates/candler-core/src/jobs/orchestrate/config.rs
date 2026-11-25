//! The start check of the files Candler installs for a harness: before the job's first run, each
//! distinct set of installed files is given to the harness's own check, in a container without
//! network that is the one a run of the cell gets, and a finding refuses the job.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use candler_harness::agent_config;
use candler_harness::driver::{DriverError, Plan};
use candler_harness::spec::{Cell, CheckVerdict, FileRoot, PreparedFile, Workspace};
use candler_schemas::primitives::Digest;
use candler_schemas::refusal::pointer_of;

use super::servers::{ServerCheckError, stage_container};
use super::session::image_of;
use crate::isolation::Isolation;
use crate::runner::Runtime;
use crate::runner::plan::{CellId, RunPlan};

/// Why the check refused the job.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigCheckError {
    /// The harness's own check reports a finding for what Candler installed, or the driver cannot
    /// describe the cell.
    #[error(transparent)]
    Driver(#[from] DriverError),
    /// The check could not run.
    #[error(transparent)]
    Check(#[from] ServerCheckError),
}

/// The harness, the image and the digest of everything the check reads: the cells that share all
/// three share one verdict.
type GroupKey = (String, Option<String>, Digest);

/// The distinct keys, each with the first plan of such a cell and every cell that has the key.
type Groups<'a> = BTreeMap<GroupKey, (&'a RunPlan, BTreeSet<CellId>)>;

fn failed(detail: &impl ToString) -> ConfigCheckError {
    ConfigCheckError::Check(ServerCheckError::ServerCheck {
        detail: detail.to_string(), // BORROW: .to_string() gives owned text
    })
}

/// The files `plan` installs, whichever root they are written under.
fn installed_files(plan: &Plan) -> impl Iterator<Item = &PreparedFile> {
    plan.prepared.files.iter().chain(&plan.workspace_files)
}

/// The digest of what the check reads in `plan`: every installed file with its root, path, mode
/// and bytes, and the words of the check command.
fn files_digest(plan: &Plan, command_words: &[String]) -> Digest {
    let mut bytes = Vec::new();
    for file in installed_files(plan) {
        bytes.extend_from_slice(
            format!(
                "{:?}\0{}\0{}\0{}\0",
                file.root,
                file.path,
                file.executable,
                file.contents.len()
            )
            .as_bytes(),
        );
        bytes.extend_from_slice(&file.contents);
    }
    for word in command_words {
        bytes.extend_from_slice(word.as_bytes());
        bytes.push(0);
    }
    Digest::of_bytes(&bytes)
}

/// The pointer into the experiment file of the setting the installed files come from.
fn setting_pointer(view: &Cell<'_>, harness: &str) -> String {
    // EXPLICIT: a value the reader refuses was refused when the runs were planned
    if agent_config::hooks::<serde_json::Value>(view, harness).ok().flatten().is_some() {
        pointer_of(["fixed", "hooks"])
    } else if agent_config::instructions(view, harness)
        .ok()
        .flatten()
        .is_some()
    {
        pointer_of(["fixed", "instructions"])
    } else {
        pointer_of(["fixed"])
    }
}

/// What Candler installed in `plan` and where, as words for a refusal.
fn installed_at(plan: &Plan) -> (String, String) {
    let names: Vec<String> = installed_files(plan)
        .map(|file| format!("`{}`", file.path))
        .collect();
    let installed = if names.is_empty() {
        String::from("its configuration")
    } else {
        format!("the files {}", names.join(", "))
    };
    let in_home = installed_files(plan).any(|file| matches!(file.root, FileRoot::Home));
    let in_workspace = installed_files(plan).any(|file| !matches!(file.root, FileRoot::Home));
    let at = match (in_home, in_workspace) {
        (true, true) => "the agent's home and the workspace",
        (true, false) => "the agent's home",
        (false, _) => "the workspace",
    };
    (installed, at.to_owned()) // BORROW: the refusal owns its words
}

/// Asks the harness's own check about the files Candler installs for each cell of `plans`, once
/// for each distinct harness, image and set of installed files, in a container without network,
/// and records a clean verdict on the cells it covered. A cell whose driver has no check is not
/// checked and keeps no verdict.
///
/// The check command prints its verdict on standard output and exits `0`: a command that exits
/// otherwise did not run.
///
/// # Errors
/// [`ConfigCheckError::Driver`] with [`DriverError::HarnessRejectedInput`] when the harness
/// reports a finding, and [`ConfigCheckError::Check`] when the check could not run.
pub async fn check_config(rt: &mut Runtime, plans: &[RunPlan]) -> Result<(), ConfigCheckError> {
    let isolation: Arc<dyn Isolation> = rt // TRAIT_OBJECT: the runtime's isolation, or a fake in tests
        .isolation
        .clone() // BORROW: the check shares the runtime's isolation
        .ok_or_else(|| failed(&"no isolation"))?;
    let scratch = rt
        .store
        .layout()
        .job_dir(&rt.job)
        .join("scratch")
        .join("config-check");
    let mut groups: Groups<'_> = BTreeMap::new();
    let mut seen: BTreeSet<&CellId> = BTreeSet::new();
    for plan in plans {
        if !seen.insert(&plan.cell) {
            continue;
        }
        if let Some(key) = group_key(rt, plan, &scratch.join("key"))? {
            groups
                .entry(key)
                .or_insert_with(|| (plan, BTreeSet::new()))
                .1
                .insert(plan.cell.clone()); // BORROW: the group owns the ids of its cells
        }
    }
    let _ = fs::remove_dir_all(&scratch); // EXPLICIT: the keys' scratch holds nothing after they are read
    let mut verdict = Ok(());
    for (index, (plan, _)) in groups.values().enumerate() {
        let root = scratch.join(index.to_string()); // BORROW: .to_string() gives owned text
        verdict = check_group(rt, isolation.as_ref(), plan, &root).await;
        let _ = fs::remove_dir_all(&root); // EXPLICIT: the scratch is removed whether or not the check passed; a leftover is swept with the job directory
        if verdict.is_err() {
            break;
        }
    }
    let _ = fs::remove_dir_all(&scratch); // EXPLICIT: the scratch root holds nothing once the groups are done
    verdict?;
    let covered: BTreeSet<CellId> = groups.into_values().flat_map(|(_, cells)| cells).collect();
    for id in covered {
        if let Some(cell) = rt.cells.get_mut(&id) {
            Arc::make_mut(cell).config_clean = true;
        }
    }
    Ok(())
}

/// The key of `plan`'s cell, or `None` when its driver has no check.
fn group_key(
    rt: &Runtime,
    plan: &RunPlan,
    root: &Path,
) -> Result<Option<GroupKey>, ConfigCheckError> {
    let cell = rt.cells.get(&plan.cell).ok_or_else(|| failed(&"no cell"))?;
    let comparable = plan.stored.as_ref().unwrap_or(&cell.comparable);
    let driven = rt
        .driver
        .plan(&Cell::new(comparable.values()), &Workspace::new(root))?;
    let Some(command) = rt.driver.config_check(&driven) else {
        return Ok(None);
    };
    let treatment = serde_json::to_string(comparable.treatment()).map_err(|e| failed(&e))?;
    let mut words = command.argv().to_vec(); // BORROW: the words are hashed with the files
    words.push(treatment);
    Ok(Some((
        rt.driver.id().to_string(), // BORROW: .to_string() gives owned text
        image_of(comparable),
        files_digest(&driven, &words),
    )))
}

/// Runs the harness's check in the container of `plan`'s cell, over a scratch workspace under
/// `root`, and reads its verdict.
async fn check_group(
    rt: &Runtime,
    isolation: &dyn Isolation, // TRAIT_OBJECT: the runtime's isolation, or a fake in tests
    plan: &RunPlan,
    root: &Path,
) -> Result<(), ConfigCheckError> {
    let (staged, container) = stage_container(rt, isolation, plan, root)
        .await
        .map_err(|e| failed(&e))?;
    let Some(command) = rt.driver.config_check(&staged) else {
        return Ok(());
    };
    let output = isolation
        .collect_exec(&container, &command)
        .await
        .map_err(|e| failed(&e))?;
    match rt.driver.read_config_check(&output) {
        CheckVerdict::Clean => Ok(()),
        CheckVerdict::Rejected { output } => {
            let cell = rt.cells.get(&plan.cell).ok_or_else(|| failed(&"no cell"))?;
            let comparable = plan.stored.as_ref().unwrap_or(&cell.comparable);
            let harness = rt.driver.id().to_string(); // BORROW: .to_string() gives owned text
            let (installed, at) = installed_at(&staged);
            Err(DriverError::HarnessRejectedInput {
                path: setting_pointer(&Cell::new(comparable.values()), harness.as_str()), // BORROW: explicit .as_str()
                installed,
                at,
                harness,
                output,
            }
            .into())
        }
        // EXHAUSTIVE: the verdict enum is non-exhaustive; a verdict this core does not know is no clean one
        _ => Err(failed(
            &"the harness gave a verdict this build does not know",
        )),
    }
}
