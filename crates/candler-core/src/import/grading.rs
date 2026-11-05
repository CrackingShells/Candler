//! Mapping legacy grading to outcome records: every row of a valid run's `expectations.jsonl`, with the evidence and `blocked_by` of its grading file, becomes one outcome record that keeps the legacy fact and marks with `legacy` only what the outcome model cannot state.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr as _;

use candler_schemas::evaluation::{
    ContextArtifact, ContextRef, EvaluatorRef, RunContext, SuiteEntry,
};
use candler_schemas::execution::Integrity;
use candler_schemas::outcome::{
    Cause, Evidence, Instrument, OutcomeRecord, Role, Status, Value as Measure, Weight,
};
use candler_schemas::primitives::{ExpectationId, Name, NameVersion, ReasonCode, RelPath};
use serde_json::{Map, Value};

use super::ImportError;
use super::legacy::{BLOCKED_BY, CONFIG, EXPECTATIONS, EXPECTATIONS_FILE, OUTPUTS, ROLE};
use super::manifest::{MappedRun, layout, pinned};
use super::read::{JsonFile, LegacyFolder, LegacyRun, rel_path};
use super::rules::{ImportRules, MarkerCode};

/// The placeholder for the root and the absolute paths of a run context: the architecture allows absolute paths in a run context outside the digest projection, and an imported run has no run root, so every absolute path in the context is this one.
const ROOT: &str = "/legacy";

/// The legacy statuses the importer knows.
#[derive(Debug, Clone, Copy)]
enum Legacy {
    Pass,
    Fail,
    Blocked,
    NotMeasured,
    NotApplicable,
}

/// One row of `expectations.jsonl` (`listed`) joined to its entry in the grading file.
struct Row<'a> {
    id: &'a str,
    listed: &'a Value,
    entry: &'a Value,
    file: &'a JsonFile,
}

/// The text of `key` in `object`.
fn text<'a>(object: &'a Value, key: &str, file: &str, run: &str) -> Result<&'a str, ImportError> {
    object
        .get(key)
        // BORROW: Value::as_str borrows the JSON string without copying
        .and_then(Value::as_str)
        .ok_or_else(|| layout(file, run, format!("`{key}` is not a string")))
}

/// The last segment of a path.
fn name_of<'a>(path: &'a RelPath) -> &'a str {
    // BORROW: explicit .as_str() instead of Deref coercion
    path.as_str().rsplit('/').next().unwrap_or_default()
}

/// The rows of `expectations.jsonl` that belong to `run`, each joined to its grading entry; every entry of the run's grading files has a row and every row an entry.
fn rows_of<'a>(
    folder: &'a LegacyFolder,
    cell: &str,
    run: &'a LegacyRun,
) -> Result<Vec<Row<'a>>, ImportError> {
    let table = EXPECTATIONS_FILE;
    let wanted = format!("run-{}", run.number);
    let mut entries: BTreeMap<(&str, &str), (&Value, &JsonFile)> = BTreeMap::new();
    for file in &run.gradings {
        // BORROW: explicit .as_str() instead of Deref coercion
        let path = file.path.as_str();
        let list = file
            .value
            .get(EXPECTATIONS)
            .and_then(Value::as_array)
            .ok_or_else(|| layout(path, &run.label, "`expectations` is not an array"))?;
        for entry in list {
            let id = text(entry, "id", path, &run.label)?;
            if entries
                .insert((name_of(&file.path), id), (entry, file))
                .is_some()
            {
                return Err(layout(path, &run.label, format!("`{id}` appears twice")));
            }
        }
    }
    let mut rows = Vec::new();
    let mut used: BTreeSet<(&str, &str)> = BTreeSet::new();
    for row in &folder.expectations {
        let same = text(row, "cell", table, &run.label)? == cell
            && text(row, CONFIG, table, &run.label)? == run.config
            && text(row, "run", table, &run.label)? == wanted;
        if !same {
            continue;
        }
        let id = text(row, "id", table, &run.label)?;
        let eval = text(row, "eval", table, &run.label)?;
        let (entry, file) = entries.get(&(eval, id)).ok_or_else(|| {
            layout(
                table,
                &run.label,
                format!("`{id}` of `{eval}` has no entry in a grading file of the run"),
            )
        })?;
        if !used.insert((eval, id)) {
            return Err(layout(table, &run.label, format!("`{id}` appears twice")));
        }
        rows.push(Row {
            id,
            listed: row,
            entry,
            file,
        });
    }
    if let Some((eval, id)) = entries.keys().find(|key| !used.contains(*key)) {
        return Err(layout(
            table,
            &run.label,
            format!("`{id}` of `{eval}` has no row"),
        ));
    }
    Ok(rows)
}

/// Checks that the row and the grading entry state the same thing in every field they share.
fn cross_check(row: &Row<'_>, run: &LegacyRun) -> Result<(), ImportError> {
    for (key, what) in [
        ("status", "statuses"),
        (ROLE, "roles"),
        ("score", "scores"),
        ("max_score", "maximum scores"),
        ("value", "values"),
    ] {
        let from_row = row.listed.get(key).unwrap_or(&Value::Null);
        let from_entry = row.entry.get(key).unwrap_or(&Value::Null);
        if from_row != from_entry {
            return Err(ImportError::Disagree {
                what,
                // BORROW: .clone() copies the run label into the error
                run: run.label.clone(),
                first: EXPECTATIONS_FILE.into(),
                second: row.file.path.to_string(),
                detail: format!("`{}`: `{from_row}` and `{from_entry}`", row.id),
            });
        }
    }
    Ok(())
}

/// The role of a row.
fn role_of(row: &Row<'_>, run: &LegacyRun) -> Result<Role, ImportError> {
    // BORROW: explicit .as_str() instead of Deref coercion
    let file = row.file.path.as_str();
    match text(row.listed, ROLE, file, &run.label)? {
        "gate" => Ok(Role::Gate),
        "quality" => Ok(Role::Quality),
        "descriptive" => Ok(Role::Descriptive),
        other => Err(layout(
            file,
            &run.label,
            format!(
                "role `{other}` of `{}` is not `gate`, `quality` or `descriptive`",
                row.id
            ),
        )),
    }
}

/// The value of a row: the JSON token decides `count`, `number` or `text`.
fn value_of(row: &Row<'_>, run: &LegacyRun) -> Result<Option<Measure>, ImportError> {
    // BORROW: explicit .as_str() instead of Deref coercion
    let file = row.file.path.as_str();
    let bad = |detail: String| layout(file, &run.label, format!("value of `{}` {detail}", row.id));
    match row.listed.get("value") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => {
            if let Some(count) = n.as_u64() {
                Ok(Some(Measure::count(count)))
            } else if n.is_i64() {
                Err(bad(format!("is the negative count {n}")))
            } else {
                let number = n
                    .as_f64()
                    .ok_or_else(|| bad(format!("is {n}, not a number")))?;
                Measure::number(number)
                    .map(Some)
                    .map_err(|error| bad(error.to_string()))
            }
        }
        // BORROW: .clone() copies the text into the value
        Some(Value::String(s)) => Ok(Some(Measure::text(s.clone()))),
        Some(Value::Bool(_) | Value::Array(_) | Value::Object(_)) => {
            Err(bad("is not a number or a string".into()))
        }
    }
}

/// A `legacy` cause with `code`.
#[allow(
    deprecated,
    reason = "the imported record marks what the clean model cannot state"
)]
fn legacy(code: &str) -> Result<Cause, ImportError> {
    Ok(Cause::Legacy(ReasonCode::from_str(code)?))
}

/// The legacy code of a `not_measured` row: the first marker rule that matches the evidence, else `unrecorded`.
fn marker_code(rules: &ImportRules, evidence: &str) -> Result<Cause, ImportError> {
    for marker in &rules.markers {
        let Some(found) = marker.pattern.captures(evidence) else {
            continue;
        };
        match &marker.code {
            // BORROW: .clone() copies the fixed code into the cause
            #[allow(
                deprecated,
                reason = "the imported record marks what the clean model cannot state"
            )]
            // BORROW: .clone() copies the value into the owned result
            MarkerCode::Fixed(code) => return Ok(Cause::Legacy(code.clone())),
            MarkerCode::Capture(group) => {
                // BORROW: explicit .as_str() instead of Deref coercion
                let code = found.name(group).map_or("", |m| m.as_str());
                if !code.is_empty() {
                    return legacy(code);
                }
            }
        }
    }
    legacy("unrecorded")
}

/// What a row gives the record besides its identity: status, weight, cause.
struct Facts {
    status: Status,
    weight: Option<Weight>,
    cause: Option<Cause>,
}

/// Maps a status, its score and its maximum score, and the grading entry's `blocked_by` and evidence, to what the record states.
fn facts_of(row: &Row<'_>, run: &LegacyRun, rules: &ImportRules) -> Result<Facts, ImportError> {
    // BORROW: explicit .as_str() instead of Deref coercion
    let file = row.file.path.as_str();
    let table = EXPECTATIONS_FILE;
    let status_text = text(row.listed, "status", table, &run.label)?;
    let legacy_status = match status_text {
        "pass" => Legacy::Pass,
        "fail" => Legacy::Fail,
        "blocked" => Legacy::Blocked,
        "not_measured" => Legacy::NotMeasured,
        "not_applicable" => Legacy::NotApplicable,
        other => {
            return Err(ImportError::Status {
                file: table.into(),
                // BORROW: .clone() copies the value into the owned result
                run: run.label.clone(),
                id: row.id.into(),
                status: other.into(),
            });
        }
    };
    cross_check(row, run)?;
    let bad = |detail: String| layout(file, &run.label, format!("`{}` {detail}", row.id));
    let whole = |key: &str| {
        row.listed
            .get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| bad(format!("has a `{key}` that is not a non-negative integer")))
    };
    let (score, max) = (whole("score")?, whole("max_score")?);
    let weight = |num: u64, den: u64| Weight::new(num, den).map_err(|error| bad(error.to_string()));
    let blocked_by = match row.entry.get(BLOCKED_BY) {
        None | Some(Value::Null) => None,
        Some(Value::String(path)) => Some(rel_path(path)?),
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => {
            return Err(bad("has a `blocked_by` that is not a path".into()));
        }
    };
    if blocked_by.is_some() && !matches!(legacy_status, Legacy::Blocked) {
        return Err(bad(format!("is `{status_text}` and has a `blocked_by`")));
    }
    let evidence = row
        .entry
        .get("evidence")
        // BORROW: Value::as_str borrows the JSON string without copying
        .and_then(Value::as_str)
        .unwrap_or_default();
    Ok(match legacy_status {
        Legacy::Pass if max == 0 => {
            if score != 0 {
                return Err(bad(format!("passes with {score}/{max}")));
            }
            Facts {
                status: Status::Pass,
                weight: Some(weight(score, max)?),
                cause: Some(legacy("zero_weight")?),
            }
        }
        Legacy::Pass => {
            if score != max {
                return Err(bad(format!("passes with {score}/{max}")));
            }
            Facts {
                status: Status::Pass,
                weight: if score == 1 {
                    None
                } else {
                    Some(weight(score, max)?)
                },
                cause: None,
            }
        }
        Legacy::Fail => {
            if max == 0 || score >= max {
                return Err(bad(format!("fails with {score}/{max}")));
            }
            Facts {
                status: Status::Fail,
                weight: if score == 0 && max == 1 {
                    None
                } else {
                    Some(weight(score, max)?)
                },
                cause: None,
            }
        }
        Legacy::Blocked => {
            if max == 0 || score != 0 {
                return Err(bad(format!("is blocked with {score}/{max}")));
            }
            Facts {
                status: Status::Blocked,
                weight: if max == 1 {
                    None
                } else {
                    Some(weight(score, max)?)
                },
                cause: Some(match blocked_by {
                    Some(path) => Cause::Artifact(path),
                    None => legacy("unrecorded")?,
                }),
            }
        }
        Legacy::NotMeasured => Facts {
            status: Status::NotMeasured,
            weight: None,
            cause: Some(marker_code(rules, evidence)?),
        },
        Legacy::NotApplicable => Facts {
            status: Status::NotApplicable,
            weight: None,
            cause: Some(Cause::Reason(ReasonCode::from_str(
                "not_applicable_to_arm",
            )?)),
        },
    })
}

/// The suite entry synthesised for a row: the legacy check's text, scale and tier as parameters.
fn suite_entry(
    row: &Row<'_>,
    run: &LegacyRun,
    role: Role,
    rules: &ImportRules,
    evaluator: &EvaluatorRef,
) -> Result<SuiteEntry, ImportError> {
    // BORROW: explicit .as_str() instead of Deref coercion
    let file = row.file.path.as_str();
    let mut params = Map::new();
    for key in ["scale", "text", "tier"] {
        let value = row
            .entry
            .get(key)
            .filter(|value| !value.is_null())
            .ok_or_else(|| layout(file, &run.label, format!("`{}` has no `{key}`", row.id)))?;
        // BORROW: .to_owned() and .clone() make the owned parameter
        params.insert(key.to_owned(), value.clone());
    }
    SuiteEntry::builder()
        .id(ExpectationId::from_str(row.id)?)
        .role(role)
        // BORROW: .clone() copies the evaluator reference and the kind into the entry
        .evaluator(evaluator.clone())
        // BORROW: .clone() copies the value into the owned result
        .kind(rules.suite_kind.clone())
        .params(params)
        .build()
        .map_err(|error| layout(file, &run.label, format!("`{}`: {error}", row.id)))
}

/// The context every record of the run shares: the files under `outputs/`, the manifest's snapshots and inputs, and the metadata the rules name.
fn run_context(
    mapped: &MappedRun,
    run: &LegacyRun,
    rules: &ImportRules,
) -> Result<RunContext, ImportError> {
    let bad = |detail: String| layout(OUTPUTS, &run.label, detail);
    let mut builder = RunContext::builder()
        // BORROW: .clone() copies the key into the context
        .run(mapped.key.clone())
        .root(ROOT.into());
    for output in &run.outputs {
        // BORROW: explicit .as_str() instead of Deref coercion
        let (_, below) = output
            .path
            // BORROW: explicit .as_str() instead of Deref coercion
            .as_str()
            .split_once(&format!("/{OUTPUTS}/"))
            .ok_or_else(|| bad(format!("`{}` is not under `outputs/`", output.path)))?;
        builder = builder.artifact(ContextArtifact::new(
            rel_path(below)?,
            format!("{ROOT}/{OUTPUTS}/{below}"),
            // BORROW: .clone() copies the digest into the context
            output.sha256.clone(),
        ));
    }
    let comparable = mapped.manifest.comparable();
    for snapshot in comparable.snapshots() {
        // BORROW: .clone() copies the kind and digest into the context
        builder = builder.snapshot(ContextRef::new(
            // BORROW: .clone() copies the value into the owned result
            snapshot.kind.clone(),
            format!("{ROOT}/snapshots/{}", snapshot.kind),
            // BORROW: .clone() copies the value into the owned result
            snapshot.digest.clone(),
        ));
    }
    for input in comparable.inputs() {
        // BORROW: .clone() copies the kind and digest into the context
        builder = builder.input(ContextRef::new(
            // BORROW: .clone() copies the value into the owned result
            input.kind.clone(),
            format!("{ROOT}/inputs/{}", input.kind),
            // BORROW: .clone() copies the value into the owned result
            input.digest.clone(),
        ));
    }
    for path in &rules.metadata {
        let value = comparable
            .values()
            .iter()
            .find(|value| value.path() == path)
            .ok_or_else(|| bad(format!("metadata path `{path}` is not a value of the cell")))?;
        if let Some(json) = value.value() {
            // BORROW: .clone() copies the path and the value into the context
            builder = builder.metadata(path.clone(), json.clone());
        }
    }
    builder.build().map_err(|error| bad(error.to_string()))
}

/// Maps the rows of one valid run to outcome records; an invalid run has none.
///
/// Each row of `expectations.jsonl` becomes one record. The row and its entry in the run's grading file must state the same status, role, score, maximum score and value. The weight is left out when it is the status's default, the cause marks only a `not_measured` code, a zero weight or an unrecorded blocker with `legacy`, and the evaluator and instrument are the ones the folder's provenance pins, never a per-run value.
///
/// # Errors
/// Returns [`ImportError::Status`] for a legacy status the importer does not know,
/// [`ImportError::Disagree`] for a row and a grading entry that differ,
/// [`ImportError::Layout`] for a row without an entry, an entry without a row, or a field the
/// mapping reads that is absent or of the wrong type, and [`ImportError::Format`] for a value
/// that is not the primitive it must be.
pub fn map_grading(
    folder: &LegacyFolder,
    mapped: &MappedRun,
    rules: &ImportRules,
) -> Result<Vec<OutcomeRecord>, ImportError> {
    let (cell, run) = folder.run(mapped.run).ok_or_else(|| {
        layout(
            EXPECTATIONS_FILE,
            "the folder",
            format!("the folder has no run at index {}", mapped.run),
        )
    })?;
    if *mapped.manifest.integrity() != Integrity::Valid {
        return Ok(Vec::new());
    }
    let pinned = pinned(folder)?;
    let request = mapped
        .manifest
        .import()
        .and_then(|record| record.requests.first())
        .map(|request| &request.request)
        .ok_or_else(|| layout("PROVENANCE.json", &run.label, "the run names no request"))?;
    let version = pinned.evaluator.version();
    let identity = NameVersion::from_str(&format!("{}@{version}", rules.instrument))?;
    // BORROW: .clone() copies the instrument digest into the instrument
    let instrument = Instrument::new(identity, pinned.instrument.clone(), pinned.dirty)
        .map_err(|error| layout("PROVENANCE.json", &run.label, error.to_string()))?;
    let evaluator_ref = EvaluatorRef::new(
        Name::from_str(pinned.evaluator.name())?,
        semver::VersionReq::parse(&format!("={version}"))
            .map_err(|error| layout("PROVENANCE.json", &run.label, error.to_string()))?,
    );
    let context = run_context(mapped, run, rules)?.context_digest();
    let mut records = Vec::new();
    for row in rows_of(folder, &cell.name, run)? {
        // BORROW: explicit .as_str() instead of Deref coercion
        let file = row.file.path.as_str();
        let facts = facts_of(&row, run, rules)?;
        let role = role_of(&row, run)?;
        let suite = suite_entry(&row, run, role, rules, &evaluator_ref)?;
        // BORROW: Value::as_str borrows the JSON string without copying
        let evidence_text = match row.entry.get("evidence").and_then(Value::as_str) {
            Some(note) if !note.is_empty() => note,
            Some(_) | None => "no legacy evidence",
        };
        // BORROW: .to_owned() makes the owned note
        let evidence = Evidence::new(None, Some(evidence_text.to_owned()))
            .map_err(|error| layout(file, &run.label, error.to_string()))?;
        let mut builder = OutcomeRecord::builder()
            // BORROW: .clone() copies the key, the evaluator, the instrument, the digests and the request id into the record
            .run(mapped.key.clone())
            .expectation_id(ExpectationId::from_str(row.id)?)
            .role(role)
            // BORROW: .clone() copies the value into the owned result
            .evaluator(pinned.evaluator.clone())
            // BORROW: .clone() copies the value into the owned result
            .instrument(instrument.clone())
            .suite_entry_digest(suite.digest())
            // BORROW: .clone() copies the value into the owned result
            .context_digest(context.clone())
            // BORROW: .clone() copies the value into the owned result
            .request_id(request.clone())
            .status(facts.status)
            .evidence(evidence);
        if let Some(value) = value_of(&row, run)? {
            builder = builder.value(value);
        }
        if let Some(weight) = facts.weight {
            builder = builder.weight(weight);
        }
        if let Some(cause) = facts.cause {
            builder = builder.cause(cause);
        }
        records.push(
            builder
                .build()
                .map_err(|error| layout(file, &run.label, format!("`{}`: {error}", row.id)))?,
        );
    }
    Ok(records)
}
