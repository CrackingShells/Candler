#![cfg(test)]
//! Guards for the run store: what a stopped writer leaves, append-only files, a copied store and
//! the frozen layout. Every check of stored files reads them with `std::fs` and
//! `serde_json`, never through the store's own readers alone.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use candler_core::store::Store;
use candler_core::store::outcome::OutcomeWriter;
use candler_core::store::run::{Existing, RunWriter};
use candler_schemas::execution::{RunManifest, RunTree};
use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::{Digest, NameVersion, RequestId, RunKey};
use candler_schemas::raw::StreamName;
use candler_schemas::trace::TraceFile;
use flate2::Compression;
use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use serde_json::{Value, json};
use sha2::Digest as _;
use tempfile::TempDir;

const EVALUATOR: &str = "htn-creator-eval@0.4.2";

fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

/// A request id: a version 7 UUID, lowercase and hyphenated, numbered by `n`.
fn request(n: u64) -> RequestId {
    format!("01926b3a-7c3e-7a1b-8c4d-{n:012x}")
        .parse()
        .expect("a request id")
}

fn manifest_json(cell: &str, value: i64, replicate: u32, attempt: u32) -> Value {
    json!({
        "schema": "run-manifest/1",
        "run": {"cell": cell, "replicate": replicate, "attempt": attempt},
        "experiment": digest_text('0'),
        "factors": [],
        "comparable": {
            "values": [{"path": "plan.a", "value": value}],
            "harness": {"name": "h", "version": "1.0.0"},
            "treatment": {"name": "t"},
            "snapshots": [],
            "inputs": [],
            "environments": []
        },
        "sources": [
            {"path": "plan.a", "source": {"kind": "experiment_file"}},
            {"path": "harness", "source": {"kind": "experiment_file"}},
            {"path": "treatment", "source": {"kind": "experiment_file"}}
        ],
        "secrets": [],
        "measured": [],
        "deviations": [],
        "integrity": {"kind": "valid"}
    })
}

/// A manifest of the cell whose one value is `value`, as replicate `replicate` and attempt
/// `attempt`; the run key carries the cell's real digest.
fn manifest_of(value: i64, replicate: u32, attempt: u32) -> RunManifest {
    let probe: RunManifest =
        serde_json::from_value(manifest_json(&digest_text('f'), value, 1, 1)).expect("a manifest");
    let cell = probe.cell_digest().to_string();
    serde_json::from_value(manifest_json(&cell, value, replicate, attempt)).expect("a manifest")
}

fn manifest(attempt: u32) -> RunManifest {
    manifest_of(1, 1, attempt)
}

/// A run tree with one file, written by hand.
fn tree() -> RunTree {
    serde_json::from_value(json!({
        "schema": "run-tree/1",
        "entries": [{"kind": "file", "path": "a.txt", "sha256": digest_text('a'), "executable": false}]
    }))
    .expect("a run tree")
}

/// A trace file of one session and one event, written by hand from the architecture's trace model.
fn trace() -> TraceFile {
    let lines = [
        r#"{"kind":"header","schema":"normalised-trace/1","reader":"claude-code-reader@0.1.0","raw":[{"stream":"stdout","digest":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}]}"#,
        r#"{"kind":"session","id":"root-1","source":{"harness":"claude-code","harness_version":"2.1.0","reader":"claude-code-reader","reader_version":"0.1.0"},"model":"claude-opus-4","started_at":"2026-11-05T10:00:00Z","ended_by":{"kind":"cap","cap":"turns"},"raw_ref":{"stream":"stdout","frame":0}}"#,
        r#"{"kind":"event","session":"root-1","event":{"seq":0,"kind":{"kind":"user_message","text":"hi"},"raw_ref":{"stream":"stdout","frame":0},"ext":{}}}"#,
    ];
    let mut text = lines.join("\n");
    text.push('\n');
    TraceFile::read(text.as_bytes()).expect("a trace file")
}

/// An outcome record of `run` for `request`.
fn record(run: &RunKey, request: &RequestId) -> OutcomeRecord {
    serde_json::from_value(json!({
        "run": {"cell": run.cell().to_string(), "replicate": run.replicate(), "attempt": run.attempt()},
        "expectation_id": "A1",
        "role": "gate",
        "evaluator": EVALUATOR,
        "instruments": [],
        "suite_entry_digest": digest_text('1'),
        "context_digest": digest_text('2'),
        "request_id": request.to_string(),
        "status": "pass",
        "weight": {"num": 1, "den": 1},
        "evidence": [],
    }))
    .expect("an outcome record")
}

fn evaluator() -> NameVersion {
    EVALUATOR.parse().expect("name@version")
}

/// A finished `.gz` of two JSON lines in `dir`, with the digest of its bytes taken by `sha2`.
fn gz_file(dir: &Path, name: &str) -> (PathBuf, Digest) {
    let path = dir.join(name);
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(b"{\"kind\":\"header\"}\n{\"kind\":\"frame\"}\n")
        .expect("gzip");
    let bytes = encoder.finish().expect("gzip");
    fs::write(&path, &bytes).expect("write the gz");
    let mut hex = String::new();
    for byte in sha2::Sha256::digest(&bytes) {
        write!(hex, "{byte:02x}").expect("write to a string");
    }
    (path, Digest::from_hex(&hex).expect("a digest"))
}

fn stdout() -> StreamName {
    "stdout".parse().expect("a stream name")
}

/// Writes steps 1 to `upto` of a run, the manifest being step 0: 1 the tree, 2 the raw file, 3 the
/// trace. The writer is returned unsealed.
fn drive(store: &Store, manifest: &RunManifest, existing: Existing, upto: usize) -> RunWriter {
    let mut writer = RunWriter::create(store, manifest, existing).expect("create the run");
    let scratch = TempDir::new().expect("a scratch directory");
    if upto >= 1 {
        writer.tree(&tree()).expect("the tree");
    }
    if upto >= 2 {
        let (gz, digest) = gz_file(scratch.path(), "stdout.gz");
        writer.raw(&stdout(), &gz, &digest).expect("the raw file");
    }
    if upto >= 3 {
        writer.trace(&trace()).expect("the trace");
    }
    writer
}

/// Every regular file under `root`, by `/`-separated relative path.
/// Copies the tree at `from` to `to`, directory by directory, with `std::fs` alone.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a listing") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copy");
        }
    }
}

fn files_of(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).expect("list a directory") {
            let entry = entry.expect("a directory entry");
            let kind = entry.file_type().expect("a file type");
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .expect("below the root")
                    .to_string_lossy()
                    .into_owned(); // BORROW: .into_owned() makes the owned path text
                found.insert(relative, fs::read(entry.path()).expect("read a file"));
            }
        }
    }
    found
}

/// Every directory under `root`, by relative path.
fn dirs_of(root: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).expect("list a directory") {
            let entry = entry.expect("a directory entry");
            if entry.file_type().expect("a file type").is_dir() {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .expect("below the root")
                    .to_string_lossy()
                    .into_owned(); // BORROW: .into_owned() makes the owned path text
                found.insert(relative);
                pending.push(entry.path());
            }
        }
    }
    found
}

/// Parses `bytes` the way the file's name says: `.json` as one value, `.jsonl` as one value per
/// line, `.jsonl.gz` as gunzipped lines. Any other name is a failure.
fn parse_completely(name: &str, bytes: &[u8]) {
    if name.ends_with(".jsonl.gz") {
        let mut plain = Vec::new();
        MultiGzDecoder::new(bytes)
            .read_to_end(&mut plain)
            .expect("a complete gzip member");
        parse_lines(name, &plain);
    } else if name == "run.done" {
        // The seal is a presence marker.
    } else {
        match name.rsplit('.').next() {
            Some("jsonl") => parse_lines(name, bytes),
            Some("json") => {
                serde_json::from_slice::<Value>(bytes).expect("a complete JSON document");
            }
            Some(_) | None => panic!("{name} is not a file a run directory holds"),
        }
    }
}

fn parse_lines(name: &str, bytes: &[u8]) {
    assert!(bytes.ends_with(b"\n"), "{name} ends with a line feed");
    for line in bytes.split(|byte| *byte == b'\n').filter(|l| !l.is_empty()) {
        serde_json::from_slice::<Value>(line).expect("a complete JSON line");
    }
}

/// Guards: Cond 6 — a run stopped after any step leaves only complete files, no `run.done`, and no
/// temporary or `.partial` name a reader lists; a retry with `ReplaceUnfinished` succeeds and keeps
/// the old directory, and `seal_interrupted` seals it `invalid {interrupted}` so the next attempt
/// number is one higher. Oracle: an independent `serde_json` walk of the directory, and the files
/// each step adds, listed by hand from R01 §Run store; the finished run is the positive control.
#[test]
fn a_run_stopped_after_step_k_leaves_complete_files_and_can_be_retried_or_sealed() {
    // Files present once step k has run: the manifest at create, then the tree, the raw file, the
    // trace, and the seal.
    let added: [&str; 5] = [
        "manifest.json",
        "tree.json",
        "raw/stdout.jsonl.gz",
        "trace.jsonl",
        "run.done",
    ];
    for k in 0..=4_usize {
        let scratch = TempDir::new().expect("a store directory");
        let store = Store::init(scratch.path()).expect("init");
        let run = manifest(1);
        let key = run.run().clone(); // BORROW: the key outlives the manifest's borrow
        let run_dir = scratch.path().join("runs").join(key.to_string());

        let writer = drive(&store, &run, Existing::Refuse, k.min(3));
        if k == 4 {
            writer.done().expect("seal the run");
        } else {
            drop(writer);
        }

        let files = files_of(&run_dir);
        let expected: BTreeSet<&str> = added.iter().take(k.saturating_add(1)).copied().collect();
        // BORROW: explicit .as_str() instead of Deref coercion
        let present: BTreeSet<&str> = files.keys().map(String::as_str).collect();
        assert_eq!(present, expected, "files after step {k}");
        for (name, bytes) in &files {
            // BORROW: explicit .as_str() instead of Deref coercion
            parse_completely(name.as_str(), bytes); // BORROW: explicit .as_str() instead of Deref coercion
        }
        let entries = store.runs().expect("list the runs");
        assert_eq!(entries.len(), 1);
        // INDEX: the listing holds exactly the one run, as just asserted
        assert_eq!(entries[0].done, k == 4); // INDEX: the key and the listing are the ones just asserted to exist
        assert_eq!(store.outcome_files(&key).expect("list").len(), 0);

        if k == 4 {
            // Positive control: a finished run is refused a retry and a seal, and its next attempt is 2.
            let refused = RunWriter::create(&store, &run, Existing::ReplaceUnfinished);
            assert!(refused.is_err(), "a finished run is never replaced");
            assert!(store.seal_interrupted(&key).is_err());
            assert_eq!(store.next_attempt(key.cell(), 1).expect("next"), 2);
            continue;
        }

        // A plain create is refused; ReplaceUnfinished moves the old directory aside and writes anew.
        assert!(RunWriter::create(&store, &run, Existing::Refuse).is_err());
        let again = RunWriter::create(&store, &run, Existing::ReplaceUnfinished).expect("retry");
        drop(again);
        let aside = scratch
            .path()
            .join("runs/.replaced")
            .join(format!("{key}-1"));
        assert_eq!(
            files_of(&aside).len(),
            files.len(),
            "the old run is kept whole"
        );
        assert_eq!(
            files_of(&run_dir).len(),
            1,
            "the retry holds its manifest only"
        );
        assert_eq!(store.runs().expect("list the runs").len(), 1);

        // Sealing: the attempt stays in the ledger, invalid, and the next attempt is one higher.
        let sealed = store.seal_interrupted(&key).expect("seal the run");
        assert_eq!(sealed.run(), &key);
        let stored: Value =
            serde_json::from_slice(&fs::read(run_dir.join("manifest.json")).expect("read"))
                .expect("a manifest");
        assert_eq!(
            stored["integrity"], // INDEX: the key and the listing are the ones just asserted to exist
            json!({"kind": "invalid", "reason": "interrupted"})
        );
        assert!(run_dir.join("run.done").is_file());
        assert_eq!(store.next_attempt(key.cell(), 1).expect("next"), 2);
        assert!(
            store.seal_interrupted(&key).is_err(),
            "a sealed run is not sealed twice"
        );
        // Another replicate and another cell do not count.
        assert_eq!(store.next_attempt(key.cell(), 2).expect("next"), 1);
        let other = manifest_of(2, 1, 1);
        assert_eq!(store.next_attempt(other.run().cell(), 1).expect("next"), 1);
    }
}

/// Writes one outcome file of `run` with `n` as its request number.
fn put_outcome(store: &Store, run: &RunKey, n: u64) {
    let id = request(n);
    let mut writer = OutcomeWriter::create(store, run, &evaluator(), &id).expect("create");
    writer.append(&record(run, &id)).expect("append");
    writer.commit().expect("commit");
}

/// Guards: I11 — a blob is stored read-only (mode 0444) and never rewritten, a blob offered with a
/// digest it does not have is refused, a finished run is never created again, and a final outcome
/// file is never committed over. Oracle: `std::fs` metadata and bytes; each refusal sits beside the
/// same call made validly.
#[test]
fn stored_files_are_append_only() {
    let scratch = TempDir::new().expect("a store directory");
    let store = Store::init(scratch.path()).expect("init");

    // Blobs: mode, the digest check, and no rewrite.
    let digest = store.put_blob(b"payload", None).expect("a blob");
    let path = store.blob_path(&digest);
    let before = fs::metadata(&path).expect("stat");
    assert_eq!(before.permissions().mode() & 0o777, 0o444);
    let wrong: Digest = digest_text('9').parse().expect("a digest");
    assert!(store.put_blob(b"other", Some(&wrong)).is_err());
    assert!(
        !store.blob_path(&wrong).exists(),
        "a refused blob leaves nothing"
    );
    assert_eq!(
        store
            .put_blob(b"payload", Some(&digest))
            .expect("same bytes"),
        digest
    );
    let after = fs::metadata(&path).expect("stat");
    assert_eq!(
        (before.ino(), before.mtime_nsec()),
        (after.ino(), after.mtime_nsec())
    );
    let source = scratch.path().join("source.bin");
    fs::write(&source, b"streamed").expect("write");
    let streamed = store
        .put_blob_from_path(&source, None)
        .expect("a streamed blob");
    assert_eq!(
        fs::metadata(store.blob_path(&streamed))
            .expect("stat")
            .permissions()
            .mode()
            & 0o777,
        0o444
    );
    assert!(store.put_blob_from_path(&source, Some(&wrong)).is_err());

    // A finished run cannot be created again; another attempt can.
    let run = manifest(1);
    drive(&store, &run, Existing::Refuse, 3)
        .done()
        .expect("seal");
    assert!(RunWriter::create(&store, &run, Existing::Refuse).is_err());
    assert!(RunWriter::create(&store, &run, Existing::ReplaceUnfinished).is_err());
    assert!(RunWriter::create(&store, &manifest(2), Existing::Refuse).is_ok());
    assert_eq!(store.read_manifest(run.run()).expect("read"), run);

    // A final outcome file is never replaced or written through: a writer opened before another one
    // commits cannot publish over its file, and its appends do not reach it.
    let key = run.run().clone(); // BORROW: the key outlives the manifest
    let id = request(1);
    let mut late = OutcomeWriter::create(&store, &key, &evaluator(), &id).expect("create");
    let mut first = OutcomeWriter::create(&store, &key, &evaluator(), &id).expect("create");
    first.append(&record(&key, &id)).expect("append");
    first.commit().expect("the first commit publishes");
    let final_path = scratch
        .path()
        .join(format!("outcomes/{key}/{EVALUATOR}/{id}.jsonl"));
    let published = fs::read(&final_path).expect("read");
    assert_eq!(
        published.split(|b| *b == b'\n').count(),
        2,
        "one line and its line feed"
    );
    late.append(&record(&key, &id)).expect("append");
    late.append(&record(&key, &id)).expect("append");
    assert!(late.commit().is_err());
    assert!(OutcomeWriter::create(&store, &key, &evaluator(), &id).is_err());
    assert_eq!(fs::read(&final_path).expect("read"), published);
}

/// Guards: I17 — a store copied with `cp -R` to another directory reads back byte for byte: every
/// file, and the manifests, trees, traces and outcomes through the readers; no stored file holds
/// the original root. Oracle: `cp -R` and a byte comparison of the two directory walks.
#[test]
fn a_copied_store_reads_byte_equal() {
    let scratch = TempDir::new().expect("a directory");
    let original = scratch.path().join("original");
    let store = Store::init(&original).expect("init");
    let run = manifest(1);
    drive(&store, &run, Existing::Refuse, 3)
        .done()
        .expect("seal");
    let key = run.run().clone(); // BORROW: the key outlives the manifest
    put_outcome(&store, &key, 1);
    put_outcome(&store, &key, 2);
    store.put_blob(b"payload", None).expect("a blob");

    let copy = scratch.path().join("copy");
    copy_tree(&original, &copy);
    assert_eq!(files_of(&original), files_of(&copy));
    assert!(files_of(&copy).len() >= 9, "the walk saw the whole store");

    let moved = Store::open(&copy).expect("open the copy");
    assert_eq!(
        moved.read_manifest(&key).expect("read"),
        store.read_manifest(&key).expect("read")
    );
    assert_eq!(
        moved.read_tree(&key).expect("read"),
        store.read_tree(&key).expect("read")
    );
    assert_eq!(
        moved.read_trace(&key).expect("read"),
        store.read_trace(&key).expect("read")
    );
    assert_eq!(moved.runs().expect("list"), store.runs().expect("list"));
    let files = moved.outcome_files(&key).expect("list");
    assert_eq!(files.len(), 2);
    assert_eq!(files, store.outcome_files(&key).expect("list"));
    for file in &files {
        assert_eq!(
            moved.read_outcomes(file).expect("read"),
            store.read_outcomes(file).expect("read")
        );
    }
    let root = original.to_string_lossy().into_owned(); // BORROW: .into_owned() makes the owned root text
    support::assert_clean(&copy, &root);
}

/// Guards: Cond 6 — the files and directories of one run with two outcome files are exactly the
/// layout of R01 §Run store: no other name, no empty directory. Oracle: the literal list written
/// from the §Run store table and its Blobs, Outcome files and Runs notes.
#[test]
fn the_layout_of_a_run_with_two_outcome_files_is_the_frozen_path_set() {
    let scratch = TempDir::new().expect("a store directory");
    let store = Store::init(scratch.path()).expect("init");
    let run = manifest(1);
    drive(&store, &run, Existing::Refuse, 3)
        .done()
        .expect("seal");
    let key = run.run().clone(); // BORROW: the key outlives the manifest
    put_outcome(&store, &key, 1);
    put_outcome(&store, &key, 2);

    let cell = key.cell().hex();
    let (one, two) = (request(1), request(2));
    let literal_files = [
        "store.json".to_owned(), // BORROW: .to_owned() makes the owned path text
        format!("outcomes/{cell}-r1-a1/htn-creator-eval@0.4.2/{one}.jsonl"),
        format!("outcomes/{cell}-r1-a1/htn-creator-eval@0.4.2/{two}.jsonl"),
        format!("runs/{cell}-r1-a1/manifest.json"),
        format!("runs/{cell}-r1-a1/raw/stdout.jsonl.gz"),
        format!("runs/{cell}-r1-a1/run.done"),
        format!("runs/{cell}-r1-a1/trace.jsonl"),
        format!("runs/{cell}-r1-a1/tree.json"),
    ];
    let literal_dirs = [
        "outcomes".to_owned(), // BORROW: .to_owned() makes the owned path text
        format!("outcomes/{cell}-r1-a1"),
        format!("outcomes/{cell}-r1-a1/htn-creator-eval@0.4.2"),
        "runs".to_owned(), // BORROW: .to_owned() makes the owned path text
        format!("runs/{cell}-r1-a1"),
        format!("runs/{cell}-r1-a1/raw"),
    ];
    let files: BTreeSet<String> = files_of(scratch.path()).into_keys().collect();
    assert_eq!(files, BTreeSet::from(literal_files));
    assert_eq!(dirs_of(scratch.path()), BTreeSet::from(literal_dirs));
    let marker: Value =
        serde_json::from_slice(&fs::read(scratch.path().join("store.json")).expect("read"))
            .expect("a marker");
    assert_eq!(marker, json!({"schema": "candler-store/1"}));
    assert!(
        Store::open(&scratch.path().join("runs")).is_err(),
        "a directory without the marker is no store"
    );
}
