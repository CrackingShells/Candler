//! Guards for the import of a legacy results folder into a store, over the vendored mini-corpus.
//! The stored records are read back through the store's own readers. The expected rows come from
//! the independent oracle; the counts are literals written by hand after reading the grading
//! files with `jq`: the three real runs hold 43 expectations each, the synthetic runs 2, 5 and 2.
//! It also holds the mapping of the folder to manifests and records (`mapping`), the rule that the
//! core names no domain (a renamed corpus imports to the same rows) and the import package names.
#![cfg(test)]

use crate::oracle;
use crate::support;
use crate::support::legacy_import;
use crate::support::legacy_rules;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use candler_core::import::ImportError;
use candler_core::import::assemble::import;
use candler_core::import::rules::{
    HarnessRule, ImportRules, ImportRulesBuilder, MarkerCode, MarkerRule, SnapshotRule,
    TreatmentRule,
};
use candler_core::isolation::image::package_name_ok;
use candler_core::store::Store;
use candler_harness::reader::TraceReader;
use candler_schemas::primitives::{Digest, FieldPath, Name, NameToken, ReasonCode, RunKey};
use flate2::Compression;
use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use legacy_import::{
    SYNTHETIC, V3, V6, assert_same_store, cell_of, copy_tree, field, import_with_reader, mini,
    project, reader, snapshot, text_of,
};
use oracle::{ExpectedRow, expected_rows, expected_runs};
use regex::Regex;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::str::FromStr as _;

/// The outcome records of every run of the store, projected as the oracle's rows.
fn stored_rows(store: &Store) -> Vec<ExpectedRow> {
    let mut rows = Vec::new();
    for entry in store.runs().expect("the runs list") {
        for file in store
            .outcome_files(&entry.key)
            .expect("the outcome files list")
        {
            for record in store.read_outcomes(&file).expect("the outcome file reads") {
                let value = serde_json::to_value(&record).expect("a record serializes");
                rows.push(project(
                    cell_of(entry.key.replicate()),
                    entry.key.replicate(),
                    &value,
                ));
            }
        }
    }
    rows.sort();
    rows
}

/// The run of `replicate` among the sealed runs of the store.
fn key_of(store: &Store, replicate: u32) -> RunKey {
    store
        .runs()
        .expect("the runs list")
        .into_iter()
        .find(|entry| entry.key.replicate() == replicate)
        .unwrap_or_else(|| panic!("no run {replicate} in the store"))
        .key
}

/// The store of a clean import of the mini-corpus, as a snapshot.
fn clean_snapshot() -> std::collections::BTreeMap<String, Option<Vec<u8>>> {
    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(store_dir.path()).expect("the store initialises");
    import_with_reader(&mini(), &store).expect("the mini-corpus imports");
    snapshot(store_dir.path())
}

/// Guards: D6 and I10 — the outcome records read back from the store equal the oracle's rows
/// except those of the contaminated run, which is done, has no outcome file, and keeps its grading
/// files as blobs named in `import.sources`; every run has a trace; the report counts 6 runs and
/// 136 outcomes; every entry of every report list has an explanation. Oracle:
/// `oracle::expected_rows`, and the counts of `jq` over the grading files.
#[test]
fn the_store_holds_the_oracle_rows_and_every_difference_is_explained() {
    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(store_dir.path()).expect("the store initialises");
    let report = import_with_reader(&mini(), &store).expect("the mini-corpus imports");

    let mut expected = expected_rows(&mini());
    // The oracle holds no row for the contaminated run: its grading files are sources only.
    assert!(
        !expected
            .iter()
            .any(|row| row.cell == SYNTHETIC && row.run_n == 1),
        "the oracle grades the contaminated run"
    );
    expected.sort();
    let stored = stored_rows(&store);
    assert_eq!(stored.len(), 136, "stored outcome records");
    assert_eq!(stored, expected);
    assert_eq!(report.runs, 6);
    assert_eq!(report.outcomes, 136);

    // The contaminated run is done and has no outcome file, and its grading files are blobs.
    let key = key_of(&store, 1);
    let sealed: Vec<(u32, bool)> = store
        .runs()
        .expect("the runs list")
        .iter()
        .map(|entry| (entry.key.replicate(), entry.done))
        .collect();
    assert_eq!(sealed.len(), 6);
    assert!(
        sealed.iter().all(|(_, done)| *done),
        "every run is done: {sealed:?}"
    );
    assert_eq!(
        store
            .outcome_files(&key)
            .expect("the outcome files list")
            .len(),
        0
    );
    assert!(!store.layout().run_outcomes_dir(&key).exists());
    let manifest = serde_json::to_value(store.read_manifest(&key).expect("the manifest reads"))
        .expect("a manifest serializes");
    assert_eq!(text_of(field(&manifest, "integrity"), "kind"), "invalid");
    assert_eq!(
        text_of(field(&manifest, "integrity"), "reason"),
        "input_leak"
    );
    let sources = field(field(&manifest, "import"), "sources")
        .as_array()
        .expect("the sources are a list");
    {
        let grading = "eval-1-tier-a-domain-structure/with_skill/run-1/eval-1-grading.json";
        let source = sources
            .iter()
            .find(|source| text_of(source, "path") == grading)
            .unwrap_or_else(|| panic!("`{grading}` is not named in import.sources"));
        let digest = Digest::from_str(&text_of(source, "sha256")).expect("a digest");
        let blob = store.layout().blob(&digest);
        assert!(blob.is_file(), "no blob for `{grading}`");
        assert_eq!(
            fs::read(&blob).expect("the blob reads"),
            fs::read(mini().join(SYNTHETIC).join(grading)).expect("the grading file reads"),
        );
    }

    // Every run has a trace.
    for entry in store.runs().expect("the runs list") {
        let trace = store.layout().trace(&entry.key);
        assert!(
            trace.metadata().is_ok_and(|meta| meta.len() > 0),
            "no trace for {}",
            entry.key
        );
        store.read_trace(&entry.key).expect("the trace reads");
    }

    // The report: one invalid run, no trace failure, and an explanation on every entry.
    assert_eq!(report.invalid_runs.len(), 1);
    assert_eq!(report.invalid_runs.first().map(|run| &run.run), Some(&key));
    assert_eq!(report.trace_failures.len(), 0);
    assert_eq!(
        report.usage_diffs.len(),
        15,
        "12 absent counters and 3 output_tokens"
    );
    assert_eq!(report.tree_diffs.len(), 7);
    assert_eq!(report.skipped.len(), 5);
    let explanations: Vec<&str> = report
        .invalid_runs
        .iter()
        // BORROW: explicit .as_str() instead of Deref coercion
        .map(|entry| entry.explanation.as_str())
        .chain(
            report
                .trace_failures
                .iter()
                // BORROW: explicit .as_str() instead of Deref coercion
                .map(|entry| entry.explanation.as_str()),
        )
        .chain(
            report
                .usage_diffs
                .iter()
                // BORROW: explicit .as_str() instead of Deref coercion
                .map(|entry| entry.explanation.as_str()),
        )
        .chain(
            report
                .tree_diffs
                .iter()
                // BORROW: explicit .as_str() instead of Deref coercion
                .map(|entry| entry.explanation.as_str()),
        )
        .chain(
            report
                .skipped
                .iter()
                // BORROW: explicit .as_str() instead of Deref coercion
                .map(|entry| entry.explanation.as_str()),
        )
        .collect();
    assert_eq!(explanations.len(), 1 + 15 + 7 + 5);
    assert!(explanations.iter().all(|text| !text.trim().is_empty()));
}

/// Guards: R01 §Golden corpus (every source file read with its sha256, each kept as a blob) — every
/// run lists its `agent_stream.jsonl.gz` and `agent_stderr.txt` in `import.sources` with the
/// file's digest and a blob of its bytes, and the raw header's `source` digest of its `stdout`
/// frames is among them. Oracle: `sha256` of the fixture files, computed here from their bytes.
#[test]
fn every_run_lists_its_stream_and_stderr_as_kept_sources() {
    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(store_dir.path()).expect("the store initialises");
    import_with_reader(&mini(), &store).expect("the mini-corpus imports");
    let runs = store.runs().expect("the runs list");
    assert_eq!(runs.len(), 6);
    for entry in runs {
        let replicate = entry.key.replicate();
        let cell = cell_of(replicate);
        let run_dir = format!("_run_cache/with_skill/run-{replicate}");
        let manifest =
            serde_json::to_value(store.read_manifest(&entry.key).expect("the manifest reads"))
                .expect("a manifest serializes");
        let sources = field(field(&manifest, "import"), "sources")
            .as_array()
            .expect("the sources are a list");
        let mut stdout_digest = None;
        for name in ["agent_stream.jsonl.gz", "agent_stderr.txt"] {
            let path = format!("{run_dir}/{name}");
            let source = sources
                .iter()
                .find(|source| text_of(source, "path") == path)
                .unwrap_or_else(|| panic!("run {replicate}: `{path}` is not in import.sources"));
            let bytes = fs::read(mini().join(cell).join(&path)).expect("the fixture file reads");
            let expected = format!(
                "sha256:{}",
                Sha256::digest(&bytes)
                    .iter()
                    .fold(String::new(), |mut hex, byte| {
                        write!(hex, "{byte:02x}").expect("a string takes text");
                        hex
                    })
            );
            assert_eq!(
                text_of(source, "sha256"),
                expected,
                "run {replicate}: {path}"
            );
            let digest = Digest::from_str(&expected).expect("a digest");
            let blob = store.blob_path(&digest);
            assert_eq!(
                fs::read(&blob).unwrap_or_else(|_| panic!("run {replicate}: no blob for `{path}`")),
                bytes,
                "run {replicate}: the blob of `{path}`"
            );
            if name == "agent_stream.jsonl.gz" {
                stdout_digest = Some(expected);
            }
        }
        // The `stdout` frames name the stream file they were cut from.
        let raw = fs::read(
            store
                .layout()
                .raw(&entry.key, &"stdout".parse().expect("a stream name")),
        )
        .expect("the stdout frames read");
        let mut text = String::new();
        MultiGzDecoder::new(raw.as_slice())
            .read_to_string(&mut text)
            .expect("the stdout frames decompress");
        let header: Value = serde_json::from_str(text.lines().next().expect("a header line"))
            .expect("the header parses");
        let source = text_of(&header, "source");
        assert_eq!(Some(source.clone()), stdout_digest, "run {replicate}");
        assert!(
            sources
                .iter()
                .any(|entry| text_of(entry, "sha256") == source),
            "run {replicate}: the raw header's source is not in import.sources"
        );
    }
}

/// Guards: Cond 1 and I11 — importing two copies of the mini-corpus, at different paths, into two
/// stores at different paths gives byte-identical stores and equal reports. Oracle: a test-side
/// walk of the relative paths and bytes of each store.
#[test]
fn two_imports_of_one_corpus_give_one_store() {
    let first_corpus = tempfile::tempdir().expect("a temporary directory");
    let second_corpus = tempfile::tempdir().expect("a temporary directory");
    copy_tree(&mini(), &first_corpus.path().join("results-a"));
    copy_tree(
        &mini(),
        &second_corpus.path().join("legacy").join("results-b"),
    );
    let first_dir = tempfile::tempdir().expect("a temporary directory");
    let second_dir = tempfile::tempdir().expect("a temporary directory");
    let first = Store::init(first_dir.path()).expect("the store initialises");
    let second = Store::init(second_dir.path()).expect("the store initialises");
    let first_report = import_with_reader(&first_corpus.path().join("results-a"), &first)
        .expect("the first copy imports");
    let second_report = import_with_reader(
        &second_corpus.path().join("legacy").join("results-b"),
        &second,
    )
    .expect("the second copy imports");
    let first_files = snapshot(first_dir.path());
    assert!(
        first_files.len() > 50,
        "the store holds {} entries",
        first_files.len()
    );
    assert_same_store(&first_files, &snapshot(second_dir.path()), "two imports");
    assert_eq!(first_report, second_report);
    assert_eq!(
        first_report.to_json().expect("a report serializes"),
        second_report.to_json().expect("a report serializes"),
    );
}

/// Imports into a new store with a reader that panics when the `fail_at`th stream (from 0) is
/// read, and returns the store directory with the panic's outcome.
fn interrupted(corpus: &Path, fail_at: usize) -> (tempfile::TempDir, Store, bool) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(dir.path()).expect("the store initialises");
    let calls = Arc::new(AtomicUsize::new(0));
    // TRAIT_OBJECT: the import's factory type returns boxed readers
    let readers = move |harness: &str| -> Option<Box<dyn TraceReader>> {
        assert!(
            calls.fetch_add(1, Ordering::SeqCst) != fail_at,
            "an injected fault while the stream {fail_at} is read"
        );
        reader(harness)
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        import(corpus, &legacy_rules::rules(), &readers, &store)
    }));
    (dir, store, outcome.is_err())
}

/// Guards: I11 — an import that was interrupted or refused leaves a store that the next import
/// completes to the bytes of a clean import: after a reader that panics on the second run, after
/// an artifact that was unreadable and is restored, and after an outcome file that is a truncated
/// `.partial`; a finished store is rewritten in no file; a finished run whose source, or whose
/// artifact, differs by one byte is refused with the run and the file before anything is written
/// (the store, blobs included, is as it was), and the unchanged folder is accepted. The unreadable
/// artifact fails in `LegacyFolder::read`, before any write: the folder is read up front, so read
/// time is the only place that state can fail. Oracle: the snapshot of a clean import, and the
/// sources' own bytes.
#[test]
fn an_import_that_stopped_is_completed_by_the_next() {
    let clean = clean_snapshot();
    let corpus_dir = tempfile::tempdir().expect("a temporary directory");
    let corpus = corpus_dir.path().join("results");
    copy_tree(&mini(), &corpus);

    // A reader that panics on the second run leaves one run done and one unfinished.
    let (dir, store, panicked) = interrupted(&corpus, 1);
    assert!(panicked, "the injected fault did not stop the import");
    let entries = store.runs().expect("the runs list");
    assert_eq!(entries.len(), 2, "a run done and a run begun");
    assert_eq!(entries.iter().filter(|entry| entry.done).count(), 1);
    import_with_reader(&corpus, &store).expect("the next import completes");
    assert_same_store(&snapshot(dir.path()), &clean, "after a panic");

    // An artifact of the second run that cannot be read stops the import; restored, it completes.
    let dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(dir.path()).expect("the store initialises");
    let artifact = corpus.join("synthetic_cell/_run_cache/with_skill/run-2/outputs/domain.py");
    let mode = fs::metadata(&artifact)
        .expect("the artifact exists")
        .permissions()
        .mode();
    fs::set_permissions(&artifact, fs::Permissions::from_mode(0o000)).expect("the mode changes");
    assert!(
        fs::read(&artifact).is_err(),
        "the artifact is still readable"
    );
    let refused = import_with_reader(&corpus, &store);
    fs::set_permissions(&artifact, fs::Permissions::from_mode(mode)).expect("the mode is restored");
    assert!(
        matches!(
            refused,
            Err(ImportError::Io { .. } | ImportError::Layout { .. })
        ),
        "{refused:?}"
    );
    assert!(
        store
            .runs()
            .expect("the runs list")
            .iter()
            .all(|entry| !entry.done),
        "a run is done after a refused import"
    );
    import_with_reader(&corpus, &store).expect("the restored folder imports");
    assert_same_store(
        &snapshot(dir.path()),
        &clean,
        "after an unreadable artifact",
    );

    // The outcome file of the third run is a truncated partial: the file is gone and half of its
    // bytes sit under the partial name.
    let dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(dir.path()).expect("the store initialises");
    import_with_reader(&corpus, &store).expect("the first import");
    let third = store
        .runs()
        .expect("the runs list")
        .get(2)
        .expect("a third run")
        .key
        // BORROW: .clone() copies the value
        .clone();
    let file = store
        .outcome_files(&third)
        .expect("the outcome files list")
        .into_iter()
        .next()
        .expect("the third run has an outcome file");
    let layout = store.layout();
    let finished = layout.outcome(&file.run, &file.evaluator, &file.request);
    let partial = layout.outcome_partial(&file.run, &file.evaluator, &file.request);
    let bytes = fs::read(&finished).expect("the outcome file reads");
    fs::remove_file(&finished).expect("the outcome file is removed");
    fs::write(
        &partial,
        bytes.get(..bytes.len() / 2).expect("half of the bytes"),
    )
    .expect("the partial is written");
    assert_eq!(
        store
            .outcome_files(&third)
            .expect("the outcome files list")
            .len(),
        0
    );
    import_with_reader(&corpus, &store).expect("the next import completes");
    assert_same_store(&snapshot(dir.path()), &clean, "after a truncated partial");
    assert_eq!(
        fs::read(&finished).expect("the outcome file is back"),
        bytes
    );

    // A finished store is rewritten in no file: no file changes its bytes, its inode or its time.
    let stamp = |root: &Path| -> Vec<(String, u64, i64, i64)> {
        let mut stamps = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(next) = pending.pop() {
            for entry in fs::read_dir(&next).expect("a directory lists") {
                let entry = entry.expect("an entry");
                let meta = entry.metadata().expect("metadata");
                if meta.is_dir() {
                    pending.push(entry.path());
                } else {
                    stamps.push((
                        entry.path().to_string_lossy().into_owned(),
                        meta.ino(),
                        meta.mtime(),
                        meta.mtime_nsec(),
                    ));
                }
            }
        }
        stamps.sort();
        stamps
    };
    let before = stamp(dir.path());
    assert!(before.len() > 50);
    import_with_reader(&corpus, &store).expect("the finished store imports again");
    assert_eq!(stamp(dir.path()), before, "a file was rewritten");
    assert_same_store(&snapshot(dir.path()), &clean, "after a third import");

    // A finished run whose source, or whose artifact, differs by one byte is refused with the run
    // and the file; the unchanged folder is accepted. The refusal comes before the first write, so
    // the store, blobs included, is byte for byte as it was.
    let manifest = store.read_manifest(&third).expect("the manifest reads");
    let settled = snapshot(dir.path());
    let cases: [(&str, Option<&[u8]>, &str); 4] = [
        (
            "synthetic_cell/eval-1-tier-a-domain-structure/with_skill/run-2/eval_metadata.json",
            Some(b"synthetic prompt"),
            "eval-1-tier-a-domain-structure/with_skill/run-2/eval_metadata.json",
        ),
        (
            "synthetic_cell/_run_cache/with_skill/run-2/outputs/domain.py",
            None,
            "domain.py",
        ),
        (
            "synthetic_cell/_run_cache/with_skill/run-2/agent_stream.jsonl.gz",
            None,
            "_run_cache/with_skill/run-2/agent_stream.jsonl.gz",
        ),
        (
            "synthetic_cell/_run_cache/with_skill/run-2/agent_stderr.txt",
            None,
            "_run_cache/with_skill/run-2/agent_stderr.txt",
        ),
    ];
    for (relative, needle, named) in cases {
        let changed_dir = tempfile::tempdir().expect("a temporary directory");
        let changed = changed_dir.path().join("results");
        copy_tree(&corpus, &changed);
        let target = changed.join(relative);
        let mut text = fs::read(&target).expect("the source reads");
        let zipped = Path::new(relative)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("gz"));
        if zipped {
            let mut plain = Vec::new();
            MultiGzDecoder::new(text.as_slice())
                .read_to_end(&mut plain)
                .expect("the stream decompresses");
            text = plain;
        }
        // The fixtures' stderr files are empty, so a byte cannot be flipped in them: one is added.
        if text.is_empty() {
            text.push(b'x');
        }
        let at = if zipped {
            // The third byte of the second line: inside the first key of its JSON object.
            text.iter()
                .position(|byte| *byte == b'\n')
                .expect("the stream has a second line")
                + 3
        } else {
            needle.map_or(0, |needle| {
                text.windows(needle.len())
                    .position(|window| window == needle)
                    .expect("the needle is in the source")
                    + needle.len()
                    - 1
            })
        };
        *text.get_mut(at).expect("the byte is in the source") ^= 1;
        if zipped {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&text).expect("the stream compresses");
            text = encoder.finish().expect("the stream compresses");
        }
        fs::write(&target, &text).expect("the source is written");
        match import_with_reader(&changed, &store) {
            Err(ImportError::Conflict { run, file, .. }) => {
                assert_eq!(run, "synthetic_cell/with_skill/run-2");
                assert_eq!(file, named);
            }
            other => panic!("a changed `{relative}` was not refused as a conflict: {other:?}"),
        }
        assert_eq!(
            store.read_manifest(&third).expect("the manifest reads"),
            manifest
        );
        assert_same_store(&snapshot(dir.path()), &settled, relative);
    }
    import_with_reader(&corpus, &store).expect("the unchanged folder is accepted");
    assert_same_store(&snapshot(dir.path()), &settled, "after the control");
}

/// Whether `path` is a relative POSIX path: no leading `/`, no backslash, no empty, `.` or `..`
/// segment and no drive letter.
fn is_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// The path-typed strings of `value`: the strings under a `path`, `filename` or `artifact` key,
/// leaving out the subtrees under `ext`, which hold what a harness reported verbatim.
fn typed_paths(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, member) in map {
                if key == "ext" {
                    continue;
                }
                // BORROW: explicit .as_str() instead of Deref coercion
                match (key.as_str(), member) {
                    ("path" | "filename" | "artifact", Value::String(text)) => {
                        // BORROW: `.clone()` copies the path into the list
                        found.push(text.clone());
                    }
                    _ => typed_paths(member, found),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                typed_paths(item, found);
            }
        }
        // EXPLICIT: a scalar holds no path-typed field
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

/// The path-typed strings of a file of the store that holds one JSON document or one per line.
fn paths_of(file: &Path) -> Vec<String> {
    let text = fs::read_to_string(file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
    let mut found = Vec::new();
    if let Ok(document) = serde_json::from_str::<Value>(&text) {
        typed_paths(&document, &mut found);
    } else {
        for line in text.lines() {
            let value: Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("a line of {} is not JSON: {e}", file.display()));
            typed_paths(&value, &mut found);
        }
    }
    found
}

/// Guards: I17 — every path-typed field of every manifest, tree, trace and outcome record of the
/// imported store is relative, and neither the folder's root nor the store's root appears in any
/// file of the store, in plain bytes or gunzipped. Oracle: the rule of `RelPath`, restated in
/// `is_relative`, and the roots of the two temporary directories.
#[test]
fn the_store_holds_relative_paths_only() {
    let corpus_dir = tempfile::tempdir().expect("a temporary directory");
    let corpus = corpus_dir.path().join("results");
    copy_tree(&mini(), &corpus);
    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(store_dir.path()).expect("the store initialises");
    import_with_reader(&corpus, &store).expect("the mini-corpus imports");

    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for entry in store.runs().expect("the runs list") {
        let layout = store.layout();
        let documents = [
            ("manifest", layout.manifest(&entry.key)),
            ("tree", layout.tree(&entry.key)),
            ("trace", layout.trace(&entry.key)),
        ];
        let outcomes: Vec<_> = store
            .outcome_files(&entry.key)
            .expect("the outcome files list")
            .iter()
            .map(|file| {
                (
                    "outcome",
                    layout.outcome(&file.run, &file.evaluator, &file.request),
                )
            })
            .collect();
        for (kind, file) in documents.into_iter().chain(outcomes) {
            for path in paths_of(&file) {
                assert!(
                    is_relative(&path),
                    "`{path}` in {kind} {} is not relative",
                    file.display()
                );
                *counts.entry(kind).or_default() += 1;
            }
        }
    }
    for kind in ["manifest", "tree", "outcome"] {
        assert!(
            counts.get(kind).copied().unwrap_or(0) > 0,
            "no path read in any {kind}: {counts:?}"
        );
    }
    // The check bites: a path under the folder's root is not relative.
    assert!(!is_relative(&corpus.join("a").to_string_lossy()));

    let roots: BTreeSet<String> = [corpus_dir.path(), store_dir.path()]
        .into_iter()
        .flat_map(|root| {
            [
                root.to_string_lossy().into_owned(),
                fs::canonicalize(root)
                    .expect("a root canonicalises")
                    .to_string_lossy()
                    .into_owned(),
            ]
        })
        .collect();
    assert!(roots.len() >= 2);
    for root in &roots {
        support::assert_clean(store_dir.path(), root);
    }
    // The scan reads what it is meant to: a name the store holds is found, in a file and in a gzip.
    let found = support::scan(store_dir.path(), "htn-creator-eval@0.4.2").expect("the store scans");
    assert_ne!(found.hits.len(), 0);
    let found = support::scan(store_dir.path(), "claude_code_version").expect("the store scans");
    assert!(
        found
            .hits
            .iter()
            .any(|hit| hit.to_string_lossy().ends_with(".gz")),
        "{:?}",
        found.hits
    );
}

/// Guards: Cond 7 — an imported request's context digest is the projection of the run's outputs, the cell's snapshots and inputs, and the rule metadata. Oracle: `context_digest_runs.toml` (Python, independent author).
#[test]
fn an_imported_request_has_the_oracle_context_digest() {
    let text = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle/vectors/context_digest_runs.toml"),
    )
    .expect("the vector reads");
    let vectors: toml::Table = text.parse().expect("the vector parses");
    let runs = vectors
        .get("run")
        .and_then(toml::Value::as_array)
        .expect("the vector lists runs");
    assert_eq!(runs.len(), 3, "vector runs");

    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(store_dir.path()).expect("the store initialises");
    import_with_reader(&mini(), &store).expect("the mini-corpus imports");

    for vector in runs {
        let text_at = |key: &str| {
            vector
                .get(key)
                .and_then(toml::Value::as_str)
                .unwrap_or_else(|| panic!("the vector lacks `{key}`"))
        };
        let (cell, run, digest) = (text_at("cell"), text_at("run"), text_at("digest"));
        let replicate: u32 = run
            .rsplit_once("run-")
            .and_then(|(_, number)| number.parse().ok())
            .unwrap_or_else(|| panic!("`{run}` is not `<config>/run-<N>`"));
        assert_eq!(cell_of(replicate), cell, "the cell of {run}");
        let key = key_of(&store, replicate);
        let files = store.outcome_files(&key).expect("the files list");
        assert!(!files.is_empty(), "no outcome file for {cell} {run}");
        for file in files {
            let records = store.read_outcomes(&file).expect("the file reads");
            assert!(!records.is_empty(), "no record for {cell} {run}");
            for record in records {
                assert_eq!(
                    record.context_digest().to_string(),
                    digest,
                    "context digest of {cell} {run}"
                );
            }
        }
    }
}

/// The `suite_entry_digest` of the record of `id` in run `replicate` of the store.
fn entry_digest(store: &Store, replicate: u32, id: &str) -> String {
    let key = key_of(store, replicate);
    for file in store.outcome_files(&key).expect("the outcome files list") {
        for record in store.read_outcomes(&file).expect("the outcome file reads") {
            let value = serde_json::to_value(&record).expect("a record serializes");
            if text_of(&value, "expectation_id") == id {
                return text_of(&value, "suite_entry_digest").clone(); // BORROW: .clone() owns the digest
            }
        }
    }
    panic!("no record of {id} in run {replicate}");
}

/// Guards: R01 §Golden corpus (the rules supply every domain name) — rules that name
/// `["scale", "text"]` import the mini-corpus with the oracle's rows and synthesise each suite
/// entry from those two keys alone, so the digest of the entry A1 of run 6 is that of an entry
/// without `tier`, while the HTN rules, which also name `tier`, give the digest of the entry with it.
/// Oracle: the `sha256` of the entry's canonical JSON, taken with `shasum` over
/// `{"depends_on":[],"kind":"legacy-check","params":{"scale":"binary","text":"Running
/// scripts/validate_domain.py on the generated domain.py exits with code 0 (no structural errors
/// or warnings)"},"role":"gate"}` written by hand, and `oracle::expected_rows`.
#[test]
fn entry_params_come_from_the_rules() {
    const WITHOUT_TIER: &str =
        "sha256:39822149ac1007442d6ec29e26d29e5c2354fc0a240f1870552ffe454163bc24";
    const WITH_TIER: &str =
        "sha256:b0db76be61754c4fdffa4c605e1c2d9dd9f526c6f659550b16b920c374d44701";
    let import_with = |rules: &ImportRules| {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::init(dir.path()).expect("the store initialises");
        import(&mini(), rules, &reader, &store).expect("the mini-corpus imports");
        (dir, store)
    };

    let scale_text = legacy_rules::builder("resolved_skill_version")
        .entry_params(legacy_rules::entry_params(&["scale", "text"]))
        .build()
        .expect("the rules build");
    let (_scale_text_dir, scale_text) = import_with(&scale_text);
    let mut expected = expected_rows(&mini());
    expected.sort();
    assert_eq!(stored_rows(&scale_text), expected);
    assert_eq!(entry_digest(&scale_text, 6, "A1"), WITHOUT_TIER);

    let (_htn_dir, htn) = import_with(&legacy_rules::rules());
    assert_eq!(entry_digest(&htn, 6, "A1"), WITH_TIER);
}

/// Guards: I12 — rules that name a key that an entry lacks are refused, and the refusal names the
/// key and the entry. Oracle: the entry ids of
/// `oracle::expected_rows`, and the grading files, none of which holds a `bogus` key.
#[test]
fn entry_params_naming_a_key_an_entry_lacks_are_refused() {
    let rules = legacy_rules::builder("resolved_skill_version")
        .entry_params(legacy_rules::entry_params(&["scale", "text", "bogus"]))
        .build()
        .expect("the rules build");
    let dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(dir.path()).expect("the store initialises");
    let error = import(&mini(), &rules, &reader, &store).expect_err("the import is refused");
    let message = error.to_string();
    assert!(
        expected_rows(&mini())
            .iter()
            .any(|row| message.contains(&format!("`{}` has no `bogus`", row.expectation_id))),
        "the refusal names neither the key nor an entry: {message}"
    );
}

/// Guards: I12 — the rules have no silent default for the entry params: a builder that was not
/// given them refuses to build, naming the field; an explicit empty list builds. Oracle: the
/// hand-written expectation.
#[test]
fn entry_params_are_required_by_the_builder() {
    let error = legacy_rules::builder("resolved_skill_version")
        .build()
        .expect_err("the rules lack the entry params");
    assert!(
        matches!(
            error,
            ImportError::MissingField {
                field: "entry_params"
            }
        ),
        "{error}"
    );
    legacy_rules::builder("resolved_skill_version")
        .entry_params(Vec::new())
        .build()
        .expect("an explicit empty list builds");
}

/// The distinct factor lists the manifests of an import of `corpus` carry, as plain strings.
fn imported_factor_lists(corpus: &Path) -> BTreeSet<Vec<String>> {
    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(store_dir.path()).expect("the store initialises");
    import_with_reader(corpus, &store).expect("the corpus imports");
    store
        .runs()
        .expect("the runs list")
        .iter()
        .map(|entry| {
            store
                .read_manifest(&entry.key)
                .expect("the manifest reads")
                .factors()
                .iter()
                .map(ToString::to_string)
                .collect()
        })
        .collect()
}

/// Guards: A8 — an imported experiment declares the same factors as its native rerun: two skill
/// versions by two models declare the model and the treatment, and no skill-version spelling. Every
/// run of the store carries the one list.
/// Oracle: the four `experiment.json` files of the corpus (the mini-corpus' two real cells and
/// their clones under another model) diffed by hand with `jq`: the leaves that differ are
/// `plan.model` and the three skill version spellings. The spellings map one to one onto the two
/// treatments `htn-creator@v0.2.3` and `htn-creator@0.2.6` (two cells each, one level each);
/// `plan.model` does not (two treatments, four pairs), so it stays. The treatments differ, so
/// `request.treatment` is declared.
/// Bite: declare the varying value paths as before (the three spellings come back).
#[test]
fn it_two_versions_declare_treatment() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    legacy_import::versions_by_models(dir.path(), &[V3, V6]);
    let lists = imported_factor_lists(dir.path());
    let want: BTreeSet<Vec<String>> = BTreeSet::from([vec![
        "plan.model".to_owned(),
        "request.treatment".to_owned(),
    ]]);
    assert_eq!(lists, want);
}

/// Guards: A8 — a one-version import declares what it declares without the treatment rule: the one
/// version by two models declares the model alone, and never `request.treatment`.
/// Oracle: the two `experiment.json` files (the real v0.2.3 cell and its clone under another
/// model) diffed by hand with `jq`: only `plan.model` differs, and both cells hold the one
/// treatment `htn-creator@v0.2.3`, so no treatment is declared.
/// Bite: declare `request.treatment` whatever the treatments.
#[test]
fn it_one_version_unchanged() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    legacy_import::versions_by_models(dir.path(), &[V3]);
    let lists = imported_factor_lists(dir.path());
    let want: BTreeSet<Vec<String>> = BTreeSet::from([vec!["plan.model".to_owned()]]);
    assert_eq!(lists, want);
}

/// The identifier segments of the legacy domain, in lowercase, that nothing imported with renamed
/// rules may hold.
const DOMAIN_SEGMENTS: [&str; 13] = [
    "htn",
    "gtpyhop",
    "pyhop",
    "rikyu",
    "planner",
    "planners",
    "tier",
    "tiers",
    "problems",
    "libcst",
    "arango",
    "dotenv",
    "plantrace",
];

/// Whether `text` holds a domain segment between non-alphanumeric separators.
fn holds_domain_segment(text: &str) -> bool {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| DOMAIN_SEGMENTS.contains(&word.to_ascii_lowercase().as_str()))
}

/// Asserts that no file or folder name under `dir`, and no text of a file under it (gunzipped when
/// it opens with the gzip magic), holds a domain segment, and that `dir` holds a file.
fn assert_no_domain_name(dir: &Path) {
    fn walk(dir: &Path, files: &mut usize, found: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("the directory lists") {
            let path = entry.expect("an entry").path();
            if holds_domain_segment(&path.file_name().unwrap_or_default().to_string_lossy()) {
                found.push(path.display().to_string());
            }
            if path.is_dir() {
                walk(&path, files, found);
                continue;
            }
            *files += 1;
            let bytes = fs::read(&path).expect("the file reads");
            let mut plain = Vec::new();
            if bytes.starts_with(&[0x1f, 0x8b]) {
                // BORROW: the decoder reads the slice of `bytes` in place
                MultiGzDecoder::new(bytes.as_slice())
                    .read_to_end(&mut plain)
                    .expect("the stream gunzips");
            } else {
                plain = bytes;
            }
            if holds_domain_segment(&String::from_utf8_lossy(&plain)) {
                found.push(path.display().to_string());
            }
        }
    }
    let (mut files, mut found) = (0, Vec::new());
    walk(dir, &mut files, &mut found);
    assert!(files > 0, "no file under {}", dir.display());
    assert!(found.is_empty(), "domain names in {found:#?}");
}

/// What the domain names become in the renamed copy of the corpus, in the order they are applied.
const RENAMES: [(&str, &str); 21] = [
    ("mcp-python-ingestion", "mcp-ingest"),
    ("gtpyhop", "planlib"),
    ("GTPyhop", "Planlib"),
    ("rikyu", "zenith"),
    ("Rikyu", "Zenith"),
    ("RIKYU", "ZENITH"),
    ("tier_d", "tier_x"),
    ("TIER_D", "TIER_X"),
    ("tier", "band"),
    ("Tier", "Band"),
    ("TIER", "BAND"),
    ("planner", "scheduler"),
    ("Planner", "Scheduler"),
    ("problems", "cases"),
    ("Problems", "Cases"),
    ("PROBLEMS", "CASES"),
    ("arango", "graphdb"),
    ("Arango", "Graphdb"),
    ("htn", "acme"),
    ("HTN", "ACME"),
    ("D-E", "X-E"),
];

/// The bytes with every domain name renamed.
fn renamed(bytes: &[u8]) -> Vec<u8> {
    let mut text = String::from_utf8_lossy(bytes).into_owned();
    for (from, to) in RENAMES {
        text = text.replace(from, to);
    }
    text.into_bytes()
}

/// Copies `from` to `to` with every directory name, file name and file renamed, a gzip file
/// decompressed, rewritten and compressed again.
fn copy_renamed(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the target directory is created");
    for entry in fs::read_dir(from).expect("the directory lists") {
        let entry = entry.expect("an entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let target = to.join(String::from_utf8(renamed(name.as_bytes())).expect("a UTF-8 name"));
        if entry.file_type().expect("a type").is_dir() {
            copy_renamed(&entry.path(), &target);
            continue;
        }
        let bytes = fs::read(entry.path()).expect("the file reads");
        let written = if bytes.starts_with(&[0x1f, 0x8b]) {
            let mut plain = Vec::new();
            // BORROW: the decoder reads the slice of `bytes` in place
            MultiGzDecoder::new(bytes.as_slice())
                .read_to_end(&mut plain)
                .expect("the stream gunzips");
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(&renamed(&plain))
                .expect("the stream compresses");
            encoder.finish().expect("the stream finishes")
        } else {
            renamed(&bytes)
        };
        fs::write(&target, written).expect("the file is written");
    }
}

/// A rules builder with everything but the markers: the instrument and the legacy format are
/// the caller's.
fn rules_without_markers(instrument: &str, format: &str, entry_key: &str) -> ImportRulesBuilder {
    let name = |text: &str| Name::from_str(text).unwrap_or_else(|e| panic!("{text}: {e}"));
    let path = |text: &str| FieldPath::from_str(text).unwrap_or_else(|e| panic!("{text}: {e}"));
    ImportRules::builder()
        .format(NameToken::from_str(format).unwrap_or_else(|e| panic!("{format}: {e}")))
        .harness(HarnessRule::new(
            name("claude-code"),
            "/claude_code_version".into(),
        ))
        .treatment(TreatmentRule::new(
            "/plan/skill_version".into(),
            path("resolved_skill_version"),
        ))
        .instrument(name(instrument))
        .snapshot(SnapshotRule::new(name("catalog"), "/catalog/sha256".into()))
        .suite_kind(name("legacy-check"))
        .entry_params(vec![name("scale"), name("text"), name(entry_key)])
        .metadata(path("plan.model"))
}

/// The status counts of the oracle's rows over the mini-corpus.
fn oracle_counts() -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in expected_rows(&mini()) {
        *counts.entry(row.status).or_default() += 1;
    }
    counts
}

/// The projected rows and the run count of an import of `folder` with `rules` into a fresh
/// store; the store folder is returned so that the caller can scan it.
fn import_rows(
    folder: &Path,
    rules: &ImportRules,
    store_dir: &Path,
) -> (usize, Vec<oracle::ExpectedRow>, BTreeSet<Vec<String>>) {
    let store = Store::init(store_dir).expect("the store initialises");
    let report = import(folder, rules, &reader, &store).expect("the folder imports");
    let mut rows = Vec::new();
    for entry in store.runs().expect("the runs list") {
        for file in store
            .outcome_files(&entry.key)
            .expect("the outcome files list")
        {
            for record in store.read_outcomes(&file).expect("the file reads") {
                let value = serde_json::to_value(&record).expect("a record serializes");
                rows.push(project("", entry.key.replicate(), &value));
            }
        }
    }
    let factors = store
        .runs()
        .expect("the runs list")
        .iter()
        .map(|entry| {
            store
                .read_manifest(&entry.key)
                .expect("the manifest reads")
                .factors()
                .iter()
                .map(ToString::to_string)
                .collect()
        })
        .collect();
    (
        usize::try_from(report.runs).expect("a run count"),
        rows,
        factors,
    )
}

/// Guards: I1 — the core names no domain: a copy of the mini-corpus whose every file, directory and
/// stream has the domain names rewritten (`tier` becomes `band`, and the entry params name `band`),
/// imported with rules that name an `acme-eval` instrument, a renamed legacy format and a renamed
/// marker, gives the oracle's run count, the oracle's rows (the domain names renamed in them) and factor
/// lists, and the store holds no domain segment. Oracle: `oracle::expected_runs` and `oracle::expected_rows` over the original
/// mini-corpus, and `DOMAIN_SEGMENTS`.
#[test]
fn a_renamed_domain_imports_to_the_same_counts_and_leaves_no_name_in_the_store() {
    let copy = tempfile::tempdir().expect("a temporary directory");
    copy_renamed(&mini(), copy.path());
    assert_no_domain_name(copy.path());

    let pattern = Regex::new(r"BAND_X_REPLAY:[^\n]*reason=(?P<code>[A-Za-z0-9_]+)")
        .expect("the pattern compiles");
    let zero = Regex::new(r"declares zero 'mappings' entries").expect("the pattern compiles");
    let rules = rules_without_markers("acme-eval", "acme-results@1", "band")
        .marker(MarkerRule::new(pattern, MarkerCode::Capture("code".into())))
        .marker(MarkerRule::new(
            zero,
            MarkerCode::Fixed(ReasonCode::from_str("zero_mappings").expect("a reason code")),
        ))
        .build()
        .expect("the renamed rules build");

    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let (runs, mut rows, factors) = import_rows(copy.path(), &rules, store_dir.path());
    assert_eq!(runs, expected_runs(&mini()).len());
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for row in &rows {
        // BORROW: `.clone()` copies the status
        *counts.entry(row.status.clone()).or_default() += 1;
    }
    assert_eq!(counts, oracle_counts());

    // Every row and every factor list is the oracle's with the domain names renamed.
    let rename = |text: &str| String::from_utf8(renamed(text.as_bytes())).expect("a UTF-8 text");
    let mut expected: Vec<oracle::ExpectedRow> = expected_rows(&mini())
        .into_iter()
        .map(|row| oracle::ExpectedRow {
            cell: String::new(),
            expectation_id: rename(&row.expectation_id),
            role: rename(&row.role),
            value: row.value.map(|value| rename(&value)),
            cause: row.cause.map(|cause| rename(&cause)),
            ..row
        })
        .collect();
    expected.sort();
    rows.sort();
    assert_eq!(rows, expected);
    let expected_factors: BTreeSet<Vec<String>> = imported_factor_lists(&mini())
        .into_iter()
        .map(|list| list.iter().map(|factor| rename(factor)).collect())
        .collect();
    assert_eq!(factors, expected_factors);
    assert_no_domain_name(store_dir.path());
}

/// Guards: I1 and D6 — a reason code comes from the rules and not from the core: with rules that
/// hold no marker, every `not_measured` record of the mini-corpus has the cause
/// `{legacy: unrecorded}`, while the oracle's rows show that the rules with markers give some
/// `not_measured` record another cause. Oracle: `oracle::expected_rows` over the mini-corpus.
#[test]
fn without_markers_every_unmeasured_record_is_unrecorded() {
    let marked: Vec<_> = expected_rows(&mini())
        .into_iter()
        .filter(|row| row.status == "not_measured")
        .filter(|row| row.cause.as_deref() != Some("legacy: unrecorded"))
        .collect();
    assert!(
        !marked.is_empty(),
        "the mini-corpus has no `not_measured` record with a marker code"
    );

    let rules = rules_without_markers("htn-creator-eval", "htn-results@1", "tier")
        .build()
        .expect("the rules without markers build");
    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let (runs, rows, _) = import_rows(&mini(), &rules, store_dir.path());
    assert_eq!(runs, 6);
    let unmeasured: Vec<_> = rows
        .iter()
        .filter(|row| row.status == "not_measured")
        .collect();
    assert!(
        !unmeasured.is_empty(),
        "the mini-corpus has no `not_measured` record"
    );
    for row in unmeasured {
        assert_eq!(row.cause.as_deref(), Some("legacy: unrecorded"), "{row:?}");
    }
}

/// Guards: an import package name the experiment records must be a field-path segment, because it
/// becomes `provenance.packages.<name>.*`. A leading `_` is no segment, so it is refused up front
/// rather than left out of the record.
#[test]
fn a_package_name_is_a_field_path_segment() {
    let cases = [
        ("numpy", true),
        ("a", true),
        ("scikit_learn", true),
        ("pkg2", true),
        ("a_", true),
        ("_x", false),
        ("_", false),
        ("2pkg", false),
        ("Numpy", false),
        ("a-b", false),
        ("a.b", false),
        ("", false),
    ];
    for (name, expected) in cases {
        assert_eq!(package_name_ok(name), expected, "{name:?}");
    }
}

/// Guards for the mapping of a legacy results folder to run manifests and outcome records, over
/// the vendored mini-corpus. The expected rows and run facts come from the independent oracle; the
/// other expectations are literals written by hand after reading the raw files with `jq` and
/// `python3 -I`.
mod mapping {
    use crate::oracle;
    use crate::support::legacy_import::{SYNTHETIC, V3, V6, field, mini, project, text_of};
    use crate::support::legacy_rules;
    use candler_testkit::{copy_tree, file_paths};

    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::io::{Read as _, Write as _};
    use std::path::{Path, PathBuf};

    use candler_core::import::ImportError;
    use candler_core::import::grading::map_grading;
    use candler_core::import::manifest::{MappedRun, map_runs};
    use candler_core::import::read::{LegacyFolder, SourceFile};
    use flate2::Compression;
    use flate2::read::MultiGzDecoder;
    use flate2::write::GzEncoder;
    use oracle::{ExpectedIntegrity, ExpectedRow, expected_rows, expected_runs};
    use serde_json::Value;
    use sha2::Digest as _;

    /// The key of `experiment.json` that holds the resolved version.
    const RESOLVED: &str = "resolved_skill_version";

    /// The folder, its mapped runs and the serialized records of each mapped run, in mapped order.
    struct Imported {
        folder: LegacyFolder,
        mapped: Vec<MappedRun>,
        records: Vec<Vec<Value>>,
    }

    impl Imported {
        fn read(root: &Path) -> Self {
            let rules = legacy_rules::rules();
            let folder = LegacyFolder::read(root, &rules).expect("the folder reads");
            let mapped = map_runs(&folder, &rules).expect("the runs map");
            let records = mapped
                .iter()
                .map(|run| {
                    map_grading(&folder, run, &rules)
                        .expect("the grading maps")
                        .iter()
                        .map(|record| serde_json::to_value(record).expect("a record serializes"))
                        .collect()
                })
                .collect();
            Self {
                folder,
                mapped,
                records,
            }
        }

        /// The cell directory name and the run number of the mapped run at `index`.
        fn name(&self, index: usize) -> (String, u32) {
            let mapped = self.mapped.get(index).expect("a mapped run");
            let (cell, run) = self
                .folder
                .run(mapped.run)
                .expect("the run of a mapped run");
            // BORROW: `.clone()` copies the cell name
            (cell.name.clone(), run.number)
        }

        /// The index of the mapped run of `cell` and `number`.
        fn index(&self, cell: &str, number: u32) -> usize {
            (0..self.mapped.len())
                // BORROW: `.to_owned()` makes the owned cell name to compare
                .find(|&index| self.name(index) == (cell.to_owned(), number))
                .unwrap_or_else(|| panic!("no mapped run {cell} run-{number}"))
        }

        /// The serialized manifest of the run of `cell` and `number`.
        fn manifest(&self, cell: &str, number: u32) -> Value {
            let mapped = self
                .mapped
                .get(self.index(cell, number))
                .expect("a mapped run");
            serde_json::to_value(&mapped.manifest).expect("a manifest serializes")
        }

        /// The serialized records of the run of `cell` and `number`.
        fn records_of(&self, cell: &str, number: u32) -> &[Value] {
            self.records
                .get(self.index(cell, number))
                .expect("the records of a mapped run")
        }
    }

    /// Guards: D6 — the mapped records, projected to cell, run number, expectation id, role, status,
    /// value, weight and cause, equal the oracle's rows as a multiset, the records of one expectation
    /// id of the real cells share one suite entry digest, and the synthetic cell's reworded A1 has
    /// another. Oracle: `oracle::expected_rows`, which reads the
    /// raw grading files and cross-checks `expectations.jsonl`.
    #[test]
    fn the_mapped_records_are_the_oracle_rows() {
        let imported = Imported::read(&mini());
        let mut mapped_rows: Vec<ExpectedRow> = Vec::new();
        let mut digests: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (index, records) in imported.records.iter().enumerate() {
            let (cell, run_n) = imported.name(index);
            for record in records {
                mapped_rows.push(project(&cell, run_n, record));
                if cell != SYNTHETIC {
                    digests
                        .entry(text_of(record, "expectation_id"))
                        .or_default()
                        .insert(text_of(record, "suite_entry_digest"));
                }
            }
        }
        let mut expected = expected_rows(&mini());
        mapped_rows.sort();
        expected.sort();
        assert_eq!(mapped_rows.len(), expected.len(), "number of records");
        assert_eq!(mapped_rows, expected);
        assert!(!digests.is_empty());
        for (id, set) in &digests {
            assert_eq!(set.len(), 1, "`{id}` has more than one suite entry digest");
        }
        // The synthetic cell words A1 differently, so its entry is another entry.
        let synthetic = imported
            .records_of(SYNTHETIC, 2)
            .iter()
            .find(|record| text_of(record, "expectation_id") == "A1")
            .map(|record| text_of(record, "suite_entry_digest"))
            .expect("a synthetic A1");
        assert!(
            digests
                .get("A1")
                .is_some_and(|set| !set.contains(&synthetic))
        );
    }

    /// The `(path, value)` pairs of the measured values of a serialized manifest.
    fn measured(manifest: &Value) -> Vec<(String, Value)> {
        let list = field(manifest, "measured")
            .as_array()
            .expect("measured is a list");
        let mut pairs: Vec<(String, Value)> = list
            .iter()
            // BORROW: `.clone()` copies the measured value
            .map(|item| (text_of(item, "path"), field(item, "value").clone()))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        pairs
    }

    /// Guards: D6 — a run's integrity, harness version and measured values equal the oracle's, the
    /// manifests list the factors that differ between the cells, and a run's start and end are the
    /// timestamps worked out by hand. Oracle: `oracle::expected_runs` for integrity, harness and
    /// measured values; the factor list computed with `jq` over the three `experiment.json` files by the
    /// rule itself: the leaves that differ (the model, the run count and three skill version
    /// spellings, `plan.out_dir` filed under `import`), minus those whose levels map one to one onto
    /// the cells' `<name>@<tag>` treatment strings (the three spellings), plus `request.treatment`
    /// because the three treatments differ. The `jq` rule, run as `jq -n -S -c -f rule.jq` over the
    /// three files with `inputs`:
    /// `def leaves: [paths(scalars) as $p | {key: ($p|map(tostring)|join(".")), value: getpath($p)}]
    /// | from_entries;`
    /// `[inputs | {t: .plan.skill_version, l: (leaves | del(.["plan.out_dir"]))}] as $cells`
    /// `| ($cells | map(.l | keys[]) | unique) as $paths`
    /// `| [ $paths[] | select(. as $p | ($cells | map(.l[$p]) | unique | length) > 1) ] as $varying`
    /// `| [ $varying[] | select(. as $p | ($cells | map([.t, .l[$p]]) | unique) as $pairs`
    /// `| ((($pairs|map(.[0])|unique|length) == ($pairs|length))`
    /// `and (($pairs|map(.[1])|unique|length) == ($pairs|length))) | not) ] as $kept`
    /// `| ($kept + (if ($cells|map(.t)|unique|length) > 1 then ["request.treatment"] else [] end))`
    /// `| sort`;
    /// start and
    /// end from `run_complete.json` and the `elapsed_seconds` of `progress.jsonl`:
    /// 2086.386 - 1882.076 = 204.310 s before `19:34:27.030762`, and 15 - 10 = 5 s before `00:00:11`.
    #[test]
    fn the_mapped_runs_are_the_oracle_runs() {
        let imported = Imported::read(&mini());
        let expected = expected_runs(&mini());
        assert_eq!(expected.len(), 6);
        assert_eq!(imported.mapped.len(), expected.len());
        for run in &expected {
            let manifest = imported.manifest(&run.cell, run.run_n);
            let integrity = field(&manifest, "integrity");
            match &run.integrity {
                ExpectedIntegrity::Valid => assert_eq!(text_of(integrity, "kind"), "valid"),
                ExpectedIntegrity::Invalid(reason) => {
                    assert_eq!(text_of(integrity, "kind"), "invalid");
                    assert_eq!(&text_of(integrity, "reason"), reason);
                }
            }
            let harness = field(field(&manifest, "comparable"), "harness");
            assert_eq!(text_of(harness, "version"), run.harness_version);
            let wanted: Vec<(String, Value)> = run
                .measured
                .iter()
                .map(|(path, json)| {
                    (
                        // BORROW: `.clone()` copies the path
                        path.clone(),
                        serde_json::from_str(json).expect("the oracle's JSON text parses"),
                    )
                })
                .collect();
            assert_eq!(
                measured(&manifest),
                wanted,
                "{} run-{}",
                run.cell,
                run.run_n
            );
            let factors: Vec<String> = field(&manifest, "factors")
                .as_array()
                .expect("factors is a list")
                .iter()
                // BORROW: `.as_str()` and `.to_owned()` make the owned factor path
                .map(|factor| factor.as_str().expect("a factor path").to_owned())
                .collect();
            assert_eq!(factors, ["plan.model", "plan.runs", "request.treatment",]);
        }
        let first = imported.manifest(V3, 6);
        assert_eq!(text_of(&first, "started_at"), "2026-08-05T19:31:02.720762Z");
        assert_eq!(
            text_of(&first, "ended_at"),
            "2026-08-05T19:34:27.030762+00:00"
        );
        let leaked = imported.manifest(SYNTHETIC, 1);
        assert_eq!(text_of(&leaked, "started_at"), "2026-08-06T00:00:06Z");
        assert_eq!(text_of(&leaked, "ended_at"), "2026-08-06T00:00:11+00:00");
    }

    /// The `[[vector]]` tables of a vector file of the oracle.
    fn vectors(name: &str) -> Vec<toml::Table> {
        let path = mini()
            .join("..")
            .join("..")
            .join("oracle/vectors")
            .join(name);
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let table: toml::Table = text.parse().unwrap_or_else(|e| panic!("{name}: {e}"));
        table
            .get("vector")
            .and_then(toml::Value::as_array)
            .unwrap_or_else(|| panic!("{name} has no vectors"))
            .iter()
            // BORROW: `.clone()` copies the vector table
            .map(|item| item.as_table().expect("a vector table").clone())
            .collect()
    }

    fn vector_text<'t>(vector: &'t toml::Table, key: &str) -> &'t str {
        vector
            .get(key)
            // BORROW: Value::as_str borrows the JSON string without copying
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("vector has no `{key}`"))
    }

    /// Guards: D6 — the mapped comparable and cell digest equal the `cell_digest.toml` vectors of the
    /// three mini-corpus cells, the request id of each real run equals `request_id.toml`, and the
    /// suite entry digest of one expectation equals the digest of its hand-written suite entry.
    /// Oracle: the vector files of the oracle, generated by `gen_vectors.py` with `rfc8785`; the suite
    /// entry JSON below and its digest are written by hand with `python3 -I` (`json.dumps` with sorted
    /// keys, compact separators, `sha256` of the UTF-8 bytes; the text is ASCII, so it is the RFC 8785
    /// form).
    #[test]
    fn the_mapped_identity_is_the_oracle_vectors() {
        let imported = Imported::read(&mini());
        let mut cells = 0;
        for vector in vectors("cell_digest.toml") {
            let cell = vector_text(&vector, "cell");
            let Some(run) = imported
                .folder
                .runs()
                .find(|(c, _)| c.name == cell)
                .map(|(_, r)| r.number)
            else {
                continue;
            };
            cells += 1;
            let manifest = imported.manifest(cell, run);
            let wanted: Value = serde_json::from_str(vector_text(&vector, "comparable"))
                .expect("the vector's comparable parses");
            assert_eq!(field(&manifest, "comparable"), &wanted, "{cell}");
            assert_eq!(
                text_of(field(&manifest, "run"), "cell"),
                vector_text(&vector, "digest"),
                "{cell}"
            );
        }
        assert_eq!(cells, 3);
        let mut requests = 0;
        for vector in vectors("request_id.toml") {
            let cell = vector_text(&vector, "cell");
            if !imported.folder.cells.iter().any(|c| c.name == cell) {
                continue;
            }
            requests += 1;
            let number = u32::try_from(
                vector
                    .get("run")
                    .and_then(toml::Value::as_integer)
                    .expect("the vector's run"),
            )
            .expect("a run number");
            let manifest = imported.manifest(cell, number);
            let listed = field(field(&manifest, "import"), "requests")
                .as_array()
                .expect("requests is a list");
            assert_eq!(listed.len(), 1);
            let request = listed.first().expect("one request");
            assert_eq!(
                text_of(request, "request"),
                vector_text(&vector, "request_id")
            );
            assert_eq!(text_of(request, "evaluator"), "htn-creator-eval@0.4.2");
        }
        assert_eq!(requests, 3);
        // The entry is {"depends_on":[],"kind":"legacy-check","params":{"scale":"binary","text":
        // "Running scripts/validate_domain.py on the generated domain.py exits with code 0 (no
        // structural errors or warnings)","tier":"structure"},"role":"gate"}.
        let a1 = imported
            .records_of(V3, 6)
            .iter()
            .find(|record| text_of(record, "expectation_id") == "A1")
            .expect("a record of A1");
        assert_eq!(
            text_of(a1, "suite_entry_digest"),
            "sha256:b0db76be61754c4fdffa4c605e1c2d9dd9f526c6f659550b16b920c374d44701"
        );
    }

    /// Guards: I10 — a contaminated run is `invalid` with the reason `input_leak` and has no outcome
    /// record, and an `incomplete` run is `valid` with every row it has. Oracle: the run listing of
    /// `PROVENANCE.json` read with `jq` (`synthetic_cell` run-1 is `contaminated`, `{V3}` run-17 is
    /// `incomplete`) and the 43 entries per real run counted by hand in the raw grading files.
    #[test]
    fn contaminated_runs_have_no_records_and_incomplete_runs_have_all() {
        let imported = Imported::read(&mini());
        let leaked = imported.manifest(SYNTHETIC, 1);
        let integrity = field(&leaked, "integrity");
        assert_eq!(text_of(integrity, "kind"), "invalid");
        assert_eq!(text_of(integrity, "reason"), "input_leak");
        assert_eq!(
            text_of(
                field(field(&leaked, "import"), "legacy_integrity"),
                "verdict"
            ),
            "contaminated"
        );
        assert_eq!(imported.records_of(SYNTHETIC, 1).len(), 0);
        let incomplete = imported.manifest(V3, 17);
        assert_eq!(text_of(field(&incomplete, "integrity"), "kind"), "valid");
        assert_eq!(
            text_of(
                field(field(&incomplete, "import"), "legacy_integrity"),
                "verdict"
            ),
            "incomplete"
        );
        assert_eq!(imported.records_of(V3, 17).len(), 43);
        assert_eq!(imported.records_of(V3, 6).len(), 43);
        assert_eq!(imported.records_of(V6, 12).len(), 43);
    }

    /// The files named `name` of `run-<number>` under the cell directory `cell`.
    fn run_files(root: &Path, cell: &str, number: u32, name: &str) -> Vec<PathBuf> {
        let tail = format!("/run-{number}/{name}");
        file_paths(&root.join(cell))
            .into_iter()
            .filter(|path| path.to_string_lossy().ends_with(&tail))
            .collect()
    }

    fn edit_json(path: &Path, edit: impl FnOnce(&mut Value)) {
        let text = fs::read_to_string(path).expect("read the file");
        let mut value: Value = serde_json::from_str(&text).expect("the file is JSON");
        edit(&mut value);
        fs::write(path, serde_json::to_string(&value).expect("serialize")).expect("write the file");
    }

    /// The entry of `PROVENANCE.json` for `V3` run-6.
    fn edit_provenance_verdict(root: &Path, verdict: &str) {
        edit_json(&root.join("PROVENANCE.json"), |value| {
            let runs = value
                .get_mut("runs")
                .and_then(Value::as_array_mut)
                .expect("runs");
            let entry = runs
                .iter_mut()
                .find(|entry| {
                    // BORROW: Value::as_str borrows the JSON string without copying
                    entry.get("cell").and_then(Value::as_str) == Some(V3)
                    // BORROW: Value::as_str borrows the JSON string without copying
                    && entry.get("run").and_then(Value::as_str) == Some("run-6")
                })
                .expect("the entry of run-6");
            *entry
                .get_mut("original_integrity")
                .and_then(|block| block.get_mut("verdict"))
                .expect("the verdict") = Value::String(verdict.into());
        });
    }

    /// Sets the status of A1 of `V3` run-6 in `expectations.jsonl`.
    fn edit_expectation_status(root: &Path, status: &str) {
        let path = root.join("expectations.jsonl");
        let text = fs::read_to_string(&path).expect("read expectations.jsonl");
        let mut out = String::new();
        let mut changed = 0;
        for line in text.lines() {
            let mut row: Value = serde_json::from_str(line).expect("a row");
            // BORROW: Value::as_str borrows the JSON string without copying
            if row.get("cell").and_then(Value::as_str) == Some(V3)
            // BORROW: Value::as_str borrows the JSON string without copying
            && row.get("run").and_then(Value::as_str) == Some("run-6")
            // BORROW: Value::as_str borrows the JSON string without copying
            && row.get("id").and_then(Value::as_str) == Some("A1")
            {
                *row.get_mut("status").expect("a status") = Value::String(status.into());
                changed += 1;
            }
            out.push_str(&serde_json::to_string(&row).expect("serialize"));
            out.push('\n');
        }
        assert_eq!(changed, 1);
        fs::write(&path, out).expect("write expectations.jsonl");
    }

    /// Sets the status of A1 in the grading file of `V3` run-6.
    fn edit_grading_status(root: &Path, status: &str) {
        let files = run_files(root, V3, 6, "eval-1-grading.json");
        assert_eq!(files.len(), 1);
        edit_json(files.first().expect("a grading file"), |value| {
            let entry = value
                .get_mut("expectations")
                .and_then(Value::as_array_mut)
                .and_then(|list| {
                    list.iter_mut()
                        // BORROW: Value::as_str borrows the JSON string without copying
                        .find(|entry| entry.get("id").and_then(Value::as_str) == Some("A1"))
                })
                .expect("A1");
            *entry.get_mut("status").expect("a status") = Value::String(status.into());
        });
    }

    /// Edits A1 of `V3` run-6 in `expectations.jsonl` with `listed` and in its grading file with
    /// `entry`, so that the two copies keep agreeing wherever the edits agree.
    fn edit_a1(root: &Path, listed: impl Fn(&mut Value), entry: impl Fn(&mut Value)) {
        let path = root.join("expectations.jsonl");
        let text = fs::read_to_string(&path).expect("read expectations.jsonl");
        let mut out = String::new();
        let mut changed = 0;
        for line in text.lines() {
            let mut row: Value = serde_json::from_str(line).expect("a row");
            // BORROW: Value::as_str borrows the JSON string without copying
            if row.get("cell").and_then(Value::as_str) == Some(V3)
            // BORROW: Value::as_str borrows the JSON string without copying
            && row.get("run").and_then(Value::as_str) == Some("run-6")
            // BORROW: Value::as_str borrows the JSON string without copying
            && row.get("id").and_then(Value::as_str) == Some("A1")
            {
                listed(&mut row);
                changed += 1;
            }
            out.push_str(&serde_json::to_string(&row).expect("serialize"));
            out.push('\n');
        }
        assert_eq!(changed, 1);
        fs::write(&path, out).expect("write expectations.jsonl");
        let files = run_files(root, V3, 6, "eval-1-grading.json");
        assert_eq!(files.len(), 1);
        edit_json(files.first().expect("a grading file"), |value| {
            let found = value
                .get_mut("expectations")
                .and_then(Value::as_array_mut)
                .and_then(|list| {
                    list.iter_mut()
                        // BORROW: Value::as_str borrows the JSON string without copying
                        .find(|item| item.get("id").and_then(Value::as_str) == Some("A1"))
                })
                .expect("A1");
            entry(found);
        });
    }

    /// Sets `key` of both copies of A1 to `value`.
    fn set_a1(root: &Path, key: &'static str, value: &Value) {
        let both = |item: &mut Value| {
            // BORROW: `.clone()` copies the value into each copy
            item[key] = value.clone();
        };
        edit_a1(root, both, both);
    }

    /// Removes `key` from both copies of A1.
    fn remove_a1(root: &Path, key: &'static str) {
        let both = |item: &mut Value| {
            item.as_object_mut().expect("an object").remove(key);
        };
        edit_a1(root, both, both);
    }

    /// Makes A1 a `status` row weighing `score/max_score`.
    fn grade_a1(root: &Path, status: &str, score: u64, max: u64) {
        set_a1(root, "status", &Value::String(status.into()));
        set_a1(root, "score", &Value::from(score));
        set_a1(root, "max_score", &Value::from(max));
    }

    /// Sets `blocked_by` of the grading copy of A1 only; the listed row has no such key.
    fn block_a1(root: &Path, blocked_by: &Value) {
        edit_a1(root, |_| {}, |item| item["blocked_by"] = blocked_by.clone());
    }

    /// A name, the edit of A1, the expected status, the expected weight and the expected cause.
    type UnusualCase<'a> = (&'a str, fn(&Path), &'a str, Option<&'a str>, &'a str);

    /// Reads, maps and grades every run of the folder at `root`.
    fn import_all(root: &Path, resolved: &str) -> Result<usize, ImportError> {
        let rules = legacy_rules::try_rules(resolved)?;
        let folder = LegacyFolder::read(root, &rules)?;
        let mapped = map_runs(&folder, &rules)?;
        let mut records = 0;
        for run in &mapped {
            records += map_grading(&folder, run, &rules)?.len();
        }
        Ok(records)
    }

    /// One injected fault: what it is, how to make it, the variant of error that refuses it and the
    /// texts the error names.
    struct Fault {
        name: &'static str,
        inject: fn(&Path),
        refused: fn(&ImportError) -> bool,
        names: &'static [&'static str],
        resolved: &'static str,
    }

    fn faults() -> Vec<Fault> {
        vec![
            Fault {
                name: "an unknown verdict",
                inject: |root| edit_provenance_verdict(root, "murky"),
                refused: |e| matches!(e, ImportError::Verdict { .. }),
                names: &["PROVENANCE.json", "run-6", "murky"],
                resolved: RESOLVED,
            },
            Fault {
                name: "PROVENANCE and the contamination catalog disagree",
                inject: |root| edit_provenance_verdict(root, "incomplete"),
                refused: |e| matches!(e, ImportError::Disagree { .. }),
                names: &["PROVENANCE.json", "contamination_catalog.jsonl", "run-6"],
                resolved: RESOLVED,
            },
            Fault {
                name: "eval_metadata provenance copies disagree within a run",
                inject: |root| {
                    let files = run_files(root, V3, 6, "eval_metadata.json");
                    assert!(files.len() > 1);
                    edit_json(files.first().expect("a copy"), |value| {
                        *value
                            .pointer_mut("/provenance/planner/sandbox/version")
                            .expect("the sandbox version") = Value::String("9.9.9".into());
                    });
                },
                refused: |e| matches!(e, ImportError::Disagree { .. }),
                names: &["eval_metadata.json", "run-6", "9.9.9"],
                resolved: RESOLVED,
            },
            Fault {
                name: "two harness versions in a cell",
                inject: |root| {
                    let files = run_files(root, V3, 17, "agent_stream.jsonl.gz");
                    assert_eq!(files.len(), 1);
                    let path = files.first().expect("a stream");
                    let mut text = String::new();
                    MultiGzDecoder::new(fs::File::open(path).expect("open"))
                        .read_to_string(&mut text)
                        .expect("gunzip");
                    assert!(text.contains("2.1.220"));
                    let text = text.replacen("2.1.220", "2.1.221", 1);
                    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                    encoder.write_all(text.as_bytes()).expect("gzip");
                    fs::write(path, encoder.finish().expect("finish")).expect("write");
                },
                refused: |e| matches!(e, ImportError::Disagree { .. }),
                names: &["2.1.220", "2.1.221", "run-17"],
                resolved: RESOLVED,
            },
            Fault {
                name: "copied_inputs disagree within a cell",
                inject: |root| {
                    let files = run_files(root, V3, 17, "timing.json");
                    assert_ne!(files.len(), 0);
                    for file in &files {
                        edit_json(file, |value| {
                            let input = value
                                .pointer_mut("/copied_inputs/0/sha256")
                                .expect("a copied input");
                            *input = Value::String("0".repeat(64));
                        });
                    }
                },
                refused: |e| matches!(e, ImportError::Disagree { .. }),
                names: &["timing.json", "run-17"],
                resolved: RESOLVED,
            },
            Fault {
                name: "a run without outputs/",
                inject: |root| {
                    let path = root.join(V3).join("_run_cache/with_skill/run-6/outputs");
                    fs::remove_dir_all(path).expect("remove outputs/");
                },
                refused: |e| matches!(e, ImportError::NoOutputs { .. }),
                names: &["run-6", "outputs"],
                resolved: RESOLVED,
            },
            Fault {
                name: "a status differs between expectations.jsonl and its grading file",
                inject: |root| edit_grading_status(root, "fail"),
                refused: |e| matches!(e, ImportError::Disagree { .. }),
                names: &["eval-1-grading.json", "expectations.jsonl", "run-6"],
                resolved: RESOLVED,
            },
            Fault {
                name: "an unknown legacy status",
                inject: |root| {
                    edit_expectation_status(root, "maybe");
                    edit_grading_status(root, "maybe");
                },
                refused: |e| matches!(e, ImportError::Status { .. }),
                names: &["expectations.jsonl", "run-6", "maybe", "A1"],
                resolved: RESOLVED,
            },
            Fault {
                name: "a grading file name in two eval directories",
                inject: |root| {
                    let from = run_files(root, V3, 6, "eval-1-grading.json");
                    let others = run_files(root, V3, 6, "eval_metadata.json");
                    let from = from.first().expect("a grading file");
                    let dir = from.parent().expect("a directory");
                    let other = others
                        .iter()
                        .filter_map(|path| path.parent())
                        .find(|parent| *parent != dir)
                        .expect("another eval directory");
                    fs::copy(from, other.join("eval-1-grading.json")).expect("copy");
                },
                refused: |e| matches!(e, ImportError::DuplicateGrading { .. }),
                names: &["eval-1-grading.json", "run-6"],
                resolved: RESOLVED,
            },
            Fault {
                name: "a fail that weighs 0/0",
                inject: |root| grade_a1(root, "fail", 0, 0),
                refused: |e| matches!(e, ImportError::Layout { .. }),
                names: &["eval-1-grading.json", "run-6", "A1"],
                resolved: RESOLVED,
            },
            Fault {
                name: "a blocked row that weighs 0/0",
                inject: |root| grade_a1(root, "blocked", 0, 0),
                refused: |e| matches!(e, ImportError::Layout { .. }),
                names: &["eval-1-grading.json", "run-6", "A1"],
                resolved: RESOLVED,
            },
            Fault {
                name: "a row without a score",
                inject: |root| remove_a1(root, "score"),
                refused: |e| matches!(e, ImportError::Layout { .. }),
                names: &["eval-1-grading.json", "run-6", "A1"],
                resolved: RESOLVED,
            },
            Fault {
                name: "a row without a maximum score",
                inject: |root| remove_a1(root, "max_score"),
                refused: |e| matches!(e, ImportError::Layout { .. }),
                names: &["eval-1-grading.json", "run-6", "A1"],
                resolved: RESOLVED,
            },
            Fault {
                name: "a blocked_by that is an absolute path",
                inject: |root| {
                    grade_a1(root, "blocked", 0, 1);
                    block_a1(root, &Value::String("/etc/passwd".into()));
                },
                refused: |e| matches!(e, ImportError::Layout { .. }),
                names: &["eval-1-grading.json", "run-6", "A1", "/etc/passwd"],
                resolved: RESOLVED,
            },
            Fault {
                name: "a blocked_by that climbs out with ..",
                inject: |root| {
                    grade_a1(root, "blocked", 0, 1);
                    block_a1(root, &Value::String("../outside.py".into()));
                },
                refused: |e| matches!(e, ImportError::Layout { .. }),
                names: &["eval-1-grading.json", "run-6", "A1", "../outside.py"],
                resolved: RESOLVED,
            },
            Fault {
                name: "a blocked_by that is empty",
                inject: |root| {
                    grade_a1(root, "blocked", 0, 1);
                    block_a1(root, &Value::String(String::new()));
                },
                refused: |e| matches!(e, ImportError::Layout { .. }),
                names: &["eval-1-grading.json", "run-6", "A1", "relative path"],
                resolved: RESOLVED,
            },
            Fault {
                name: "a blocked_by that is not a string",
                inject: |root| {
                    grade_a1(root, "blocked", 0, 1);
                    block_a1(root, &Value::from(7));
                },
                refused: |e| matches!(e, ImportError::Layout { .. }),
                names: &["eval-1-grading.json", "run-6", "A1"],
                resolved: RESOLVED,
            },
            Fault {
                name: "a blocked_by on a row that is not blocked",
                inject: |root| block_a1(root, &Value::String("domain.py".into())),
                refused: |e| matches!(e, ImportError::Layout { .. }),
                names: &["eval-1-grading.json", "run-6", "A1"],
                resolved: RESOLVED,
            },
            Fault {
                name: "a resolved-version key that is not a top-level key",
                inject: |_| {},
                refused: |e| matches!(e, ImportError::Rules { .. }),
                names: &["treatment.resolved", "plan.skill_version"],
                resolved: "plan.skill_version",
            },
        ]
    }

    /// Guards: D6 — a fault in the folder is refused with an error that names the file and the
    /// run, and the same folder without the fault imports: the unmodified copy is the positive
    /// control. Oracle: the faults are written by hand, one per row of the table, into a copy of the
    /// mini-corpus; the expected variant and the file and run names come from the error contract of
    /// the leaf and are literals, and the control imports the 136 rows counted by hand.
    #[test]
    fn every_fault_is_refused_naming_its_file_and_run() {
        let control = tempfile::tempdir().expect("a temporary directory");
        copy_tree(&mini(), control.path());
        assert_eq!(
            import_all(control.path(), RESOLVED).expect("the control imports"),
            136
        );
        for fault in faults() {
            let copy = tempfile::tempdir().expect("a temporary directory");
            copy_tree(&mini(), copy.path());
            (fault.inject)(copy.path());
            let error = import_all(copy.path(), fault.resolved)
                .expect_err(&format!("`{}` is not refused", fault.name));
            assert!(
                (fault.refused)(&error),
                "{}: wrong error {error:?}",
                fault.name
            );
            let message = error.to_string();
            for name in fault.names {
                assert!(
                    message.contains(name),
                    "{}: the error does not name `{name}`: {message}",
                    fault.name
                );
            }
        }
    }

    /// Guards: D6 — a `pass` weighing 0/0 gets the cause `{legacy: zero_weight}` and keeps its 0/0
    /// weight, and a blocked row with no `blocked_by`, or a null one, gets the cause
    /// `{legacy: unrecorded}` with the default weight left out. Oracle: the mapping rules of the
    /// `legacy_mapping` leaf (D6, step 2), written here as literals.
    #[test]
    fn unusual_grades_get_the_causes_the_leaf_names() {
        let a1 = |root: &Path| -> ExpectedRow {
            let imported = Imported::read(root);
            let record = imported
                .records_of(V3, 6)
                .iter()
                .find(|record| text_of(record, "expectation_id") == "A1")
                .expect("A1 of run-6");
            project(V3, 6, record)
        };
        let cases: [UnusualCase<'_>; 3] = [
            (
                "pass 0/0",
                |root| grade_a1(root, "pass", 0, 0),
                "pass",
                Some("0/0"),
                "legacy: zero_weight",
            ),
            (
                "blocked without blocked_by",
                |root| grade_a1(root, "blocked", 0, 1),
                "blocked",
                None,
                "legacy: unrecorded",
            ),
            (
                "blocked with a null blocked_by",
                |root| {
                    grade_a1(root, "blocked", 0, 1);
                    block_a1(root, &Value::Null);
                },
                "blocked",
                None,
                "legacy: unrecorded",
            ),
        ];
        for (name, inject, status, weight, cause) in cases {
            let copy = tempfile::tempdir().expect("a temporary directory");
            copy_tree(&mini(), copy.path());
            inject(copy.path());
            let row = a1(copy.path());
            assert_eq!(row.status, status, "{name}");
            assert_eq!(row.weight.as_deref(), weight, "{name}");
            assert_eq!(row.cause.as_deref(), Some(cause), "{name}");
        }
    }

    /// The hex digest of `bytes` as `sha256:<hex>`, computed here and not by the importer.
    fn digest(bytes: &[u8]) -> String {
        let mut hex = String::new();
        for byte in sha2::Sha256::digest(bytes) {
            hex.extend(char::from_digit(u32::from(byte >> 4), 16));
            hex.extend(char::from_digit(u32::from(byte & 0xf), 16));
        }
        format!("sha256:{hex}")
    }

    /// Guards: I12 — every file of the folder is exactly one of a source of the folder, a cell or a
    /// run, an artifact under `outputs/`, a stream or stderr file, or a skipped entry whose directory
    /// covers the files below it; a stream or stderr file is a stream that is also among its run's
    /// sources, with the same digest; every file of the model exists on disk, with the digest of its
    /// bytes; a file the model neither holds nor lists as skipped with its reason would be left out
    /// silently. Oracle: a `std::fs` walk of the mini-corpus and a SHA-256 of each file's bytes made in
    /// the test.
    #[test]
    fn every_file_is_accounted_for_exactly_once() {
        let imported = Imported::read(&mini());
        let folder = &imported.folder;
        let mut sources: BTreeMap<String, &SourceFile> = BTreeMap::new();
        let mut artifacts: BTreeMap<String, &SourceFile> = BTreeMap::new();
        let mut streams: BTreeMap<String, &SourceFile> = BTreeMap::new();
        for file in &folder.sources {
            // BORROW: `.to_owned()` makes the owned path key
            sources.insert(file.path.as_str().to_owned(), file);
        }
        for cell in &folder.cells {
            for file in &cell.sources {
                // BORROW: `.to_owned()` makes the owned path key
                sources.insert(file.path.as_str().to_owned(), file);
            }
            for run in &cell.runs {
                for file in &run.sources {
                    // BORROW: `.to_owned()` makes the owned path key
                    sources.insert(file.path.as_str().to_owned(), file);
                }
                for file in &run.outputs {
                    // BORROW: `.to_owned()` makes the owned path key
                    artifacts.insert(file.path.as_str().to_owned(), file);
                }
                for file in std::iter::once(&run.stream).chain(run.stderr.iter()) {
                    // BORROW: `.to_owned()` makes the owned path key
                    streams.insert(file.path.as_str().to_owned(), file);
                }
            }
        }
        let skipped: Vec<String> = folder
            .skipped
            .iter()
            // BORROW: `.to_owned()` makes the owned path
            .map(|entry| entry.path.as_str().to_owned())
            .collect();
        assert_ne!(skipped.len(), 0);
        let root = mini();
        let mut counts = [0_usize; 4];
        for path in file_paths(&root) {
            let relative = path
                .strip_prefix(&root)
                .expect("under the root")
                .to_string_lossy()
                .into_owned();
            let covered = skipped
                .iter()
                .any(|entry| relative == *entry || relative.starts_with(&format!("{entry}/")));
            // A stream or stderr file is also a run source and counts as a stream only.
            let held = [
                sources
                    .get(&relative)
                    .filter(|_| !streams.contains_key(&relative)),
                artifacts.get(&relative),
                streams.get(&relative),
            ];
            let kinds = held.iter().flatten().count() + usize::from(covered);
            assert_eq!(kinds, 1, "`{relative}` is accounted for {kinds} times");
            for (slot, file) in held.iter().enumerate() {
                if let Some(file) = file {
                    let bytes = fs::read(&path).expect("read the file");
                    // BORROW: explicit `.as_str()` instead of Deref coercion
                    assert_eq!(file.path.as_str(), relative);
                    assert_eq!(file.sha256.to_string(), digest(&bytes), "{relative}");
                    assert_eq!(file.bytes, bytes, "{relative}");
                    if let Some(count) = counts.get_mut(slot) {
                        *count += 1;
                    }
                }
            }
            if covered && let Some(count) = counts.get_mut(3) {
                *count += 1;
            }
        }
        assert!(counts.iter().all(|&count| count > 0), "{counts:?}");
        for (path, stream) in &streams {
            let source = sources.get(path).unwrap_or_else(|| {
                panic!("the stream file `{path}` is not among the run's sources")
            });
            assert_eq!(source.sha256, stream.sha256, "{path}");
        }
        let on_disk: BTreeSet<String> = file_paths(&root)
            .iter()
            .map(|path| {
                path.strip_prefix(&root)
                    .expect("under the root")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        for path in sources.keys().chain(artifacts.keys()).chain(streams.keys()) {
            assert!(
                on_disk.contains(path),
                "the model holds `{path}`, which is not on disk"
            );
        }
    }
}
