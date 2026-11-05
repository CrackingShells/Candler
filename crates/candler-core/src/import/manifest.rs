//! Mapping legacy runs to run manifests and integrity: the cell a run belongs to, its identity, its timing and its legacy verdict become one run manifest, and the folder's pinned evaluator names the request the run's outcome file gets.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr as _;

use candler_schemas::execution::{
    Comparable, DigestRef, GitCommit, HarnessIdentity, ImportRecord, ImportSource, ImportedRequest,
    Integrity, InvalidReason, RunManifest, TreatmentRef, experiment_digest, varying_paths,
};
use candler_schemas::primitives::{
    Digest, FieldPath, InstrumentDigest, Name, NameVersion, RelPath, RequestId, RunKey, Timestamp,
};
use candler_schemas::values::{JsonValue, MeasuredValue, ResolvedValue, SourceEntry, ValueSource};
use serde_json::Value;
use sha2::Digest as _;

use super::ImportError;
use super::read::{LegacyCell, LegacyFolder, LegacyRun, rel_path};
use super::rules::ImportRules;

/// A legacy run mapped to its manifest: the key, the manifest and the run's index in [`LegacyFolder::runs`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MappedRun {
    /// The key of the run: its cell, its run number and attempt 1.
    pub key: RunKey,
    /// The run's manifest.
    pub manifest: RunManifest,
    /// The index of the run in [`LegacyFolder::runs`].
    pub run: usize,
}

/// The evaluator the folder was regraded with and when it finished, as its provenance file pins them.
#[derive(Debug, Clone)]
pub(super) struct Pinned {
    /// The evaluator, as `name@version`.
    pub(super) evaluator: NameVersion,
    /// The digest of the evaluator's instrument: `git:` and the commit it ran at.
    pub(super) instrument: InstrumentDigest,
    /// Whether the evaluator's checkout had uncommitted changes.
    pub(super) dirty: bool,
    /// When the regrade finished.
    pub(super) finished: Timestamp,
}

/// An import error that names `file` of `run` and says `detail`.
pub(super) fn layout(file: &str, run: &str, detail: impl Into<String>) -> ImportError {
    ImportError::Layout {
        file: file.into(),
        run: run.into(),
        detail: detail.into(),
    }
}

/// The string at `pointer` of the folder's provenance file.
fn provenance_text<'a>(folder: &'a LegacyFolder, pointer: &str) -> Result<&'a str, ImportError> {
    folder
        .provenance
        .value
        .pointer(pointer)
        // BORROW: Value::as_str borrows the JSON string without copying
        .and_then(Value::as_str)
        .ok_or_else(|| {
            layout(
                // BORROW: explicit .as_str() instead of Deref coercion
                folder.provenance.path.as_str(),
                "the folder",
                format!("`{pointer}` is not a string"),
            )
        })
}

/// The evaluator, its instrument and finish time the folder's provenance file pins.
pub(super) fn pinned(folder: &LegacyFolder) -> Result<Pinned, ImportError> {
    // BORROW: explicit .as_str() instead of Deref coercion
    let file = folder.provenance.path.as_str();
    let evaluator = NameVersion::from_str(provenance_text(folder, "/pinned/describe")?)?;
    let instrument = InstrumentDigest::from_str(&format!(
        "git:{}",
        provenance_text(folder, "/pinned/commit")?
    ))?;
    let dirty = folder
        .provenance
        .value
        .pointer("/pinned/dirty")
        .and_then(Value::as_bool)
        .ok_or_else(|| layout(file, "the folder", "`/pinned/dirty` is not a boolean"))?;
    let finished = Timestamp::from_str(provenance_text(folder, "/regraded_at/finished")?)?;
    Ok(Pinned {
        evaluator,
        instrument,
        dirty,
        finished,
    })
}

/// Collects the leaves under `value`: objects are descended, anything else, an array included, is a leaf.
fn leaves(prefix: &str, value: &Value, out: &mut Vec<(String, Value)>) {
    if let Value::Object(map) = value {
        for (key, child) in map {
            leaves(&format!("{prefix}.{key}"), child, out);
        }
    } else {
        // BORROW: .to_owned() and .clone() make the owned leaf
        out.push((prefix.to_owned(), value.clone()));
    }
}

/// What every run of one cell directory shares: the comparable, the experiment file's path in the cell and the `provenance.*` leaves.
struct CellModel {
    comparable: Comparable,
    experiment: RelPath,
    provenance_leaves: Vec<(String, Value)>,
}

/// The `provenance.*` leaves of `experiment.json` and all the leaves a cell digest holds.
type Leaves = (Vec<(String, Value)>, Vec<(String, Value)>);

/// Reads the leaves of `experiment.json` under `plan.*` (without `plan.out_dir`), `provenance.*` and `resolved_skill_version`.
fn experiment_leaves(cell: &LegacyCell) -> Result<Leaves, ImportError> {
    // BORROW: explicit .as_str() instead of Deref coercion
    let file = cell.experiment.path.as_str();
    let doc = &cell.experiment.value;
    let mut plan = Vec::new();
    let mut provenance = Vec::new();
    for (root, out) in [("plan", &mut plan), ("provenance", &mut provenance)] {
        let part = doc
            .get(root)
            .filter(|part| part.is_object())
            .ok_or_else(|| layout(file, &cell.name, format!("`{root}` is not an object")))?;
        leaves(root, part, out);
    }
    plan.retain(|(path, _)| path != "plan.out_dir" && !path.starts_with("plan.out_dir."));
    let skill = doc
        .get("resolved_skill_version")
        .ok_or_else(|| layout(file, &cell.name, "`resolved_skill_version` is missing"))?;
    let mut all = plan;
    // BORROW: .clone() copies the provenance leaves; the cell digest holds them too
    all.extend(provenance.clone());
    // BORROW: .to_owned() and .clone() make the owned leaf
    all.push(("resolved_skill_version".to_owned(), skill.clone()));
    Ok((provenance, all))
}

/// The `copied_inputs` of a run's timing file as digest references.
fn copied_inputs(run: &LegacyRun) -> Result<BTreeSet<DigestRef>, ImportError> {
    // BORROW: explicit .as_str() instead of Deref coercion
    let file = run.timing.path.as_str();
    let mut inputs = BTreeSet::new();
    let Some(list) = run.timing.value.get("copied_inputs") else {
        return Ok(inputs);
    };
    let items = list
        .as_array()
        .ok_or_else(|| layout(file, &run.label, "`copied_inputs` is not an array"))?;
    for item in items {
        let text = |key: &str| {
            // BORROW: Value::as_str borrows the JSON string without copying
            item.get(key).and_then(Value::as_str).ok_or_else(|| {
                layout(
                    file,
                    &run.label,
                    format!("a copied input has no string `{key}`"),
                )
            })
        };
        let kind = Name::from_str(text("role")?)?;
        let digest = Digest::from_hex(text("sha256")?)?;
        inputs.insert(DigestRef::new(kind, digest));
    }
    Ok(inputs)
}

/// Builds what every run of `cell` shares.
fn cell_model(
    folder: &LegacyFolder,
    rules: &ImportRules,
    cell: &LegacyCell,
) -> Result<CellModel, ImportError> {
    // BORROW: explicit .as_str() instead of Deref coercion
    let experiment_file = cell.experiment.path.as_str();
    let experiment = rel_path(cell.relative(&cell.experiment.path).ok_or_else(|| {
        layout(
            experiment_file,
            &cell.name,
            "the experiment file is outside the cell",
        )
    })?)?;
    let (provenance_leaves, all) = experiment_leaves(cell)?;
    let mut builder = Comparable::builder();
    for (path, value) in all {
        let path = FieldPath::from_str(&path)?;
        builder = builder.value(if value.is_null() {
            ResolvedValue::unrecorded(path)
        } else {
            let json = JsonValue::new(value)
                .map_err(|error| layout(experiment_file, &cell.name, error.to_string()))?;
            ResolvedValue::recorded(path, json)
        });
    }
    let mut later = cell.runs.iter();
    let first = later
        .next()
        .ok_or_else(|| layout(experiment_file, &cell.name, "the cell has no run"))?;
    for run in later {
        if run.harness_version != first.harness_version {
            return Err(ImportError::Disagree {
                what: "harness versions",
                // BORROW: .clone() copies the value into the owned result
                run: cell.name.clone(),
                first: first.stream.path.to_string(),
                second: run.stream.path.to_string(),
                detail: format!("`{}` and `{}`", first.harness_version, run.harness_version),
            });
        }
    }
    let version = semver::Version::parse(&first.harness_version).map_err(|error| {
        layout(
            // BORROW: explicit .as_str() instead of Deref coercion
            first.stream.path.as_str(),
            &first.label,
            format!(
                "harness version `{}` is not semver ({error})",
                first.harness_version
            ),
        )
    })?;
    // BORROW: .clone() copies the harness name into the cell
    builder = builder.harness(HarnessIdentity::new(rules.harness.name.clone(), version));
    // BORROW: explicit .as_str() instead of Deref coercion
    let pointer = rules.treatment.pointer.as_str();
    let treatment = cell
        .experiment
        .value
        .pointer(pointer)
        // BORROW: Value::as_str borrows the JSON string without copying
        .and_then(Value::as_str)
        .and_then(|text| text.split_once('@'))
        .ok_or_else(|| {
            layout(
                experiment_file,
                &cell.name,
                format!("`{pointer}` is not a string written `<name>@<tag>`"),
            )
        })?;
    let mut treatment_ref = TreatmentRef::new(Name::from_str(treatment.0)?);
    // BORROW: .to_owned() makes the owned tag
    treatment_ref.tag = Some(treatment.1.to_owned());
    builder = builder.treatment(treatment_ref);
    for rule in &rules.snapshots {
        let hex = provenance_text(folder, &rule.pointer)?;
        // BORROW: .clone() copies the snapshot kind into the cell
        builder = builder.snapshot(DigestRef::new(rule.kind.clone(), Digest::from_hex(hex)?));
    }
    let first_inputs = copied_inputs(first)?;
    for run in cell.runs.iter().skip(1) {
        let inputs = copied_inputs(run)?;
        if inputs != first_inputs {
            return Err(ImportError::Disagree {
                what: "copied inputs",
                // BORROW: .clone() copies the value into the owned result
                run: cell.name.clone(),
                first: first.timing.path.to_string(),
                second: run.timing.path.to_string(),
                detail: format!("{} and {} inputs differ", first_inputs.len(), inputs.len()),
            });
        }
    }
    for input in first_inputs {
        builder = builder.input(input);
    }
    let comparable = builder
        .build()
        .map_err(|error| layout(experiment_file, &cell.name, error.to_string()))?;
    Ok(CellModel {
        comparable,
        experiment,
        provenance_leaves,
    })
}

/// The legacy verdict of a run: `PROVENANCE.json`'s, checked against the cell's contamination catalog, and the legacy integrity block.
fn integrity_of(
    folder: &LegacyFolder,
    cell: &LegacyCell,
    run: &LegacyRun,
) -> Result<(Integrity, Value), ImportError> {
    // BORROW: explicit .as_str() instead of Deref coercion
    let file = folder.provenance.path.as_str();
    let wanted = format!("run-{}", run.number);
    let entry = folder
        .provenance
        .value
        .get("runs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|entry| {
            // BORROW: Value::as_str borrows the JSON string without copying
            entry.get("cell").and_then(Value::as_str) == Some(cell.name.as_str())
                // BORROW: Value::as_str borrows the JSON string without copying
                && entry.get("config").and_then(Value::as_str) == Some(run.config.as_str())
                // BORROW: Value::as_str borrows the JSON string without copying
                && entry.get("run").and_then(Value::as_str) == Some(wanted.as_str())
        })
        .ok_or_else(|| layout(file, &run.label, "no `runs` entry names this run"))?;
    let original = entry
        .get("original_integrity")
        .filter(|block| block.is_object())
        .ok_or_else(|| layout(file, &run.label, "`original_integrity` is not an object"))?;
    let verdict = original
        .get("verdict")
        // BORROW: Value::as_str borrows the JSON string without copying
        .and_then(Value::as_str)
        .ok_or_else(|| {
            layout(
                file,
                &run.label,
                "`original_integrity.verdict` is not a string",
            )
        })?;
    let integrity = match verdict {
        "clean" | "incomplete" => Integrity::Valid,
        "contaminated" => Integrity::invalid(InvalidReason::InputLeak),
        other => {
            return Err(ImportError::Verdict {
                file: file.into(),
                // BORROW: .clone() copies the value into the owned result
                run: run.label.clone(),
                verdict: other.into(),
            });
        }
    };
    let row = cell.catalog.iter().find(|row| {
        // BORROW: Value::as_str borrows the JSON string without copying
        row.get("config").and_then(Value::as_str) == Some(run.config.as_str())
            && row.get("run_number").and_then(Value::as_u64) == Some(u64::from(run.number))
    });
    let catalog = row
        .and_then(|row| row.get("verdict"))
        // BORROW: Value::as_str borrows the JSON string without copying
        .and_then(Value::as_str)
        .unwrap_or("clean");
    if catalog != verdict {
        return Err(ImportError::Disagree {
            what: "integrity verdicts",
            // BORROW: .clone() copies the value into the owned result
            run: run.label.clone(),
            first: file.into(),
            second: format!("{}/contamination_catalog.jsonl", cell.name),
            detail: format!("`{verdict}` and `{catalog}`"),
        });
    }
    // BORROW: .clone() copies the legacy integrity block into the import record
    Ok((integrity, original.clone()))
}

/// The `elapsed_seconds` of the first progress event of `kind` for `run`.
fn elapsed(cell: &LegacyCell, run: &LegacyRun, kind: &str) -> Option<f64> {
    cell.progress
        .iter()
        .find(|event| {
            // BORROW: Value::as_str borrows the JSON string without copying
            event.get("kind").and_then(Value::as_str) == Some(kind)
                && event.get("run_number").and_then(Value::as_u64) == Some(u64::from(run.number))
                // BORROW: Value::as_str borrows the JSON string without copying
                && event.get("config").and_then(Value::as_str) == Some(run.config.as_str())
        })
        .and_then(|event| event.get("elapsed_seconds"))
        .and_then(Value::as_f64)
}

/// Whole milliseconds in `seconds`, rounded.
#[allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    reason = "a run lasts hours, far inside the range of i64 milliseconds"
)]
fn millis(seconds: f64) -> i64 {
    // CAST: f64 -> i64, rounded; the duration of a run is finite and far below 2^63 ms
    (seconds * 1000.0).round() as i64
}

/// A run's start and end, from its completion file and the cell's progress events; the start is absent without both events, and both are absent without the completion file.
fn timing_of(
    cell: &LegacyCell,
    run: &LegacyRun,
) -> Result<(Option<Timestamp>, Option<Timestamp>), ImportError> {
    let Some(complete) = &run.run_complete else {
        return Ok((None, None));
    };
    // BORROW: explicit .as_str() instead of Deref coercion
    let file = complete.path.as_str();
    let text = complete
        .value
        .get("completed_at")
        // BORROW: Value::as_str borrows the JSON string without copying
        .and_then(Value::as_str)
        .ok_or_else(|| layout(file, &run.label, "`completed_at` is not a string"))?;
    let ended = Timestamp::from_str(text)?;
    let (Some(start), Some(end)) = (
        elapsed(cell, run, "run_start"),
        elapsed(cell, run, "run_complete"),
    ) else {
        return Ok((None, Some(ended)));
    };
    let lasted = millis(end - start);
    if lasted < 0 {
        return Err(layout(
            file,
            &run.label,
            "the run completed before it started",
        ));
    }
    let started = ended
        .instant()
        .checked_sub(jiff::SignedDuration::from_millis(lasted))
        .map_err(|error| layout(file, &run.label, error.to_string()))?;
    Ok((
        Some(Timestamp::from_str(&started.to_string())?),
        Some(ended),
    ))
}

/// The deterministic request id of the outcome file of `key` written by `evaluator`: a `UUIDv7` whose time is the whole seconds of `finished` and whose random bits are the first 74 bits of the SHA-256 of the run key and the evaluator.
pub(super) fn request_id(
    key: &RunKey,
    evaluator: &NameVersion,
    finished: &Timestamp,
) -> Result<RequestId, ImportError> {
    let seconds = u64::try_from(finished.instant().as_second()).map_err(|error| {
        layout(
            "PROVENANCE.json",
            "the folder",
            format!("the regrade finished before 1970 ({error})"),
        )
    })?;
    let mut hasher = sha2::Sha256::new();
    hasher.update(key.to_string().as_bytes());
    hasher.update([0u8]);
    hasher.update(evaluator.to_string().as_bytes());
    let bits = hasher
        .finalize()
        .iter()
        .take(10)
        .fold(0u128, |acc, byte| (acc << 8) | u128::from(*byte))
        >> 6;
    let rand_a = (bits >> 62) & 0xfff;
    let rand_b = bits & ((1u128 << 62) - 1);
    let time = u128::from(seconds) * 1000;
    let uuid = (time << 80) | (0x7 << 76) | (rand_a << 64) | (0b10 << 62) | rand_b;
    Ok(RequestId::from_str(
        &uuid::Uuid::from_u128(uuid).hyphenated().to_string(),
    )?)
}

/// The `provenance.*` values the run measured that `experiment.json` holds as `null`, each with the metadata copy it came from.
fn measured_of(
    cell: &LegacyCell,
    run: &LegacyRun,
    model: &CellModel,
) -> Result<Vec<MeasuredValue>, ImportError> {
    let nulls: BTreeSet<&str> = model
        .provenance_leaves
        .iter()
        .filter(|(_, value)| value.is_null())
        // BORROW: explicit .as_str() instead of Deref coercion
        .map(|(path, _)| path.as_str())
        .collect();
    let mut seen: BTreeMap<String, (Value, &RelPath)> = BTreeMap::new();
    for copy in &run.eval_metadata {
        let mut found = Vec::new();
        if let Some(block) = copy.value.get("provenance") {
            leaves("provenance", block, &mut found);
        }
        for (path, value) in found {
            // BORROW: explicit .as_str() instead of Deref coercion
            if value.is_null() || !nulls.contains(path.as_str()) {
                continue;
            }
            match seen.get(&path) {
                Some((first, first_path)) if *first != value => {
                    return Err(ImportError::Disagree {
                        what: "eval_metadata provenance copies",
                        // BORROW: .clone() copies the value into the owned result
                        run: run.label.clone(),
                        first: first_path.to_string(),
                        second: copy.path.to_string(),
                        detail: format!("`{path}` is `{first}` and `{value}`"),
                    });
                }
                Some(_) => {}
                None => {
                    seen.insert(path, (value, &copy.path));
                }
            }
        }
    }
    let mut measured = Vec::new();
    for (path, (value, from)) in seen {
        let file = cell.relative(from).ok_or_else(|| {
            // BORROW: explicit .as_str() instead of Deref coercion
            layout(
                // BORROW: explicit .as_str() instead of Deref coercion
                from.as_str(),
                &run.label,
                "the metadata copy is outside the cell",
            )
        })?;
        let json = JsonValue::new(value)
            // BORROW: explicit .as_str() instead of Deref coercion
            .map_err(|error| layout(from.as_str(), &run.label, error.to_string()))?;
        measured.push(MeasuredValue::new(
            FieldPath::from_str(&path)?,
            json,
            ValueSource::imported(rel_path(file)?),
        ));
    }
    Ok(measured)
}

/// Adds a source to the list of a run's sources, refusing a path that is already in it.
fn add_source<'a>(
    by_path: &mut BTreeMap<String, &'a Digest>,
    path: String,
    sha256: &'a Digest,
    run: &LegacyRun,
) -> Result<(), ImportError> {
    if by_path.contains_key(&path) {
        return Err(layout(&path, &run.label, "two sources have the same path"));
    }
    by_path.insert(path, sha256);
    Ok(())
}

/// The files a run's manifest records as read, each with its digest: the run's and its cell's by cell-relative path, then the folder's by its own, sorted by path.
fn import_sources(
    folder: &LegacyFolder,
    cell: &LegacyCell,
    run: &LegacyRun,
) -> Result<Vec<ImportSource>, ImportError> {
    let mut by_path: BTreeMap<String, &Digest> = BTreeMap::new();
    for file in run.sources.iter().chain(&cell.sources) {
        let relative = cell.relative(&file.path).ok_or_else(|| {
            // BORROW: explicit .as_str() instead of Deref coercion
            layout(
                // BORROW: explicit .as_str() instead of Deref coercion
                file.path.as_str(),
                &run.label,
                "the source is outside the cell",
            )
        })?;
        // BORROW: .to_owned() makes the owned copy
        add_source(&mut by_path, relative.to_owned(), &file.sha256, run)?;
    }
    for file in &folder.sources {
        add_source(&mut by_path, file.path.to_string(), &file.sha256, run)?;
    }
    by_path
        .into_iter()
        // BORROW: .clone() copies the digest into the source list
        .map(|(path, sha256)| Ok(ImportSource::new(rel_path(&path)?, sha256.clone())))
        .collect()
}

/// Maps every run of the folder to its manifest, in the order of [`LegacyFolder::runs`].
///
/// A run's manifest holds the cell that its `experiment.json` and the folder's provenance describe, the run key with attempt 1, the legacy verdict as integrity, the measured values the cell left `null`, the run's start and end, and an import record naming every file read. A run that is not contaminated also names the request id its outcome file gets.
///
/// # Errors
/// Returns [`ImportError::Verdict`] for a verdict other than `clean`, `incomplete` or
/// `contaminated`, [`ImportError::Disagree`] for files that state different things about one run
/// or cell, [`ImportError::Layout`] for a file that lacks what the mapping reads, and
/// [`ImportError::Format`] for a value that is not the primitive it must be.
pub fn map_runs(folder: &LegacyFolder, rules: &ImportRules) -> Result<Vec<MappedRun>, ImportError> {
    let pinned = pinned(folder)?;
    let base_commit = GitCommit::from_str(provenance_text(folder, "/corpus/base_commit")?)
        // BORROW: explicit .as_str() instead of Deref coercion
        .map_err(|error| {
            layout(
                // BORROW: explicit .as_str() instead of Deref coercion
                folder.provenance.path.as_str(),
                "the folder",
                error.to_string(),
            )
        })?;
    let mut models = Vec::with_capacity(folder.cells.len());
    for cell in &folder.cells {
        models.push(cell_model(folder, rules, cell)?);
    }
    let lists: Vec<Vec<ResolvedValue>> = models
        .iter()
        // BORROW: .to_vec() copies the cell's values for the comparison across cells
        .map(|model| model.comparable.values().to_vec())
        .collect();
    let factors = varying_paths(&lists);
    let digests: Vec<Digest> = models
        .iter()
        .map(|model| model.comparable.digest())
        .collect();
    let experiment = experiment_digest(&digests);
    let harness = FieldPath::from_str("harness")?;
    let treatment = FieldPath::from_str("treatment")?;
    let snapshots = FieldPath::from_str("snapshots")?;
    let inputs = FieldPath::from_str("inputs")?;
    let mut mapped = Vec::new();
    for (cell, model) in folder.cells.iter().zip(&models) {
        for run in &cell.runs {
            let key = RunKey::new(model.comparable.digest(), run.number, 1)?;
            let relative = |path: &RelPath| -> Result<RelPath, ImportError> {
                rel_path(cell.relative(path).ok_or_else(|| {
                    // BORROW: explicit .as_str() instead of Deref coercion
                    layout(path.as_str(), &run.label, "the file is outside the cell")
                })?)
            };
            let (integrity, legacy) = integrity_of(folder, cell, run)?;
            let (started, ended) = timing_of(cell, run)?;
            // BORROW: .clone() copies the key, the digest and the cell into the manifest
            let mut builder = RunManifest::builder()
                // BORROW: .clone() copies the value into the owned result
                .run(key.clone())
                // BORROW: .clone() copies the value into the owned result
                .experiment(experiment.clone())
                // BORROW: .clone() copies the value into the owned result
                .comparable(model.comparable.clone())
                // BORROW: .clone() copies the value into the owned result
                .integrity(integrity.clone());
            for factor in &factors {
                // BORROW: .clone() copies the factor into the manifest
                builder = builder.factor(factor.clone());
            }
            for value in model.comparable.values() {
                let source = if value.value().is_some() {
                    // BORROW: .clone() copies the experiment path into the source
                    ValueSource::imported(model.experiment.clone())
                } else {
                    ValueSource::Unrecorded
                };
                // BORROW: .clone() copies the path into the source entry
                builder = builder.source(SourceEntry::new(value.path().clone(), source));
            }
            // BORROW: .clone() copies the typed roots and paths into the source entries
            builder = builder
                .source(SourceEntry::new(
                    // BORROW: .clone() copies the value into the owned result
                    harness.clone(),
                    ValueSource::imported(relative(&run.stream.path)?),
                ))
                .source(SourceEntry::new(
                    // BORROW: .clone() copies the value into the owned result
                    treatment.clone(),
                    // BORROW: .clone() copies the value into the owned result
                    ValueSource::imported(model.experiment.clone()),
                ))
                .source(SourceEntry::new(
                    // BORROW: .clone() copies the value into the owned result
                    snapshots.clone(),
                    // BORROW: .clone() copies the value into the owned result
                    ValueSource::imported(folder.provenance.path.clone()),
                ));
            if !model.comparable.inputs().is_empty() {
                builder = builder.source(SourceEntry::new(
                    // BORROW: .clone() copies the value into the owned result
                    inputs.clone(),
                    ValueSource::imported(relative(&run.timing.path)?),
                ));
            }
            for measured in measured_of(cell, run, model)? {
                builder = builder.measured(measured);
            }
            if let Some(started) = started {
                builder = builder.started_at(started);
            }
            if let Some(ended) = ended {
                builder = builder.ended_at(ended);
            }
            let json = JsonValue::new(legacy)
                .map_err(|error| layout("PROVENANCE.json", &run.label, error.to_string()))?;
            // BORROW: .clone() copies the format label and the commit into the import record
            let mut record = ImportRecord::new(rules.format.clone(), base_commit.clone(), json);
            record.sources = import_sources(folder, cell, run)?;
            if integrity == Integrity::Valid {
                record.requests.push(ImportedRequest::new(
                    // BORROW: .clone() copies the evaluator into the request
                    pinned.evaluator.clone(),
                    request_id(&key, &pinned.evaluator, &pinned.finished)?,
                ));
            }
            let manifest = builder.import(record).build().map_err(|error| {
                layout(
                    &format!("{}/experiment.json", cell.name),
                    &run.label,
                    error.to_string(),
                )
            })?;
            let index = mapped.len();
            mapped.push(MappedRun {
                key,
                manifest,
                run: index,
            });
        }
    }
    Ok(mapped)
}
