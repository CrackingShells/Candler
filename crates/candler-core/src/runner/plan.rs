//! Planning the runs of a job: which cells run, with which attempt, and which are already sealed.
//!
//! A plan never removes or renames a run directory. An attempt that never reached `run.done` is
//! sealed as `invalid(interrupted)` and stays; the next attempt takes the next number. A cell that
//! has stored runs is replayed from the stored `Comparable`, never rebuilt from the current files.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::spec::Cell;

use candler_schemas::execution::{
    Comparable, Deviation, ExecutionError, ExperimentFile, Factor, Integrity, Level,
    SecretPresence, experiment_digest,
};
use candler_schemas::primitives::{Digest, FieldPath, JobId, RunKey, SecretName};
use candler_schemas::values::{JsonValue, ResolvedValue, SourceEntry, ValueSource};
use rustix::fs::{FlockOperation, flock};
use rustix::io::Errno;
use serde_json::Value;

use crate::experiment::paths::{self, HARNESS, ISOLATION_IMAGE, TREATMENT};
use crate::jobs::dir::JobDir;
use crate::jobs::lock::{JobLock, LockState};
use crate::resolve::layers::{Resolution, is_setting};
use crate::resolve::steps::session::{self, SessionRefusal};
use crate::store::Store;

/// The digest of a cell.
pub type CellId = Digest;

/// The path of the harness version, a re-checked fact.
const HARNESS_VERSION: &str = "harness.version";
/// The path of the secrets, a re-checked fact.
const SECRETS: &str = "secrets";

/// What planning reads.
#[derive(Clone)]
pub struct PlanCtx {
    /// The store.
    pub store: Store,
    /// The path of the experiment's lock file; the caller derives it from the experiment.
    pub experiment_lock: PathBuf,
    /// The secrets that are present now, by name; the caller fills it from the secret resolution
    /// it already does for the gateway token.
    pub secrets_present: BTreeSet<SecretName>,
    /// The harness's driver, which settles whether a cell may run a session and refuses a setting
    /// it cannot honour; `None` when the caller has none, and both are then left unchecked.
    // TRAIT_OBJECT: the binary selects the driver at runtime
    pub driver: Option<Arc<dyn HarnessDriver>>,
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

/// The lock on an experiment, released when it drops. When the plan resumes a stored group under
/// another digest than the fresh one, the group's lock is held with it and released with it.
#[derive(Debug)]
pub struct ExperimentLock {
    path: PathBuf,
    // The descriptor holds the lock; dropping it releases it.
    _file: File,
    // The descriptor of the stored group's lock file, when the plan resumes such a group.
    _group: Option<File>,
}

impl ExperimentLock {
    /// Holds the stored group's lock file with this lock.
    fn with_group(self, group: File) -> Self {
        Self {
            _group: Some(group),
            ..self
        }
    }

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
    /// The experiment's digest: the stored group's when a past experiment answers, else the one
    /// [`experiment_of`] gives.
    pub experiment: Digest,
    /// The cell each plan runs as the current files and resolution build it, by the plan's cell.
    pub fresh: BTreeMap<CellId, Comparable>,
    /// The cells whose `runner.idle_s` is the session rule's default, derived for the cell, by
    /// the cell's id as in [`PlanOutcome::fresh`].
    pub idle_derived: BTreeSet<CellId>,
}

/// The digest of the experiment `file` makes under `resolution`, from its fresh cells.
///
/// # Errors
/// [`PlanError::Cell`] when a cell cannot be built.
pub fn experiment_of(file: &ExperimentFile, resolution: &Resolution) -> Result<Digest, PlanError> {
    let cells = fresh_cells(file, resolution, None)?;
    let digests: Vec<Digest> = cells.iter().map(|cell| cell.comparable.digest()).collect();
    // BORROW: explicit .as_slice() instead of Deref coercion
    Ok(experiment_digest(digests.as_slice()))
}

/// A deviation as `path {stored: <value>, observed: <value>}`, for a message; a value that was
/// not recorded reads `absent`. A deviation holds names and digests, never a secret.
fn describe(deviation: &Deviation) -> String {
    let text = |value: &Option<JsonValue>| {
        value
            .as_ref()
            .map_or_else(|| "absent".to_owned(), |json| json.as_value().to_string()) // BORROW: owned text for the message
    };
    format!(
        "{} {{stored: {}, observed: {}}}",
        deviation.path,
        text(&deviation.stored),
        text(&deviation.observed)
    )
}

/// The deviations of a list, for a message.
fn describe_all(deviations: &[Deviation]) -> String {
    deviations
        .iter()
        .map(describe)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Why planning failed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PlanError {
    /// Two names of one root map to one deviation path, so a deviation there would be ambiguous.
    #[error("the names {first} and {second} both map to one deviation path")]
    NameCollision {
        /// The first name.
        first: String,
        /// The second name.
        second: String,
    },
    /// Another supervisor holds the experiment, or a job that owns an unfinished attempt is alive.
    #[error("the experiment is held by another supervisor")]
    ExperimentLocked,
    /// A re-checked fact differs from the stored cell and the plan was not forced.
    #[error("a re-checked fact differs from the stored cell: {}", describe_all(.0))]
    Changed(Vec<Deviation>),
    /// Some cells of a past experiment answer cells of this one and others answer none, so the
    /// past experiment cannot be replayed.
    #[error(
        "the past experiment {experiment} has stored cells no cell of this experiment answers: {}",
        cells.join(", ")
    )]
    StoredCellUnknown {
        /// The digest of the past experiment.
        experiment: String,
        /// The digests of its stored cells that no cell of this experiment answers.
        cells: Vec<String>,
    },
    /// Two stored cells both answer one cell of the experiment, or two past experiments both
    /// match it, so none can be replayed; one stored cell that answers two cells names itself
    /// twice.
    #[error("the stored cells {first} and {second} both answer one cell of the experiment")]
    StoredCellAmbiguous {
        /// The digest of the first stored cell.
        first: String,
        /// The digest of the second stored cell.
        second: String,
    },
    /// The highest attempt of a cell and replicate is `u32::MAX`, so no next attempt exists.
    #[error("replicate {replicate} of the cell {cell} has no attempt number left")]
    AttemptOverflow {
        /// The digest of the cell.
        cell: String,
        /// The replicate.
        replicate: u32,
    },
    /// A cell's session settings are refused: the mode or the idle is not valid, the idle is set
    /// without session mode, or the driver cannot run a session.
    #[error(transparent)]
    Session(#[from] SessionRefusal),
    /// The harness's driver cannot honour a value of a cell.
    #[error(transparent)]
    Driver(#[from] DriverError),
    /// A factor of the experiment varies a setting, which no cell can vary.
    #[error("the setting {path} cannot be a factor; write it in [fixed]")]
    SettingAsFactor {
        /// The path of the setting the factor varies.
        path: String,
    },
    /// A cell of the experiment cannot be built from the resolution.
    #[error("a cell cannot be built: {source}")]
    Cell {
        /// What the cell builder said.
        #[source]
        source: ExecutionError,
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
    let file = lock_file(path)?;
    Ok(ExperimentLock {
        path: path.to_path_buf(),
        _file: file,
        _group: None,
    })
}

/// Opens the lock file at `path` and takes its lock without waiting.
fn lock_file(path: &Path) -> Result<File, PlanError> {
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
        Ok(()) => Ok(file),
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
    /// The paths the experiment file or an explicit argument sets in the cell.
    declared: Vec<FieldPath>,
    /// The cell built from the effective values and everything else the resolution holds.
    comparable: Comparable,
    /// Whether the session rule derived the cell's `runner.idle_s`.
    idle_derived: bool,
}

/// Reports whether two paths are the same or one lies below the other.
fn overlaps(first: &FieldPath, second: &FieldPath) -> bool {
    first.starts_with(second) || second.starts_with(first)
}

/// The path of the first setting a factor varies: its own path when a value level sets it and that
/// is a setting, or a leaf that a table level fills when that is a setting.
fn setting_of_factor(factor: &Factor) -> Option<String> {
    factor.levels.iter().find_map(|level| {
        if let Level::Fill(table) = level {
            setting_in(factor.path.as_str(), table)
        } else {
            is_setting(&factor.path).then(|| factor.path.to_string())
        }
    })
}

/// The path of the first leaf below `prefix` in `table` that is a setting. A leaf is any value
/// that is not a table.
fn setting_in(prefix: &str, table: &serde_json::Map<String, Value>) -> Option<String> {
    table.iter().find_map(|(key, value)| {
        let path = format!("{prefix}.{key}");
        if let Value::Object(inner) = value {
            setting_in(path.as_str(), inner)
        } else {
            path.parse::<FieldPath>()
                .ok()
                .filter(is_setting)
                .map(|_| path)
        }
    })
}

/// Builds every cell of `file` under `resolution`. A path the file writes that [`is_setting`] names
/// is a setting of the run, which the resolution holds, and no value of a cell.
///
/// # Errors
/// [`PlanError::SettingAsFactor`] when a factor of `file` varies a setting, [`PlanError::Cell`]
/// when a cell cannot be built, [`PlanError::Session`] when a cell breaks the session rule, and
/// [`PlanError::Driver`] when the driver cannot honour a value of a settled cell.
fn fresh_cells(
    file: &ExperimentFile,
    resolution: &Resolution,
    driver: Option<&dyn HarnessDriver>,
) -> Result<Vec<FreshCell>, PlanError> {
    let session_capable = driver.map(|driver| driver.capabilities().session);
    if let Some(path) = file.factors().iter().find_map(setting_of_factor) {
        return Err(PlanError::SettingAsFactor { path });
    }
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
            .filter(|value| !is_explicit(value.path()) && !is_setting(value.path()))
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
        let derived_idle = {
            let at = |path: &str| {
                values
                    .iter()
                    .find(|value| value.path().as_str() == path)
                    .and_then(ResolvedValue::value)
                    .map(JsonValue::as_value)
            };
            let settled = session::settle(
                at(paths::RUNNER_MODE),
                at(paths::RUNNER_IDLE_S),
                session_capable,
            )?;
            settled.derive_idle
        };
        if derived_idle {
            let (path, value) = session::default_idle().map_err(|error| {
                PlanError::Session(SessionRefusal::Refused {
                    check: String::from(session::STEP_NAME),
                    detail: error.to_string(),
                })
            })?;
            values.push(ResolvedValue::recorded(path, value));
        }
        // BORROW: explicit .as_slice() instead of Deref coercion
        let comparable = Comparable::from_resolved(values.as_slice())
            .map_err(|source| PlanError::Cell { source })?;
        let mut declared: Vec<FieldPath> = effective
            .iter()
            .map(|value| value.path().clone()) // BORROW: the cell owns its declared paths
            .collect();
        declared.extend(
            resolution
                .values
                .iter()
                .filter(|value| is_explicit(value.path()))
                .map(|value| value.path().clone()), // BORROW: the cell owns its declared paths
        );
        cells.push(FreshCell {
            declared,
            comparable,
            idle_derived: derived_idle,
        });
    }
    if let Some(driver) = driver {
        for cell in &cells {
            driver.check_settings(&Cell::new(cell.comparable.values()))?;
        }
    }
    Ok(cells)
}

/// What the store keeps of one stored cell: its record and the secrets any attempt saw.
struct Stored {
    comparable: Comparable,
    secrets: Vec<SecretPresence>,
    experiment: Digest,
}

/// The stored cells of the store, grouped by the experiment that wrote them. Each cell's record
/// is read from its last attempt, and its secrets are the names marked present in any attempt.
fn stored_groups(store: &Store) -> Result<Vec<Vec<Stored>>, PlanError> {
    let mut by_cell: BTreeMap<Digest, Vec<RunKey>> = BTreeMap::new();
    for entry in store.runs().map_err(store_error)? {
        by_cell
            .entry(entry.key.cell().clone()) // BORROW: the map owns the cell digest
            .or_default()
            .push(entry.key);
    }
    let mut groups: BTreeMap<Digest, Vec<Stored>> = BTreeMap::new();
    for keys in by_cell.values() {
        let mut secrets: Vec<SecretPresence> = Vec::new();
        let mut last: Option<(&RunKey, RunManifestView)> = None;
        for key in keys {
            let manifest = store.read_manifest(key).map_err(store_error)?;
            for secret in manifest.secrets().iter().filter(|secret| secret.present) {
                if !secrets.iter().any(|seen| seen.name == secret.name) {
                    secrets.push(secret.clone()); // BORROW: the cell owns the names it saw
                }
            }
            let order = (key.replicate(), key.attempt());
            if last
                .as_ref()
                .is_none_or(|(held, _)| order >= (held.replicate(), held.attempt()))
            {
                last = Some((
                    key,
                    RunManifestView {
                        // BORROW: .clone() gives the plan its own copy of the stored cell
                        comparable: manifest.comparable().clone(),
                        // BORROW: .clone() copies the experiment digest
                        experiment: manifest.experiment().clone(),
                    },
                ));
            }
        }
        if let Some((_, view)) = last {
            groups
                .entry(view.experiment.clone()) // BORROW: the map and the cell both keep the digest
                .or_default()
                .push(Stored {
                    comparable: view.comparable,
                    secrets,
                    experiment: view.experiment,
                });
        }
    }
    Ok(groups.into_values().collect())
}

/// The parts of the last manifest of a cell that the plan keeps.
struct RunManifestView {
    comparable: Comparable,
    experiment: Digest,
}

/// Reports whether a stored cell answers a fresh cell: they agree on every path but the facts.
///
/// A path the experiment file or an explicit argument sets must hold the same value in both. A
/// stored value the fresh cell has no counterpart for is a mismatch. Any other fresh value (from
/// a configuration file, a default or a derived step) may differ from the stored one, which is
/// then replayed.
fn answers(stored: &Comparable, cell: &FreshCell) -> bool {
    let fresh = cell.comparable.values();
    let counterpart = |path: &FieldPath| fresh.iter().find(|value| value.path() == path);
    let is_fact = |path: &FieldPath| path.as_str() == ISOLATION_IMAGE; // BORROW: explicit .as_str()
    if stored.harness().name != cell.comparable.harness().name {
        return false;
    }
    for value in stored
        .values()
        .iter()
        .filter(|value| !is_fact(value.path()))
    {
        match counterpart(value.path()) {
            None => return false,
            Some(own) if cell.declared.contains(value.path()) && own != value => return false,
            Some(_) => {}
        }
    }
    cell.declared
        .iter()
        .filter(|path| !is_fact(path))
        .all(|path| {
            stored
                .values()
                .iter()
                .any(|value| value.path() == path && counterpart(path) == Some(value))
        })
}

/// The stored cell that answers each cell of the experiment, taken from the one past experiment
/// whose cells map one to one onto the cells of this one.
///
/// A past experiment none of whose cells answers is another experiment and is ignored.
fn match_stored<'a>(
    cells: &[FreshCell],
    groups: &'a [Vec<Stored>],
) -> Result<Vec<Option<&'a Stored>>, PlanError> {
    let mut chosen: Option<(&'a Stored, Vec<Option<&'a Stored>>)> = None;
    for group in groups {
        let hits: Vec<Vec<usize>> = group
            .iter()
            .map(|candidate| {
                (0..cells.len())
                    .filter(|index| {
                        cells
                            .get(*index)
                            .is_some_and(|cell| answers(&candidate.comparable, cell))
                    })
                    .collect()
            })
            .collect();
        if hits.iter().all(Vec::is_empty) {
            continue;
        }
        let unmatched: Vec<String> = group
            .iter()
            .zip(&hits)
            .filter(|(_, found)| found.is_empty())
            .map(|(candidate, _)| candidate.comparable.digest().to_string())
            .collect();
        if let Some(first) = group.first().filter(|_| !unmatched.is_empty()) {
            return Err(PlanError::StoredCellUnknown {
                experiment: first.experiment.to_string(),
                cells: unmatched,
            });
        }
        let mut slots: Vec<Option<&'a Stored>> = vec![None; cells.len()];
        for (candidate, found) in group.iter().zip(&hits) {
            let digest = candidate.comparable.digest().to_string();
            // BORROW: explicit .as_slice() instead of Deref coercion
            let [index] = found.as_slice() else {
                return Err(PlanError::StoredCellAmbiguous {
                    // BORROW: the error owns the digest text
                    first: digest.clone(),
                    second: digest,
                });
            };
            let Some(slot) = slots.get_mut(*index) else {
                continue;
            };
            if let Some(held) = slot {
                return Err(PlanError::StoredCellAmbiguous {
                    first: held.comparable.digest().to_string(),
                    second: digest,
                });
            }
            *slot = Some(candidate);
        }
        let first = group.first();
        match (chosen.take(), first) {
            (Some((held, _)), Some(next)) => {
                return Err(PlanError::StoredCellAmbiguous {
                    first: held.comparable.digest().to_string(),
                    second: next.comparable.digest().to_string(),
                });
            }
            (None, Some(next)) => chosen = Some((next, slots)),
            (held, None) => chosen = held,
        }
    }
    Ok(chosen.map_or_else(|| vec![None; cells.len()], |(_, slots)| slots))
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

/// A field-path segment for `name`: lower case, with every other character `_`.
fn segment(name: &str) -> String {
    let mut text: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if !text.starts_with(|c: char| c.is_ascii_lowercase()) {
        text.insert(0, 'x');
    }
    text
}

/// Refuses two different names of one root, across `stored` and `fresh`, that map to one
/// deviation path.
fn check_names(stored: &Comparable, fresh: &Comparable) -> Result<(), PlanError> {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for cell in [stored, fresh] {
        let inputs = cell
            .inputs()
            .iter()
            // BORROW: explicit .as_str() instead of Deref coercion
            .map(|r| ("inputs", r.kind.as_str()));
        let environments = cell
            .environments()
            .iter()
            .map(|r| ("environments", r.plugin.name()));
        for (root, name) in inputs.chain(environments) {
            let path = format!("{root}.{}", segment(name));
            match seen.get(&path) {
                Some(first) if first != name => {
                    return Err(PlanError::NameCollision {
                        // BORROW: the error owns both names
                        first: first.clone(),
                        second: name.to_owned(),
                    });
                }
                Some(_) => {}
                None => {
                    // BORROW: the map owns the name
                    seen.insert(path, name.to_owned());
                }
            }
        }
    }
    Ok(())
}

/// The digests of the typed roots that `stored` and `fresh` hold under one name each, as a
/// deviation per name whose digest differs: `inputs.<kind>` and `environments.<plugin>`.
fn check_roots(stored: &Comparable, fresh: &Comparable) -> Vec<Deviation> {
    let digests = |cell: &Comparable| -> BTreeMap<String, String> {
        let inputs = cell
            .inputs()
            .iter()
            // BORROW: explicit .as_str() instead of Deref coercion
            .map(|r| {
                (
                    format!("inputs.{}", segment(r.kind.as_str())),
                    r.digest.to_string(),
                )
            });
        let environments = cell.environments().iter().map(|r| {
            (
                format!("environments.{}", segment(r.plugin.name())),
                r.lock_digest.to_string(),
            )
        });
        inputs.chain(environments).collect()
    };
    let (was, now) = (digests(stored), digests(fresh));
    let paths: BTreeSet<&String> = was.keys().chain(now.keys()).collect();
    paths
        .into_iter()
        .filter_map(|path| {
            let text = |map: &BTreeMap<String, String>| map.get(path).cloned().map(Value::String);
            differ(path.as_str(), text(&was), text(&now)) // BORROW: explicit .as_str()
        })
        .collect()
}

/// The facts of a stored cell that the fresh cell and resolution no longer hold: the image ID,
/// the harness version, the treatment's tag-to-commit pin and the digest of each input and
/// environment, in path order. A deviation holds names and digests, never a secret.
#[must_use]
pub fn check_facts(
    stored: &Comparable,
    fresh: &Comparable,
    resolution: &Resolution,
) -> Vec<Deviation> {
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
    let mut all: Vec<Deviation> = [
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
    .chain(check_roots(stored, fresh))
    .collect();
    // BORROW: explicit .as_str() instead of Deref coercion
    all.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));
    all
}

/// The deviation of the secrets: one entry at `secrets` that lists, by name, the secrets the
/// stored attempts saw present and the ones that are still present, when some are gone.
#[must_use]
pub fn check_secrets(
    stored: &[SecretPresence],
    present: &BTreeSet<SecretName>,
) -> Option<Deviation> {
    let was: BTreeSet<&SecretName> = stored
        .iter()
        .filter(|secret| secret.present)
        .map(|secret| &secret.name)
        .collect();
    let still: BTreeSet<&SecretName> = was
        .iter()
        .copied()
        .filter(|name| present.contains(*name))
        .collect();
    let names = |set: BTreeSet<&SecretName>| {
        Value::Array(
            set.into_iter()
                .map(|n| Value::String(n.to_string()))
                .collect(),
        )
    };
    differ(SECRETS, Some(names(was)), Some(names(still)))
}

/// Reports whether `job` still holds its lock.
fn job_is_live(store: &Store, job: &JobId) -> Result<bool, PlanError> {
    let path = store.layout().job_dir(job);
    match fs::metadata(&path) {
        Ok(_) => {}
        // EXPLICIT: a job whose directory is gone holds no lock
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(store_error(format!("{}: {error}", path.display()))),
    }
    let dir = JobDir::open(store.layout(), job).map_err(store_error)?;
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
    let cells = fresh_cells(file, resolution, ctx.driver.as_deref())?;
    let groups = stored_groups(&ctx.store)?;
    // BORROW: explicit .as_slice() instead of Deref coercion
    let matched = match_stored(cells.as_slice(), groups.as_slice())?;

    let experiment = matched.iter().flatten().next().map_or_else(
        || {
            let all: Vec<Digest> = cells.iter().map(|cell| cell.comparable.digest()).collect();
            // BORROW: explicit .as_slice() instead of Deref coercion
            experiment_digest(all.as_slice())
        },
        // BORROW: the outcome owns the stored group's digest
        |held| held.experiment.clone(),
    );
    // a stored group under another digest than the fresh one has its own lock file
    let fresh_experiment = {
        let all: Vec<Digest> = cells.iter().map(|cell| cell.comparable.digest()).collect();
        // BORROW: explicit .as_slice() instead of Deref coercion
        experiment_digest(all.as_slice())
    };
    let mut lock = lock;
    if experiment != fresh_experiment {
        let path = ctx.store.layout().experiment_lock(&experiment);
        match mode {
            PlanMode::Hold => {
                let group = lock_file(path.as_path())?;
                lock = lock.map(|held| held.with_group(group));
            }
            PlanMode::Check => probe_lock(path.as_path())?,
        }
    }
    let mut fresh: BTreeMap<CellId, Comparable> = BTreeMap::new();
    let mut idle_derived: BTreeSet<CellId> = BTreeSet::new();
    let mut digests: BTreeSet<Digest> = BTreeSet::new();
    let mut per_cell: Vec<(Digest, Option<Comparable>, Vec<Deviation>)> = Vec::new();
    let mut changed: Vec<Deviation> = Vec::new();
    for (cell, found) in cells.iter().zip(matched) {
        let (digest, pinned, deviations) = match found {
            Some(held) => {
                check_names(&held.comparable, &cell.comparable)?;
                let mut deviations = check_facts(&held.comparable, &cell.comparable, resolution);
                // BORROW: explicit .as_slice() instead of Deref coercion
                deviations.extend(check_secrets(held.secrets.as_slice(), &ctx.secrets_present));
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
        // BORROW: the map keeps the digest and the fresh cell beside the plan
        fresh.insert(digest.clone(), cell.comparable.clone());
        if cell.idle_derived {
            idle_derived.insert(digest.clone());
        }
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
            if entries.iter().any(|entry| {
                entry.key.cell() == &digest
                    && entry.key.replicate() == replicate
                    && entry.key.attempt() == attempt
            }) {
                return Err(PlanError::AttemptOverflow {
                    cell: digest.to_string(),
                    replicate,
                });
            }
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
        experiment,
        fresh,
        idle_derived,
    })
}
