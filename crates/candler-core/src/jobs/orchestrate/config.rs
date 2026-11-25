//! The start check of the files Candler installs for a harness: before the job's first run, each
//! distinct set of installed files is given to the harness's own check, in a container without
//! network that is the one a run of the cell gets, and a finding refuses the job.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use candler_harness::agent_config;
use candler_harness::driver::{DriverError, Plan};
use candler_harness::spec::{Cell, CheckVerdict, CommandSpec, EnvValue, FileRoot, PreparedFile};
use candler_schemas::execution::paths;
use candler_schemas::primitives::Digest;
use candler_schemas::process::ExitStatus;
use candler_schemas::refusal::pointer_of;

use super::servers::{check_each, stage_container};
use super::session::image_of;
use crate::isolation::model::ContainerPlan;
use crate::isolation::{ExecOutput, Isolation};
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
    /// The check could not run, or printed nothing a verdict can be read from.
    #[error("the harness check could not run: {detail}")]
    Check {
        /// What went wrong.
        detail: String,
    },
}

/// The harness, the image and the digest of everything the check reads: the cells that share all
/// three share one verdict.
type GroupKey = (String, Option<String>, Digest);

/// A distinct check: the cell it is run for, what that cell was staged as under its scratch root,
/// the check command, and every cell that shares its key.
struct Group<'a> {
    plan: &'a RunPlan,
    staged: Plan,
    container: ContainerPlan,
    command: CommandSpec,
    cells: BTreeSet<CellId>,
}

fn failed(detail: &impl ToString) -> ConfigCheckError {
    ConfigCheckError::Check {
        detail: detail.to_string(), // BORROW: .to_string() gives owned text
    }
}

/// The exit codes of the check's wrapper that say the command did not run: the in-container
/// timeout, docker's own failure, a command that cannot be executed and one that is not found.
const NOT_RUN: [i32; 4] = [124, 125, 126, 127];

/// The bytes the driver reads its verdict from: standard output followed by standard error.
///
/// The exit status does not decide the verdict, as a harness may report a finding by exiting
/// non-zero.
///
/// # Errors
/// [`ConfigCheckError::Check`] when the command did not run (it exited with one of [`NOT_RUN`] or
/// was ended by a signal), when either stream was cut at the capture limit, or when it printed
/// nothing.
fn verdict_bytes(output: ExecOutput) -> Result<Vec<u8>, ConfigCheckError> {
    match output.status {
        ExitStatus::Signal { signal } => {
            return Err(failed(&format!("the check was ended by signal {signal}")));
        }
        ExitStatus::Code { code } if NOT_RUN.contains(&code) => {
            return Err(failed(&format!(
                "the check command did not run: it exited {code}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        ExitStatus::Code { .. } => {}
    }
    if output.truncated {
        return Err(failed(&"the check printed more than the core keeps"));
    }
    let mut bytes = output.stdout;
    bytes.extend_from_slice(&output.stderr);
    if bytes.is_empty() {
        return Err(failed(&"the check printed nothing"));
    }
    Ok(bytes)
}

/// The files `plan` installs, whichever root they are written under.
fn installed_files(plan: &Plan) -> impl Iterator<Item = &PreparedFile> {
    plan.prepared.files.iter().chain(&plan.workspace_files)
}

/// Adds `files` to `bytes`, each with its root, path, mode and bytes, after a tag and a count so
/// that two lists never run into each other.
fn add_files(bytes: &mut Vec<u8>, tag: &str, files: &[PreparedFile]) {
    bytes.extend_from_slice(format!("{tag}\0{}\0", files.len()).as_bytes());
    for file in files {
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
}

/// The digest of what the check reads, which is the one thing the cells of a group share: the
/// files of the agent's home and, apart from them, the read-only files of the workspace, each
/// with its root, path, mode and bytes; the check command's words, working directory and
/// environment overlay; the treatment; and the environment the check container is given.
///
/// The cell's declared inputs are not mounted in the check container, so they are not in the key.
fn files_digest(
    plan: &Plan,
    container: &ContainerPlan,
    command: &CommandSpec,
    treatment: &str,
) -> Digest {
    let mut bytes = Vec::new();
    add_files(&mut bytes, "home", &plan.prepared.files);
    add_files(&mut bytes, "workspace", &plan.workspace_files);
    for word in command.argv() {
        bytes.extend_from_slice(word.as_bytes());
        bytes.push(0);
    }
    bytes.extend_from_slice(format!("cwd\0{:?}\0", command.cwd()).as_bytes());
    for entry in command.env_overlay() {
        let value = match &entry.value {
            EnvValue::Literal(text) => format!("literal\0{text}"),
            EnvValue::Secret(name) => format!("secret\0{name}"),
            // EXHAUSTIVE: the value enum is non-exhaustive; a value this core does not know is hashed by its shape
            other => format!("{other:?}"),
        };
        bytes.extend_from_slice(format!("env\0{}\0{value}\0", entry.name).as_bytes());
    }
    for (name, value) in &container.env {
        bytes.extend_from_slice(format!("container-env\0{name}\0{value}\0").as_bytes());
    }
    bytes.extend_from_slice(treatment.as_bytes());
    Digest::of_bytes(&bytes)
}

/// The pointer into the experiment file of the setting the installed files come from.
fn setting_pointer(view: &Cell<'_>, harness: &str) -> String {
    // EXPLICIT: a value the reader refuses was refused when the runs were planned
    if agent_config::hooks::<serde_json::Value>(view, harness)
        .ok()
        .flatten()
        .is_some()
    {
        pointer_of(["fixed", "hooks"])
    } else if let Some(found) = agent_config::instructions(view, harness).ok().flatten() {
        let key = if found.workspace.is_some() {
            paths::INSTRUCTIONS_WORKSPACE
        } else {
            paths::INSTRUCTIONS_GLOBAL
        };
        pointer_of(["fixed", key])
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
/// The check command's standard output followed by its standard error is the driver's to read,
/// whatever the exit status; see [`verdict_bytes`] for when the check did not run.
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
    let groups = match grouped(rt, isolation.as_ref(), plans, &scratch).await {
        Ok(groups) => groups,
        Err(error) => {
            let _ = fs::remove_dir_all(&scratch); // EXPLICIT: the staged cells are of no use once the check is refused
            return Err(error);
        }
    };
    let mut covered: BTreeSet<CellId> = BTreeSet::new();
    let mut ordered = Vec::with_capacity(groups.len());
    for (root, group) in groups.into_values() {
        covered.extend(group.cells.iter().cloned()); // BORROW: the covered set owns the ids
        ordered.push((root, group));
    }
    let (reader, held) = (&*rt, isolation.as_ref()); // BORROW: the checks read the runtime
    check_each(&scratch, ordered, |group, _root| async move {
        check_group(reader, held, &group).await
    })
    .await?;
    for id in covered {
        if let Some(cell) = rt.cells.get_mut(&id) {
            Arc::make_mut(cell).config_clean = true;
        }
    }
    Ok(())
}

/// Stages each distinct cell of `plans` once, under a root of its own below `scratch`, and
/// groups the cells whose staged files, check and container agree. The first cell of a group
/// keeps its staging and its root; the root of any other is removed. A cell whose driver has no
/// check is not staged further.
///
/// # Errors
/// [`ConfigCheckError::Check`] when a cell cannot be staged.
async fn grouped<'a>(
    rt: &Runtime,
    isolation: &dyn Isolation, // TRAIT_OBJECT: the runtime's isolation, or a fake in tests
    plans: &'a [RunPlan],
    scratch: &Path,
) -> Result<BTreeMap<GroupKey, (PathBuf, Group<'a>)>, ConfigCheckError> {
    let mut groups: BTreeMap<GroupKey, (PathBuf, Group<'a>)> = BTreeMap::new();
    let mut seen: BTreeSet<&CellId> = BTreeSet::new();
    for plan in plans {
        if !seen.insert(&plan.cell) {
            continue;
        }
        let root = scratch.join(seen.len().to_string()); // BORROW: .to_string() gives owned text
        let (staged, mut container) = stage_container(rt, isolation, plan, &root, false)
            .await
            .map_err(|e| failed(&e))?;
        let Some(command) = rt.driver.config_check(&staged) else {
            let _ = fs::remove_dir_all(&root); // EXPLICIT: a cell with no check keeps nothing of its staging
            continue;
        };
        // The check's own variables win over the container's, as the isolation refuses a name
        // set twice: a state variable of the run, which the check places elsewhere, gives way.
        container.env.retain(|(name, _)| {
            !command
                .env_overlay()
                .iter()
                .any(|entry| entry.name == *name)
        });
        let key = group_key(rt, plan, &staged, &container, &command)?;
        match groups.entry(key) {
            Entry::Occupied(mut held) => {
                held.get_mut().1.cells.insert(plan.cell.clone()); // BORROW: the group owns the ids of its cells
                let _ = fs::remove_dir_all(&root); // EXPLICIT: the group's first cell is the one that is checked
            }
            Entry::Vacant(slot) => {
                let cells = BTreeSet::from([plan.cell.clone()]); // BORROW: the group owns the ids of its cells
                slot.insert((
                    root,
                    Group {
                        plan,
                        staged,
                        container,
                        command,
                        cells,
                    },
                ));
            }
        }
    }
    Ok(groups)
}

/// The key of `plan`'s cell, from what was staged for it.
fn group_key(
    rt: &Runtime,
    plan: &RunPlan,
    staged: &Plan,
    container: &ContainerPlan,
    command: &CommandSpec,
) -> Result<GroupKey, ConfigCheckError> {
    let cell = rt.cells.get(&plan.cell).ok_or_else(|| failed(&"no cell"))?;
    let comparable = plan.stored.as_ref().unwrap_or(&cell.comparable);
    let treatment = serde_json::to_string(comparable.treatment()).map_err(|e| failed(&e))?;
    Ok((
        rt.driver.id().to_string(), // BORROW: .to_string() gives owned text
        image_of(comparable),
        files_digest(staged, container, command, &treatment),
    ))
}

/// Runs the harness's check of `group` in the container its first cell was staged as, and reads
/// its verdict.
async fn check_group(
    rt: &Runtime,
    isolation: &dyn Isolation, // TRAIT_OBJECT: the runtime's isolation, or a fake in tests
    group: &Group<'_>,
) -> Result<(), ConfigCheckError> {
    let ran = isolation
        .collect_exec(&group.container, &group.command)
        .await
        .map_err(|e| failed(&e))?;
    let output = verdict_bytes(ran)?;
    match rt.driver.read_config_check(&output) {
        CheckVerdict::Clean => Ok(()),
        CheckVerdict::Rejected { output } => {
            let cell = rt
                .cells
                .get(&group.plan.cell)
                .ok_or_else(|| failed(&"no cell"))?;
            let comparable = group.plan.stored.as_ref().unwrap_or(&cell.comparable);
            let harness = rt.driver.id().to_string(); // BORROW: .to_string() gives owned text
            let (installed, at) = installed_at(&group.staged);
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

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use candler_schemas::primitives::FieldPath;
    use candler_schemas::values::{JsonValue, ResolvedValue};
    use serde_json::{Value, json};

    use super::*;

    fn resolved(path: &str, value: Value) -> ResolvedValue {
        ResolvedValue::recorded(
            FieldPath::from_str(path).expect("a field path"),
            JsonValue::new(value).expect("a JSON value"),
        )
    }

    /// Guards: the job-start check points at the instruction setting the way the declaration's
    /// own refusals do, `/fixed/instructions.workspace` and `/fixed/instructions.global`, never
    /// at `/fixed/instructions`, which is no key of the file. Oracle: the pointers the declaration
    /// refuses with, spelled by hand. Bite: point at `/fixed/instructions`.
    #[test]
    fn the_instruction_setting_is_pointed_at_as_the_declaration_spells_it() {
        let both = [
            resolved("instructions.workspace", json!("be brief")),
            resolved("instructions.global", json!({"claude-code": "be kind"})),
        ];
        assert_eq!(
            setting_pointer(&Cell::new(&both), "claude-code"),
            "/fixed/instructions.workspace"
        );
        let global = [resolved(
            "instructions.global",
            json!({"claude-code": "be kind"}),
        )];
        assert_eq!(
            setting_pointer(&Cell::new(&global), "claude-code"),
            "/fixed/instructions.global"
        );
    }
}
