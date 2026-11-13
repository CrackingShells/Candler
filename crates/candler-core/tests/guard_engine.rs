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
use std::num::NonZeroU32;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use candler_core::evaluation::engine::{
    EvaluateReport, EvaluateRequest, EvaluatorSlots, RunSelection, evaluate, evaluate_reporting,
};
use candler_core::evaluation::registry::EnvironmentKey;
use candler_core::evaluation::selection::select::select;
use candler_core::jobs::slots::{Holder, Slots, default_evaluator_slots, evaluator_slots_dir};
use candler_core::store::Store;
use candler_core::store::outcome::OutcomeWriter;
use candler_core::supervise::signal::Signaller;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::EventPayload;
use candler_schemas::outcome::{Cause, OutcomeRecord, Status};
use candler_schemas::primitives::{NameVersion, RunKey};
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
    tmp: tempfile::TempDir,
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
        tmp,
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
            self.request(
                plugins,
                path,
                RunSelection::Glob(glob.to_owned()),
                self.pool(2),
                deadline,
            ),
            // BORROW: .clone() hands the engine its own handle on the supervisor
            self.supervisor.clone(),
            cancel,
        )
        .await
        .expect("the engine grades the runs")
    }

    /// An evaluator pool of `count` slots under the rig's scratch directory.
    fn pool(&self, count: u32) -> EvaluatorSlots {
        EvaluatorSlots {
            dir: self.tmp.path().join("evaluator-slots"),
            count: NonZeroU32::new(count).expect("a slot"),
            job: "guard-engine".to_owned(),
        }
    }

    /// The request that grades `selection` of the rig's store with `plugins` and the suite file
    /// `suite`, from the pool `slots`.
    fn request(
        &self,
        plugins: &[PathBuf],
        suite: PathBuf,
        selection: RunSelection,
        slots: EvaluatorSlots,
        deadline: Option<Duration>,
    ) -> EvaluateRequest {
        EvaluateRequest {
            store: self.store.clone(),
            plugins: plugins.to_vec(),
            suite,
            selection,
            slots,
            env_root: self.env_root.clone(),
            request_deadline: deadline,
            record_dir: None,
        }
    }

    /// Writes the fixture suite `suite` to the rig's suite directory with every `(token, text)`
    /// of `fills` replaced, and returns its path.
    fn suite_file(&self, suite: &str, fills: &[(&str, String)]) -> PathBuf {
        let mut text = fs::read_to_string(fixtures().join(suite)).expect("the suite reads");
        for (token, fill) in fills {
            text = text.replace(token, fill);
        }
        let path = self.suites.join(suite);
        fs::write(&path, text).expect("the suite writes");
        path
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

/// Guards: E2 (the default pool size) — an evaluator is short and CPU-bound, so the default pool
/// is half the CPUs rounded down and never fewer than one. Oracle: the table written by hand,
/// 0, 1, 2, 3, 4 and 16 CPUs giving 1, 1, 1, 1, 2 and 8.
/// Bite: use the CPU count whole (16 gives 16).
#[test]
fn e2_default_evaluator_slots_follow_the_cpu_count() {
    for (cpus, slots) in [(0, 1), (1, 1), (2, 1), (3, 1), (4, 2), (16, 8)] {
        assert_eq!(default_evaluator_slots(cpus), slots, "{cpus} CPUs");
    }
}

/// Reads the lines `<pid> start|end <monotonic>` of an interval file, as `(start, end)` per pid.
fn intervals(file: &Path) -> Vec<(f64, f64)> {
    let mut by_pid: BTreeMap<String, (Option<f64>, Option<f64>)> = BTreeMap::new();
    for line in fs::read_to_string(file).expect("the interval file").lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [pid, what, at] = fields.as_slice() else {
            panic!("a malformed interval line: {line:?}");
        };
        let at: f64 = at.parse().expect("a time");
        let entry = by_pid.entry((*pid).to_owned()).or_default();
        match *what {
            "start" => entry.0 = Some(at),
            "end" => entry.1 = Some(at),
            other => panic!("an unknown mark {other:?}"),
        }
    }
    by_pid
        .into_values()
        .map(|(start, end)| (start.expect("a start"), end.expect("an end")))
        .collect()
}

/// Whether any two of the intervals overlap.
fn overlap(spans: &[(f64, f64)]) -> bool {
    spans
        .iter()
        .enumerate()
        .any(|(i, a)| spans.iter().skip(i + 1).any(|b| a.0 < b.1 && b.0 < a.1))
}

/// Guards: E2 — an evaluator request waits for a slot of the evaluator pool and holds it while it
/// runs: with one slot two requests of the `interval` kind never overlap, with two they do (the
/// positive control), and while they run the pool's holder files name the call's job. Oracle: the
/// plug-in's own `start` and `end` lines, written by the evaluator process, and the slot files of
/// `Slots::held`.
/// Bite: keep a tokio semaphore instead of the pool, or take no slot: no holder file is ever seen
/// and the one-slot run overlaps.
#[tokio::test]
async fn e2_one_evaluator_slot_serialises_requests_and_two_overlap() {
    for (count, overlaps) in [(1_u32, false), (2, true)] {
        let rig = rig_of("plugin_pool.toml").await;
        let marks = rig.suites.join("interval.log");
        let suite = rig.suite_file(
            "suite_pool_interval.toml",
            &[
                ("@FILE@", marks.display().to_string()),
                ("@SLEEP@", "1.0".to_owned()),
            ],
        );
        let slots = rig.pool(count);
        let (dir, job) = (slots.dir.clone(), slots.job.clone());
        let request = rig.request(
            std::slice::from_ref(&rig.plugin),
            suite,
            RunSelection::Glob("*-r{6,12}-a1".to_owned()),
            slots,
            Some(Duration::from_secs(120)),
        );
        let (_keep, cancel) = watch::channel(false);
        let done = AtomicBool::new(false);
        let grading = async {
            let report = evaluate(request, Arc::clone(&rig.supervisor), cancel).await;
            done.store(true, Ordering::SeqCst);
            report
        };
        let watching = async {
            let mut most = 0;
            while !done.load(Ordering::SeqCst) {
                if dir.is_dir() {
                    let held = Slots::open(&dir, count)
                        .expect("the pool opens")
                        .held()
                        .expect("the pool reads");
                    let mine = held
                        .iter()
                        .filter(|info| !info.stale)
                        .filter(|info| info.holder.as_ref().is_some_and(|h| h.job == job))
                        .count();
                    most = most.max(mine);
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            most
        };
        let (report, most) = tokio::join!(grading, watching);
        let report = report.expect("the engine grades the runs");
        assert!(
            report.failed_requests.is_empty(),
            "{:?}",
            report.failed_requests
        );
        assert_eq!(report.request_ids.len(), 2, "one request per run");
        let spans = intervals(&marks);
        assert_eq!(spans.len(), 2, "two evaluator processes ran");
        assert_eq!(overlap(&spans), overlaps, "{count} slot(s): {spans:?}");
        assert!(most >= 1, "{count} slot(s): a holder of this job was seen");
        assert!(
            most <= usize::try_from(count).expect("a small count"),
            "never more holders than slots"
        );
    }
}

/// Guards: E2 — the evaluator pool is not the agent pool: with every slot of the agent pool
/// directory held, an evaluation over its own pool of one ends inside 60 s with no failed request.
/// Oracle: the clock, and `Slots::try_acquire` showing the agent pool full.
/// Bite: draw the evaluators from the agent pool: the held slots stall the call until the timeout.
#[tokio::test]
async fn e2_agent_slots_held_by_a_job_do_not_delay_an_evaluator() {
    let rig = rig_of("plugin_pool.toml").await;
    let state = rig.suites.join("state");
    let agent_dir = state.join("candler").join("slots");
    let agents = Slots::open(&agent_dir, 2).expect("the agent pool opens");
    let holder = Holder {
        job: "an-agent-job".to_owned(),
        run: None,
    };
    let _first = agents.acquire(&holder).await.expect("the first slot");
    let _second = agents.acquire(&holder).await.expect("the second slot");
    assert!(
        agents.try_acquire(&holder).expect("a probe").is_none(),
        "the agent pool is full"
    );
    let slots = EvaluatorSlots {
        dir: evaluator_slots_dir(&state),
        count: NonZeroU32::MIN,
        job: "an-evaluate-job".to_owned(),
    };
    assert_ne!(slots.dir, agent_dir, "a directory of its own");
    let suite = rig.suite_file("suite_pool_ok.toml", &[]);
    let request = rig.request(
        std::slice::from_ref(&rig.plugin),
        suite,
        RunSelection::Glob("*-r{6,12}-a1".to_owned()),
        slots,
        Some(Duration::from_secs(120)),
    );
    let (_keep, cancel) = watch::channel(false);
    let report = tokio::time::timeout(
        Duration::from_secs(60),
        evaluate(request, Arc::clone(&rig.supervisor), cancel),
    )
    .await
    .expect("the evaluation ends inside 60 s while every agent slot is held")
    .expect("the engine grades the runs");
    assert!(
        report.failed_requests.is_empty(),
        "{:?}",
        report.failed_requests
    );
    assert_eq!(report.request_ids.len(), 2, "one request per run");
}

/// What a sink saw, and the outcome events that arrived before their record was in the store.
struct Events {
    /// The store the records are published to.
    store: PathBuf,
    /// Every event, with its run.
    seen: Mutex<Vec<(Option<RunKey>, EventPayload)>>,
    /// The ids of the records whose event came before their file was visible.
    early: Mutex<Vec<String>>,
}

impl Events {
    fn new(store: &Path) -> Arc<Self> {
        Arc::new(Self {
            store: store.to_path_buf(),
            seen: Mutex::new(Vec::new()),
            early: Mutex::new(Vec::new()),
        })
    }

    /// The `evaluation.outcome` events, each as `(run, record as JSON)`, in arrival order.
    fn outcomes(&self) -> Vec<(Option<RunKey>, Value)> {
        self.seen
            .lock()
            .expect("a lock")
            .iter()
            .filter_map(|(run, payload)| {
                if let EventPayload::EvaluationOutcome { record, .. } = payload {
                    Some((
                        run.clone(),
                        serde_json::to_value(record.as_ref()).expect("a record serialises"),
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    /// The `outcomes` of the `evaluation.planned` events, in arrival order.
    fn planned(&self) -> Vec<u32> {
        self.seen
            .lock()
            .expect("a lock")
            .iter()
            .filter_map(|(_, payload)| {
                if let EventPayload::EvaluationPlanned { outcomes, .. } = payload {
                    Some(*outcomes)
                } else {
                    None
                }
            })
            .collect()
    }
}

impl EventSink for Events {
    fn emit(&self, run: Option<&RunKey>, payload: EventPayload) {
        if let EventPayload::EvaluationOutcome { record, .. } = &payload {
            let store = Store::open(&self.store).expect("the store opens");
            let wanted = serde_json::to_value(record.as_ref()).expect("a record serialises");
            let visible = store
                .outcome_files(record.run())
                .expect("the outcome files list")
                .iter()
                .filter(|file| file.request == *record.request_id())
                .any(|file| {
                    store
                        .read_outcomes(file)
                        .expect("the file reads")
                        .iter()
                        .any(|read| serde_json::to_value(read).expect("a record") == wanted)
                });
            if !visible {
                self.early
                    .lock()
                    .expect("a lock")
                    .push(record.expectation_id().to_string());
            }
        }
        self.seen
            .lock()
            .expect("a lock")
            .push((run.cloned(), payload));
    }
}

/// Guards: E1 — through `evaluate_reporting` the engine reports one `evaluation.outcome` per
/// record it commits, each carrying its run, each equal to its line of the outcome file and in the
/// file's order, and each arriving only once its file is published; one `evaluation.planned`
/// before them counts the records. A call cancelled before it starts, and a request cancelled in
/// flight, report none and leave no file. Oracle: the outcome files read back from the store, and
/// the store as the sink sees it at the moment of each event.
/// Bite: commit records and send no event; send the event before the commit.
#[tokio::test]
async fn e1_engine_emits_one_outcome_event_per_committed_record() {
    let rig = rig().await;
    let events = Events::new(&rig.store);
    let suite = rig.suite_file("suite_dependencies.toml", &[]);
    let request = rig.request(
        std::slice::from_ref(&rig.plugin),
        suite,
        RunSelection::Glob("*-r{6,12}-a1".to_owned()),
        rig.pool(2),
        Some(Duration::from_secs(120)),
    );
    let (_keep, cancel) = watch::channel(false);
    // TRAIT_OBJECT: the engine takes its sink as a trait object
    // BORROW: .clone() shares the log between the engine and the test
    let sink: Arc<dyn EventSink> = events.clone();
    let report = evaluate_reporting(request, Arc::clone(&rig.supervisor), cancel, sink)
        .await
        .expect("the engine grades the runs");
    assert!(
        report.failed_requests.is_empty(),
        "{:?}",
        report.failed_requests
    );
    assert_eq!(report.outcome_files.len(), 2, "one file per run");

    let mut read_back = Vec::new();
    for replicate in [6, 12] {
        let run = rig.run_of(replicate);
        for (_, record) in rig.records(&run) {
            read_back.push((
                Some(run.clone()),
                serde_json::to_value(&record).expect("a record serialises"),
            ));
        }
    }
    let reported = events.outcomes();
    assert_eq!(
        reported.len(),
        read_back.len(),
        "one event per committed record"
    );
    assert_eq!(
        reported.len(),
        usize::try_from(report.counts.values().sum::<u64>()).expect("a small count"),
        "the counts of the report agree"
    );
    for run in [rig.run_of(6), rig.run_of(12)] {
        let of_run = |list: &[(Option<RunKey>, Value)]| -> Vec<Value> {
            list.iter()
                .filter(|(key, _)| key.as_ref() == Some(&run))
                .map(|(_, record)| record.clone())
                .collect()
        };
        assert_eq!(
            of_run(&reported),
            of_run(&read_back),
            "{run}: each record equals its file's line, in order"
        );
    }
    assert!(
        events.early.lock().expect("a lock").is_empty(),
        "an event came before its record was published: {:?}",
        events.early.lock().expect("a lock")
    );
    assert_eq!(
        events.planned(),
        [u32::try_from(reported.len()).expect("a small count")],
        "one `evaluation.planned`, counting the records"
    );
    let first_outcome = events
        .seen
        .lock()
        .expect("a lock")
        .iter()
        .position(|(_, payload)| matches!(payload, EventPayload::EvaluationOutcome { .. }))
        .expect("an outcome event");
    let planned_at = events
        .seen
        .lock()
        .expect("a lock")
        .iter()
        .position(|(_, payload)| matches!(payload, EventPayload::EvaluationPlanned { .. }))
        .expect("a planned event");
    assert!(planned_at < first_outcome, "the plan comes first");

    // A call cancelled before it starts reports nothing and commits nothing.
    let rig = rig_of("plugin_pool.toml").await;
    let events = Events::new(&rig.store);
    let marks = rig.suites.join("never.log");
    let suite = rig.suite_file(
        "suite_pool_interval.toml",
        &[
            ("@FILE@", marks.display().to_string()),
            ("@SLEEP@", "30".to_owned()),
        ],
    );
    let request = rig.request(
        std::slice::from_ref(&rig.plugin),
        suite.clone(), // BORROW: the second call reads the same suite
        RunSelection::Glob("*-r{6,12}-a1".to_owned()),
        rig.pool(2),
        None,
    );
    let (_keep, cancel) = watch::channel(true);
    // TRAIT_OBJECT: the engine takes its sink as a trait object
    // BORROW: .clone() shares the log between the engine and the test
    let sink: Arc<dyn EventSink> = events.clone();
    let report = evaluate_reporting(request, Arc::clone(&rig.supervisor), cancel, sink)
        .await
        .expect("a cancelled call returns");
    assert!(events.outcomes().is_empty(), "no event");
    assert!(report.outcome_files.is_empty(), "no file");
    assert!(!marks.exists(), "no request started");

    // A request cancelled in flight reports nothing, commits nothing and is a failed request.
    let request = rig.request(
        std::slice::from_ref(&rig.plugin),
        suite,
        RunSelection::Glob("*-r6-a1".to_owned()),
        rig.pool(2),
        None,
    );
    let (cancel_tx, cancel) = watch::channel(false);
    // TRAIT_OBJECT: the engine takes its sink as a trait object
    // BORROW: .clone() shares the log between the engine and the test
    let sink: Arc<dyn EventSink> = events.clone();
    let grading = evaluate_reporting(request, Arc::clone(&rig.supervisor), cancel, sink);
    let cancelling = async {
        while !marks.exists() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        cancel_tx.send(true).expect("a receiver");
    };
    let (report, ()) = tokio::join!(grading, cancelling);
    let report = report.expect("a cancelled call returns");
    assert!(
        events.outcomes().is_empty(),
        "no event for a cancelled request"
    );
    assert!(
        report.outcome_files.is_empty(),
        "no file for a cancelled request"
    );
    assert_eq!(
        report.failed_requests.len(),
        1,
        "the request is a failed one"
    );
    assert!(
        rig.records_of(&rig.run_of(6), "pool").is_empty(),
        "nothing was committed for it"
    );
}

/// How many outcome files of `pool@0.1.0`, the version the fixture plug-in describes, `run` has.
fn pool_files(rig: &Rig, run: &RunKey) -> usize {
    Store::open(&rig.store)
        .expect("the store opens")
        .outcome_files(run)
        .expect("the outcome files list")
        .iter()
        .filter(|file| file.evaluator.to_string() == "pool@0.1.0")
        .count()
}

/// The done runs of the rig's store.
fn done_runs(rig: &Rig) -> Vec<RunKey> {
    Store::open(&rig.store)
        .expect("the store opens")
        .runs()
        .expect("the runs list")
        .into_iter()
        .filter(|entry| entry.done)
        .map(|entry| entry.key)
        .collect()
}

/// Guards: E1 — `Keys` grades exactly the keys it holds; after a call with `Glob` over one run,
/// `Pending` grades the other runs and not that one, a second `Pending` grades none, and an outcome
/// file of another version of the evaluator does not make a run complete. Oracle: the count of
/// requests and the outcome files in the store, by run, written out by hand.
/// Bite: `Pending` selecting every run; `Pending` counting a file of another evaluator version.
#[tokio::test]
async fn e1_selection_keys_and_pending_pick_the_runs_they_name() {
    let rig = rig_of("plugin_pool.toml").await;
    let suite = rig.suite_file("suite_pool_ok.toml", &[]);
    let runs = done_runs(&rig);
    assert!(runs.len() >= 4, "the mini corpus has runs to choose among");
    let (first, second) = (rig.run_of(6), rig.run_of(12));
    let grade = |selection: RunSelection| {
        let request = rig.request(
            std::slice::from_ref(&rig.plugin),
            suite.clone(), // BORROW: every call reads the same suite
            selection,
            rig.pool(2),
            Some(Duration::from_secs(120)),
        );
        let (keep, cancel) = watch::channel(false);
        let supervisor = Arc::clone(&rig.supervisor);
        async move {
            let report = evaluate(request, supervisor, cancel)
                .await
                .expect("the engine grades the runs");
            drop(keep);
            report
        }
    };
    let graded = |run: &RunKey| pool_files(&rig, run);

    let report = grade(RunSelection::Keys(vec![first.clone(), second.clone()])).await;
    assert_eq!(report.request_ids.len(), 2, "exactly the two keys");
    assert_eq!((graded(&first), graded(&second)), (1, 1));
    for other in runs.iter().filter(|run| **run != first && **run != second) {
        assert_eq!(graded(other), 0, "{other} was not named");
    }

    // Keep one run for the Glob call: start from a clean set of results for the others.
    let rig2 = rig_of("plugin_pool.toml").await;
    let suite2 = rig2.suite_file("suite_pool_ok.toml", &[]);
    let call = |selection: RunSelection| {
        let request = rig2.request(
            std::slice::from_ref(&rig2.plugin),
            suite2.clone(), // BORROW: every call reads the same suite
            selection,
            rig2.pool(2),
            Some(Duration::from_secs(120)),
        );
        let (keep, cancel) = watch::channel(false);
        let supervisor = Arc::clone(&rig2.supervisor);
        async move {
            let report = evaluate(request, supervisor, cancel)
                .await
                .expect("the engine grades the runs");
            drop(keep);
            report
        }
    };
    let graded2 = |run: &RunKey| pool_files(&rig2, run);
    let runs2 = done_runs(&rig2);
    let (one, other) = (rig2.run_of(6), rig2.run_of(12));

    let report = call(RunSelection::Glob("*-r6-a1".to_owned())).await;
    assert_eq!(report.request_ids.len(), 1, "the glob names one run");
    assert_eq!(graded2(&one), 1);

    // An outcome file of another version of the evaluator, for the other run, is no completion.
    let record = rig2
        .records_of(&one, "pool")
        .pop()
        .expect("the graded run has a record")
        .1;
    let mut text = serde_json::to_value(&record).expect("a record serialises");
    text["evaluator"] = Value::from("pool@0.0.1");
    text["run"] = serde_json::to_value(&other).expect("a key serialises");
    let foreign: OutcomeRecord = serde_json::from_value(text).expect("a record of another version");
    let store = Store::open(&rig2.store).expect("the store opens");
    let version: NameVersion = "pool@0.0.1".parse().expect("an identity");
    let mut writer =
        OutcomeWriter::create(&store, &other, &version, foreign.request_id()).expect("a writer");
    writer.append(&foreign).expect("the record is written");
    writer.commit().expect("the file is published");

    let report = call(RunSelection::Pending).await;
    assert_eq!(
        report.request_ids.len(),
        runs2.len() - 1,
        "every run but the graded one"
    );
    assert_eq!(graded2(&one), 1, "the graded run is not graded again");
    assert_eq!(
        graded2(&other),
        1,
        "the run with a file of another version is graded"
    );
    for run in &runs2 {
        assert_eq!(graded2(run), 1, "{run} has its one record");
    }

    let report = call(RunSelection::Pending).await;
    assert!(
        report.request_ids.is_empty(),
        "a second Pending grades none"
    );
    assert_eq!(report.outcome_files, Vec::<PathBuf>::new());
}

/// Keeps the marker of the record of every process the supervisor spawns.
#[derive(Default)]
struct Spawned {
    markers: Mutex<Vec<String>>,
}

impl EventSink for Spawned {
    fn emit(&self, _run: Option<&RunKey>, payload: EventPayload) {
        if let EventPayload::ProcessSpawned { record, .. } = payload {
            let marker = record.marker().to_string();
            self.markers.lock().expect("a lock").push(marker);
        }
    }
}

/// Guards: E3 (the marker) — every evaluator process of an evaluation carries a marker of its own,
/// `evaluation-<id>`, never the constant `candler-evaluation`: `ProcMarkerSweeper` kills by an
/// exact marker, so a shared one would let one job's sweep end another job's evaluators. Oracle:
/// the `marker` of the child record each `process.spawned` carries.
/// Bite: the constant marker back (two requests share it).
#[tokio::test]
async fn e3_each_request_carries_its_own_marker() {
    let rig = rig_of("plugin_pool.toml").await;
    let suite = rig.suite_file("suite_pool_ok.toml", &[]);
    let request = rig.request(
        std::slice::from_ref(&rig.plugin),
        suite,
        RunSelection::Glob("*-r{6,12}-a1".to_owned()),
        rig.pool(2),
        Some(Duration::from_secs(120)),
    );
    let spawned = Arc::new(Spawned::default());
    // TRAIT_OBJECT: the supervisor takes its sink as a trait object
    // BORROW: .clone() shares the markers between the supervisor and the test
    let sink: Arc<dyn EventSink> = spawned.clone();
    let (_keep, cancel) = watch::channel(false);
    let report = evaluate(request, Arc::new(Supervisor::new(sink)), cancel)
        .await
        .expect("the engine grades the runs");
    assert_eq!(report.request_ids.len(), 2, "two requests");

    let mut markers = spawned.markers.lock().expect("a lock").clone();
    markers.sort();
    // One describe and one process per request.
    assert_eq!(markers.len(), 3, "{markers:?}");
    let mut distinct = markers.clone(); // BORROW: the check below reads the sorted list
    distinct.dedup();
    assert_eq!(distinct, markers, "no two processes share a marker");
    for marker in &markers {
        assert!(marker.starts_with("evaluation-"), "{marker}");
        assert_ne!(marker, "candler-evaluation");
    }
}
