#![cfg(test)]
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
    ContainerPlan, ContainerSweep, GatewayHandle, ImageId, ImageRef, Inspect, InspectState, Labels,
    Limits, NetworkName, SweepScope, UserSpec,
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

const IMAGE_ID: &str = "sha256:5555555555555555555555555555555555555555555555555555555555555555";
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
    ) -> BoxFuture<'a, Result<candler_core::isolation::IsolatedRequest, IsolationError>> {
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
