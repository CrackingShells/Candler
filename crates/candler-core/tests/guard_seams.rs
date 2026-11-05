#![cfg(test)]
//! Guards for the seams of one run: a fake harness is spawned under the supervisor, its output is
//! framed, stored with the run writer, read back by the trace reader, graded by the outcome writer
//! and indexed. Expected paths, names and lines are literals written from R01 §Run store and the
//! first vector of `cell_digest.toml`; none comes from the store's own layout.

mod support;

use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use candler_core::frames::{FrameMode, FrameReader, RawFileInfo};
use candler_core::store::Store;
use candler_core::store::outcome::OutcomeWriter;
use candler_core::store::run::{Existing, RunWriter};
use candler_core::supervise::spawn::{OutputSpec, SpawnRequest};
use candler_core::supervise::{EventSink, Supervisor};
use candler_core::trace::normalise;
use candler_harness::reader::TraceReader;
use candler_harness::spec::{StopSignal, StopSpec};
use candler_harness_claude_code::ClaudeCodeReader;
use candler_schemas::execution::{Envelope, EventPayload, RunManifest};
use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::{
    Digest, JobId, Marker, NameVersion, RequestId, RunKey, Timestamp,
};
use candler_schemas::raw::StreamName;
use flate2::read::MultiGzDecoder;
use serde_json::{Value, json};
use sha2::Digest as _;
use tempfile::TempDir;

/// The job of the run.
const JOB: &str = "018f2b3c-4d5e-7a6b-8c7d-9e0f1a2b3c4d";
/// The secret that goes in through the fake harness's environment and nowhere else.
const CANARY: &str = "canary-5e0a91c37b2d4f68";
/// The hex of the digest of the first vector of `cell_digest.toml`.
const CELL_HEX: &str = "bb140e82b44189facbeeebc4e9a8326d58570ba3dd54c0def45e4033e2ec2d96";
/// The directory of the run, `<cell hex>-r<replicate>-a<attempt>` with attempt 1.
const RUN_DIR: &str = "bb140e82b44189facbeeebc4e9a8326d58570ba3dd54c0def45e4033e2ec2d96-r1-a1";
/// The evaluator that grades the run, as `name@version`.
const EVALUATOR: &str = "htn-creator-eval@0.4.2";
/// The request id of the outcome file: a version 7 UUID.
const REQUEST: &str = "01926b3a-7c3e-7a1b-8c4d-000000000001";
/// The mini-corpus cell and run whose stream the fake harness plays back.
const STREAM: &str = "tests/fixtures/mini-corpus/20260806-035940_v0.2.3_claude-haiku-4-5_model-claude-haiku-4-5_skill_version-htn-creator-v0.2.3/_run_cache/with_skill/run-6/agent_stream.jsonl.gz";

/// The path `relative` below the crate's directory.
fn crate_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// Appends each envelope of the job to `events.jsonl`, numbering them from 0.
struct JobLog {
    path: PathBuf,
    next: Mutex<u64>,
}

impl EventSink for JobLog {
    fn emit(&self, run: Option<&RunKey>, payload: EventPayload) {
        let mut next = self.next.lock().expect("a lock");
        let mut builder = Envelope::builder()
            .seq(*next)
            .at("2026-11-05T10:00:00Z".parse::<Timestamp>().expect("a time"))
            .job_id(JOB.parse::<JobId>().expect("a job id"))
            .event(payload);
        if let Some(run) = run {
            builder = builder.run(run.clone()); // BORROW: .clone() gives the envelope its own key
        }
        let mut line = serde_json::to_vec(&builder.build().expect("an envelope")).expect("JSON");
        line.push(b'\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path.as_path()) // BORROW: explicit .as_path()
            .expect("the event log");
        file.write_all(line.as_slice()).expect("a write"); // BORROW: explicit .as_slice()
        *next += 1;
    }
}

/// The `comparable` text and the digest of the first `[[vector]]` of `cell_digest.toml`.
fn first_vector() -> (String, String) {
    let path = crate_path("tests/oracle/vectors/cell_digest.toml");
    let text = fs::read_to_string(&path).expect("the vector file");
    let table: toml::Table = text.parse().expect("TOML");
    let vector = table
        .get("vector")
        // BORROW: toml::Value::as_array borrows the TOML array without copying
        .and_then(toml::Value::as_array)
        .and_then(|vectors| vectors.first())
        // BORROW: toml::Value::as_table borrows the TOML table without copying
        .and_then(toml::Value::as_table)
        .expect("a first vector");
    let field = |key: &str| {
        vector
            .get(key)
            // BORROW: toml::Value::as_str borrows the TOML string without copying
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("the vector has no `{key}`"))
            .to_owned() // BORROW: .to_owned() gives the text to the caller
    };
    (field("comparable"), field("digest"))
}

/// A manifest around `comparable`: one source per value path and one for each typed root the comparable holds.
fn manifest(comparable: &str, digest: &str) -> RunManifest {
    let comparable: Value = serde_json::from_str(comparable).expect("a comparable");
    let mut sources: Vec<Value> = comparable
        .get("values")
        // BORROW: Value::as_array borrows the JSON array without copying
        .and_then(Value::as_array)
        .expect("values")
        .iter()
        .map(|value| json!({"path": value.get("path"), "source": {"kind": "experiment_file"}}))
        .collect();
    for root in [
        "harness",
        "treatment",
        "snapshots",
        "inputs",
        "environments",
    ] {
        let present = match comparable.get(root) {
            Some(Value::Array(items)) => !items.is_empty(),
            Some(Value::Object(fields)) => !fields.is_empty(),
            Some(Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)) | None => {
                false
            }
        };
        if !present {
            continue;
        }
        sources.push(json!({"path": root, "source": {"kind": "experiment_file"}}));
    }
    serde_json::from_value(json!({
        "schema": "run-manifest/1",
        "run": {"cell": digest, "replicate": 1, "attempt": 1},
        "experiment": format!("sha256:{}", "0".repeat(64)),
        "factors": [],
        "comparable": comparable,
        "sources": sources,
        "secrets": [],
        "measured": [],
        "deviations": [],
        "integrity": {"kind": "valid"}
    }))
    .expect("a manifest")
}

/// A passing outcome of `run` for the expectation `expectation`.
fn outcome(run: &RunKey, expectation: &str) -> OutcomeRecord {
    serde_json::from_value(json!({
        "run": {"cell": run.cell().to_string(), "replicate": run.replicate(), "attempt": run.attempt()},
        "expectation_id": expectation,
        "role": "gate",
        "evaluator": EVALUATOR,
        "instruments": [],
        "suite_entry_digest": format!("sha256:{}", "1".repeat(64)),
        "context_digest": format!("sha256:{}", "2".repeat(64)),
        "request_id": REQUEST,
        "status": "pass",
        "weight": {"num": 1, "den": 1},
        "evidence": [],
    }))
    .expect("an outcome record")
}

/// The stream name `name`.
fn stream(name: &str) -> StreamName {
    name.parse().expect("a stream name")
}

/// What one pipeline run left behind.
struct Seamed {
    /// Keeps the store and the frame files alive.
    _dir: TempDir,
    root: PathBuf,
    store: Store,
    key: RunKey,
    job_dir: PathBuf,
    records: PathBuf,
    /// The `Debug` of the request that was spawned.
    request_debug: String,
}

/// Runs the whole pipeline once into a new store: spawn the fake harness with the canary in its
/// environment, store its raw streams, normalise the trace, grade by hand, index.
async fn pipeline() -> Seamed {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = dir.path().join("store");
    let store = Store::init(root.as_path()).expect("a store"); // BORROW: explicit .as_path()
    let job_dir = root.join("jobs").join(JOB);
    let records = job_dir.join("children");
    fs::create_dir_all(records.as_path()).expect("the job directory"); // BORROW: explicit .as_path()
    let frames = dir.path().join("frames");
    fs::create_dir_all(frames.as_path()).expect("the frame directory"); // BORROW: explicit .as_path()

    let (comparable, digest) = first_vector();
    let manifest = manifest(comparable.as_str(), digest.as_str()); // BORROW: explicit .as_str()
    let key = manifest.run().clone(); // BORROW: .clone() keeps the run key
    let harness = crate_path("tests/fixtures/seams/fake_harness.sh");
    let marker: Marker = "m1-seams-guard".parse().expect("a marker");
    let request = SpawnRequest::builder()
        .argv(vec![
            "sh".to_owned(), // BORROW: .to_owned() makes the owned argument
            harness.display().to_string(),
            crate_path(STREAM).display().to_string(),
        ])
        .env(vec![
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()), // BORROW: .to_owned() makes the owned variable
            ("CANDLER_SEAMS_CANARY".to_owned(), CANARY.to_owned()), // BORROW: .to_owned() makes the owned variable
        ])
        .marker(marker)
        .record_dir(records.clone()) // BORROW: .clone() keeps the path for the test
        .stdout(OutputSpec::framed(
            frames.clone(), // BORROW: .clone() keeps the path for the test
            stream("stdout"),
            FrameMode::Lines,
        ))
        .stderr(OutputSpec::framed(
            frames.clone(), // BORROW: .clone() keeps the path for the test
            stream("stderr"),
            FrameMode::Lines,
        ))
        .run(key.clone()) // BORROW: .clone() keeps the run key
        .stop(StopSpec::new(
            None,
            StopSignal::Term,
            Duration::from_millis(500),
        ))
        .deadline(Duration::from_secs(60))
        .build()
        .expect("a request");
    let request_debug = format!("{request:?}");

    let log = Arc::new(JobLog {
        path: job_dir.join("events.jsonl"),
        next: Mutex::new(0),
    });
    // TRAIT_OBJECT: the supervisor takes its sink as a trait object
    // BORROW: .clone() shares the log
    let sink: Arc<dyn EventSink> = log.clone();
    let supervisor = Supervisor::new(sink);
    let child = supervisor.spawn(request).expect("a child");
    let exit = child.wait().await.expect("an exit");
    assert_eq!(exit.raw.len(), 2, "two raw files");

    let mut writer = RunWriter::create(&store, &manifest, Existing::Refuse).expect("a run");
    for info in &exit.raw {
        let name = info
            .path
            .file_name()
            // BORROW: OsStr::to_str borrows the file name without copying
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".jsonl.gz"))
            .expect("a frame file name");
        writer
            .raw(&stream(name), info.path.as_path(), &info.digest) // BORROW: explicit .as_path()
            .expect("a stored stream");
    }
    let stdout = exit
        .raw
        .iter()
        .find(|info| info.path.ends_with("stdout.jsonl.gz"))
        .expect("the stdout file");
    // The frame file moved into the store; describe it at its new place.
    let stored = store
        .raw_path(&key, &stream("stdout"))
        .expect("a stored stdout");
    let reader = ClaudeCodeReader::new().expect("the reader");
    let info = RawFileInfo::new(stored, stdout.digest.clone(), stdout.frames); // BORROW: .clone() copies the digest
    // TRAIT_OBJECT: normalise takes its reader as a trait object
    let trace = normalise(Box::new(reader), &info).expect("a trace");
    writer.trace(&trace).expect("a stored trace");
    writer.done().expect("a finished run");

    let evaluator: NameVersion = EVALUATOR.parse().expect("name@version");
    let request_id: RequestId = REQUEST.parse().expect("a request id");
    let mut outcomes =
        OutcomeWriter::create(&store, &key, &evaluator, &request_id).expect("an outcome file");
    outcomes.append(&outcome(&key, "A1")).expect("a record");
    outcomes.append(&outcome(&key, "A2")).expect("a record");
    outcomes.commit().expect("a committed file");
    store.rebuild_index().expect("an index");

    Seamed {
        _dir: dir,
        root,
        store,
        key,
        job_dir,
        records,
        request_debug,
    }
}

/// The number of lines of the gunzipped fixture stream, counted by `lines()`.
fn fixture_lines() -> usize {
    let file = fs::File::open(crate_path(STREAM)).expect("the fixture stream");
    BufReader::new(MultiGzDecoder::new(file)).lines().count()
}

/// The sha256 of the file at `path` as a digest.
fn digest_of(path: &Path) -> Digest {
    let bytes = fs::read(path).expect("a stored file");
    let mut hex = String::new();
    for byte in sha2::Sha256::digest(&bytes) {
        write!(hex, "{byte:02x}").expect("write to a string");
    }
    Digest::from_hex(hex.as_str()).expect("a digest") // BORROW: explicit .as_str()
}

/// Every file under `root` as a path relative to it with `/` separators, sorted.
fn walk(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in fs::read_dir(&next).expect("a directory") {
            let entry = entry.expect("an entry");
            let kind = entry.file_type().expect("a file type");
            if kind.is_dir() {
                pending.push(entry.path());
            } else {
                let relative = entry.path();
                let relative = relative.strip_prefix(root).expect("below the root");
                found.push(relative.to_string_lossy().into_owned());
            }
        }
    }
    found.sort();
    found
}

/// Guards: I3 — the stored raw stdout, normalised again by a fresh reader whose `id()` the trace
/// header names, gives a trace file byte-equal to the stored one, and the stream has one frame per
/// line of the fixture. Oracle: the fixture stream counted by `lines()` over a `MultiGzDecoder`,
/// and the stored file hashed by `sha2` instead of by the pipeline.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stored_stream_replays_to_the_stored_trace() {
    let seamed = pipeline().await;
    let stdout = stream("stdout");
    let path = seamed
        .store
        .raw_path(&seamed.key, &stdout)
        .expect("a stored stdout");
    let mut frames = 0;
    // BORROW: explicit .as_path()
    for frame in FrameReader::open(path.as_path()).expect("a frame file") {
        frame.expect("a frame");
        frames += 1;
    }
    let lines = fixture_lines();
    assert!(lines > 0, "the fixture stream is empty");
    assert_eq!(frames, lines);

    let stored = seamed
        .store
        .read_trace(&seamed.key)
        .expect("a stored trace");
    let fresh = ClaudeCodeReader::new().expect("the reader");
    assert_eq!(stored.header().reader, fresh.id());
    let info = RawFileInfo::new(
        path.clone(),              // BORROW: .clone() keeps the path
        digest_of(path.as_path()), // BORROW: explicit .as_path()
        u64::try_from(frames).expect("a count"),
    );
    // TRAIT_OBJECT: normalise takes its reader as a trait object
    let again = normalise(Box::new(fresh), &info).expect("a replayed trace");
    assert!(!stored.events().is_empty(), "the trace holds no event");
    assert!(
        again.write() == stored.write(),
        "the replayed trace differs from the stored one"
    );
}

/// Guards: I19 — a canary passed in the fake harness's environment is found nowhere under the
/// store, the job directory, the record directory or the event log, and not in the `Debug` of the
/// request. Oracle: `tests/support::assert_clean`, which fails when it reads no file, and the
/// event log read back as text.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_secret_in_the_environment_reaches_no_file() {
    let seamed = pipeline().await;
    support::assert_clean(seamed.root.as_path(), CANARY); // BORROW: explicit .as_path()
    support::assert_clean(seamed.job_dir.as_path(), CANARY); // BORROW: explicit .as_path()
    support::assert_clean(seamed.records.as_path(), CANARY); // BORROW: explicit .as_path()
    let log = fs::read_to_string(seamed.job_dir.join("events.jsonl")).expect("the event log");
    assert!(log.lines().count() > 1, "the event log holds no events");
    assert!(!log.contains(CANARY), "the event log holds the canary");
    assert!(
        seamed.request_debug.contains("CANDLER_SEAMS_CANARY"),
        "the request's Debug does not name the variable: {}",
        seamed.request_debug
    );
    assert!(
        !seamed.request_debug.contains(CANARY),
        "the Debug holds the canary"
    );
}

/// Guards: Cond 6 — the run's manifest carries the comparable of the first vector of
/// `cell_digest.toml`, and the run, its outcome file and the index lines sit at the paths R01 §Run
/// store gives for that digest. Oracle: the vector file written by `gen_vectors.py` with
/// `rfc8785`, and the literal paths and lines below, written by hand from R01.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_run_sits_at_the_paths_of_its_cell_digest() {
    let seamed = pipeline().await;
    let (comparable, digest) = first_vector();
    assert_eq!(digest, format!("sha256:{CELL_HEX}"));
    let manifest = seamed
        .store
        .read_manifest(&seamed.key)
        .expect("a stored manifest");
    let wanted: Value = serde_json::from_str(comparable.as_str()).expect("a comparable"); // BORROW: explicit .as_str()
    assert_eq!(
        serde_json::to_value(manifest.comparable()).expect("JSON"),
        wanted
    );
    assert_eq!(seamed.key.to_string(), RUN_DIR);

    let mut files = walk(seamed.root.as_path()); // BORROW: explicit .as_path()
    files.retain(|file| !file.starts_with("jobs/") && file != "index/lock");
    let outcome = format!("outcomes/{RUN_DIR}/{EVALUATOR}/{REQUEST}.jsonl");
    let wanted_files = [
        "index/outcomes.jsonl".to_owned(), // BORROW: .to_owned() makes the owned path
        "index/runs.jsonl".to_owned(),     // BORROW: .to_owned() makes the owned path
        outcome,
        format!("runs/{RUN_DIR}/manifest.json"),
        format!("runs/{RUN_DIR}/raw/stderr.jsonl.gz"),
        format!("runs/{RUN_DIR}/raw/stdout.jsonl.gz"),
        format!("runs/{RUN_DIR}/run.done"),
        format!("runs/{RUN_DIR}/trace.jsonl"),
        "store.json".to_owned(), // BORROW: .to_owned() makes the owned path
    ];
    assert_eq!(files, wanted_files);

    // One JSON object per line; the order of its keys is not part of the layout.
    let index = |name: &str| -> Vec<Value> {
        fs::read_to_string(seamed.root.join("index").join(name))
            .expect("an index file")
            .lines()
            .map(|line| serde_json::from_str(line).expect("a JSON line"))
            .collect()
    };
    assert_eq!(index("runs.jsonl"), [json!({"run": RUN_DIR})]);
    assert_eq!(
        index("outcomes.jsonl"),
        [json!({"run": RUN_DIR, "evaluator": EVALUATOR, "request": REQUEST})]
    );
}
