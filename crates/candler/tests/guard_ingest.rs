#![cfg(test)]
//! Guards for `candler ingest` over raw Claude Code trace directories and legacy results folders:
//! every stream becomes one sealed run, the same run when ingested again, with relative paths only,
//! and an ingest that is killed leaves a store that a second ingest finishes as one fresh ingest
//! would. Streams are written into a temporary directory by the tests; the binary is the real one,
//! run from an unrelated directory.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use candler_core::import::digest_of;
use candler_core::import::raw::{RawIngestError, Recognised, find_streams, ingest_raw};
use candler_core::import::streams::ReaderFactory;
use candler_core::store::Store;
use candler_harness::reader::TraceReader;
use candler_harness_claude_code::{ClaudeCodeReader, detect};
use serde_json::{Value, json};
use tempfile::TempDir;

/// The first line of a Claude Code stream: an init line naming `version`, with a `tag` that makes
/// the bytes of each stream unique.
fn init_line(version: &str, tag: &str) -> String {
    format!(
        r#"{{"type":"system","subtype":"init","session_id":"S","model":"m-{tag}","claude_code_version":"{version}"}}"#
    )
}

/// A short stream: an init line and a result line.
fn stream(version: &str, tag: &str) -> Vec<u8> {
    let result = r#"{"type":"result","subtype":"success","is_error":false,"session_id":"S"}"#;
    format!("{}\n{result}\n", init_line(version, tag)).into_bytes()
}

/// A long stream of about `bytes` bytes: the init line, filler lines, a result line.
fn long_stream(tag: &str, bytes: usize) -> Vec<u8> {
    let mut text = format!("{}\n", init_line("2.1.220", tag));
    let pad = "x".repeat(180);
    let mut n = 0_u32;
    while text.len() < bytes {
        write!(text, r#"{{"type":"filler","n":{n},"pad":"{pad}"}}"#).expect("a string writes");
        text.push('\n');
        n += 1;
    }
    text.push_str(r#"{"type":"result","subtype":"success","is_error":false,"session_id":"S"}"#);
    text.push('\n');
    text.into_bytes()
}

/// The Claude Code recogniser the binary uses: the first non-blank line is an init line.
fn recognise(sample: &[u8]) -> Option<Recognised> {
    let line = sample
        .split(|byte| *byte == b'\n')
        .find(|line| !line.iter().all(u8::is_ascii_whitespace))?;
    let Value::Object(map) = serde_json::from_slice::<Value>(line).ok()? else {
        return None;
    };
    let version = detect::init_version(&map)?;
    let supported = detect::supports(&version);
    Some(Recognised::new(
        "claude-code".parse().ok()?,
        version,
        supported,
    ))
}

/// A factory of Claude Code readers.
fn readers() -> Box<ReaderFactory> {
    Box::new(|| {
        let reader: Box<dyn TraceReader> =
            Box::new(ClaudeCodeReader::new().expect("the reader builds"));
        reader
    })
}

/// The sha256 of the file at `path`, as `shasum -a 256` prints it: the oracle of every digest.
fn shasum(path: &Path) -> String {
    let out = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .expect("shasum runs");
    let text = String::from_utf8(out.stdout).expect("shasum prints text");
    let hex = text.split_whitespace().next().expect("a digest").to_owned();
    format!("sha256:{hex}")
}

/// Writes `bytes` at `dir/name`, creating the folders.
fn put(dir: &Path, name: &str, bytes: &[u8]) {
    let path = dir.join(name);
    fs::create_dir_all(path.parent().expect("a parent")).expect("the folders are made");
    fs::write(path, bytes).expect("the file is written");
}

/// Compresses the file at `path` into `path.gz` with `gzip -n` and removes the plain file.
fn gzip(path: &Path) {
    let status = Command::new("gzip")
        .arg("-n")
        .arg(path)
        .status()
        .expect("gzip runs");
    assert!(status.success(), "gzip compressed {path:?}");
}

/// The directory of the first fixture: two streams, one gzipped, one nested, and one `.jsonl`
/// file no harness recognises.
fn fixture_directory(root: &Path) -> PathBuf {
    let dir = root.join("traces");
    put(&dir, "a.jsonl", &stream("2.1.220", "a"));
    put(&dir, "b.jsonl", &stream("2.1.220", "b"));
    gzip(&dir.join("b.jsonl"));
    put(&dir, "nested/deeper/c.jsonl", &stream("2.1.220", "c"));
    put(&dir, "notes.jsonl", b"{\"type\":\"other\"}\n");
    put(&dir, "readme.txt", b"not a candidate\n");
    dir
}

/// Every file under `dir` by its path relative to `dir`, with its digest, leaving out the hidden
/// entries of the store (`.tmp-*`, `.replaced`, `.import-scratch`).
fn listing(dir: &Path) -> BTreeMap<String, String> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.expect("an entry").path();
            let name = path.file_name().expect("a name").to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let rel = path.strip_prefix(base).expect("under the base");
                let bytes = fs::read(&path).expect("a file reads");
                out.insert(
                    rel.to_string_lossy().into_owned(),
                    digest_of(&bytes).expect("a digest").to_string(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The manifests of every run directory of `store`, as JSON, by run directory name.
fn manifests(store: &Path) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for entry in fs::read_dir(store.join("runs")).expect("runs lists") {
        let path = entry.expect("an entry").path();
        let name = path
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .into_owned();
        if name.starts_with('.') {
            continue;
        }
        let text = fs::read_to_string(path.join("manifest.json")).expect("a manifest reads");
        out.insert(
            name,
            serde_json::from_str(&text).expect("a manifest is JSON"),
        );
    }
    out
}

/// Every string of `value`, at any depth.
fn strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => items.iter().for_each(|item| strings(item, out)),
        Value::Object(map) => map.values().for_each(|item| strings(item, out)),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// Guards: I1 — a directory with two streams (one gzipped), a nested stream and a `.jsonl` file no
/// harness recognises gives three sealed runs, one entry in `skipped`, and for each run an
/// `import.sources` entry holding the stream's path relative to the ingested directory and its
/// sha256. Oracle: the sha256 `shasum -a 256` prints for each file on disk, and the relative
/// paths the test wrote. Bite: a stream left out of the walk, a digest taken of the plain text
/// instead of the file, or a source path that is not relative to the directory fails the sources.
#[test]
fn i1_each_stream_becomes_one_run() {
    let dir = TempDir::new().expect("a directory");
    let traces = fixture_directory(dir.path());
    let store = Store::init(&dir.path().join("store")).expect("a store");
    let report =
        ingest_raw(&store, &traces, &recognise, readers().as_ref()).expect("the ingest runs");
    assert_eq!((report.created, report.existing), (3, 0));
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].path, "notes.jsonl");
    assert_eq!(report.skipped[0].reason.as_str(), "not_a_stream");

    let mut expected: BTreeMap<String, String> = BTreeMap::new();
    for rel in ["a.jsonl", "b.jsonl.gz", "nested/deeper/c.jsonl"] {
        expected.insert(rel.to_owned(), shasum(&traces.join(rel)));
    }
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for (name, manifest) in manifests(&dir.path().join("store")) {
        let sources = manifest["import"]["sources"].as_array().expect("sources");
        assert_eq!(sources.len(), 1, "{name}");
        seen.insert(
            sources[0]["path"].as_str().expect("a path").to_owned(),
            sources[0]["sha256"].as_str().expect("a digest").to_owned(),
        );
        assert_eq!(manifest["integrity"], json!({"kind": "valid"}), "{name}");
        assert!(manifest["import"].get("base_commit").is_none(), "{name}");
        assert!(manifest.get("started_at").is_none(), "{name}");
        let run = dir.path().join("store/runs").join(&name);
        assert!(run.join("run.done").is_file(), "{name} is sealed");
        assert!(run.join("trace.jsonl").is_file(), "{name} has its trace");
    }
    assert_eq!(seen, expected);
}

/// Guards: I1, I17 — no field of a stored manifest or tree, and no source path, starts with `/` or
/// holds the temporary directory's text. Oracle: the temporary directory's own path, which the
/// test created, and the rule that a stored path is relative. Bite: a source path stored as the
/// absolute path of the stream fails both checks.
#[test]
fn i1_stored_paths_are_relative() {
    let dir = TempDir::new().expect("a directory");
    let traces = fixture_directory(dir.path());
    let store_dir = dir.path().join("store");
    let store = Store::init(&store_dir).expect("a store");
    ingest_raw(&store, &traces, &recognise, readers().as_ref()).expect("the ingest runs");
    let root = dir.path().to_string_lossy().into_owned();
    let mut checked = 0_usize;
    for entry in fs::read_dir(store_dir.join("runs")).expect("runs lists") {
        let run = entry.expect("an entry").path();
        for file in ["manifest.json", "tree.json"] {
            let text = fs::read_to_string(run.join(file)).expect("a file reads");
            assert!(
                !text.contains(root.as_str()),
                "{file} holds the directory's text"
            );
            let value: Value = serde_json::from_str(&text).expect("JSON");
            let mut all = Vec::new();
            strings(&value, &mut all);
            for text in all {
                assert!(!text.starts_with('/'), "{file} holds {text}");
                checked += 1;
            }
        }
    }
    assert!(checked > 10, "the check saw {checked} strings");
}

/// Guards: I1 — ingesting the same directory again creates nothing: the second call reports
/// `created` 0 and `existing` 3, and every file of `runs/` has the digest it had. Oracle: the byte
/// digests of the store's files before and after, and the counts of the fixture. Bite: a key taken
/// from the count of streams seen in the call, or a stream written again without checking the
/// store, adds runs or moves a digest.
#[test]
fn i1_ingesting_again_creates_nothing() {
    let dir = TempDir::new().expect("a directory");
    let traces = fixture_directory(dir.path());
    let store_dir = dir.path().join("store");
    let store = Store::init(&store_dir).expect("a store");
    ingest_raw(&store, &traces, &recognise, readers().as_ref()).expect("the first ingest");
    let before = listing(&store_dir.join("runs"));
    assert!(before.len() >= 3 * 4, "{} files", before.len());
    let again =
        ingest_raw(&store, &traces, &recognise, readers().as_ref()).expect("the second ingest");
    assert_eq!((again.created, again.existing), (0, 3));
    assert_eq!(listing(&store_dir.join("runs")), before);
}

/// Guards: I1 — a directory that grows by one stream adds one run: the second ingest of the old
/// streams and a new one reports `created` 1 and `existing` 3, and the old runs keep their keys.
/// Oracle: the run directory names before and after, and the counts of the fixture. Bite: a
/// replicate numbered from the streams of this call instead of the highest in the store gives the
/// new stream a key an old run holds, and fails the names or the counts.
#[test]
fn i1_a_grown_directory_adds_only_the_new_run() {
    let dir = TempDir::new().expect("a directory");
    let traces = fixture_directory(dir.path());
    let store_dir = dir.path().join("store");
    let store = Store::init(&store_dir).expect("a store");
    ingest_raw(&store, &traces, &recognise, readers().as_ref()).expect("the first ingest");
    let before: Vec<String> = manifests(&store_dir).into_keys().collect();
    put(&traces, "later/d.jsonl", &stream("2.1.220", "d"));
    let again =
        ingest_raw(&store, &traces, &recognise, readers().as_ref()).expect("the second ingest");
    assert_eq!((again.created, again.existing), (1, 3));
    let after: Vec<String> = manifests(&store_dir).into_keys().collect();
    assert_eq!(after.len(), 4);
    assert!(before.iter().all(|name| after.contains(name)));
    let replicates: Vec<&str> = after
        .iter()
        .filter_map(|name| name.split("-r").nth(1))
        .collect();
    let mut sorted = replicates.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, ["1-a1", "2-a1", "3-a1", "4-a1"]);
}

/// Guards: I1, R01 error model — a stream whose init line names Claude Code 2.0.0, outside the
/// supported range, is an invalid run `format_unknown` with its raw file kept and its seal
/// written. Oracle: the version the test wrote into the init line and the supported range
/// `>=2.1.220, <2.2.0` of the reader's documentation. Bite: a run sealed valid, or one whose raw
/// file is dropped, fails the manifest or the file check.
#[test]
fn i1_an_unsupported_version_is_an_invalid_run() {
    let dir = TempDir::new().expect("a directory");
    let traces = dir.path().join("traces");
    put(&traces, "old.jsonl", &stream("2.0.0", "old"));
    let store_dir = dir.path().join("store");
    let store = Store::init(&store_dir).expect("a store");
    let report =
        ingest_raw(&store, &traces, &recognise, readers().as_ref()).expect("the ingest runs");
    assert_eq!(report.created, 1);
    let all = manifests(&store_dir);
    let (name, manifest) = all.iter().next().expect("one run");
    assert_eq!(
        manifest["integrity"],
        json!({"kind": "invalid", "reason": "format_unknown"})
    );
    let run = store_dir.join("runs").join(name);
    assert!(run.join("run.done").is_file());
    assert!(run.join("raw/stdout.jsonl.gz").is_file());
    assert!(!run.join("trace.jsonl").exists());
}

/// Guards: I1 — a directory with no stream is refused with `NoStream` and the store holds no run.
/// Oracle: the directory the test wrote, which holds a `.jsonl` file no harness recognises and a
/// text file. Bite: a walk that counts every `.jsonl` file as a stream seals a run and fails.
#[test]
fn i1_a_directory_with_no_stream_is_refused() {
    let dir = TempDir::new().expect("a directory");
    let traces = dir.path().join("traces");
    put(&traces, "notes.jsonl", b"{\"type\":\"other\"}\n");
    put(&traces, "readme.txt", b"text\n");
    let store_dir = dir.path().join("store");
    let store = Store::init(&store_dir).expect("a store");
    let error = ingest_raw(&store, &traces, &recognise, readers().as_ref())
        .expect_err("a directory with no stream is refused");
    assert!(matches!(error, RawIngestError::NoStream { .. }), "{error}");
    assert_eq!(store.runs().expect("runs list").len(), 0);
    assert!(matches!(
        find_streams(&traces.join("missing"), &recognise),
        Err(RawIngestError::Unreadable { .. })
    ));
}

/// The real binary, run from `home` over `store`, with no configuration from the environment.
fn candler(home: &Path, store: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .current_dir(home)
        .env_remove("CANDLER_STORE")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .arg("--store")
        .arg(store)
        .args(args)
        .output()
        .expect("the binary runs")
}

/// Guards: I1, I14 — `candler ingest <directory>` run from an unrelated directory with a relative
/// path prints the counts of the fixture, `{"runs": 3, "created": 3, "existing": 0, "skipped":
/// [{"path": "notes.jsonl", "reason": "not_a_stream"}]}`, and a second call prints
/// `created` 0 and `existing` 3. Oracle: the fixture the test wrote and the output shape the
/// leaf's spec gives. Bite: a path resolved against the checkout instead of the caller's
/// directory, or a verb that prints the runs written in the call as `runs`, fails the lines.
#[test]
fn i1_the_verb_ingests_a_directory() {
    let dir = TempDir::new().expect("a directory");
    fixture_directory(dir.path());
    let store = dir.path().join("store");
    let out = candler(dir.path(), &store, &["ingest", "traces"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let first: Value = serde_json::from_slice(&out.stdout).expect("a JSON line");
    assert_eq!(
        first,
        json!({"runs": 3, "created": 3, "existing": 0,
               "skipped": [{"path": "notes.jsonl", "reason": "not_a_stream"}]})
    );
    let out = candler(dir.path(), &store, &["ingest", "traces"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let second: Value = serde_json::from_slice(&out.stdout).expect("a JSON line");
    assert_eq!(second["runs"], 3);
    assert_eq!(second["created"], 0);
    assert_eq!(second["existing"], 3);
}

/// Guards: I1 — a path that holds no stream is refused with exit 2 and the code
/// `import.no_stream`, and the message says a results folder needs `--rules`. Oracle: a folder the
/// test wrote that holds the grading files of a results folder and no stream, and the code the
/// ruling of 2026-11-11 gives `RawIngestError`. Bite: a verb that treats any folder as a trace
/// directory, seals no run and exits 0 fails the status.
#[test]
fn i1_a_results_folder_without_rules_is_refused() {
    let dir = TempDir::new().expect("a directory");
    let folder = dir.path().join("results");
    put(&folder, "cell/run-1/eval-1-grading.json", b"{}\n");
    put(&folder, "expectations.jsonl", b"{\"id\":\"A1\"}\n");
    let store = dir.path().join("store");
    let folder_arg = folder.to_str().expect("a UTF-8 path");
    let out = candler(
        dir.path(),
        &store,
        &["--error-format", "json", "ingest", folder_arg],
    );
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("import.no_stream"), "{stderr}");
    assert!(stderr.contains("--rules"), "{stderr}");
}

/// The sha256 of `fixtures/cli/mini-corpus-rules.toml`'s corpus counts are the hand literals of
/// `guard_cli.rs`: 6 runs and 136 records.
const MINI_RUNS: u64 = 6;
/// See [`MINI_RUNS`].
const MINI_RECORDS: u64 = 136;

/// Guards: I2 — a results folder given with `--rules` behaves as `import` did: the M1 mini-corpus
/// prints `{"runs": 6, "records": 136}`, and `import` is a hidden alias that prints the same.
/// Oracle: the counts `guard_cli.rs` hand-counted from the 42 grading files of the corpus, which
/// the importer's output never produces. Bite: a rename that drops the routing of `--rules` to
/// the importer sends the folder to the raw path and exits 2 with `import.no_stream`.
#[test]
fn i2_a_results_folder_gives_the_mini_corpus_counts() {
    let dir = TempDir::new().expect("a directory");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let corpus = root.join("../candler-core/tests/fixtures/mini-corpus");
    let rules = root.join("tests/fixtures/cli/mini-corpus-rules.toml");
    let corpus_arg = corpus.to_str().expect("a UTF-8 path");
    let rules_arg = rules.to_str().expect("a UTF-8 path");
    let out = candler(
        dir.path(),
        &dir.path().join("store"),
        &["ingest", corpus_arg, "--rules", rules_arg],
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let summary: Value = serde_json::from_slice(&out.stdout).expect("a JSON line");
    assert_eq!(summary, json!({"runs": MINI_RUNS, "records": MINI_RECORDS}));
}

/// Guards: I2 — `candler import …` prints exactly what `candler ingest …` prints, and `import` is
/// not listed in `--help`. Oracle: the two outputs compared with each other and the help text.
/// Bite: an alias removed from the command fails the first call; an alias made visible shows in
/// the help.
#[test]
fn i2_the_old_name_is_an_alias() {
    let dir = TempDir::new().expect("a directory");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let corpus = root.join("../candler-core/tests/fixtures/mini-corpus");
    let rules = root.join("tests/fixtures/cli/mini-corpus-rules.toml");
    let corpus_arg = corpus.to_str().expect("a UTF-8 path");
    let rules_arg = rules.to_str().expect("a UTF-8 path");
    let new = candler(
        dir.path(),
        &dir.path().join("new"),
        &["ingest", corpus_arg, "--rules", rules_arg],
    );
    let old = candler(
        dir.path(),
        &dir.path().join("old"),
        &["import", corpus_arg, "--rules", rules_arg],
    );
    assert_eq!(new.status.code(), Some(0), "{new:?}");
    assert_eq!(old.status.code(), Some(0), "{old:?}");
    assert_eq!(new.stdout, old.stdout);
    let help = Command::new(env!("CARGO_BIN_EXE_candler"))
        .arg("--help")
        .output()
        .expect("the binary runs");
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("ingest"), "{help}");
    assert!(
        !help
            .lines()
            .any(|line| line.trim_start().starts_with("import")),
        "{help}"
    );
}

/// Starts `candler ingest` over `traces` into `store`, as the test's own child.
fn spawn_ingest(home: &Path, store: &Path, traces: &Path) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .current_dir(home)
        .env_remove("CANDLER_STORE")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .arg("--store")
        .arg(store)
        .arg("ingest")
        .arg(traces)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the child starts")
}

/// The run directories of `store` that hold `run.done`.
fn sealed(store: &Path) -> usize {
    let Ok(entries) = fs::read_dir(store.join("runs")) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("run.done").is_file())
        .count()
}

/// Writes `count` streams of about `bytes` bytes each into `traces`.
fn write_streams(traces: &Path, count: usize, bytes: usize) {
    for n in 0..count {
        put(
            traces,
            &format!("s{n:03}.jsonl"),
            &long_stream(&format!("s{n}"), bytes),
        );
    }
}

/// Ingests `traces` into `store` and kills the child once a run is sealed, growing the fixture and
/// trying again until the kill lands in the middle of the ingest. Returns the store's directory.
fn killed_store(dir: &Path, traces: &Path, count: usize) -> PathBuf {
    let mut size = 200 * 1024;
    for attempt in 0..3 {
        let store = dir.join(format!("killed-{attempt}"));
        let mut child = spawn_ingest(dir, &store, traces);
        let deadline = Instant::now() + Duration::from_secs(120);
        while sealed(&store) == 0 && Instant::now() < deadline {
            if child.try_wait().expect("the child polls").is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        child.kill().expect("the child is killed");
        child.wait().expect("the child is reaped");
        let done = sealed(&store);
        if done >= 1 && done < count {
            return store;
        }
        size *= 2;
        write_streams(traces, count, size);
    }
    panic!("the kill never landed in the middle of an ingest");
}

/// Guards: I4 — an ingest killed after at least one run is sealed and before all are, then run
/// again into the same store, leaves the files of `runs/` that a fresh ingest of the same
/// directory into a second store leaves, each with its digest. Oracle: the second store, written
/// by one uninterrupted ingest, and the non-vacuity check that at least one run was sealed and
/// fewer than 40 were when the child was killed. Bite: a run sealed before its trace is written, a
/// replicate taken from the count of streams of the call instead of the highest in the store, or
/// `Existing::Refuse` in place of `ReplaceUnfinished` leaves a store that differs from the fresh
/// one, or an ingest that refuses.
#[test]
fn i4_a_killed_ingest_resumes_to_the_fresh_store() {
    let dir = TempDir::new().expect("a directory");
    let traces = dir.path().join("traces");
    write_streams(&traces, 40, 200 * 1024);
    let killed = killed_store(dir.path(), &traces, 40);
    let out = candler(
        dir.path(),
        &killed,
        &["ingest", traces.to_str().expect("a UTF-8 path")],
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let fresh = dir.path().join("fresh");
    let out = candler(
        dir.path(),
        &fresh,
        &["ingest", traces.to_str().expect("a UTF-8 path")],
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let resumed = listing(&killed.join("runs"));
    let reference = listing(&fresh.join("runs"));
    assert!(reference.len() >= 40 * 4);
    assert_eq!(resumed, reference);
}

/// Guards: I4 — after the kill, every run directory without `run.done` is not a record (the
/// store lists it as unfinished) and every sealed run has a parseable manifest and, when it is
/// valid, a parseable trace. Oracle: the kill of the test's own child and the store's own reading
/// of `run.done`; the fixture's streams are valid, so every sealed run must be valid. Bite: a seal
/// written before the trace leaves a sealed run without a trace and fails the trace check.
#[test]
fn i4_nothing_half_written_is_a_record() {
    let dir = TempDir::new().expect("a directory");
    let traces = dir.path().join("traces");
    write_streams(&traces, 40, 200 * 1024);
    let killed = killed_store(dir.path(), &traces, 40);
    let store = Store::open(&killed).expect("the store opens");
    let mut sealed_seen = 0_usize;
    for entry in store.runs().expect("runs list") {
        let run_dir = killed.join("runs").join(entry.key.to_string());
        assert_eq!(entry.done, run_dir.join("run.done").is_file());
        if !entry.done {
            continue;
        }
        sealed_seen += 1;
        let manifest = store
            .read_manifest(&entry.key)
            .expect("a sealed manifest parses");
        assert_eq!(
            serde_json::to_value(manifest.integrity()).expect("JSON"),
            json!({"kind": "valid"})
        );
        store.read_trace(&entry.key).expect("a sealed trace parses");
    }
    assert!(sealed_seen >= 1);
}

/// Guards: I4 — a store from a complete ingest with one run's `run.done` removed is repaired by
/// ingesting again: the run is sealed again under the same key, no run is added, no `.replaced`
/// directory is left, and every file has the digest it had. Oracle: the store's listing before the
/// removal and after the repair. Bite: `Existing::Refuse` in place of `ReplaceUnfinished` refuses
/// the repair, and a replicate taken from the streams of the call adds a run.
#[test]
fn i4_an_unsealed_run_is_replaced() {
    let dir = TempDir::new().expect("a directory");
    let traces = fixture_directory(dir.path());
    let store_dir = dir.path().join("store");
    let arg = traces.to_str().expect("a UTF-8 path");
    let out = candler(dir.path(), &store_dir, &["ingest", arg]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let before = listing(&store_dir.join("runs"));
    let victim = fs::read_dir(store_dir.join("runs"))
        .expect("runs lists")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.join("run.done").is_file())
        .expect("a sealed run");
    fs::remove_file(victim.join("run.done")).expect("the seal is removed");
    let out = candler(dir.path(), &store_dir, &["ingest", arg]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let summary: Value = serde_json::from_slice(&out.stdout).expect("a JSON line");
    assert_eq!(summary["created"], 1);
    assert_eq!(summary["existing"], 2);
    assert_eq!(listing(&store_dir.join("runs")), before);
    assert!(!store_dir.join("runs/.replaced").exists());
    assert_eq!(
        Store::open(&store_dir)
            .expect("opens")
            .runs()
            .expect("lists")
            .len(),
        3
    );
}
