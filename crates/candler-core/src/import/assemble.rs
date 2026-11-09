//! Assembling the runs of a legacy folder into a store: every run is written step by step and sealed last, so that an import that stops anywhere is finished by running it again.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use candler_schemas::execution::{Integrity, RunManifest, RunTree};
use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::{Digest, NameVersion, RequestId, RunKey};

use super::ImportError;
use super::assets::{import_assets, plan_assets, put_sources};
use super::grading::map_grading;
use super::legacy::VERDICT;
use super::manifest::{MappedRun, map_runs};
use super::read::{LegacyFolder, LegacyRun, SkipReason};
use super::report::{ImportReport, InvalidRun, SkippedEntry};
use super::rules::ImportRules;
use super::streams::{ReaderFactory, TraceFailure, frame_streams, usage_diffs};
use crate::frames::{FrameReader, RawFileInfo};
use crate::store::outcome::{OutcomeFileRef, OutcomeWriter};
use crate::store::run::{Existing, RunWriter};
use crate::store::{Store, StoreError};
use crate::trace::normalise;

/// The hidden directory of the store where streams are framed before they move into their run.
const SCRATCH: &str = ".import-scratch";

/// The error of a store that refuses a step of `run`.
fn store_error(run: &str, error: &StoreError) -> ImportError {
    ImportError::Store {
        // BORROW: .to_owned() makes the owned label
        run: run.to_owned(),
        detail: error.to_string(),
    }
}

/// The error of a file system call on `path`.
fn io_error(path: &Path, error: &io::Error) -> ImportError {
    ImportError::Io {
        path: path.display().to_string(),
        detail: error.to_string(),
    }
}

/// Removes the directory at `path` with what it holds; a directory that is absent is already removed.
fn remove_tree(path: &Path) -> Result<(), ImportError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        // EXPLICIT: an absent directory is the wanted state
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(path, &error)),
    }
}

/// Removes the copies of `key` that `Existing::ReplaceUnfinished` moved aside, so that the store holds only what a clean import holds.
///
/// Only an unfinished run is ever moved aside, and an unfinished run is not a sealed record: I11
/// protects the records `run.done` seals, and a sealed run is never replaced or removed here.
fn discard_replaced(store: &Store, key: &RunKey) -> Result<(), ImportError> {
    let layout = store.layout();
    let mut n = 1_u32;
    loop {
        let aside = layout.replaced_dir(key, n);
        if !aside.exists() {
            break;
        }
        remove_tree(&aside)?;
        n = n.saturating_add(1);
    }
    if let Some(parent) = layout.replaced_dir(key, 1).parent() {
        // EXPLICIT: the directory of moved-aside runs is removed when empty; one that still holds another run's copy stays
        let _ = fs::remove_dir(parent);
    }
    Ok(())
}

/// The first source, or part of the manifest, in which `stored` differs from `mapped`, with what differs.
fn first_difference(
    stored: &RunManifest,
    mapped: &RunManifest,
) -> Result<(String, String), ImportError> {
    let sources = |manifest: &RunManifest| -> BTreeMap<String, String> {
        manifest
            .import()
            .map(|record| {
                record
                    .sources
                    .iter()
                    .map(|source| (source.path.to_string(), source.sha256.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    };
    let rules = |manifest: &RunManifest| manifest.import().and_then(|record| record.rules.clone());
    let (was, is) = (rules(stored), rules(mapped));
    if was != is {
        // BORROW: .to_string() makes the digest text
        let show = |digest: Option<Digest>| digest.map_or("absent".to_owned(), |d| d.to_string());
        return Ok((
            "import.rules".to_owned(),
            format!(
                "the store holds {}, the rules file holds {}",
                show(was),
                show(is)
            ),
        ));
    }
    let (old, new) = (sources(stored), sources(mapped));
    let paths: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
    for path in paths {
        let (was, is) = (old.get(path), new.get(path));
        if was != is {
            // BORROW: .clone() copies the digest text
            let show = |digest: Option<&String>| digest.map_or("absent".to_owned(), String::clone);
            return Ok((
                // BORROW: .clone() copies the path into the error
                path.clone(),
                format!(
                    "the store holds {}, the folder holds {}",
                    show(was),
                    show(is)
                ),
            ));
        }
    }
    let to_value = |manifest: &RunManifest| {
        serde_json::to_value(manifest).map_err(|e| ImportError::Json {
            file: "manifest.json".into(),
            detail: e.to_string(),
        })
    };
    let (old, new) = (to_value(stored)?, to_value(mapped)?);
    let names: BTreeSet<&String> = old
        // BORROW: Value::as_object lists the keys
        .as_object()
        .into_iter()
        // BORROW: Value::as_object lists the keys
        .chain(new.as_object())
        .flat_map(serde_json::Map::keys)
        .collect();
    for name in names {
        if old.get(name) != new.get(name) {
            return Ok((
                // BORROW: .to_owned() makes the owned name
                "manifest.json".to_owned(),
                format!("the field `{name}` differs"),
            ));
        }
    }
    Ok((
        // BORROW: .to_owned() makes the owned name
        "manifest.json".to_owned(),
        // BORROW: .to_owned() makes the owned text
        "the manifests differ".to_owned(),
    ))
}

/// The first path in which the stored tree differs from the one the folder builds, with what differs.
fn first_tree_difference(
    stored: &RunTree,
    built: &RunTree,
) -> Result<(String, String), ImportError> {
    let entries = |tree: &RunTree| -> Result<BTreeMap<String, String>, ImportError> {
        tree.entries()
            .iter()
            .map(|entry| {
                let text = serde_json::to_string(entry).map_err(|e| ImportError::Json {
                    file: "tree.json".into(),
                    detail: e.to_string(),
                })?;
                Ok((entry.path().to_string(), text))
            })
            .collect()
    };
    let (old, new) = (entries(stored)?, entries(built)?);
    let paths: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
    for path in paths {
        let (was, is) = (old.get(path), new.get(path));
        if was != is {
            // BORROW: .clone() copies the entry text
            let show = |entry: Option<&String>| entry.map_or("absent".to_owned(), String::clone);
            return Ok((
                // BORROW: .clone() copies the path into the error
                path.clone(),
                format!(
                    "the stored tree holds {}, the folder builds {}",
                    show(was),
                    show(is)
                ),
            ));
        }
    }
    Ok((
        // BORROW: .to_owned() makes the owned name
        "tree.json".to_owned(),
        // BORROW: .to_owned() makes the owned text
        "the trees differ".to_owned(),
    ))
}

/// The stream a frame file holds.
fn stream_of(info: &RawFileInfo) -> Result<candler_schemas::raw::StreamName, ImportError> {
    // BORROW: explicit .as_path()
    let reader = FrameReader::open(info.path.as_path()).map_err(|e| ImportError::Io {
        path: info.path.display().to_string(),
        detail: e.to_string(),
    })?;
    // BORROW: .clone() gives the caller its own name
    Ok(reader.header().stream.clone())
}

/// Why `reason` leaves an entry out.
const fn skip_explanation(reason: SkipReason) -> &'static str {
    match reason {
        SkipReason::AppleDouble => {
            "an AppleDouble file written by an archive tool; it holds file metadata, not a result"
        }
        SkipReason::FinderMetadata => "a Finder metadata file; it holds no result",
        SkipReason::ArchiveMetadata => {
            "the metadata directory of an archive tool; it holds no result"
        }
        SkipReason::SharedInputs => {
            "inputs the runs of a cell share, copied from the repository; the run manifests name the inputs they read"
        }
    }
}

/// Refuses a finished run of the store whose manifest or tree differs from what the folder maps, and writes nothing.
fn check_sealed(store: &Store, mapped: &MappedRun, run: &LegacyRun) -> Result<(), ImportError> {
    // BORROW: explicit .as_str()
    let (label, key) = (run.label.as_str(), &mapped.key);
    let stored = store
        .read_manifest(key)
        .map_err(|e| store_error(label, &e))?;
    if stored != mapped.manifest {
        let (file, detail) = first_difference(&stored, &mapped.manifest)?;
        return Err(ImportError::Conflict {
            // BORROW: .to_owned() makes the owned label
            run: label.to_owned(),
            file,
            detail,
        });
    }
    let built = plan_assets(run, key)?;
    let tree = store.read_tree(key).map_err(|e| store_error(label, &e))?;
    if tree != built.tree {
        let (file, detail) = first_tree_difference(&tree, &built.tree)?;
        return Err(ImportError::Conflict {
            // BORROW: .to_owned() makes the owned label
            run: label.to_owned(),
            file,
            detail,
        });
    }
    Ok(())
}

/// What the import needs to write one run.
struct Context<'a> {
    store: &'a Store,
    readers: &'a ReaderFactory,
    scratch: &'a Path,
}

/// The request and evaluator the outcome file of a valid run gets.
fn request_of(
    manifest: &RunManifest,
    label: &str,
) -> Result<(NameVersion, RequestId), ImportError> {
    manifest
        .import()
        .and_then(|record| record.requests.first())
        .map(|request| {
            (
                // BORROW: .clone() copies the evaluator
                request.evaluator.clone(),
                // BORROW: .clone() copies the request id
                request.request.clone(),
            )
        })
        .ok_or_else(|| ImportError::Layout {
            file: "manifest.json".into(),
            // BORROW: .to_owned() makes the owned label
            run: label.to_owned(),
            detail: "a valid run names no request for its outcome file".into(),
        })
}

/// Writes the outcome file of a valid run unless the store already holds it.
fn write_outcomes(
    store: &Store,
    manifest: &RunManifest,
    records: &[OutcomeRecord],
    label: &str,
) -> Result<(), ImportError> {
    let key = manifest.run();
    let (evaluator, request) = request_of(manifest, label)?;
    let target = OutcomeFileRef::new(
        // BORROW: .clone() copies the key into the name
        key.clone(),
        // BORROW: .clone() copies the evaluator into the name
        evaluator.clone(),
        // BORROW: .clone() copies the request id into the name
        request.clone(),
    );
    let present = store
        .outcome_files(key)
        .map_err(|e| store_error(label, &e))?;
    if present.contains(&target) {
        return Ok(());
    }
    let mut writer = OutcomeWriter::create(store, key, &evaluator, &request)
        .map_err(|e| store_error(label, &e))?;
    for record in records {
        writer.append(record).map_err(|e| store_error(label, &e))?;
    }
    writer.commit().map_err(|e| store_error(label, &e))
}

/// Writes one run, or checks the finished one the store holds, and adds what it finds to `report`.
fn import_run(
    ctx: &Context<'_>,
    mapped: &MappedRun,
    run: &LegacyRun,
    records: &[OutcomeRecord],
    done: bool,
    report: &mut ImportReport,
) -> Result<(), ImportError> {
    // BORROW: explicit .as_str()
    let (store, label, key) = (ctx.store, run.label.as_str(), &mapped.key);
    let frames_dir = ctx.scratch.join(key.to_string());
    // BORROW: explicit .as_path()
    let framed = frame_streams(run, frames_dir.as_path())?;
    let assets = import_assets(store, run, key)?;
    report.tree_diffs.extend(assets.tree_diffs);
    let stdout = framed.first().ok_or_else(|| ImportError::Layout {
        file: run.stream.path.to_string(),
        // BORROW: .to_owned() makes the owned label
        run: label.to_owned(),
        detail: "no stream was framed".into(),
    })?;
    // BORROW: .clone() keeps the framed file while it moves
    let mut info = stdout.clone();
    let mut writer = None;
    // A finished run was checked against the folder before the first write.
    if !done {
        let mut w = RunWriter::create(store, &mapped.manifest, Existing::ReplaceUnfinished)
            .map_err(|e| store_error(label, &e))?;
        discard_replaced(store, key)?;
        w.tree(&assets.tree).map_err(|e| store_error(label, &e))?;
        for file in &framed {
            let stream = stream_of(file)?;
            // BORROW: explicit .as_path()
            w.raw(&stream, file.path.as_path(), &file.digest)
                .map_err(|e| store_error(label, &e))?;
            let stored = store.raw_path(key, &stream);
            if let (true, Some(stored)) = (file.path == stdout.path, stored) {
                info = RawFileInfo::new(
                    stored,
                    // BORROW: .clone() copies the digest
                    file.digest.clone(),
                    file.frames,
                );
            }
        }
        writer = Some(w);
    }
    match normalise((ctx.readers)(), &info) {
        Ok(trace) => {
            report
                .usage_diffs
                .extend(usage_diffs(key, &trace, &run.timing)?);
            if let Some(w) = writer.as_mut() {
                w.trace(&trace).map_err(|e| store_error(label, &e))?;
            }
        }
        Err(error) => report.trace_failures.push(TraceFailure::new(
            // BORROW: .clone() gives the failure its own key
            key.clone(),
            error.to_string(),
        )),
    }
    if let Some(w) = writer {
        w.done().map_err(|e| store_error(label, &e))?;
    }
    // BORROW: explicit .as_path()
    remove_tree(frames_dir.as_path())?;
    if matches!(mapped.manifest.integrity(), Integrity::Valid) {
        write_outcomes(store, &mapped.manifest, records, label)?;
    }
    Ok(())
}

/// The report entry of a run that does not count.
fn invalid_run(mapped: &MappedRun) -> Result<Option<InvalidRun>, ImportError> {
    let Integrity::Invalid { reason, .. } = mapped.manifest.integrity() else {
        return Ok(None);
    };
    let json_error = |e: serde_json::Error| ImportError::Json {
        file: "manifest.json".into(),
        detail: e.to_string(),
    };
    let integrity = match mapped.manifest.import() {
        Some(record) => serde_json::to_value(&record.legacy_integrity).map_err(json_error)?,
        None => serde_json::Value::Null,
    };
    let verdict = integrity
        .get(VERDICT)
        // BORROW: Value::as_str reads the verdict
        .and_then(serde_json::Value::as_str)
        // BORROW: .to_owned() makes the owned verdict
        .map_or_else(|| "unrecorded".to_owned(), str::to_owned);
    let code = serde_json::to_value(reason)
        .map_err(json_error)?
        // BORROW: Value::as_str reads the code
        .as_str()
        // BORROW: .to_owned() makes the owned code
        .map(str::to_owned)
        .ok_or_else(|| ImportError::Json {
            file: "manifest.json".into(),
            detail: "the invalid reason is not a string".into(),
        })?;
    Ok(Some(InvalidRun::new(
        // BORROW: .clone() gives the entry its own key
        mapped.key.clone(),
        *reason,
        format!(
            "the legacy verdict is `{verdict}`, which maps to the invalid reason `{code}`; the run is imported done with its files kept as sources, and has no outcome file"
        ),
    )))
}

/// Imports the legacy results folder at `folder` into `store`.
///
/// The folder is read and every run is mapped, graded and checked, and every finished run of the
/// store is compared with the folder, before the store is written: a finished run whose manifest or
/// tree differs is refused and the import writes nothing. Then each run is imported in key order: a
/// finished run the folder agrees with is left as it is, and an unfinished run is replaced. A run is written as its manifest, its tree, its raw streams and its trace, and sealed
/// last; a valid run then gets its outcome file unless the store holds it. The index is rebuilt
/// when it does not list what the store holds, and the scratch directory of the framing is removed
/// however the import ends, so the store holds the same files however many times, and in however
/// many pieces, the import ran. The report is computed from the folder and the stored streams, so it does not
/// depend on what the store held before.
///
/// # Errors
/// Returns the errors of reading and mapping the folder, [`ImportError::Conflict`] naming the run
/// and the first differing source or output when a finished run differs, [`ImportError::Store`] when the
/// store refuses a step, and [`ImportError::Io`] when a file cannot be read or removed.
pub fn import(
    folder: &Path,
    rules: &ImportRules,
    readers: &ReaderFactory,
    store: &Store,
) -> Result<ImportReport, ImportError> {
    let scratch = store.layout().root().join(SCRATCH);
    // BORROW: explicit .as_path()
    let report = import_runs(folder, rules, readers, store, scratch.as_path());
    // BORROW: explicit .as_path()
    let cleaned = remove_tree(scratch.as_path());
    let report = report?;
    cleaned?;
    Ok(report)
}

/// The steps of [`import`], with the scratch directory of the framing at `scratch`.
fn import_runs(
    folder: &Path,
    rules: &ImportRules,
    readers: &ReaderFactory,
    store: &Store,
    scratch: &Path,
) -> Result<ImportReport, ImportError> {
    let legacy = LegacyFolder::read(folder, rules)?;
    let mut mapped = map_runs(&legacy, rules)?;
    mapped.sort_by(|a, b| a.key.cmp(&b.key));
    let mut plans = Vec::with_capacity(mapped.len());
    for run in &mapped {
        plans.push((run, map_grading(&legacy, run, rules)?));
    }
    let sealed: BTreeSet<RunKey> = store
        .runs()
        .map_err(|e| store_error("the store", &e))?
        .into_iter()
        .filter(|entry| entry.done)
        .map(|entry| entry.key)
        .collect();
    // Every finished run is checked before the first write, so a refused import writes nothing.
    for (run, _) in &plans {
        if sealed.contains(&run.key) {
            let (_, legacy_run) = legacy.run(run.run).ok_or_else(|| ImportError::Layout {
                file: "folder".into(),
                run: run.key.to_string(),
                detail: "a mapped run is not in the folder".into(),
            })?;
            check_sealed(store, run, legacy_run)?;
        }
    }
    remove_tree(scratch)?;
    // BORROW: explicit .as_slice()
    put_sources(store, "the folder", legacy.sources.as_slice())?;
    for cell in &legacy.cells {
        // BORROW: explicit .as_str() and .as_slice()
        put_sources(store, cell.name.as_str(), cell.sources.as_slice())?;
    }
    let ctx = Context {
        store,
        readers,
        scratch,
    };
    let mut report = ImportReport::new();
    for (run, records) in &plans {
        let (_, legacy_run) = legacy.run(run.run).ok_or_else(|| ImportError::Layout {
            file: "folder".into(),
            run: run.key.to_string(),
            detail: "a mapped run is not in the folder".into(),
        })?;
        import_run(
            &ctx,
            run,
            legacy_run,
            // BORROW: explicit .as_slice()
            records.as_slice(),
            sealed.contains(&run.key),
            &mut report,
        )?;
        report.runs = report.runs.saturating_add(1);
        report.outcomes = report
            .outcomes
            .saturating_add(u64::try_from(records.len()).unwrap_or(u64::MAX));
        report.invalid_runs.extend(invalid_run(run)?);
    }
    if !index_is_current(store)? {
        store
            .rebuild_index()
            .map_err(|e| store_error("the store", &e))?;
    }
    report.skipped = legacy
        .skipped
        .iter()
        .map(|entry| {
            SkippedEntry::new(
                // BORROW: .clone() copies the path into the entry
                entry.path.clone(),
                entry.reason,
                // BORROW: .to_owned() makes the owned text
                skip_explanation(entry.reason).to_owned(),
            )
        })
        .collect();
    report.sort();
    Ok(report)
}

/// Whether the index files list the sealed runs and the finished outcome files of the store, in
/// the order `rebuild_index` writes them, and end on a whole line. A store that holds only what an
/// earlier import wrote then keeps its index files as they are.
fn index_is_current(store: &Store) -> Result<bool, ImportError> {
    let entries = store.runs().map_err(|e| store_error("the store", &e))?;
    let sealed: Vec<RunKey> = entries
        .iter()
        .filter(|entry| entry.done)
        // BORROW: .clone() copies the key into the list
        .map(|entry| entry.key.clone())
        .collect();
    let mut by_name: Vec<&RunKey> = entries.iter().map(|entry| &entry.key).collect();
    by_name.sort_by_key(ToString::to_string);
    let mut files = Vec::new();
    for key in by_name {
        files.extend(
            store
                .outcome_files(key)
                .map_err(|e| store_error("the store", &e))?,
        );
    }
    let layout = store.layout();
    Ok(store
        .indexed_runs()
        .map_err(|e| store_error("the store", &e))?
        == sealed
        && store
            .indexed_outcomes()
            .map_err(|e| store_error("the store", &e))?
            == files
        // BORROW: explicit .as_path()
        && ends_on_a_line(layout.index_runs().as_path())?
        // BORROW: explicit .as_path()
        && ends_on_a_line(layout.index_outcomes().as_path())?)
}

/// Whether the file at `path` is missing, empty or ends with a line feed.
fn ends_on_a_line(path: &Path) -> Result<bool, ImportError> {
    match fs::read(path) {
        Ok(bytes) => Ok(bytes.last().is_none_or(|last| *last == b'\n')),
        // EXPLICIT: an absent index file ends on no partial line
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(io_error(path, &e)),
    }
}
