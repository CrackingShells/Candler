//! Guards for the import of a legacy results folder into a store, over the vendored mini-corpus.
//! The stored records are read back through the store's own readers. The expected rows come from
//! the independent oracle; the counts are literals written by hand after reading the grading
//! files with `jq`: the three real runs hold 43 expectations each, the synthetic runs 2, 5 and 2.
#![cfg(test)]

use crate::oracle;
use crate::support;
use crate::support::legacy_import;
use crate::support::legacy_rules;

use std::collections::BTreeSet;
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
use candler_core::import::rules::ImportRules;
use candler_core::store::Store;
use candler_harness::reader::TraceReader;
use candler_schemas::primitives::{Digest, RunKey};
use flate2::Compression;
use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use legacy_import::{
    SYNTHETIC, assert_same_store, cell_of, copy_tree, field, import_with_reader, mini, project,
    reader, snapshot, text_of,
};
use oracle::{ExpectedRow, expected_rows};
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
    let readers = move || -> Box<dyn TraceReader> {
        assert!(
            calls.fetch_add(1, Ordering::SeqCst) != fail_at,
            "an injected fault while the stream {fail_at} is read"
        );
        reader()
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
