#![cfg(test)]
//! Guards for the session controls (R01 §Session runner): a job that runs a session cell
//! first checks that the instrument works, and only the instrument can refuse.
//!
//! The runner under test is a fake that returns the measures a harness would, so each row is a
//! broken or a working harness without a container. The real controls run in
//! `e2e/controls_e2e.rs`.

#[path = "support/session_rig.rs"]
mod rig;
mod support;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use candler_core::isolation::docker::{DockerCli, DockerError};
use candler_core::jobs::model::JobSpec;
use candler_core::jobs::orchestrate::controls::{Control, ControlRunner, Identity, Measures};
use candler_core::jobs::orchestrate::gate_controls;
use candler_core::jobs::orchestrate::session::{JobHost, OrchestrateError};
use candler_core::runner::Runtime;
use candler_core::runner::evidence::Inputs;
use candler_core::runner::evidence::session::ControlRef;
use candler_core::runner::exec::CellRun;
use candler_core::supervise::Supervisor;
use candler_schemas::primitives::Digest;
use futures::future::BoxFuture;
use serde_json::{Value, json};
use support::job_orchestrate::{Rig, spec};
use support::run_plan::Facts;

/// The key the fake runner reports: the base image's content digits and the digest of the controls.
const BASE_SRC: &str = "0123456789ab";
const DIGEST_A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const DIGEST_B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// How a row of the gate table breaks the control it names.
type Fault = (Control, fn(Measures) -> Measures);

/// What the fake runner was asked, and what it answers.
struct Fake {
    digest: Mutex<String>,
    base_id: Mutex<String>,
    broken: Mutex<Option<Fault>>,
    identified: AtomicUsize,
    runs: Mutex<Vec<String>>,
}

impl Fake {
    fn new(digest: &str) -> Arc<Self> {
        Arc::new(Self {
            digest: Mutex::new(digest.to_owned()),
            base_id: Mutex::new(format!("sha256:{}", "1".repeat(64))),
            broken: Mutex::new(None),
            identified: AtomicUsize::new(0),
            runs: Mutex::new(Vec::new()),
        })
    }

    fn runs(&self) -> usize {
        self.runs.lock().expect("the log").len()
    }
}

/// The measures of a working harness running `control`: the script's turns, the last turn ending
/// after the C1 job, and the close `idle_s` plus 0.3 s after it.
fn working(control: Control) -> Measures {
    let yielded = if control == Control::C1 {
        20_500
    } else {
        1_100
    };
    Measures {
        valid: true,
        completed: true,
        idle_s: Some(control.idle_s()),
        turns: Some(control.turns()),
        nothing_pending: Some(true),
        end: Some("closed".to_owned()),
        close_forced: Some(false),
        yielded_at_ms: Some(yielded),
        duration_ms: Some(yielded + control.idle_s() * 1000 + 300),
    }
}

impl ControlRunner for Fake {
    fn identify<'a>(&'a self, _rt: &'a Runtime) -> BoxFuture<'a, Result<Identity, String>> {
        self.identified.fetch_add(1, Ordering::SeqCst);
        let digest: Digest = self
            .digest
            .lock()
            .expect("the digest")
            .parse()
            .expect("a digest");
        let base_id = self.base_id.lock().expect("the base id").clone();
        Box::pin(async move {
            Ok(Identity {
                base_src: BASE_SRC.to_owned(),
                base_id,
                control_digest: digest,
            })
        })
    }

    fn run<'a>(
        &'a self,
        _rt: &'a Runtime,
        identity: &'a Identity,
        harness: &'a Value,
        control: Control,
        _cancel: tokio::sync::watch::Receiver<Option<candler_core::jobs::model::JobCause>>,
    ) -> BoxFuture<'a, Result<Measures, String>> {
        self.runs
            .lock()
            .expect("the log")
            .push(format!("{identity:?} {harness} {control:?}"));
        let measures = match *self.broken.lock().expect("the fault") {
            Some((broken, mutate)) if broken == control => mutate(working(control)),
            _ => working(control),
        };
        Box::pin(async move { Ok(measures) })
    }
}

/// A host that holds the fake runner (or none) and nothing else.
struct Host(Option<Arc<Fake>>);

impl JobHost for Host {
    fn docker_cli(
        &self,
        _supervisor: Arc<Supervisor>,
        _record_dir: PathBuf,
    ) -> Result<DockerCli, DockerError> {
        Err(DockerError::Unavailable {
            detail: "no docker".to_owned(),
        })
    }

    fn gateway<'a>(
        &'a self,
        _rt: &'a Runtime,
        _spec: &'a JobSpec,
    ) -> BoxFuture<'a, Result<Arc<dyn candler_core::isolation::gateway::Gateway>, OrchestrateError>>
    {
        Box::pin(async {
            Err(OrchestrateError::Gateway {
                reason: "no gateway".to_owned(),
            })
        })
    }

    fn controls(&self) -> Result<Arc<dyn ControlRunner>, String> {
        let runner: Arc<dyn ControlRunner> = self.0.clone().ok_or("no runner")?;
        Ok(runner)
    }
}

/// The job's handle on its own stop.
fn ctl() -> candler_core::jobs::model::JobControl {
    candler_core::jobs::model::JobControl {
        cancel: Arc::new(tokio::sync::watch::channel(None).0),
    }
}

/// A cell of mode `mode` (`session` or `headless`) whose model is `model`.
fn cell(mode: &str, model: &str, idle: bool) -> CellRun {
    let mut values = vec![
        json!({"path": "budget.per_run.wall_s", "value": 60}),
        json!({"path": "isolation.image", "value": format!("sha256:{}", "5".repeat(64))}),
        json!({"path": "model.id", "value": model}),
    ];
    if idle {
        values.push(json!({"path": "runner.idle_s", "value": 60}));
    }
    values.push(json!({"path": "runner.mode", "value": mode}));
    let comparable = serde_json::from_value(json!({
        "values": values,
        "harness": {"name": "claude-code", "version": "2.1.311"},
        "treatment": {"name": "htn"},
        "snapshots": [],
        "inputs": [],
        "environments": [],
    }))
    .expect("a comparable");
    CellRun::new(
        comparable,
        DIGEST_A.parse().expect("a digest"),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
        Inputs::default(),
    )
}

/// A job whose cells are `cells`, over a fresh store.
fn job(cells: Vec<CellRun>) -> (Rig, Runtime, JobSpec) {
    let rig = Rig::new(0);
    let mut rt = rig.runtime();
    rt.cells = cells
        .into_iter()
        .map(|cell| (cell.comparable.digest(), Arc::new(cell)))
        .collect::<BTreeMap<_, _>>();
    let facts = Facts {
        secrets: Vec::new(),
        ..Facts::base()
    };
    let job_spec = spec(&["m"], &facts, 60, 1);
    (rig, rt, job_spec)
}

fn records(rt: &Runtime) -> Vec<String> {
    let dir = rt.store.layout().root().join("controls");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|entry| Some(entry.ok()?.file_name().to_string_lossy().into_owned()))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// The harness the jobs' resolution names (`Facts::base`: `claude-code` at `2.1.0`).
const HARNESS: &str = "claude-code 2.1.0";

/// Guards: R01 §Session runner — a session job whose harness fails a control is refused
/// for session mode, naming the harness version and the control, and writes no record; a working
/// harness passes; a job with no session cell, and a session job on a host with no runner, never
/// touch a control (the second is refused, the first runs).
/// Oracle: the fake model's scripts by hand (c1: two turns, the last after the 20 s job; c2: one
/// turn) and the close rule (the input closes `idle_s` after the last turn): a harness that
/// ignores EOF or never delivers a task notification fails the shape its row breaks.
/// Bite: close the band on one side only (the late and the early rows), skip the turns check (the
/// notification row), refuse for the model instead of the harness version (every refusal row).
#[tokio::test]
async fn controls_refuse_a_broken_harness_only() {
    let rows: Vec<(&str, Option<Fault>, Option<&str>)> = vec![
        ("a working harness", None, None),
        (
            "c2 closes 1.5 s late",
            Some((Control::C2, |m| Measures {
                duration_ms: m.duration_ms.map(|d| d + 1_200),
                ..m
            })),
            Some("c2"),
        ),
        (
            "c2 closes before idle_s",
            Some((Control::C2, |m| Measures {
                duration_ms: m.duration_ms.map(|d| d - 600),
                ..m
            })),
            Some("c2"),
        ),
        (
            "c2 never stops",
            Some((Control::C2, |m| Measures {
                turns: Some(2),
                end: Some("cap".to_owned()),
                ..m
            })),
            Some("c2"),
        ),
        (
            "c2 ignores end of input",
            Some((Control::C2, |m| Measures {
                close_forced: Some(true),
                ..m
            })),
            Some("c2"),
        ),
        (
            "c2 ran with another idle_s",
            Some((Control::C2, |m| Measures {
                idle_s: Some(60),
                ..m
            })),
            Some("c2"),
        ),
        (
            "c1 notifications are never delivered",
            Some((Control::C1, |m| Measures {
                turns: Some(1),
                yielded_at_ms: Some(400),
                nothing_pending: Some(false),
                ..m
            })),
            Some("c1"),
        ),
    ];
    for (name, fault, refused_control) in rows {
        let (_rig, mut rt, spec) = job(vec![cell("session", "m", true)]);
        let fake = Fake::new(DIGEST_A);
        *fake.broken.lock().expect("the fault") = fault;
        let host = Host(Some(Arc::clone(&fake)));
        let outcome = gate_controls(&mut rt, &spec, &host, &ctl()).await;
        match (outcome, refused_control) {
            (Ok(()), None) => {
                assert_eq!(records(&rt).len(), 1, "{name}: one record is written");
            }
            (
                Err(OrchestrateError::Controls {
                    harness, control, ..
                }),
                Some(expected),
            ) => {
                assert_eq!(harness, HARNESS, "{name}: names the harness version");
                assert_eq!(control, expected, "{name}: names the failed control");
                assert!(
                    records(&rt).is_empty(),
                    "{name}: a failing verdict writes nothing"
                );
            }
            (other, expected) => {
                panic!("{name}: got {other:?}, expected a refusal of {expected:?}")
            }
        }
    }

    // a job with only headless cells never identifies, runs or reads a control
    let (_rig, mut rt, spec) = job(vec![cell("headless", "m", false)]);
    let fake = Fake::new(DIGEST_A);
    gate_controls(&mut rt, &spec, &Host(Some(Arc::clone(&fake))), &ctl())
        .await
        .expect("headless is not gated");
    assert_eq!(
        fake.identified.load(Ordering::SeqCst),
        0,
        "headless: nothing identified"
    );
    assert_eq!(fake.runs(), 0, "headless: no control runs");
    assert!(records(&rt).is_empty(), "headless: no record is made");
    assert!(
        rt.cells.values().all(|cell| cell.control.is_none()),
        "headless: no record relied on"
    );

    // a session job on a host that cannot run controls is refused, not let through
    let (_rig, mut rt, spec) = job(vec![cell("session", "m", true)]);
    let refused = gate_controls(&mut rt, &spec, &Host(None), &ctl()).await;
    assert!(
        matches!(&refused, Err(OrchestrateError::Controls { control, .. }) if control == "setup"),
        "no runner: {refused:?}"
    );
}

/// Guards: R01 §Session runner — a passing record is reused without a re-run by the next
/// job on the same base image under the same controls, a changed control digest is another key and
/// re-runs, and every session cell of the job carries the record it relies on while a headless
/// cell of the same job carries none.
/// Oracle: the record's name is `<base content digits>-<first 16 hex of the control digest>.json`
/// below `controls/`, written by hand from the digests the fake reports; the cell's reference is
/// that path and the digest of the file's bytes, computed with `sha256sum` semantics by the test.
/// A base image rebuilt from the same sources (another image id) reuses the record.
/// Bite: refuse a record whose base image id differs (the rebuilt-base step fails), key the record on the base image alone (the changed digest row finds the old record),
/// or re-run on every job (the reuse row's run count grows).
#[tokio::test]
async fn controls_record_is_reused_until_the_control_changes() {
    let (_rig, mut rt, spec) = job(vec![
        cell("session", "m", true),
        cell("headless", "n", false),
    ]);
    let fake = Fake::new(DIGEST_A);
    let host = Host(Some(Arc::clone(&fake)));
    gate_controls(&mut rt, &spec, &host, &ctl())
        .await
        .expect("the first job passes");
    assert_eq!(fake.runs(), 2, "C1 and C2 ran once each");
    let name = format!("{BASE_SRC}-{}.json", "a".repeat(16));
    assert_eq!(
        records(&rt),
        vec![name.clone()],
        "one record, named by its key"
    );
    let bytes =
        std::fs::read(rt.store.layout().root().join("controls").join(&name)).expect("the record");
    let sealed = {
        use sha2::Digest as _;
        use std::fmt::Write as _;
        let mut text = String::from("sha256:");
        for byte in sha2::Sha256::digest(&bytes) {
            write!(text, "{byte:02x}").expect("a string write");
        }
        text
    };
    for cell in rt.cells.values() {
        let session = cell.comparable.values().iter().any(|value| {
            value.path().as_str() == "runner.mode"
                && value.value().is_some_and(|v| v.as_value() == "session")
        });
        match (&cell.control, session) {
            (Some(control), true) => {
                assert_eq!(control.path, format!("controls/{name}"));
                assert_eq!(control.digest.to_string(), sealed);
            }
            (None, false) => {}
            (found, session) => panic!("session {session} cell relies on {found:?}"),
        }
    }

    let (_rig2, mut again, _) = job(vec![cell("session", "m", true)]);
    // the second job shares the first job's store
    again.store = rt.store.clone();
    gate_controls(&mut again, &spec, &host, &ctl())
        .await
        .expect("the second job passes");
    assert_eq!(
        fake.runs(),
        2,
        "a record that is there is reused without a run"
    );
    // a base image rebuilt from the same sources has another id and the same content
    *fake.base_id.lock().expect("the base id") = format!("sha256:{}", "2".repeat(64));
    gate_controls(&mut again, &spec, &host, &ctl())
        .await
        .expect("the rebuilt base passes");
    assert_eq!(fake.runs(), 2, "a rebuilt base reuses the record");

    *fake.digest.lock().expect("the digest") = DIGEST_B.to_owned();
    gate_controls(&mut again, &spec, &host, &ctl())
        .await
        .expect("the third job passes");
    assert_eq!(
        fake.runs(),
        4,
        "a changed control digest runs both controls again"
    );
    assert_eq!(
        records(&rt).len(),
        2,
        "the old record stays, the new one is another key"
    );
}

/// Guards: R01 §Session runner — no model's outcome or tool list reaches the control
/// verdict: the runner is given the base image, the harness and the control and nothing of any
/// cell's model, and two jobs that differ only in the model of their cells get the same record.
/// Oracle: the arguments the fake runner logs (by hand: identity, harness, control) and the
/// record names written for models that fail every task, call every tool or refuse.
/// Bite: pass the cell's `model.id` into the runner, or key the refusal on it (the logged
/// arguments name the model).
#[tokio::test]
async fn controls_never_model_reaches_the_verdict() {
    let mut names = Vec::new();
    for model in [
        "c1",
        "gpt-never-finishes",
        "refuses-every-task",
        "calls-every-tool",
    ] {
        let (_rig, mut rt, spec) = job(vec![cell("session", model, true)]);
        let fake = Fake::new(DIGEST_A);
        gate_controls(&mut rt, &spec, &Host(Some(Arc::clone(&fake))), &ctl())
            .await
            .unwrap_or_else(|error| panic!("{model}: {error}"));
        names.extend(records(&rt));
        let seen = fake.runs.lock().expect("the log").join("\n");
        assert!(
            !seen.contains(model) || model == "c1",
            "{model}: the runner saw the model: {seen}"
        );
        assert!(
            !seen.contains("model"),
            "{model}: the runner saw a model setting: {seen}"
        );
    }
    assert_eq!(names.len(), 4, "a record for each job");
    names.dedup();
    assert_eq!(names.len(), 1, "every model gets the same record");
}

/// The stub's script for c2's single turn, then end of input, then exit 0.
fn c2_script() -> Vec<Value> {
    let mut steps = rig::replay_of("c2", &BTreeMap::new());
    steps.push(rig::wait_eof(None));
    steps.push(rig::exit(0));
    steps
}

/// Guards: R01 §Session runner — a session run records the control record it relied on at
/// `session.control` (the record's path and the digest of its bytes), and a run that relied on
/// none records none.
/// Oracle: the reference the test gives the cell, read back from the sealed run's manifest.
/// Bite: leave `session.control` out of the run's measures (the first assertion fails), or record
/// it on a run that relied on none (the second).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn controls_recorded_on_the_session_run() {
    let reference = ControlRef {
        path: format!("controls/{BASE_SRC}-{}.json", "a".repeat(16)),
        digest: DIGEST_B.parse().expect("a digest"),
    };
    let values = rig::cell_values(&[]);
    let with = rig::run_controlled(
        &c2_script(),
        &values,
        rig::Driver::new(true),
        None,
        Some(reference.clone()),
    )
    .await;
    with.outcome();
    assert_eq!(
        with.measured("session.control"),
        Some(json!({"path": reference.path, "digest": DIGEST_B})),
        "the run names its record"
    );
    let without = rig::run(&c2_script(), &values, rig::Driver::new(true), None).await;
    without.outcome();
    assert_eq!(
        without.measured("session.control"),
        None,
        "no record, none named"
    );
    assert!(
        without.measured("session.turns").is_some(),
        "the run is a sealed session run"
    );
}
