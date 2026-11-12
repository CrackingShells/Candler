#![cfg(test)]
//! Guards for the record of a run's agent container: the isolation writes it from the run's
//! cidfile through the job's recorder, and every destroy path then finds the container recorded.
//! The docker half is the recording fake of `test-support`; no test reaches docker.

mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use candler_core::isolation::container::DockerIsolation;
use candler_core::isolation::container::guard::ContainerGuard;
use candler_core::isolation::container::sweeper::{ContainerSweeper, SweepError};
use candler_core::isolation::driver::DockerDriver;
use candler_core::isolation::gateway::ChildRecorder;
use candler_core::isolation::model::{
    ContainerPlan, ContainerSweep, GatewayHandle, ImageId, ImageRef, Inspect, InspectState, Labels,
    Limits, NetworkName, SweepScope, UserSpec,
};
use candler_core::isolation::testing::{RecordingDestroyer, RecordingDocker};
use candler_core::isolation::{Isolation as _, IsolationError};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::model::{JobCause, JobControl, JobOutcome};
use candler_core::jobs::orchestrate::orchestrate_with;
use candler_core::supervise::EventSink;
use candler_schemas::container::{ContainerFound, ContainerRole};
use candler_schemas::execution::{EventPayload, JobState};
use candler_schemas::primitives::{ContainerId, JobId, RunKey, Timestamp};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use support::container_stop::{cid, container_ref, digest, inspect, job, labels, run_key};
use support::job_orchestrate::{FakeAgent, FakeHost, Rig, spec};
use support::job_runtime::Recorder;
use support::job_store::job_record;
use support::run_plan::{Facts, Run, fresh_store, manifest};
use tokio::sync::watch;

/// The sha256 digest the scripted inspect gives as the agent's image id.
const IMAGE_ID: &str = "sha256:5555555555555555555555555555555555555555555555555555555555555555";

/// The creation time the scripted inspect gives, as docker prints it.
const CREATED: &str = "2026-03-04T05:06:07.123456789Z";

/// The directory of this crate.
fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A job directory in a fresh store.
fn job_dir(job: &JobId) -> (tempfile::TempDir, JobDir) {
    let (dir, store, _lock) = fresh_store();
    let created = JobDir::create(
        store.layout(),
        job.clone(), // BORROW: the directory owns a copy of the id
        &job_record(job, JobState::Running),
    )
    .expect("a job directory");
    (dir, created)
}

/// The labels docker reports for an agent container of `job` and the run `key`.
fn agent_labels(job: &JobId, key: &RunKey) -> Labels {
    labels(job, "agent", Some(key)).with("candler.experiment", &digest(9).to_string())
}

/// What docker answers for the agent `id` carrying `labels`.
fn agent_inspect(id: &ContainerId, labels: Labels) -> Inspect {
    let mut answer = inspect(id, labels);
    answer.image = ImageId(IMAGE_ID.to_owned()); // BORROW: .to_owned() gives the answer its own text
    answer.created = CREATED.parse::<Timestamp>().expect("a time");
    answer.state = InspectState::new(
        "running".to_owned(), // BORROW: .to_owned() gives the state its own text
        true,
        0,
        false,
        None,
        None,
    );
    answer
}

/// The plan of a run of `job`, with its cidfile under `dir`.
fn plan_in(dir: &Path, job: &JobId) -> ContainerPlan {
    ContainerPlan {
        image: ImageRef {
            id: ImageId(IMAGE_ID.to_owned()), // BORROW: .to_owned() gives the image its own text
            harness_version: None,
        },
        labels: Labels::new()
            .with("candler.job", job.as_str()) // BORROW: explicit .as_str()
            .with("candler.role", "agent"),
        mounts: Vec::new(),
        env: Vec::new(),
        gateway: Some(support::job_orchestrate::quiet_gateway()),
        state: Vec::new(),
        network: NetworkName::for_job(job),
        egress: None,
        limits: Limits {
            pids_limit: 64,
            memory: String::from("1g"),
            cpus: 1.0,
        },
        cidfile: dir.join("agent.cid"),
        user: UserSpec::Engine,
        workdir: String::from("/work"),
        home: String::from("/home/agent"),
        expiry: Duration::from_secs(10),
        project_root: dir.to_path_buf(),
        store_root: dir.to_path_buf(),
    }
}

/// An isolation over the recording `docker` that records through `dir`.
fn isolation_over(docker: RecordingDocker, dir: &JobDir, job: &JobId) -> DockerIsolation {
    let read: Arc<dyn DockerDriver> = Arc::new(docker); // TRAIT_OBJECT: the isolation reads through the driver trait, a recording fake here
    let guard = ContainerGuard::new(
        Arc::new(RecordingDestroyer::new()),
        Arc::clone(&read),
        Arc::new(dir.clone()), // BORROW: the guard's view owns its directory handle
    );
    let gateway = GatewayHandle::new(
        container_ref(&inspect(&cid(0xfff), labels(job, "gateway", None))),
        "http://127.0.0.1:1".parse().expect("a url"),
        NetworkName::for_job(job),
        NetworkName::egress_for_job(job),
        String::from("dummy"),
    )
    .expect("a handle");
    let recorder: Arc<dyn ChildRecorder> = Arc::new(dir.clone()); // BORROW: the recorder owns its directory handle; TRAIT_OBJECT: the isolation takes the recorder as a trait object
    DockerIsolation::new(gateway, read, Arc::new(guard)).with_recorder(recorder)
}

/// The files of `children/` of `dir`, by name.
fn children_of(dir: &JobDir) -> BTreeSet<String> {
    children_of_dir(&dir.children_dir())
}

/// Guards: R01 §Process supervision — the agent's container is recorded in the job's `children/`
/// as a `container-record/1` from its own inspect: its role, creation time, image and labels.
/// Oracle: the scripted inspect's values as literals, and the `container-record/1` schema file's
/// member names and version marker.
/// Bite: set the role of the built record to `ContainerRole::Gateway` in
/// `DockerIsolation::agent_record`.
#[tokio::test]
async fn record_container_writes_the_agents_record() {
    let current = job(1);
    let key = run_key(3);
    let id = cid(0xabc);
    let (_keep, dir) = job_dir(&current);
    let work = tempfile::tempdir().expect("a temp dir");
    let plan = plan_in(work.path(), &current);
    fs::write(&plan.cidfile, format!("{}\n", id.as_str())).expect("a cidfile"); // BORROW: explicit .as_str()
    let docker = RecordingDocker::new().with_inspect(
        id.as_str(), // BORROW: explicit .as_str()
        agent_inspect(&id, agent_labels(&current, &key)),
    );
    let isolation = isolation_over(docker, &dir, &current);

    let recorded = isolation
        .record_container(&plan, &key)
        .await
        .expect("a record");
    assert_eq!(recorded, Some(id.clone())); // BORROW: .clone() keeps the id for the path

    let path = dir.children_dir().join(record_name(&id));
    let written: Value =
        serde_json::from_slice(&fs::read(&path).expect("the record file")).expect("record JSON");
    let schema: Value = serde_json::from_slice(
        &fs::read(here().join("../candler-schemas/schema/container-record.schema.json"))
            .expect("the schema file"),
    )
    .expect("schema JSON");
    let members = |value: &Value| -> BTreeSet<String> {
        value
            .as_object()
            .expect("an object")
            .keys()
            .cloned() // BORROW: the set owns its names
            .collect()
    };
    assert_eq!(members(&written), members(&schema["properties"]));
    assert_eq!(written["schema"], schema["properties"]["schema"]["const"]);
    assert_eq!(written["role"], "agent");
    assert_eq!(written["cid"], id.as_str()); // BORROW: explicit .as_str()
    assert_eq!(written["created_at"], CREATED);
    assert_eq!(written["image_id"], IMAGE_ID);
    assert_eq!(written["labels"]["job"], current.as_str()); // BORROW: explicit .as_str()
    assert_eq!(written["labels"]["run"], key.to_string());
    assert_eq!(written["labels"]["experiment"], digest(9).to_string());
    assert_eq!(children_of(&dir).len(), 1);
}

/// Guards: R01 §Process supervision — recording is idempotent: a second call leaves the record
/// file byte-identical and returns the same id.
/// Oracle: the file's bytes before and after the second call; the spawn time is read from the
/// clock at each build, so a second write would differ.
/// Bite: drop the `recorded` set check at the top of `DockerIsolation::record_container`.
#[tokio::test]
async fn record_container_twice_leaves_one_identical_file() {
    let current = job(1);
    let key = run_key(3);
    let id = cid(0xabc);
    let (_keep, dir) = job_dir(&current);
    let work = tempfile::tempdir().expect("a temp dir");
    let plan = plan_in(work.path(), &current);
    fs::write(&plan.cidfile, id.as_str()).expect("a cidfile"); // BORROW: explicit .as_str()
    let docker = RecordingDocker::new().with_inspect(
        id.as_str(), // BORROW: explicit .as_str()
        agent_inspect(&id, agent_labels(&current, &key)),
    );
    let isolation = isolation_over(docker, &dir, &current);
    let path = dir.children_dir().join(record_name(&id));

    let first = isolation.record_container(&plan, &key).await;
    let before = fs::read(&path).expect("the record file");
    std::thread::sleep(Duration::from_millis(20));
    let second = isolation.record_container(&plan, &key).await;
    let after = fs::read(&path).expect("the record file");

    assert_eq!(first, Ok(Some(id.clone()))); // BORROW: .clone() compares with the id
    assert_eq!(second, first);
    assert_eq!(after, before, "the second call rewrote the record");
    assert_eq!(children_of(&dir).len(), 1);
}

/// Guards: R01 §Process supervision — a cidfile that is absent, or that docker cannot inspect
/// yet, records nothing and answers `Ok(None)`; a container of another job or run is refused.
/// Oracle: the children directory's listing, and the error variant.
/// Bite: return `Ok(Some(cid))` before the inspect in `DockerIsolation::record_container`.
#[tokio::test]
async fn record_container_writes_nothing_without_a_container() {
    let current = job(1);
    let key = run_key(3);
    let id = cid(0xabc);
    let (_keep, dir) = job_dir(&current);
    let work = tempfile::tempdir().expect("a temp dir");
    let plan = plan_in(work.path(), &current);

    // No cidfile: the client has not written it.
    let isolation = isolation_over(RecordingDocker::new(), &dir, &current);
    assert_eq!(isolation.record_container(&plan, &key).await, Ok(None));
    assert!(children_of(&dir).is_empty());

    // A cidfile whose container docker cannot inspect.
    fs::write(&plan.cidfile, id.as_str()).expect("a cidfile"); // BORROW: explicit .as_str()
    assert_eq!(isolation.record_container(&plan, &key).await, Ok(None));
    assert!(children_of(&dir).is_empty());

    // A container of another job, and of another run, is refused and recorded nowhere.
    for (case, labels) in [
        ("another job", agent_labels(&job(2), &key)),
        ("another run", agent_labels(&current, &run_key(4))),
    ] {
        let docker = RecordingDocker::new().with_inspect(
            id.as_str(), // BORROW: explicit .as_str()
            agent_inspect(&id, labels),
        );
        let refusing = isolation_over(docker, &dir, &current);
        let outcome = refusing.record_container(&plan, &key).await;
        assert!(
            matches!(outcome, Err(IsolationError::Plan { .. })),
            "{case}: {outcome:?}"
        );
        assert!(children_of(&dir).is_empty(), "{case}");
    }
}

/// The facts of the one-cell experiment of the job guards: no secret is declared.
fn facts() -> Facts {
    Facts {
        secrets: Vec::new(),
        ..Facts::base()
    }
}

/// The key of the run of `m1` that a job over the one-cell experiment plans.
fn planned_key(facts: &Facts) -> RunKey {
    manifest(&Run {
        model: "m1",
        replicate: 1,
        attempt: 1,
        facts,
        integrity: json!({"kind": "valid"}),
        sealed: true,
        job: None,
    })
    .run()
    .clone() // BORROW: the key outlives the manifest
}

/// Scripts the fake client's answers for the agent `id` of the run `key`: `container inspect`
/// answers its inspect, `container ls` lists it.
fn script_agent(rig: &Rig, id: &ContainerId, key: &RunKey) {
    let labels = json!({
        "candler.job": rig.job.to_string(),
        "candler.run": key.to_string(),
        "candler.role": "agent",
        "candler.experiment": digest(9).to_string(),
    });
    let answer = json!({
        "Id": id.as_str(),
        "Created": CREATED,
        "Image": IMAGE_ID,
        "Config": {"Labels": labels},
        "State": {
            "Status": "running",
            "Running": true,
            "ExitCode": 0,
            "OOMKilled": false,
            "StartedAt": null,
            "FinishedAt": null,
        },
    });
    let canned = rig.canned();
    fs::write(canned.join("container-inspect.out"), answer.to_string()).expect("an inspect");
    fs::write(canned.join("container-ls.out"), id.as_str()).expect("a listing");
}

/// The `children/` directory of the job of `rig`.
fn children_dir(rig: &Rig) -> PathBuf {
    JobDir::open(rig.store.layout(), &rig.job)
        .expect("the job directory")
        .children_dir()
}

/// The name of the record file of the container `id`.
fn record_name(id: &ContainerId) -> String {
    format!("container-{}.json", &id.as_str()[..12]) // INDEX: the id is 64 hex digits
}

/// Runs the job of `rig` over the one-cell experiment on `host`, to its end, reporting to
/// `sink`; `during` runs on another thread with the job's control while the job runs.
fn run_job(
    rig: &Rig,
    host: Arc<FakeHost>,
    sink: Arc<Recorder>,
    during: impl FnOnce(&JobControl) + Send,
) -> JobOutcome {
    let job = spec(&["m1"], &facts(), 3600, 1);
    let (cancel, _keep) = watch::channel::<Option<JobCause>>(None);
    let ctl = JobControl {
        cancel: Arc::new(cancel),
    };
    // TRAIT_OBJECT: the job takes its sink as a trait object
    let events: Arc<dyn EventSink> = sink;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let body = orchestrate_with(rig.runtime_with_sink(events), job, ctl.clone(), host); // BORROW: the helper thread holds the control too
    std::thread::scope(|scope| {
        scope.spawn(|| during(&ctl));
        runtime.block_on(body)
    })
}

/// How many times the fake client was asked to `container rm`.
fn rm_calls(rig: &Rig) -> usize {
    fs::read_to_string(&rig.log)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Vec<String>>(line).ok())
        .filter(|argv| argv.starts_with(&[String::from("container"), String::from("rm")]))
        .count()
}

/// Guards: R01 §Process supervision — an agent whose run-scoped `rm` failed is left to the
/// job-close `CurrentJob` sweep, which finds it recorded and destroys it; without the record the
/// sweep refuses it as unrecorded.
/// Oracle: the one-shot refusal the script consumed, the fake client's log of `container rm`
/// calls, and the `container.destroyed` event's `found`.
/// Bite: remove the poll in `drive` and the try after the wait, so no record is written.
#[test]
fn job_close_destroys_an_agent_whose_run_rm_failed_as_recorded() {
    let facts = facts();
    let key = planned_key(&facts);
    let id = cid(0xabc);
    let rig = Rig::new(0);
    script_agent(&rig, &id, &key);
    fs::write(rig.canned().join("container-rm.once"), "").expect("a one-shot refusal");
    let host = rig.host_with_agent(FakeAgent {
        cid: id.clone(), // BORROW: the agent owns its id
        sleep_s: 1,
        sweeper: None,
        stdout: String::new(),
    });
    let sink = Arc::new(Recorder::default());

    let outcome = run_job(&rig, host, Arc::clone(&sink), |_ctl| {}); // BORROW: the test reads the sink after the job
    assert_eq!(outcome.runs.len(), 1, "the run ended");

    let events = sink.events();
    let destroyed: Vec<_> = events
        .iter()
        .filter_map(|(run, payload)| {
            if let EventPayload::ContainerDestroyed {
                cid, role, found, ..
            } = payload
            {
                Some((run.clone(), cid.clone(), *role, *found)) // BORROW: the list owns the event's fields
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        destroyed,
        vec![(
            None,
            id.clone(),
            ContainerRole::Agent,
            ContainerFound::Recorded
        )], // BORROW: .clone() compares with the id
        "the job-close sweep destroyed the agent, found by its record"
    );
    assert!(
        !rig.canned().join("container-rm.once").exists(),
        "the run-scoped rm was refused"
    );
    assert_eq!(rm_calls(&rig), 1, "only the sweep's rm reached the client");
    let created: Vec<_> = events
        .iter()
        .filter(|(_, payload)| {
            matches!(
                payload,
                EventPayload::ContainerCreated {
                    role: ContainerRole::Agent,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(
        created.len(),
        1,
        "container.created of the agent: {events:?}"
    );
    assert!(children_of_dir(&children_dir(&rig)).contains(&record_name(&id)));
}

/// The `*.json` files of the directory `dir`, by name.
fn children_of_dir(dir: &Path) -> BTreeSet<String> {
    fs::read_dir(dir)
        .expect("the children directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| Path::new(name).extension().is_some_and(|ext| ext == "json"))
        .collect()
}

/// A sweeper that notes, at each sweep, whether the agent's record file exists, and destroys
/// nothing.
struct CheckingSweeper {
    record: PathBuf,
    seen: Mutex<Vec<bool>>,
}

impl ContainerSweeper for CheckingSweeper {
    fn sweep<'a>(
        &'a self,
        _scope: &'a SweepScope,
    ) -> BoxFuture<'a, Result<ContainerSweep, SweepError>> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(self.record.is_file());
        Box::pin(async { Ok(ContainerSweep::default()) })
    }
}

/// Guards: R01 §Process supervision — the agent's record appears in `children/` while its run is
/// live, and is there when the stop hook's destroy step sweeps a run ended by its stop handle.
/// Oracle: the record file's presence, awaited before the stop and read from inside a recording
/// sweeper's `sweep` call.
/// Bite: remove the poll in `drive`, keeping the try after the wait.
#[test]
fn stopped_run_has_its_agent_recorded_before_the_hook_sweeps() {
    let facts = facts();
    let key = planned_key(&facts);
    let id = cid(0xabc);
    let rig = Rig::new(0);
    script_agent(&rig, &id, &key);
    let children = children_dir(&rig);
    let sweeper = Arc::new(CheckingSweeper {
        record: children.join(record_name(&id)),
        seen: Mutex::new(Vec::new()),
    });
    let hook_sweeper: Arc<dyn ContainerSweeper> = sweeper.clone(); // BORROW: the test reads the sweeper after the job; TRAIT_OBJECT: the hook sweeps through a trait object
    let host = rig.host_with_agent(FakeAgent {
        cid: id.clone(), // BORROW: the agent owns its id
        sleep_s: 30,
        sweeper: Some(hook_sweeper),
        stdout: String::new(),
    });
    let sink = Arc::new(Recorder::default());

    let outcome = run_job(&rig, host, sink, |ctl| {
        // The stop is sent only once the record exists, so the run is live when it is recorded.
        let record = children.join(record_name(&id));
        let began = Instant::now();
        while !record.is_file() {
            if began.elapsed() >= Duration::from_secs(10) {
                // Stop the run first, so the job ends and the failure is reported.
                let _ = ctl.cancel.send(Some(JobCause::Killed));
                panic!("the record did not appear while the run was live");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = ctl.cancel.send(Some(JobCause::Killed));
    });
    assert_eq!(outcome.runs.len(), 1, "the run ended");

    let seen = sweeper
        .seen
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone(); // BORROW: .clone() copies the list out of the lock
    assert!(!seen.is_empty(), "the stop hook swept");
    assert!(
        seen.iter().all(|&exists| exists),
        "the record was missing at a sweep: {seen:?}"
    );
}

/// The stream the fake agent prints: an init that lists two tools and one MCP server, a user line
/// and a result that reports two turns, a cost and a usage.
fn agent_stream() -> String {
    let session = "33333333-3333-4333-8333-333333333333";
    [
        json!({"type": "system", "subtype": "init", "model": "m", "claude_code_version": "2.1.230",
               "tools": ["Bash", "Agent"], "mcp_servers": [{"name": "s", "status": "connected"}],
               "session_id": session}),
        json!({"type": "user", "parent_tool_use_id": null,
               "message": {"role": "user", "content": [{"type": "text", "text": "go"}]},
               "session_id": session, "timestamp": "2026-01-02T03:04:01.000Z"}),
        json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 2,
               "duration_ms": 10, "total_cost_usd": 0.25,
               "usage": {"input_tokens": 7, "output_tokens": 3,
                         "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0},
               "session_id": session}),
    ]
    .iter()
    .fold(String::new(), |mut text, line| {
        text.push_str(&line.to_string());
        text.push('\n');
        text
    })
}

/// Guards: R01 §Run manifest (`run.ended_by`, `run.usage`, `run.cost_usd`, `run.num_turns`,
/// `census.*`) — a run records the end the driver classifies from what its live reader observed
/// (not the exit-only classification), the inventory the reader reported, and the totals the
/// reader took from the harness's closing report.
/// Oracle: the stream the fake agent prints, written here by hand, and a driver whose observed end
/// (`agent_error`) differs from its exit classification (`completed`).
/// Bite: `drive` calling `classify_exit` in place of `classify_end`, the census call given `None`
/// in place of the observed inventory, or the totals taken from an empty view.
#[test]
fn harness_census_and_end_come_from_the_drivers_live_reader() {
    let facts = facts();
    let key = planned_key(&facts);
    let id = cid(0xabc);
    let rig = Rig::new(0);
    script_agent(&rig, &id, &key);
    let host = rig.host_with_agent(FakeAgent {
        cid: id.clone(), // BORROW: the agent owns its id
        sleep_s: 0,
        sweeper: None,
        stdout: agent_stream(),
    });
    let job = spec(&["m1"], &facts, 3600, 1);
    let (cancel, _keep) = watch::channel::<Option<JobCause>>(None);
    let ctl = JobControl {
        cancel: Arc::new(cancel),
    };
    // TRAIT_OBJECT: the job takes its sink as a trait object
    let events: Arc<dyn EventSink> = Arc::new(Recorder::default());
    let mut runtime = rig.runtime_with_sink(events);
    runtime.driver = Arc::new(support::job_orchestrate::ObservingDriver(
        support::job_orchestrate::QuietDriver,
    ));
    let outcome = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime")
        .block_on(orchestrate_with(runtime, job, ctl, host));
    assert_eq!(outcome.runs.len(), 1, "the run ended");

    let run = rig.store.layout().root().join("runs").join(key.to_string());
    assert!(run.join("run.done").is_file(), "the run is sealed");
    let manifest: Value =
        serde_json::from_slice(&fs::read(run.join("manifest.json")).expect("a manifest"))
            .expect("manifest JSON");
    let measured = |path: &str| -> Option<Value> {
        manifest["measured"]
            .as_array()
            .expect("a measured list")
            .iter()
            .find(|entry| entry["path"] == path)
            .map(|entry| entry["value"].clone()) // BORROW: the answer owns its value
    };
    assert_eq!(
        measured("run.ended_by"),
        Some(json!({"kind": "agent_error"})),
        "the end is the driver's observed end"
    );
    assert_eq!(measured("census.tools"), Some(json!(["Bash", "Agent"])));
    assert_eq!(
        measured("census.init_tools"),
        Some(json!(["Agent", "Bash"]))
    );
    assert_eq!(
        measured("census.mcp_servers"),
        Some(json!([{"name": "s", "status": "connected"}]))
    );
    assert_eq!(measured("run.num_turns"), Some(json!(2)));
    assert_eq!(measured("run.cost_usd"), Some(json!(0.25)));
    assert_eq!(
        measured("run.usage"),
        Some(json!({"input_tokens": 7, "output_tokens": 3,
                    "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0}))
    );
}
