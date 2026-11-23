#![cfg(test)]
//! Guards for the record of a run's agent container: the isolation creates the container and
//! writes its record through the job's recorder before the start is spawned, and every destroy
//! path then finds the container recorded.
//! The docker half is the recording fake of `test-support`; no test reaches docker.

use crate::support;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use candler_core::isolation::container::DockerIsolation;
use candler_core::isolation::container::guard::ContainerGuard;
use candler_core::isolation::container::mounts::{MountMode, MountSpec};
use candler_core::isolation::container::sweeper::LabelSweeper;
use candler_core::isolation::container::sweeper::{ContainerSweeper, SweepError};
use candler_core::isolation::docker::{DockerCli, DockerError};
use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
use candler_core::isolation::gateway::ChildRecorder;
use candler_core::isolation::gateway::Gateway;
use candler_core::isolation::model::{
    ContainerOutcome, ContainerPlan, ContainerSweep, GatewayHandle, ImageId, ImageRef, Inspect,
    InspectState, Labels, Limits, NetworkName, SweepScope, UserSpec,
};
use candler_core::isolation::testing::{RecordingDestroyer, RecordingDocker};
use candler_core::isolation::{IsolatedRequest, Isolation, IsolationError, RunOutputs};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::model::{JobCause, JobControl, JobOutcome, JobSpec};
use candler_core::jobs::orchestrate::orchestrate_with;
use candler_core::jobs::orchestrate::schedule::Clock;
use candler_core::jobs::orchestrate::session::{JobHost, OrchestrateError};
use candler_core::jobs::view::JobView;
use candler_core::runner::Runtime;
use candler_core::supervise::spawn::{OutputSpec, SpawnRequest};
use candler_core::supervise::stop::{Ended, Exit};
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::spec::{Cell, CommandSpec, Framing, Workspace};
use candler_schemas::container::{ContainerFound, ContainerRole};
use candler_schemas::execution::{EventPayload, InvalidReason, JobState};
use candler_schemas::primitives::{ContainerId, JobId, Marker, RunKey, Timestamp};
use candler_schemas::process::ExitStatus;
use candler_schemas::trace::EndedBy;
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

/// The plan of a run of `job`, whose client record directory is `dir`.
fn plan_in(dir: &Path, job: &JobId) -> ContainerPlan {
    ContainerPlan {
        image: ImageRef {
            id: ImageId(IMAGE_ID.to_owned()), // BORROW: .to_owned() gives the image its own text
            harness_version: None,
        },
        labels: Labels::new()
            .with("candler.job", job.as_str()) // BORROW: explicit .as_str()
            .with("candler.role", "agent"),
        mounts: vec![MountSpec {
            source: dir.join("home"),
            target: String::from("/home/agent"),
            mode: MountMode::ReadWrite,
        }],
        env: Vec::new(),
        gateway: Some(support::job_orchestrate::quiet_gateway()),
        state: Vec::new(),
        network: NetworkName::for_job(job),
        egress: None,
        add_hosts: Vec::new(),
        limits: Limits {
            pids_limit: 64,
            memory: String::from("1g"),
            cpus: 1.0,
        },
        record_dir: dir.to_path_buf(),
        user: UserSpec::Engine,
        workdir: String::from("/work"),
        home: String::from("/home/agent"),
        expiry: Duration::from_secs(10),
        project_root: dir.to_path_buf(),
        store_root: dir.to_path_buf(),
        interactive: false,
    }
}

/// An isolation over the recording `read` driver that records through `dir`.
fn isolation_with(
    read: Arc<dyn DockerDriver>,
    destroyer: Arc<RecordingDestroyer>,
    dir: &JobDir,
    job: &JobId,
) -> DockerIsolation {
    let guard = ContainerGuard::new(
        destroyer,
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
    DockerIsolation::new(gateway, read, Arc::new(guard))
        .with_recorder(recorder)
        .with_sweeper(Arc::new(NoSweep))
}

/// The files of `children/` of `dir`, by name.
fn children_of(dir: &JobDir) -> BTreeSet<String> {
    children_of_dir(&dir.children_dir())
}

/// A sweeper that destroys nothing.
struct NoSweep;

impl ContainerSweeper for NoSweep {
    fn sweep<'a>(
        &'a self,
        _scope: &'a SweepScope,
    ) -> BoxFuture<'a, Result<ContainerSweep, SweepError>> {
        Box::pin(async { Ok(ContainerSweep::default()) })
    }
}

/// The outputs of a request that is never spawned.
fn outputs_in(dir: &Path) -> RunOutputs {
    let framed = |stream: &str| {
        OutputSpec::framed(
            dir.to_path_buf(),
            stream.parse().expect("a stream"),
            Framing::Lines,
        )
    };
    RunOutputs::new(framed("stdout"), framed("stderr"))
}

/// The command the run's container would run.
fn agent_command() -> CommandSpec {
    CommandSpec::builder()
        .argv(vec![String::from("agent")])
        .build()
        .expect("a command")
}

/// Asks `isolation` for the run `key`'s request over the plan.
async fn request_for(
    isolation: &DockerIsolation,
    plan: &ContainerPlan,
    key: &RunKey,
    dir: &Path,
) -> Result<IsolatedRequest, IsolationError> {
    fs::create_dir_all(dir.join("home")).expect("a home");
    isolation
        .run_request(
            plan,
            agent_command(),
            "create-test".parse().expect("a marker"),
            key.clone(), // BORROW: the request owns its key
            outputs_in(dir),
        )
        .await
}

/// Guards: R01 §Process supervision — the agent's container is created, then recorded in the
/// job's `children/` as a `container-record/1` from its own inspect (role, creation time, image,
/// labels), and only then is the request that starts it returned; the client attaches to the
/// container the create named. The create is the only docker call before the inspect.
/// Oracle: the order of the recording fake's calls, the scripted inspect's values as literals,
/// and the `container-record/1` schema file's member names and version marker.
/// Bite: the record written after the request is returned (the write removed from
/// `DockerIsolation::run_request`), or the role of the built record set to
/// `ContainerRole::Gateway` in `DockerIsolation::agent_record`.
#[tokio::test]
async fn create_records_before_start() {
    let current = job(1);
    let key = run_key(3);
    let id = cid(0xabc);
    let (_keep, dir) = job_dir(&current);
    let work = tempfile::tempdir().expect("a temp dir");
    let plan = plan_in(work.path(), &current);
    let docker = Arc::new(
        RecordingDocker::new()
            .with_created(Ok(id.clone())) // BORROW: the fake owns its answer
            .with_inspect(
                id.as_str(), // BORROW: explicit .as_str()
                agent_inspect(&id, agent_labels(&current, &key)),
            ),
    );
    let read: Arc<dyn DockerDriver> = docker.clone(); // BORROW: the test reads the calls; TRAIT_OBJECT: the isolation takes the driver as a trait object
    let isolation = isolation_with(read, Arc::new(RecordingDestroyer::new()), &dir, &current);

    let isolated = request_for(&isolation, &plan, &key, work.path())
        .await
        .expect("a request");
    assert_eq!(isolated.container, Some(id.clone())); // BORROW: .clone() keeps the id for the path

    let calls = docker.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert!(calls[0].starts_with("create "), "{calls:?}");
    assert!(!calls[0].starts_with("create run "), "{calls:?}");
    assert_eq!(calls[1], format!("inspect {}", id.as_str())); // BORROW: explicit .as_str()

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

/// Guards: R01 §Process supervision — a create docker refuses starts nothing and leaves no
/// record: the call fails, no inspect follows, `children/` stays empty; a container of another
/// job or run is refused before anything is recorded or started.
/// Oracle: the fake's call list, the children directory's listing and the error variants.
/// Bite: the record written before the create's answer is read, or the job/run label check of
/// `DockerIsolation::agent_record` removed.
#[tokio::test]
async fn create_failure_spawns_nothing() {
    let current = job(1);
    let key = run_key(3);
    let id = cid(0xabc);
    let work = tempfile::tempdir().expect("a temp dir");
    let plan = plan_in(work.path(), &current);

    let (_keep, dir) = job_dir(&current);
    let docker = Arc::new(
        RecordingDocker::new().with_created(Err(DockerError::Failed {
            detail: String::from("no such image"),
        })),
    );
    let read: Arc<dyn DockerDriver> = docker.clone(); // BORROW: the test reads the calls; TRAIT_OBJECT: the isolation takes the driver as a trait object
    let isolation = isolation_with(read, Arc::new(RecordingDestroyer::new()), &dir, &current);
    let outcome = request_for(&isolation, &plan, &key, work.path()).await;
    assert!(
        matches!(outcome, Err(IsolationError::Docker { .. })),
        "{:?}",
        outcome.map(|_| ())
    );
    assert_eq!(docker.calls().len(), 1, "{:?}", docker.calls());
    assert!(children_of(&dir).is_empty());

    for (case, labels) in [
        ("another job", agent_labels(&job(2), &key)),
        ("another run", agent_labels(&current, &run_key(4))),
    ] {
        let (_keep, dir) = job_dir(&current);
        let docker = RecordingDocker::new()
            .with_created(Ok(id.clone())) // BORROW: the fake owns its answer
            .with_inspect(
                id.as_str(), // BORROW: explicit .as_str()
                agent_inspect(&id, labels),
            );
        let read: Arc<dyn DockerDriver> = Arc::new(docker); // TRAIT_OBJECT: the isolation takes the driver as a trait object
        let isolation = isolation_with(read, Arc::new(RecordingDestroyer::new()), &dir, &current);
        let outcome = request_for(&isolation, &plan, &key, work.path()).await;
        assert!(
            matches!(outcome, Err(IsolationError::Plan { .. })),
            "{case}: {:?}",
            outcome.map(|_| ())
        );
        assert!(children_of(&dir).is_empty(), "{case}");
    }
}

/// Guards: R01 §Process supervision — a supervisor that dies between the create and the start
/// leaves a recorded container, which the job's sweep finds by its record alone (docker lists no
/// container by label) and destroys.
/// Oracle: the destroy driver's calls (`stop`, `rm` of the id) after the request was made and
/// dropped unspawned.
/// Bite: the record not written by `DockerIsolation::run_request` (the container is then
/// unrecorded and the guard refuses it).
#[tokio::test]
async fn create_then_crash_is_reaped() {
    let current = job(1);
    let key = run_key(3);
    let id = cid(0xabc);
    let (_keep, dir) = job_dir(&current);
    let work = tempfile::tempdir().expect("a temp dir");
    let plan = plan_in(work.path(), &current);
    let answer = agent_inspect(&id, agent_labels(&current, &key));
    let docker = RecordingDocker::new()
        .with_created(Ok(id.clone())) // BORROW: the fake owns its answer
        .with_inspect(id.as_str(), answer); // BORROW: explicit .as_str()
    let read: Arc<dyn DockerDriver> = Arc::new(docker); // TRAIT_OBJECT: the isolation takes the driver as a trait object
    let isolation = isolation_with(
        Arc::clone(&read), // BORROW: the sweep reads through the same driver
        Arc::new(RecordingDestroyer::new()),
        &dir,
        &current,
    );
    let request = request_for(&isolation, &plan, &key, work.path()).await;
    // The supervisor dies here: the request is never spawned.
    drop(request);

    let destroyer = Arc::new(RecordingDestroyer::new());
    let kill: Arc<dyn DestroyDriver> = destroyer.clone(); // BORROW: the test reads the calls; TRAIT_OBJECT: the guard holds a trait object
    let view: Arc<dyn JobView> = Arc::new(dir.clone()); // BORROW: the view owns its directory handle; TRAIT_OBJECT: the guard reads records through a trait object
    let guard = Arc::new(ContainerGuard::new(
        kill,
        Arc::clone(&read),
        Arc::clone(&view),
    )); // BORROW: the sweeper shares the driver and view
    let sweeper = LabelSweeper::new(read, guard, view, Duration::from_secs(5));
    let swept = sweeper
        .sweep(&SweepScope::CurrentJob)
        .await
        .expect("a sweep");

    let full = id.as_str(); // BORROW: explicit .as_str()
    assert_eq!(
        destroyer.calls(),
        [format!("stop {full}"), format!("rm {full}")]
    );
    assert_eq!(swept.destroyed.len(), 1, "{swept:?}");
}

/// Guards: R01 §Process supervision — the collect after a run destroys the container the run's
/// create named, by the run's own key, through the guard: `stop` then `rm` of the recorded id; a
/// run that created nothing destroys nothing and reports an unknown container.
/// Oracle: the recording destroy driver's call list.
/// Bite: `DockerIsolation::collect_and_destroy` reading the id from anything but the run's own
/// created container (or the scope taken from the container's own run label).
#[tokio::test]
async fn create_collect_destroys_the_runs_own_container() {
    let current = job(1);
    let key = run_key(3);
    let id = cid(0xabc);
    let (_keep, dir) = job_dir(&current);
    let work = tempfile::tempdir().expect("a temp dir");
    let plan = plan_in(work.path(), &current);
    let read: Arc<dyn DockerDriver> = Arc::new(
        RecordingDocker::new()
            .with_created(Ok(id.clone())) // BORROW: the fake owns its answer
            .with_inspect(
                id.as_str(), // BORROW: explicit .as_str()
                agent_inspect(&id, agent_labels(&current, &key)),
            ),
    ); // TRAIT_OBJECT: the isolation takes the driver as a trait object
    let destroyer = Arc::new(RecordingDestroyer::new());
    let isolation = isolation_with(read, Arc::clone(&destroyer), &dir, &current); // BORROW: the test reads the calls
    let exit = Exit::new(
        ExitStatus::Code { code: 0 },
        Ended::Exited,
        Vec::new(),
        true,
        Vec::new(),
        Vec::new(),
    );
    let workspace = Workspace::new(work.path());

    let nothing = isolation
        .collect_and_destroy(&plan, &workspace, &exit, &key)
        .await
        .expect("an outcome");
    assert!(nothing.sweep.unknown && nothing.sweep.destroyed.is_empty());
    assert_eq!(destroyer.calls(), Vec::<String>::new());

    request_for(&isolation, &plan, &key, work.path())
        .await
        .expect("a request");
    let outcome = isolation
        .collect_and_destroy(&plan, &workspace, &exit, &key)
        .await
        .expect("an outcome");
    let full = id.as_str(); // BORROW: explicit .as_str()
    assert_eq!(
        destroyer.calls(),
        [format!("stop {full}"), format!("rm {full}")]
    );
    assert_eq!(outcome.sweep.destroyed.len(), 1);
}

/// The facts of the one-cell experiment of the job guards: no secret is declared.
fn facts() -> Facts {
    Facts {
        secrets: Vec::new(),
        commit: support::job_orchestrate::skill_repo().1.clone(), // BORROW: the facts own the commit text
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
    host: Arc<dyn JobHost>, // TRAIT_OBJECT: a test host is any host over the fake client
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
/// Bite: remove the record write from `DockerIsolation::run_request`, so no record is written.
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
/// Bite: move the record write after the request is returned, so no record exists while the run is live.
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

/// Guards: R01 §Process supervision — a stop that was requested classifies the run as a stop
/// (`budget_stop` for a cap, `killed` for a cancel) even when the attached client returns by
/// itself with the container's exit code before the supervisor's ladder ends it.
/// Oracle: the end the job's own stop causes name (`JobCause::BudgetStop` is `budget_stop`,
/// `JobCause::Killed` is `killed`), and a fake client that exits 0 by itself two seconds after the
/// cancel, inside the five seconds the harness gives the client to leave: the rows are written
/// from the docs of the two causes, not from `classify_end`.
/// Bite: count only `Ended::Stopped` as a stop in `drive` (the stop given to `classify_end` is
/// dropped when the client exited by itself), so the run is not a stop.
#[test]
fn stop_that_races_the_clients_own_exit_is_still_a_stop() {
    let rows = [
        (
            JobCause::BudgetStop,
            InvalidReason::BudgetStop,
            EndedBy::BudgetStop,
        ),
        (JobCause::Killed, InvalidReason::Killed, EndedBy::Killed),
    ];
    for (cause, reason, ended_by) in rows {
        let facts = facts();
        let key = planned_key(&facts);
        let id = cid(0xabc);
        let rig = Rig::new(0);
        script_agent(&rig, &id, &key);
        let children = children_dir(&rig);
        let host = rig.host_with_agent(FakeAgent {
            cid: id.clone(), // BORROW: the agent owns its id
            sleep_s: 2,
            sweeper: None,
            stdout: String::new(),
        });
        let sink = Arc::new(Recorder::default());
        let outcome = run_job(&rig, host, sink, |ctl| {
            let record = children.join(record_name(&id));
            let began = Instant::now();
            while !record.is_file() && began.elapsed() < Duration::from_secs(10) {
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = ctl.cancel.send(Some(cause));
        });
        assert_eq!(outcome.runs.len(), 1, "{cause:?}: the run ended");
        let run = &outcome.runs[0]; // INDEX: the length was asserted
        assert_eq!(run.invalid, Some(reason), "{cause:?}: the reason");
        assert_eq!(run.ended_by, ended_by, "{cause:?}: the end");
    }
}

/// A job host over the fake client whose isolation creates and records the agent container, then
/// hands the run a client program that does not exist, so the spawn fails.
struct UnstartableHost(Arc<FakeHost>);

/// The isolation of [`UnstartableHost`].
struct Unstartable {
    inner: Arc<dyn Isolation>, // TRAIT_OBJECT: the host's own isolation is a trait object
}

impl Isolation for Unstartable {
    fn plan(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<ContainerPlan, IsolationError> {
        self.inner.plan(cell, workspace)
    }

    fn run_request<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        command: CommandSpec,
        marker: Marker,
        key: RunKey,
        outputs: RunOutputs,
    ) -> BoxFuture<'a, Result<IsolatedRequest, IsolationError>> {
        Box::pin(async move {
            let made = self
                .inner
                .run_request(plan, command, marker.clone(), key.clone(), outputs) // BORROW: the replacement carries them too
                .await?;
            let framed = |stream: &str| {
                OutputSpec::framed(
                    plan.record_dir.clone(), // BORROW: the spec owns its directory
                    stream.parse().expect("a stream name"),
                    Framing::Lines,
                )
            };
            let request = SpawnRequest::builder()
                .argv(vec![String::from("/nonexistent/candler-fixb/docker")])
                .marker(marker)
                .run(key)
                .stdout(framed("stdout"))
                .stderr(framed("stderr"))
                .record_dir(plan.record_dir.clone()) // BORROW: the request owns its directory
                .build()
                .expect("a request");
            let mut broken = IsolatedRequest::new(request, made.end_facts);
            if let Some(cid) = made.container {
                broken = broken.with_container(cid);
            }
            Ok(broken)
        })
    }

    fn collect_exec<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        command: &'a CommandSpec,
    ) -> BoxFuture<'a, Result<Vec<u8>, IsolationError>> {
        self.inner.collect_exec(plan, command)
    }

    fn collect_and_destroy<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        workspace: &'a Workspace<'a>,
        exit: &'a Exit,
        key: &'a RunKey,
    ) -> BoxFuture<'a, Result<ContainerOutcome, IsolationError>> {
        self.inner.collect_and_destroy(plan, workspace, exit, key)
    }
}

impl JobHost for UnstartableHost {
    fn docker_cli(
        &self,
        supervisor: Arc<Supervisor>,
        record_dir: PathBuf,
    ) -> Result<DockerCli, DockerError> {
        self.0.docker_cli(supervisor, record_dir)
    }

    fn gateway<'a>(
        &'a self,
        rt: &'a Runtime,
        spec: &'a JobSpec,
        // TRAIT_OBJECT: the gateway is a fake, as the session takes it from the host
    ) -> BoxFuture<'a, Result<Arc<dyn Gateway>, OrchestrateError>> {
        self.0.gateway(rt, spec)
    }

    // TRAIT_OBJECT: the host hands the runs an isolation as a trait object
    fn isolation(&self, isolation: DockerIsolation) -> Arc<dyn Isolation> {
        Arc::new(Unstartable {
            inner: self.0.isolation(isolation),
        })
    }

    fn clock(&self) -> Clock {
        self.0.clock()
    }
}

/// Guards: R01 §Process supervision — an agent container that was created and recorded is
/// destroyed by its own run when the docker client cannot be started, and not left to the
/// job-close sweep alone.
/// Oracle: the fake client's log of `container rm` calls, counted by hand: the run's own destroy
/// and then the job-close sweep's, which the fake's static listing and record still offer (two),
/// against the sweep's alone (one).
/// Bite: remove the destroy on the spawn-failure path of `drive`, so only the job-close sweep
/// reaches the client (one `rm`).
#[test]
fn agent_container_is_destroyed_when_its_client_cannot_start() {
    let facts = facts();
    let key = planned_key(&facts);
    let id = cid(0xabc);
    let rig = Rig::new(0);
    script_agent(&rig, &id, &key);
    let host = rig.host_with_agent(FakeAgent {
        cid: id.clone(), // BORROW: the agent owns its id
        sleep_s: 0,
        sweeper: None,
        stdout: String::new(),
    });
    let sink = Arc::new(Recorder::default());
    let outcome = run_job(
        &rig,
        Arc::new(UnstartableHost(host)),
        Arc::clone(&sink),
        |_ctl| {},
    ); // BORROW: the test reads the sink after the job
    let _ = outcome;

    assert_eq!(
        rm_calls(&rig),
        2,
        "the run's own destroy and the job-close sweep's both reached the client"
    );
}

/// Guards: the `env` table of a cell reaches the plan of the harness process with its name and
/// value, once.
/// Oracle: the name and value are written by hand.
/// Bite: the table passed to the gateway instead of the plan's environment, or dropped.
#[test]
fn env_reaches_the_harness_process() {
    use candler_core::isolation::container::DockerDirs;
    use candler_schemas::primitives::FieldPath;
    use candler_schemas::values::{JsonValue, ResolvedValue};

    let current = job(1);
    let (_keep, dir) = job_dir(&current);
    let work = tempfile::tempdir().expect("a temp dir");
    let root = work.path().join("run").join("workspace");
    fs::create_dir_all(&root).expect("a workspace");
    let read: Arc<dyn DockerDriver> = Arc::new(RecordingDocker::new());
    let isolation = isolation_with(read, Arc::new(RecordingDestroyer::new()), &dir, &current)
        .with_dirs(DockerDirs {
            children: dir.children_dir(),
            project_root: work.path().to_path_buf(),
            store_root: work.path().to_path_buf(),
        });
    let value = |path: &str, json: Value| {
        ResolvedValue::recorded(
            path.parse::<FieldPath>().expect("a field path"),
            JsonValue::new(json).expect("a JSON value"),
        )
    };
    let values = vec![
        value("isolation.image", json!("img:1")),
        value("budget.per_run.wall_s", json!(60)),
        value("env", json!({"TOOL_MODE": "fast"})),
    ];
    let plan = isolation
        .plan(&Cell::new(&values), &Workspace::new(&root))
        .expect("a plan");
    let named: Vec<&(String, String)> = plan
        .env
        .iter()
        .filter(|(name, _)| name == "TOOL_MODE")
        .collect();
    assert_eq!(
        named,
        vec![&(String::from("TOOL_MODE"), String::from("fast"))],
        "{:?}",
        plan.env
    );
}

mod unstarted_destroy {
    //! Guards for a container that was created and never ran: the run's creator destroys it by the id
    //! `docker create` returned, through the guard in the run's scope, and reports its own error.
    //! The docker half is the recording fake of `test-support`; no test reaches docker.

    use std::fs;
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    use candler_core::isolation::container::DockerIsolation;
    use candler_core::isolation::container::guard::ContainerGuard;
    use candler_core::isolation::container::mounts::{MountMode, MountSpec};
    use candler_core::isolation::container::sweeper::{ContainerSweeper, SweepError};
    use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
    use candler_core::isolation::gateway::ChildRecorder;
    use candler_core::isolation::model::{
        ContainerPlan, ContainerSweep, GatewayHandle, ImageId, ImageRef, Inspect, InspectState,
        Labels, Limits, NetworkName, SweepScope, UserSpec,
    };
    use candler_core::isolation::testing::{RecordingDestroyer, RecordingDocker};
    use candler_core::isolation::{Isolation, IsolationError, RunOutputs};
    use candler_core::jobs::dir::JobDir;
    use candler_core::supervise::spawn::OutputSpec;
    use candler_harness::spec::{CommandSpec, Framing};
    use candler_schemas::execution::JobState;
    use candler_schemas::primitives::{ContainerId, JobId, Timestamp};
    use futures::future::BoxFuture;

    use crate::support;
    use support::container_stop::{cid, container_ref, digest, inspect, job, labels, run_key};
    use support::job_store::job_record;
    use support::run_plan::fresh_store;

    const IMAGE_ID: &str =
        "sha256:5555555555555555555555555555555555555555555555555555555555555555";
    const CREATED: &str = "2026-03-04T05:06:07.123456789Z";

    /// A sweeper that destroys nothing.
    struct NoSweep;

    impl ContainerSweeper for NoSweep {
        fn sweep<'a>(
            &'a self,
            _scope: &'a SweepScope,
        ) -> BoxFuture<'a, Result<ContainerSweep, SweepError>> {
            Box::pin(async { Ok(ContainerSweep::default()) })
        }
    }

    fn plan_in(dir: &Path, job: &JobId) -> ContainerPlan {
        ContainerPlan {
            image: ImageRef {
                id: ImageId(IMAGE_ID.to_owned()), // BORROW: .to_owned() gives the image its own text
                harness_version: None,
            },
            labels: Labels::new()
                .with("candler.job", job.as_str()) // BORROW: explicit .as_str()
                .with("candler.role", "agent"),
            mounts: vec![MountSpec {
                source: dir.join("home"),
                target: String::from("/home/agent"),
                mode: MountMode::ReadWrite,
            }],
            env: Vec::new(),
            gateway: Some(support::job_orchestrate::quiet_gateway()),
            state: Vec::new(),
            network: NetworkName::for_job(job),
            egress: None,
            add_hosts: Vec::new(),
            limits: Limits {
                pids_limit: 64,
                memory: String::from("1g"),
                cpus: 1.0,
            },
            record_dir: dir.to_path_buf(),
            user: UserSpec::Engine,
            workdir: String::from("/work"),
            home: String::from("/home/agent"),
            expiry: Duration::from_secs(10),
            project_root: dir.to_path_buf(),
            store_root: dir.to_path_buf(),
            interactive: false,
        }
    }

    fn agent_inspect(id: &ContainerId, labels: Labels) -> Inspect {
        let mut answer = inspect(id, labels);
        answer.image = ImageId(IMAGE_ID.to_owned()); // BORROW: .to_owned() gives the answer its own text
        answer.created = CREATED.parse::<Timestamp>().expect("a time");
        answer.state = InspectState::new("running".to_owned(), true, 0, false, None, None); // BORROW: .to_owned() gives the state its own text
        answer
    }

    fn outputs_in(dir: &Path) -> RunOutputs {
        let framed = |stream: &str| {
            OutputSpec::framed(
                dir.to_path_buf(),
                stream.parse().expect("a stream"),
                Framing::Lines,
            )
        };
        RunOutputs::new(framed("stdout"), framed("stderr"))
    }

    /// Guards: R03 B2 — a container created for a run whose record cannot be written, or whose start
    /// request cannot be built, is destroyed by the id `docker create` returned, through the guard in
    /// the run's scope, and the run reports its own error.
    /// Oracle: the hand-derived call list of the recording fake: one `create`, one verification
    /// (`inspect`) of the returned id, then the destroy driver's `stop` and `rm` of that same id and
    /// nothing else; the job's `children/` holds no record the failing row did not write.
    /// Bite: the destroy dropped from the error arm of `DockerIsolation::run_request`; each row turns
    /// red with no `stop`/`rm` in the destroy driver's log.
    #[tokio::test]
    async fn unstarted_destroy() {
        let current = job(1);
        let key = run_key(3);
        let id = cid(0xabc);
        for case in ["record", "start_request"] {
            let (_keep, dir) = {
                let (keep, store, _lock) = fresh_store();
                let created = JobDir::create(
                    store.layout(),
                    current.clone(), // BORROW: the directory owns a copy of the id
                    &job_record(&current, JobState::Running),
                )
                .expect("a job directory");
                (keep, created)
            };
            let work = tempfile::tempdir().expect("a temp dir");
            fs::create_dir_all(work.path().join("home")).expect("a home");
            let plan = plan_in(work.path(), &current);
            let mut fake = RecordingDocker::new()
                .with_created(Ok(id.clone())) // BORROW: the fake owns its answer
                .with_inspect(
                    id.as_str(), // BORROW: explicit .as_str()
                    agent_inspect(
                        &id,
                        labels(&current, "agent", Some(&key))
                            .with("candler.experiment", &digest(9).to_string()),
                    ),
                );
            if case == "start_request" {
                fake = fake.with_attach_refused();
            } else {
                // The record cannot be written: `children/` is a file.
                let children = dir.children_dir();
                let _ = fs::remove_dir_all(&children);
                fs::write(&children, b"not a directory").expect("a file in the way");
            }
            let docker = Arc::new(fake);
            let read: Arc<dyn DockerDriver> = docker.clone(); // BORROW: the test reads the calls; TRAIT_OBJECT: the isolation takes the driver as a trait object
            let destroyer = Arc::new(RecordingDestroyer::new());
            let destroy: Arc<dyn DestroyDriver> = destroyer.clone(); // BORROW: the test reads the calls; TRAIT_OBJECT: the guard takes the driver as a trait object
            let guard = ContainerGuard::new(
                destroy,
                Arc::clone(&read),
                Arc::new(dir.clone()), // BORROW: the guard's view owns its directory handle
            );
            let gateway = GatewayHandle::new(
                container_ref(&inspect(&cid(0xfff), labels(&current, "gateway", None))),
                "http://127.0.0.1:1".parse().expect("a url"),
                NetworkName::for_job(&current),
                NetworkName::egress_for_job(&current),
                String::from("dummy"),
            )
            .expect("a handle");
            let recorder: Arc<dyn ChildRecorder> = Arc::new(dir.clone()); // BORROW: the recorder owns its directory handle; TRAIT_OBJECT: the isolation takes the recorder as a trait object
            let isolation = DockerIsolation::new(gateway, read, Arc::new(guard))
                .with_recorder(recorder)
                .with_sweeper(Arc::new(NoSweep));
            let outcome = isolation
                .run_request(
                    &plan,
                    CommandSpec::builder()
                        .argv(vec![String::from("agent")])
                        .build()
                        .expect("a command"),
                    "unstarted".parse().expect("a marker"),
                    key.clone(), // BORROW: the request owns its key
                    outputs_in(work.path()),
                )
                .await;
            let kind_ok = matches!(
                (&outcome, case),
                (Err(IsolationError::Plan { .. }), "start_request") | (Err(_), "record")
            );
            assert!(kind_ok, "{case}: {:?}", outcome.map(|_| ()));
            let full = id.as_str(); // BORROW: explicit .as_str()
            let calls = docker.calls();
            assert_eq!(
                calls.iter().filter(|c| c.starts_with("create")).count(),
                1,
                "{case}: {calls:?}"
            );
            assert_eq!(
                destroyer.calls(),
                [format!("stop {full}"), format!("rm {full}")],
                "{case}: the created container is destroyed by its id: {calls:?}"
            );
        }
    }

    /// An isolation over the session rig's stub whose plan declares no interactive client, so the
    /// child has no stdin pipe and a session cannot start on it; it records each destroy by run key.
    struct NoStdin {
        inner: support::session_rig::StubIsolation,
        destroyed: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Isolation for NoStdin {
        fn plan(
            &self,
            cell: &candler_harness::spec::Cell<'_>,
            workspace: &candler_harness::spec::Workspace<'_>,
        ) -> Result<ContainerPlan, IsolationError> {
            let mut plan = self.inner.plan(cell, workspace)?;
            plan.interactive = false;
            Ok(plan)
        }

        fn run_request<'a>(
            &'a self,
            plan: &'a ContainerPlan,
            command: CommandSpec,
            marker: candler_schemas::primitives::Marker,
            key: candler_schemas::primitives::RunKey,
            outputs: RunOutputs,
        ) -> BoxFuture<'a, Result<candler_core::isolation::IsolatedRequest, IsolationError>>
        {
            self.inner.run_request(plan, command, marker, key, outputs)
        }

        fn collect_and_destroy<'a>(
            &'a self,
            plan: &'a ContainerPlan,
            workspace: &'a candler_harness::spec::Workspace<'a>,
            exit: &'a candler_core::supervise::stop::Exit,
            key: &'a candler_schemas::primitives::RunKey,
        ) -> BoxFuture<'a, Result<candler_core::isolation::model::ContainerOutcome, IsolationError>>
        {
            self.destroyed
                .lock()
                .expect("the destroy log")
                .push(key.to_string());
            self.inner.collect_and_destroy(plan, workspace, exit, key)
        }
    }

    /// Guards: R03 B2 — a run whose container was created and whose session cannot start (the child
    /// has no stdin pipe) destroys that container once, by the run's key, and reports the session's
    /// own error.
    /// Oracle: the destroy log of the isolation the run was given: exactly one entry, the run's key.
    /// Bite: the `destroy_unstarted` call dropped from the session-start error arm of `drive`; the
    /// log is empty.
    #[tokio::test]
    async fn unstarted_destroy_session_start() {
        let rig = support::job_orchestrate::Rig::new(0);
        let dir = rig.dir.path().join("stub");
        fs::create_dir_all(&dir).expect("a directory");
        fs::write(dir.join("stub.py"), "import time\ntime.sleep(30)\n").expect("a stub");
        let script = dir.join("script.json");
        fs::write(&script, b"{}").expect("a script");
        let destroyed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let isolation = Arc::new(NoStdin {
            inner: support::session_rig::StubIsolation { dir, script },
            destroyed: Arc::clone(&destroyed),
        });
        let values = support::session_rig::cell_values(&[]);
        let (result, _manifest, run_dir, _rig) = support::session_rig::execute_controlled(
            rig,
            isolation,
            &values,
            support::session_rig::Driver::new(true),
            None,
            None,
        )
        .await;
        let err = result.map(|_| ()).expect_err("the session cannot start");
        assert!(err.to_string().contains("no stdin pipe"), "{err}");
        let key = run_dir
            .file_name()
            .expect("a run key")
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            *destroyed.lock().expect("the destroy log"),
            [key],
            "the created container is destroyed once, by the run's key"
        );
    }
}
