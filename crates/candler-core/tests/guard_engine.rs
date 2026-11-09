//! Guards for the evaluation engine against the helper's plug-ins: dependencies and `applies_to`
//! decide what is sent, the context digest the engine writes is the digest of what the plug-in
//! received, and two regrades under one key are an I4 violation. The plug-ins run for real, from
//! `python/candler-evaluator` under `uv run --frozen`; every expected value is a literal or comes
//! from the independent Python vectors.
#![cfg(test)]
#![cfg(unix)]

#[path = "support/legacy_import.rs"]
mod legacy_import;
#[path = "support/legacy_rules.rs"]
mod legacy_rules;
#[path = "oracle/mod.rs"]
mod oracle;

use std::collections::BTreeMap;
use std::fs;
use std::num::NonZeroUsize;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use candler_core::evaluation::engine::{EvaluateReport, EvaluateRequest, evaluate};
use candler_core::evaluation::registry::EnvironmentKey;
use candler_core::evaluation::selection::select::select;
use candler_core::store::Store;
use candler_core::supervise::signal::Signaller;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::EventPayload;
use candler_schemas::outcome::{Cause, OutcomeRecord, Status};
use candler_schemas::primitives::RunKey;
use legacy_import::{import_with_reader, mini};
use rustix::io::Errno;
use rustix::process::{Pid, Signal};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use tokio::sync::watch;

/// Takes the supervisor's events and drops them.
struct Quiet;

impl EventSink for Quiet {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// Records each (group, signal) it is asked to send and sends none.
#[derive(Default)]
struct Recording {
    calls: Mutex<Vec<(i32, i32)>>,
}

impl Signaller for Recording {
    fn signal_group(&self, group: Pid, signal: Signal) -> Result<(), Errno> {
        self.calls
            .lock()
            .expect("a lock")
            .push((group.as_raw_nonzero().get(), signal.as_raw()));
        Ok(())
    }
}

/// The root of the repository.
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The helper's project, from which every plug-in runs.
fn helper() -> PathBuf {
    repo().join("python/candler-evaluator")
}

/// The fixtures of this guard.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine")
}

/// A store with the mini-corpus, a built plug-in environment and the supervisor to run under.
struct Rig {
    /// Keeps the scratch directory alive.
    _tmp: tempfile::TempDir,
    /// The store root.
    store: PathBuf,
    /// The plug-in manifest.
    plugin: PathBuf,
    /// The root of the plug-in environments.
    env_root: PathBuf,
    /// Where the suites are written.
    suites: PathBuf,
    /// The supervisor, whose stops go to the recording fake.
    supervisor: Arc<Supervisor>,
    /// The fake that records the stops.
    recording: Arc<Recording>,
}

/// Builds the rig: the import of the mini-corpus, the manifest with its paths filled in, and the
/// environment of the helper, synced by `uv` and marked `ready`.
async fn rig() -> Rig {
    rig_of("plugin.toml").await
}

/// The same rig, with the manifest template `manifest_name` of the fixtures.
async fn rig_of(manifest_name: &str) -> Rig {
    let tmp = tempfile::tempdir().expect("a scratch directory");
    let store_dir = tmp.path().join("store");
    fs::create_dir(&store_dir).expect("a store directory");
    let store = Store::init(&store_dir).expect("the store initialises");
    import_with_reader(&mini(), &store).expect("the corpus imports");

    // The plug-in directory holds the manifest and a link to the helper's project, which is the
    // `uv` project the manifest names.
    let plugin_dir = tmp.path().join("plugin");
    fs::create_dir(&plugin_dir).expect("a plug-in directory");
    symlink(helper(), plugin_dir.join("env")).expect("a link to the helper");
    let manifest = fs::read_to_string(fixtures().join(manifest_name))
        .expect("the manifest template reads")
        .replace(
            "@EXAMPLES@",
            &helper().join("examples").display().to_string(),
        )
        .replace("@KINDS@", &fixtures().join("kinds").display().to_string());
    let plugin = plugin_dir.join("plugin.toml");
    fs::write(&plugin, manifest).expect("the manifest writes");

    let env_root = tmp.path().join("envs");
    let key = EnvironmentKey::of_files(&helper().join("uv.lock"), &helper().join("pyproject.toml"))
        .await
        .expect("the environment key");
    let dir = key.dir(&env_root);
    fs::create_dir_all(&dir).expect("an environment directory");
    let synced = Command::new("uv")
        .args(["sync", "--frozen"])
        .env("UV_PROJECT_ENVIRONMENT", dir.join(".venv"))
        .current_dir(helper())
        .status()
        .expect("uv runs");
    assert!(synced.success(), "uv sync built the helper's environment");
    fs::write(dir.join("ready"), key.digest()).expect("the ready marker writes");

    let suites = tmp.path().join("suites");
    fs::create_dir(&suites).expect("a suite directory");
    let recording = Arc::new(Recording::default());
    // TRAIT_OBJECT: the supervisor takes its signaller as a trait object
    // BORROW: .clone() shares the log between the supervisor and the test
    let signaller: Arc<dyn Signaller> = recording.clone();
    let sink: Arc<dyn EventSink> = Arc::new(Quiet);
    Rig {
        _tmp: tmp,
        store: store_dir,
        plugin,
        env_root,
        suites,
        supervisor: Arc::new(Supervisor::new(sink).with_signaller(signaller)),
        recording,
    }
}

impl Rig {
    /// Grades the runs of `glob` with the fixture suite `suite`, whose `@COUNT@` is `count`.
    async fn grade(&self, suite: &str, glob: &str, count: &Path) -> EvaluateReport {
        self.grade_with(std::slice::from_ref(&self.plugin), suite, glob, count)
            .await
    }

    /// The same, with every plug-in manifest of `plugins`.
    async fn grade_with(
        &self,
        plugins: &[PathBuf],
        suite: &str,
        glob: &str,
        count: &Path,
    ) -> EvaluateReport {
        self.grade_within(plugins, suite, glob, count, Some(Duration::from_secs(120)))
            .await
    }

    /// The same, with the request deadline `deadline`; `None` leaves it to the engine.
    async fn grade_within(
        &self,
        plugins: &[PathBuf],
        suite: &str,
        glob: &str,
        count: &Path,
        deadline: Option<Duration>,
    ) -> EvaluateReport {
        let text = fs::read_to_string(fixtures().join(suite))
            .expect("the suite reads")
            .replace("@COUNT@", &count.display().to_string());
        let path = self.suites.join(suite);
        fs::write(&path, text).expect("the suite writes");
        let (_keep, cancel) = watch::channel(false);
        evaluate(
            EvaluateRequest {
                store: self.store.clone(),
                plugins: plugins.to_vec(),
                suite: path,
                runs: Some(glob.to_owned()),
                concurrency: NonZeroUsize::new(2).expect("two"),
                env_root: self.env_root.clone(),
                request_deadline: deadline,
            },
            // BORROW: .clone() hands the engine its own handle on the supervisor
            self.supervisor.clone(),
            cancel,
        )
        .await
        .expect("the engine grades the runs")
    }

    /// The records of the `engine` evaluator's outcome files of `run`, each with its request id.
    fn records(&self, run: &RunKey) -> Vec<(String, OutcomeRecord)> {
        self.records_of(run, "engine")
    }

    /// The records of the outcome files of `run` that the evaluator `name` wrote, each with its
    /// request id.
    fn records_of(&self, run: &RunKey, name: &str) -> Vec<(String, OutcomeRecord)> {
        let store = Store::open(&self.store).expect("the store opens");
        let mut found = Vec::new();
        // The import left outcome files of its own beside the engine's.
        for file in store.outcome_files(run).expect("the outcome files list") {
            if file.evaluator.name() != name {
                continue;
            }
            for record in store.read_outcomes(&file).expect("the file reads") {
                found.push((record.request_id().to_string(), record));
            }
        }
        found
    }

    /// The sealed run of `replicate`.
    fn run_of(&self, replicate: u32) -> RunKey {
        Store::open(&self.store)
            .expect("the store opens")
            .runs()
            .expect("the runs list")
            .into_iter()
            .find(|entry| entry.key.replicate() == replicate)
            .unwrap_or_else(|| panic!("no run {replicate} in the store"))
            .key
    }
}

/// The record of `id` among `records`.
fn record<'r>(records: &'r [(String, OutcomeRecord)], id: &str) -> &'r (String, OutcomeRecord) {
    records
        .iter()
        .find(|(_, record)| record.expectation_id().as_str() == id)
        .unwrap_or_else(|| panic!("no record for {id}"))
}

/// The JSON an echo record carries as its note.
fn echoed(record: &OutcomeRecord) -> Value {
    let note = record
        .evidence()
        .iter()
        .find_map(|evidence| evidence.note())
        .expect("the echo record has a note");
    serde_json::from_str(note).expect("the note is JSON")
}

/// The expectation of a cause that names one.
fn blocked_by(record: &OutcomeRecord) -> String {
    match record.cause() {
        Some(Cause::Expectation(id)) => id.to_string(),
        other => panic!("expected an expectation cause, got {other:?}"),
    }
}

/// The reason code of a cause that is one.
fn reason_of(record: &OutcomeRecord) -> String {
    match record.cause() {
        Some(Cause::Reason(code)) => code.to_string(),
        other => panic!("expected a reason cause, got {other:?}"),
    }
}

/// Guards: R6 — an entry whose `applies_to` misses the run is `not_applicable` with the reason
/// `not_applicable_to_arm`; the dependent of a `fail` is `blocked` by it, and the record the
/// engine keeps is its own, not the plug-in's echo; the dependent of a `pass` is sent in the same
/// request as its dependency and receives that dependency's record in `prior`, which the echo
/// kind returns. A second suite lists D1 before its dependency P3: D1 is sent after P3 and sees it
/// in `prior`; the dependent N2 of the `not_applicable` N1 is `blocked` and not sent. The same
/// verdicts hold when the suite is written as JSON, and when the
/// manifest's command is a bare `python -m` that only the activated environment resolves.
/// Oracle: the hand literals below.
#[tokio::test]
async fn dependencies_and_applies_to_decide_what_is_sent() {
    for (manifest, suite) in [
        ("plugin.toml", "suite_dependencies.toml"),
        ("plugin.toml", "suite_dependencies.json"),
        ("plugin_bare.toml", "suite_dependencies.toml"),
    ] {
        dependency_verdicts(manifest, suite).await;
    }
    order_and_blocking_verdicts().await;
}

/// Grades run 6 with `suite_order.toml` and checks what the hand literals say: N2 depends on the
/// `not_applicable` N1, so it is `blocked` by N1 and not sent; D1 is listed before its dependency
/// P3 and is sent after it, so P3 is the only item D1 sees before it.
async fn order_and_blocking_verdicts() {
    let rig = rig().await;
    let report = rig
        .grade("suite_order.toml", "*-r6-a1", Path::new("unused"))
        .await;
    assert!(
        report.failed_requests.is_empty(),
        "{:?}",
        report.failed_requests
    );
    let records = rig.records(&rig.run_of(6));
    assert_eq!(records.len(), 4, "one record per entry");
    assert_eq!(record(&records, "N1").1.status(), Status::NotApplicable);
    assert_eq!(record(&records, "N2").1.status(), Status::Blocked);
    assert_eq!(blocked_by(&record(&records, "N2").1), "N1");
    assert_eq!(record(&records, "P3").1.status(), Status::Pass);
    assert_eq!(record(&records, "D1").1.status(), Status::Pass);
    assert_eq!(
        record(&records, "D1").0,
        record(&records, "P3").0,
        "one request"
    );
    let ids: Vec<String> = echoed(&record(&records, "D1").1)["prior"]
        .as_array()
        .expect("prior is a list")
        .iter()
        .map(|entry| entry["expectation"].as_str().expect("an id").to_owned())
        .collect();
    assert_eq!(ids, ["P3"], "the dependency is sent before D1");
}

/// Grades run 6 with `suite` under the manifest template `manifest` and checks the verdicts.
async fn dependency_verdicts(manifest: &str, suite: &str) {
    let rig = rig_of(manifest).await;
    let report = rig.grade(suite, "*-r6-a1", Path::new("unused")).await;
    assert!(
        report.failed_requests.is_empty(),
        "{:?}",
        report.failed_requests
    );
    assert_eq!(
        report.counts,
        BTreeMap::from([
            ("blocked".to_owned(), 1),
            ("fail".to_owned(), 1),
            ("not_applicable".to_owned(), 1),
            ("pass".to_owned(), 2),
        ]),
        "the status counts"
    );

    let records = rig.records(&rig.run_of(6));
    assert_eq!(records.len(), 5, "one record per entry");
    let statuses: Vec<(&str, Status)> = ["N1", "F1", "F2", "P1", "P2"]
        .iter()
        .map(|id| (*id, record(&records, id).1.status()))
        .collect();
    assert_eq!(
        statuses,
        [
            ("N1", Status::NotApplicable),
            ("F1", Status::Fail),
            ("F2", Status::Blocked),
            ("P1", Status::Pass),
            ("P2", Status::Pass),
        ]
    );
    assert_eq!(
        reason_of(&record(&records, "N1").1),
        "not_applicable_to_arm"
    );
    assert_eq!(blocked_by(&record(&records, "F2").1), "F1");
    let blocked_note = record(&records, "F2")
        .1
        .evidence()
        .iter()
        .find_map(|evidence| evidence.note())
        .expect("the blocked record has a note");
    assert_eq!(blocked_note, "the dependency F1 did not pass");

    // One evaluator and one stage: the four sent entries share one request.
    for id in ["F1", "F2", "P1", "P2"] {
        assert_eq!(record(&records, id).0, record(&records, "P1").0, "{id}");
    }

    // The plug-in handed P2 the records of the items before it in the request, P1 among them:
    // one request carries the independent items too, and a kind reads a dependency by its id.
    let seen = echoed(&record(&records, "P2").1);
    let prior = seen["prior"].as_array().expect("prior is a list");
    let ids: Vec<&str> = prior
        .iter()
        .map(|entry| entry["expectation"].as_str().expect("an id"))
        .collect();
    assert_eq!(ids, ["F1", "F2", "P1"], "the earlier items of the request");
    let p1 = prior
        .iter()
        .find(|entry| entry["expectation"] == "P1")
        .expect("P1 is in prior");
    assert_eq!(p1["outcome"]["status"], "pass");

    assert!(
        rig.recording.calls.lock().expect("a lock").is_empty(),
        "nothing was stopped"
    );
}

/// Guards: R6 (A1) — one request carries every applicable entry of one evaluator at one stage:
/// two independent entries of one evaluator share a request id, and the entry of a second
/// evaluator has a request of its own. Oracle: the hand literals below, read from the outcome
/// files.
#[tokio::test]
async fn independent_entries_of_one_evaluator_share_one_request() {
    let rig = rig().await;
    let second = rig.plugin.with_file_name("plugin_second.toml");
    let manifest = fs::read_to_string(fixtures().join("plugin_second.toml"))
        .expect("the second manifest template reads")
        .replace(
            "@EXAMPLES@",
            &helper().join("examples").display().to_string(),
        )
        .replace("@KINDS@", &fixtures().join("kinds").display().to_string());
    fs::write(&second, manifest).expect("the second manifest writes");
    let plugins = [rig.plugin.clone(), second];
    let report = rig
        .grade_with(
            &plugins,
            "suite_batching.toml",
            "*-r6-a1",
            Path::new("unused"),
        )
        .await;
    assert!(
        report.failed_requests.is_empty(),
        "{:?}",
        report.failed_requests
    );
    assert_eq!(report.request_ids.len(), 2, "one request per evaluator");

    let run = rig.run_of(6);
    let engine = rig.records_of(&run, "engine");
    let other = rig.records_of(&run, "second");
    assert_eq!(engine.len(), 2, "B1 and B2");
    assert_eq!(other.len(), 1, "C1");
    assert_eq!(record(&engine, "B1").0, record(&engine, "B2").0);
    assert_ne!(record(&engine, "B1").0, record(&other, "C1").0);
}

/// Guards: R6 (A1) — the deadline an engine gives a request: with no override, the sum of the item
/// deadlines plus the 30 s margin; with an override, the override itself. `suite_deadlines.toml`
/// has two items in one request, with item deadlines of 40 s and 5 s, so by hand the request is
/// given 40 + 5 + 30 = 75 s, and a flat override of 12 s is given as 12 s. Oracle: the fixture's
/// literals and R01 §Deadlines; the margin constant is not read.
#[tokio::test]
async fn the_request_deadline_is_the_sum_of_the_item_deadlines_plus_the_margin() {
    for (override_, expected) in [
        (None, Duration::from_secs(75)),
        (Some(Duration::from_secs(12)), Duration::from_secs(12)),
    ] {
        let rig = rig().await;
        let report = rig
            .grade_within(
                std::slice::from_ref(&rig.plugin),
                "suite_deadlines.toml",
                "*-r6-a1",
                Path::new("unused"),
                override_,
            )
            .await;
        assert!(
            report.failed_requests.is_empty(),
            "{:?}",
            report.failed_requests
        );
        assert_eq!(report.request_ids.len(), 1, "one request for both items");
        assert_eq!(
            report.request_deadlines,
            [(report.request_ids[0].clone(), expected)],
            "override {override_:?}"
        );
    }
}

/// The sha256 of `bytes`, taken here and spelled as a digest.
fn sha256_text(bytes: &[u8]) -> String {
    let mut text = String::from("sha256:");
    for byte in Sha256::digest(bytes) {
        text.extend(char::from_digit(u32::from(byte >> 4), 16));
        text.extend(char::from_digit(u32::from(byte & 0xf), 16));
    }
    text
}

/// The vector table of the live contexts.
fn live_vectors() -> toml::Table {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle/vectors/live_context_runs.toml"),
    )
    .expect("the vectors read")
    .parse()
    .expect("the vectors parse")
}

/// Guards: R9 — the echo plug-in returns the context it received, and the `context_digest` the
/// engine persisted is the RFC 8785 sha256 of that echo, which is the independent vector of each
/// of the three real runs. Oracle: `live_context_runs.toml`, written by `gen_live_context.py`.
#[tokio::test]
async fn the_persisted_context_digest_is_the_digest_of_what_the_plugin_received() {
    let rig = rig().await;
    let report = rig
        .grade("suite_echo.toml", "*-r{6,12,17}-a1", Path::new("unused"))
        .await;
    assert!(
        report.failed_requests.is_empty(),
        "{:?}",
        report.failed_requests
    );
    assert_eq!(report.request_ids.len(), 3, "one request per run");

    let vectors = live_vectors();
    let runs = vectors
        .get("run")
        .and_then(toml::Value::as_array)
        .expect("the vectors list runs");
    assert_eq!(runs.len(), 3, "vector runs");
    for vector in runs {
        let text = |key: &str| {
            vector
                .get(key)
                .and_then(toml::Value::as_str)
                .unwrap_or_else(|| panic!("the vector lacks `{key}`"))
                .to_owned()
        };
        let (name, projection, digest) = (text("run"), text("projection"), text("digest"));
        let replicate: u32 = name
            .rsplit_once("run-")
            .and_then(|(_, number)| number.parse().ok())
            .expect("`<config>/run-<N>`");
        let records = rig.records(&rig.run_of(replicate));
        assert_eq!(records.len(), 1, "{name} has one record");
        let (_, echo) = record(&records, "E1");
        assert_eq!(echo.status(), Status::Pass, "{name}");

        let seen = echoed(echo);
        let expected: Value = serde_json::from_str(&projection).expect("the projection parses");
        let mut received = seen.clone();
        received
            .as_object_mut()
            .expect("the echo is an object")
            .remove("prior");
        assert_eq!(received, expected, "{name}: what the plug-in received");

        let jcs = candler_schemas::canonical::to_jcs(&received).expect("a canonical form");
        assert_eq!(sha256_text(jcs.as_bytes()), digest, "{name}: the vector");
        assert_eq!(
            echo.context_digest().to_string(),
            digest,
            "{name}: the digest the engine persisted"
        );
    }
}

/// Guards: R5 — two regrades of one run with the counter kind each send a request, write their
/// own outcome file, and the second file's value differs under the same I4 key, so selection flags
/// it. Oracle: the hand literals: the counter reads 1, then 2.
#[tokio::test]
async fn a_regrade_that_changes_a_value_is_an_i4_violation() {
    let rig = rig().await;
    let count = rig.suites.join("count");
    let first = rig.grade("suite_counter.toml", "*-r6-a1", &count).await;
    let second = rig.grade("suite_counter.toml", "*-r6-a1", &count).await;
    assert_eq!(first.request_ids.len(), 1, "the first regrade sends");
    assert_eq!(
        second.request_ids.len(),
        1,
        "the second regrade sends again"
    );
    assert_ne!(
        first.request_ids, second.request_ids,
        "a new request each time"
    );
    assert_eq!(fs::read_to_string(&count).expect("the count"), "2");

    let run = rig.run_of(6);
    let records = rig.records(&run);
    assert_eq!(records.len(), 2, "one outcome file per regrade");
    let selection = select(records.into_iter().map(|(_, record)| record));
    assert_eq!(selection.violations().len(), 1, "one I4 violation");
    let violation = selection.violations().first().expect("a violation");
    assert_eq!(violation.key.expectation.as_str(), "C1");
    let count_of = |record: &OutcomeRecord| {
        serde_json::to_value(record).expect("a record serialises")["value"]["count"].clone()
    };
    assert_eq!(count_of(&violation.first), 1);
    assert_eq!(count_of(&violation.second), 2);
    assert_eq!(
        violation.first.request_id().to_string(),
        first.request_ids.first().expect("a request").to_string()
    );
}
