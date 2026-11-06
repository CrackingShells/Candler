//! Planning the runs of a job: which cells run, with which attempt, and which are already sealed.
//!
//! A plan never removes or renames a run directory. An attempt that never reached `run.done` is
//! sealed as `invalid(interrupted)` and stays; the next attempt takes the next number. A cell that
//! has stored runs is replayed from the stored `Comparable`, never rebuilt from the current files.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use candler_schemas::execution::{
    Comparable, Deviation, ExperimentFile, Integrity, SecretPresence,
};
use candler_schemas::primitives::{Digest, FieldPath, JobId, RunKey};
use candler_schemas::values::{JsonValue, ResolvedValue, SourceEntry, ValueSource};
use rustix::fs::{FlockOperation, flock};
use rustix::io::Errno;
use serde_json::Value;

use crate::experiment::paths::{HARNESS, ISOLATION_IMAGE, TREATMENT};
use crate::jobs::dir::JobDir;
use crate::jobs::lock::{JobLock, LockState};
use crate::resolve::layers::Resolution;
use crate::store::Store;

/// The digest of a cell.
pub type CellId = Digest;

/// The path of the harness version, a re-checked fact.
const HARNESS_VERSION: &str = "harness.version";
/// The path of the secrets, a re-checked fact.
const SECRETS: &str = "secrets";
/// The name of a preflight check that reports whether one secret is present.
const SECRET_CHECK: &str = "secret";

/// What planning reads.
#[derive(Debug, Clone)]
pub struct PlanCtx {
    /// The store.
    pub store: Store,
    /// The path of the experiment's lock file; the caller derives it from the experiment.
    pub experiment_lock: PathBuf,
}

/// Whether a forced plan may accept a differing re-check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Force {
    /// A difference refuses the plan.
    No,
    /// A difference is accepted and recorded as a deviation.
    Yes,
}

/// One run to start.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RunPlan {
    /// The run's key: the cell, the replicate and the attempt.
    pub key: RunKey,
    /// The cell.
    pub cell: CellId,
    /// The comparable record a sealed run of the cell stored, when there is one.
    pub stored: Option<Comparable>,
    /// The attempt number.
    pub attempt: u32,
    /// The deviations a forced plan accepted.
    pub deviations: Vec<Deviation>,
}

/// Whether planning takes the experiment lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlanMode {
    /// Keeps no lock and writes nothing: `candler run` plans this way before it detaches, so a
    /// refusal reaches the terminal. A held lock is still refused, by trying it and letting go.
    Check,
    /// Takes the experiment lock and returns it in the outcome; the supervisor plans this way.
    Hold,
}

/// The lock on an experiment, released when it drops.
#[derive(Debug)]
pub struct ExperimentLock {
    path: PathBuf,
    // The descriptor holds the lock; dropping it releases it.
    _file: File,
}

impl ExperimentLock {
    /// The lock file's path.
    #[must_use]
    pub fn path<'a>(&'a self) -> &'a Path {
        self.path.as_path()
    }
}

/// What planning decided.
#[derive(Debug)]
pub struct PlanOutcome {
    /// The runs to start.
    pub plans: Vec<RunPlan>,
    /// The runs that were already sealed and are skipped.
    pub sealed: Vec<RunKey>,
    /// The experiment lock, held under [`PlanMode::Hold`] and `None` under [`PlanMode::Check`].
    pub lock: Option<ExperimentLock>,
}

/// The paths of a list of deviations, for a message.
fn paths_of(deviations: &[Deviation]) -> String {
    deviations
        .iter()
        .map(|deviation| deviation.path.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Why planning failed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PlanError {
    /// Another supervisor holds the experiment, or a job that owns an unfinished attempt is alive.
    #[error("the experiment is held by another supervisor")]
    ExperimentLocked,
    /// A re-checked fact differs from the stored cell and the plan was not forced.
    #[error("a re-checked fact differs from the stored cell: {}", paths_of(.0))]
    Changed(Vec<Deviation>),
    /// A stored cell of this experiment has no counterpart in the experiment file.
    #[error("the stored cell {cell} has no counterpart in the experiment")]
    StoredCellUnknown {
        /// The digest of the stored cell.
        cell: String,
    },
    /// Two stored cells both answer one cell of the experiment, so none can be replayed.
    #[error("the stored cells {first} and {second} both answer one cell of the experiment")]
    StoredCellAmbiguous {
        /// The digest of the first stored cell.
        first: String,
        /// The digest of the second stored cell.
        second: String,
    },
    /// A cell of the experiment cannot be built from the resolution.
    #[error("a cell cannot be built: {detail}")]
    Cell {
        /// What the cell builder said.
        detail: String,
    },
    /// The store could not be read or sealed.
    #[error("the store failed: {detail}")]
    Store {
        /// What failed.
        detail: String,
    },
    /// The experiment lock could not be tried.
    #[error("the experiment lock failed: {detail}")]
    Lock {
        /// What failed.
        detail: String,
    },
}

/// Wraps a store failure.
fn store_error(error: impl std::fmt::Display) -> PlanError {
    PlanError::Store {
        detail: error.to_string(),
    }
}

/// Wraps a lock failure.
fn lock_error(path: &Path, error: impl std::fmt::Display) -> PlanError {
    PlanError::Lock {
        detail: format!("{}: {error}", path.display()),
    }
}

/// Takes the experiment lock without waiting.
fn take_lock(path: &Path) -> Result<ExperimentLock, PlanError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| lock_error(parent, e))?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| lock_error(path, e))?;
    match flock(&file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(ExperimentLock {
            path: path.to_path_buf(),
            _file: file,
        }),
        Err(Errno::WOULDBLOCK) => Err(PlanError::ExperimentLocked),
        Err(error) => Err(lock_error(path, error)),
    }
}

/// Tries the lock and lets go at once; a lock file that is not there is not held, and none is
/// created.
fn probe_lock(path: &Path) -> Result<(), PlanError> {
    let file = match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(lock_error(path, error)),
    };
    match flock(&file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(()),
        Err(Errno::WOULDBLOCK) => Err(PlanError::ExperimentLocked),
        Err(error) => Err(lock_error(path, error)),
    }
}

/// A cell of the experiment under the fresh resolution.
struct FreshCell {
    /// The values the experiment file gives the cell, after explicit arguments replace theirs.
    effective: Vec<ResolvedValue>,
    /// The cell built from the effective values and everything else the resolution holds.
    comparable: Comparable,
}

/// Reports whether two paths are the same or one lies below the other.
fn overlaps(first: &FieldPath, second: &FieldPath) -> bool {
    first.starts_with(second) || second.starts_with(first)
}

/// Builds every cell of `file` under `resolution`.
fn fresh_cells(
    file: &ExperimentFile,
    resolution: &Resolution,
) -> Result<Vec<FreshCell>, PlanError> {
    let explicit: Vec<&FieldPath> = resolution
        .sources
        .iter()
        .filter(|entry| matches!(entry.source(), ValueSource::ExplicitArgument))
        .map(SourceEntry::path)
        .collect();
    let is_explicit = |path: &FieldPath| explicit.iter().any(|own| overlaps(own, path));
    let mut cells = Vec::new();
    for cell in file.cells() {
        let effective: Vec<ResolvedValue> = cell
            .into_iter()
            .filter(|value| !is_explicit(value.path()))
            .collect();
        let mut values: Vec<ResolvedValue> = resolution
            .values
            .iter()
            .filter(|value| {
                is_explicit(value.path())
                    || !effective
                        .iter()
                        .any(|own| overlaps(own.path(), value.path()))
            })
            // BORROW: .cloned() gives the cell its own values
            .cloned()
            .collect();
        // BORROW: .cloned() gives the cell its own values
        values.extend(effective.iter().cloned());
        // BORROW: explicit .as_slice() instead of Deref coercion
        let comparable =
            Comparable::from_resolved(values.as_slice()).map_err(|e| PlanError::Cell {
                detail: e.to_string(),
            })?;
        cells.push(FreshCell {
            effective,
            comparable,
        });
    }
    Ok(cells)
}

/// What the store keeps of one stored cell: its record, the secrets it saw and its experiment.
struct Stored {
    comparable: Comparable,
    secrets: Vec<SecretPresence>,
    experiment: Digest,
}

/// The stored cells of the store, one per cell digest, read from the last attempt of each.
fn stored_cells(store: &Store) -> Result<Vec<Stored>, PlanError> {
    let mut last: Vec<RunKey> = Vec::new();
    for entry in store.runs().map_err(store_error)? {
        match last.last_mut() {
            Some(held) if held.cell() == entry.key.cell() => *held = entry.key,
            _ => last.push(entry.key),
        }
    }
    let mut stored = Vec::new();
    for key in &last {
        let manifest = store.read_manifest(key).map_err(store_error)?;
        stored.push(Stored {
            // BORROW: .clone() gives the plan its own copy of the stored cell
            comparable: manifest.comparable().clone(),
            // BORROW: .to_vec() copies the stored secrets
            secrets: manifest.secrets().to_vec(),
            // BORROW: .clone() copies the experiment digest
            experiment: manifest.experiment().clone(),
        });
    }
    Ok(stored)
}

/// Reports whether a stored cell holds every value the experiment gives a cell.
fn answers(stored: &Comparable, cell: &FreshCell) -> bool {
    cell.effective
        .iter()
        .all(|value| stored.values().contains(value))
}

/// The stored cell of each cell of the experiment, and a refusal of any stored cell of the same
/// experiment that no cell answers or of two that answer one cell.
fn match_stored<'a>(
    cells: &[FreshCell],
    stored: &'a [Stored],
) -> Result<Vec<Option<&'a Stored>>, PlanError> {
    let mut chosen: Vec<Option<&'a Stored>> = Vec::with_capacity(cells.len());
    let mut used: BTreeSet<usize> = BTreeSet::new();
    for cell in cells {
        let mut found: Option<(usize, &'a Stored)> = None;
        for (index, candidate) in stored.iter().enumerate() {
            if !answers(&candidate.comparable, cell) {
                continue;
            }
            if let Some((_, first)) = found {
                return Err(PlanError::StoredCellAmbiguous {
                    first: first.comparable.digest().to_string(),
                    second: candidate.comparable.digest().to_string(),
                });
            }
            found = Some((index, candidate));
        }
        if let Some((index, candidate)) = found {
            used.insert(index);
            chosen.push(Some(candidate));
        } else {
            chosen.push(None);
        }
    }
    let experiments: BTreeSet<&Digest> = stored
        .iter()
        .enumerate()
        .filter(|(index, _)| used.contains(index))
        .map(|(_, candidate)| &candidate.experiment)
        .collect();
    for (index, candidate) in stored.iter().enumerate() {
        if !used.contains(&index) && experiments.contains(&candidate.experiment) {
            return Err(PlanError::StoredCellUnknown {
                cell: candidate.comparable.digest().to_string(),
            });
        }
    }
    Ok(chosen)
}

/// The JSON a resolution holds at `path`.
fn resolved_json<'a>(resolution: &'a Resolution, path: &str) -> Option<&'a Value> {
    resolution
        .values
        .iter()
        // BORROW: explicit .as_str() instead of Deref coercion
        .find(|value| value.path().as_str() == path)
        .and_then(ResolvedValue::value)
        .map(JsonValue::as_value)
}

/// A deviation at `path` when the two facts differ.
fn differ(path: &str, stored: Option<Value>, observed: Option<Value>) -> Option<Deviation> {
    if stored == observed {
        return None;
    }
    let path: FieldPath = path.parse().ok()?;
    // EXPLICIT: a value that is not a JSON value the schema accepts is recorded as absent
    let wrap = |value: Option<Value>| value.and_then(|v| JsonValue::new(v).ok());
    Some(Deviation::new(path, wrap(stored), wrap(observed)))
}

/// The facts of a stored cell that the fresh resolution no longer holds: the image ID, the
/// harness version and the treatment's tag-to-commit pin, in path order. A deviation holds names
/// and digests, never a secret.
#[must_use]
pub fn check_facts(stored: &Comparable, resolution: &Resolution) -> Vec<Deviation> {
    let stored_image = stored
        .values()
        .iter()
        // BORROW: explicit .as_str() instead of Deref coercion
        .find(|value| value.path().as_str() == ISOLATION_IMAGE)
        .and_then(ResolvedValue::value)
        .map(|json| json.as_value().clone()); // BORROW: the deviation owns the stored value
    let observed_version = resolved_json(resolution, HARNESS)
        .and_then(|harness| harness.get("version"))
        .cloned(); // BORROW: the deviation owns the observed value
    let stored_treatment = serde_json::to_value(stored.treatment()).ok();
    [
        differ(
            HARNESS_VERSION,
            Some(Value::String(stored.harness().version.to_string())),
            observed_version,
        ),
        differ(
            ISOLATION_IMAGE,
            stored_image,
            // BORROW: the deviation owns the observed value
            resolved_json(resolution, ISOLATION_IMAGE).cloned(),
        ),
        differ(
            TREATMENT,
            stored_treatment,
            // BORROW: the deviation owns the observed value
            resolved_json(resolution, TREATMENT).cloned(),
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The deviation of the secrets: one entry at `secrets` that lists, by name, the secrets the
/// stored run saw present and the ones the resolution still finds present, when some are gone.
///
/// The resolution reports a present secret as a passed preflight check named `secret@<NAME>`
/// with each `_` of the name written `-`; a secret with no such check is absent.
#[must_use]
pub fn check_secrets(stored: &[SecretPresence], resolution: &Resolution) -> Option<Deviation> {
    let present: BTreeSet<&str> = resolution
        .preflight
        .iter()
        .flat_map(|report| report.checks.iter())
        .filter(|check| check.passed && check.name.name() == SECRET_CHECK)
        .map(|check| check.name.version())
        .collect();
    let mut was: Vec<String> = Vec::new();
    let mut still: Vec<String> = Vec::new();
    for secret in stored.iter().filter(|secret| secret.present) {
        let name = secret.name.to_string();
        if present.contains(name.replace('_', "-").as_str()) {
            still.push(name.clone()); // BORROW: the list owns the name
        }
        was.push(name);
    }
    was.sort();
    still.sort();
    let names = |list: Vec<String>| Value::Array(list.into_iter().map(Value::String).collect());
    differ(SECRETS, Some(names(was)), Some(names(still)))
}

/// Reports whether `job` still holds its lock.
fn job_is_live(store: &Store, job: &JobId) -> Result<bool, PlanError> {
    let Ok(dir) = JobDir::open(store.layout(), job) else {
        return Ok(false);
    };
    match JobLock::probe(&dir).map_err(store_error)? {
        LockState::Held => Ok(true),
        LockState::Free => Ok(false),
    }
}

/// Seals every unfinished attempt of `cells` as `invalid(interrupted)`; nothing is removed or
/// moved, and a job that still holds its lock refuses the plan before anything is sealed.
fn seal_open(store: &Store, cells: &BTreeSet<Digest>) -> Result<(), PlanError> {
    let open: Vec<RunKey> = store
        .runs()
        .map_err(store_error)?
        .into_iter()
        .filter(|entry| !entry.done && cells.contains(entry.key.cell()))
        .map(|entry| entry.key)
        .collect();
    for key in &open {
        let manifest = store.read_manifest(key).map_err(store_error)?;
        if let Some(job) = manifest.job()
            && job_is_live(store, job)?
        {
            return Err(PlanError::ExperimentLocked);
        }
    }
    for key in &open {
        store.seal_interrupted(key).map_err(store_error)?;
    }
    Ok(())
}

/// Decides the runs of `file` under `resolution`, taking the experiment lock when `mode` is
/// [`PlanMode::Hold`].
///
/// A cell with a stored run is replayed from the stored `Comparable`, and its four re-checked
/// facts (image ID, harness version, secrets present, tag-to-commit) must still hold. Under
/// [`PlanMode::Hold`] every unfinished attempt of the experiment is sealed first; a replicate
/// with a valid sealed attempt is skipped and any other gets the next attempt number.
///
/// # Errors
/// [`PlanError::ExperimentLocked`] when another supervisor holds the experiment,
/// [`PlanError::Changed`] when a re-checked fact differs and `force` is [`Force::No`], and the
/// other [`PlanError`] variants when the cells or the store cannot be read.
pub fn plan_runs(
    ctx: &PlanCtx,
    file: &ExperimentFile,
    resolution: &Resolution,
    force: Force,
    mode: PlanMode,
) -> Result<PlanOutcome, PlanError> {
    let lock = match mode {
        PlanMode::Hold => Some(take_lock(ctx.experiment_lock.as_path())?),
        PlanMode::Check => {
            probe_lock(ctx.experiment_lock.as_path())?;
            None
        }
    };
    let cells = fresh_cells(file, resolution)?;
    let stored = stored_cells(&ctx.store)?;
    let matched = match_stored(cells.as_slice(), stored.as_slice())?;

    let mut digests: BTreeSet<Digest> = BTreeSet::new();
    let mut per_cell: Vec<(Digest, Option<Comparable>, Vec<Deviation>)> = Vec::new();
    let mut changed: Vec<Deviation> = Vec::new();
    for (cell, found) in cells.iter().zip(matched) {
        let (digest, pinned, deviations) = match found {
            Some(held) => {
                let mut deviations = check_facts(&held.comparable, resolution);
                deviations.extend(check_secrets(held.secrets.as_slice(), resolution));
                (
                    held.comparable.digest(),
                    // BORROW: the plan carries the stored cell as written
                    Some(held.comparable.clone()),
                    deviations,
                )
            }
            None => (cell.comparable.digest(), None, Vec::new()),
        };
        for deviation in &deviations {
            if !changed.contains(deviation) {
                // BORROW: the refusal lists each changed fact once
                changed.push(deviation.clone());
            }
        }
        digests.insert(digest.clone()); // BORROW: the set keeps the digest beside the plan
        per_cell.push((digest, pinned, deviations));
    }
    if force == Force::No && !changed.is_empty() {
        return Err(PlanError::Changed(changed));
    }

    if mode == PlanMode::Hold {
        seal_open(&ctx.store, &digests)?;
    }

    let entries = ctx.store.runs().map_err(store_error)?;
    let mut plans = Vec::new();
    let mut sealed = Vec::new();
    for (digest, pinned, deviations) in per_cell {
        for replicate in 1..=file.replicates() {
            let mut valid: Option<RunKey> = None;
            for entry in entries.iter().filter(|entry| {
                entry.done && entry.key.cell() == &digest && entry.key.replicate() == replicate
            }) {
                let manifest = ctx.store.read_manifest(&entry.key).map_err(store_error)?;
                if matches!(manifest.integrity(), Integrity::Valid) {
                    valid = Some(entry.key.clone()); // BORROW: the outcome owns the sealed key
                }
            }
            if let Some(key) = valid {
                sealed.push(key);
                continue;
            }
            let attempt = ctx
                .store
                .next_attempt(&digest, replicate)
                .map_err(store_error)?;
            let key = RunKey::new(digest.clone(), replicate, attempt) // BORROW: the key owns its cell
                .map_err(store_error)?;
            plans.push(RunPlan {
                key,
                cell: digest.clone(),   // BORROW: the plan owns its cell digest
                stored: pinned.clone(), // BORROW: the plan owns the stored cell
                attempt,
                deviations: deviations.clone(), // BORROW: every new attempt owns the deviations
            });
        }
    }
    Ok(PlanOutcome {
        plans,
        sealed,
        lock,
    })
}
