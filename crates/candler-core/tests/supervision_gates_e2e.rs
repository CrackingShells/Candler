//! The real-Docker guard of the supervision gates: a supervisor that is killed leaves nothing.
//!
//! Every container and network the test makes carries `candler.job=<test id>`. The test reads each
//! id from a label query, checks the container's job label through `docker inspect`, and only
//! then allows it in the [`AllowlistDriver`], which refuses any other id. Every destroy goes
//! through the [`ContainerGuard`]. The one signal the test sends is the SIGKILL to the supervisor
//! it spawned and holds a `Child` of.
#![cfg(test)]

use std::os::unix::process::ExitStatusExt as _;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use candler_core::isolation::container::guard::{ContainerGuard, LostProof};
use candler_core::isolation::container::sweeper::LabelSweeper;
use candler_core::isolation::docker::{ClientEnv, DockerCli, DockerDestroyer};
use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
use candler_core::isolation::model::{LabelSel, NetworkName};
use candler_core::isolation::testing::{AllowlistDriver, FakeJobView};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::JobEvents;
use candler_core::jobs::lock::JobLock;
use candler_core::jobs::lost::{
    JobLiveness, RecoverOutcome, RecoverReport, classify, has_started, recover,
};
use candler_core::jobs::runtime::{JobEntry, supervise_entry};
use candler_core::jobs::view::JobView;
use candler_core::store::Store;
use candler_core::supervise::platform::process_start;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::{JobEnd, JobState};
use candler_schemas::primitives::JobId;

// The aggregate binary also loads the support files its sibling modules load on their own.
#[allow(
    clippy::duplicate_mod,
    reason = "guard_container_e2e loads this module with its siblings"
)]
#[path = "support/mod.rs"]
mod support;

use support::assert_clean;
use support::gateway::{canary, client_env, random_job};
use support::job_runtime::{job_spec, services};
use support::job_store::job_record;
use support::supervision_gates::{
    ENV_JOB, ENV_SIDECAR, ENV_STORE, ENV_TOKEN, ENV_WORK, TEST_LABEL, daemon_up, gates_body,
    label_of, labelled, make_certs, marked, network_exists, require_images, run_fake, run_key,
    running, sidecar_image_id,
};

/// How long the supervisor child may take to start the gateway and the agent.
const READY_LIMIT: Duration = Duration::from_secs(120);

/// How long a recovered job's marked host processes may take to be seen gone.
const GONE_LIMIT: Duration = Duration::from_secs(15);

/// The re-exec entry of the supervisor child that B36 starts; it guards nothing itself. It does
/// nothing unless `CANDLER_TEST_GATES_JOB` names a job: run on its own it is an empty test.
#[test]
fn supervisor_child() {
    let Ok(job) = std::env::var(ENV_JOB) else {
        return;
    };
    let root = std::env::var(ENV_STORE).expect("a store");
    let store = Store::open(Path::new(&root)).expect("a store");
    let entry = JobEntry {
        store,
        job: job.parse().expect("a job id"),
        spec: job_spec(),
        services: services(),
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    // The test SIGKILLs this process while the body waits, so the call never returns.
    let outcome = runtime
        .block_on(supervise_entry(entry, gates_body))
        .expect("the job ends in order");
    assert_eq!(outcome.end, JobEnd::Killed);
}

/// The real docker client behind an allowlist, and the means to recover a lost job through the
/// guard, as `candler ps` does. Nothing is destroyed but through the guard.
struct Rig {
    job: JobId,
    dir: JobDir,
    destroyer: Arc<AllowlistDriver<DockerDestroyer>>,
    reads: Arc<dyn DockerDriver>,
    sup: Arc<Supervisor>,
    sink: Arc<dyn EventSink>,
}

impl Rig {
    fn new(dir: &JobDir) -> Self {
        let events = Arc::new(JobEvents::open(dir).expect("the job's log"));
        // TRAIT_OBJECT: the supervisor and the guard take their sink as a trait object
        let sink: Arc<dyn EventSink> = events;
        let sup = Arc::new(Supervisor::new(Arc::clone(&sink)));
        let cli = DockerCli::new(
            Arc::clone(&sup),
            ClientEnv::from_process(),
            dir.control_dir(),
        )
        .expect("a docker client");
        let destroyer = Arc::new(AllowlistDriver::new(cli.destroyer_for_tests()));
        Self {
            job: dir.job().clone(), // BORROW: the rig owns its job id
            dir: dir.clone(),       // BORROW: the rig owns its directory
            destroyer,
            // TRAIT_OBJECT: the guard and the sweeper read through the docker client
            reads: Arc::new(cli),
            sup,
            sink,
        }
    }

    /// Allows the destroy of every container that carries the job's label and of the job's two
    /// networks, once `docker inspect` agrees the container carries the label.
    fn allow_job(&self) {
        for cid in labelled(&self.job) {
            assert_eq!(
                label_of(&cid, "candler.job").as_deref(),
                Some(self.job.to_string().as_str()),
                "{cid} does not carry the job's label"
            );
            self.destroyer.allow_container(&cid);
        }
        for net in [
            NetworkName::for_job(&self.job),
            NetworkName::egress_for_job(&self.job),
        ] {
            self.destroyer.allow_network(net.as_str()); // BORROW: .as_str() names the network
        }
    }

    /// Recovers the lost job of the directory: the production composition of `candler ps`, over
    /// the allowlisted destroy half.
    async fn recover(&self) -> RecoverReport {
        // TRAIT_OBJECT: the guard and the sweeper read the job through its directory
        let view: Arc<dyn JobView> = Arc::new(self.dir.clone()); // BORROW: the view owns its directory
        // TRAIT_OBJECT: the guard holds its drivers as trait objects
        let destroy: Arc<dyn DestroyDriver> = self.destroyer.clone(); // BORROW: .clone() shares the allowlist
        let guard = Arc::new(
            ContainerGuard::new(destroy, Arc::clone(&self.reads), Arc::clone(&view))
                .with_events(Arc::clone(&self.sink)),
        );
        let sweeper = LabelSweeper::new(
            Arc::clone(&self.reads),
            guard,
            view,
            Duration::from_secs(60),
        )
        .with_events(Arc::clone(&self.sink));
        recover(&self.sup, &sweeper, &self.dir)
            .await
            .expect("the recovery runs")
    }
}

impl Rig {
    /// Destroys what the job labelled, through a guard whose view is another job's, as the guard
    /// requires of a lost job's recovery. The lock and the log it proves the job lost from are a
    /// fresh directory's of the same job id, so that a job a recovery already ended (or a bitten
    /// one wrongly ended) is still cleaned up. It is the cleanup of a failed test, not part of
    /// what the test asserts.
    async fn destroy_leftovers(&self) {
        let scratch = tempfile::tempdir().expect("a scratch directory");
        let store = Store::init(&scratch.path().join("store")).expect("a store");
        let fresh = JobDir::create(
            store.layout(),
            self.job.clone(), // BORROW: the directory owns its job id
            &job_record(&self.job, JobState::Running),
        )
        .expect("a job directory");
        let lock = match JobLock::acquire(&fresh) {
            Ok(lock) => lock,
            Err(error) => {
                eprintln!("cleanup: the job's lock is not ours: {error}");
                return;
            }
        };
        let proof = LostProof::held(&lock);
        // TRAIT_OBJECT: the guard holds its drivers as trait objects
        let destroy: Arc<dyn DestroyDriver> = self.destroyer.clone(); // BORROW: .clone() shares the allowlist
        let guard = ContainerGuard::new(
            destroy,
            Arc::clone(&self.reads),
            FakeJobView::new(random_job()).shared(),
        );
        self.allow_job();
        let sel = LabelSel {
            key: String::from("candler.job"),
            value: self.job.to_string(),
        };
        let mut listed = self.reads.ps_by_label(&sel).await.unwrap_or_default();
        listed.sort_by_key(|container| container.labels.get("candler.role") != Some("agent"));
        for container in &listed {
            if let Err(error) = guard.destroy_lost(container, &proof).await {
                eprintln!("cleanup: {} was not destroyed: {error}", container.cid);
            }
        }
        for net in [
            NetworkName::for_job(&self.job),
            NetworkName::egress_for_job(&self.job),
        ] {
            if let Err(error) = guard.destroy_network_lost(&net, &proof).await {
                eprintln!("cleanup: {} was not destroyed: {error}", net.as_str()); // BORROW: .as_str() names the network
            }
        }
    }
}

/// Ends what a failed test left: the supervisor it spawned, then whatever the job labelled, by
/// destroying it through the guard.
struct Cleanup {
    child: Option<Child>,
    rig: Rig,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            // EXPLICIT: the supervisor is the process this test spawned and holds a handle of
            let _ = child.kill();
            let _ = child.wait();
        }
        if labelled(&self.rig.job).is_empty() {
            return;
        }
        let rig = &self.rig;
        std::thread::scope(|scope| {
            let _ = scope
                .spawn(|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("a runtime");
                    runtime.block_on(rig.destroy_leftovers());
                })
                .join();
        });
    }
}

/// The text of a file the child wrote, or nothing.
fn text_of(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// Waits until `ready` exists and returns the pid it names, or panics with what the child said.
fn wait_ready(child: &mut Child, work: &Path, logs: &Path) -> u32 {
    let until = Instant::now() + READY_LIMIT;
    loop {
        if let Ok(text) = std::fs::read_to_string(work.join("ready")) {
            return text.trim().parse().expect("a pid");
        }
        let exited = child.try_wait().expect("the supervisor's status");
        assert!(
            exited.is_none() && Instant::now() < until,
            "the supervisor did not become ready (exited: {exited:?}): {}",
            text_of(&logs.join("child.err"))
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Guards: I15, I19, I20, I22 — a supervisor killed with SIGKILL while its three containers (the gateway sidecar, the fake upstream and the agent) run leaves, once the lost job is recovered, no container labelled for the job, no host process marked for it, no network of it, and the canary token nowhere in the store, the job directory, its events or the supervisor's logs.
/// Oracle: `docker ps -a --filter label=candler.job=<id>`, `docker network inspect`, the host process table, and `assert_clean` over the files. The host process check differs by OS: on Linux it scans `/proc/*/environ` for the job's `CANDLER_RUN` marker; on macOS, where no tool shows a process environment, it checks only that the agent client's pid, with its start time, is gone.
/// Bite: the lost-job path skips the container sweep.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
async fn b36_supervisor_sigkill_nothing_left() {
    assert!(daemon_up(), "docker is not running: docker desktop start");
    require_images();
    let sidecar = sidecar_image_id();

    let tmp = tempfile::tempdir().expect("a temporary directory");
    let work = tmp.path().join("work");
    let logs = tmp.path().join("logs");
    for dir in [&work, &logs] {
        std::fs::create_dir_all(dir).expect("a directory");
    }
    let certs = work.join("certs");
    make_certs(&certs);
    let root = tmp.path().join("store");
    let store = Store::init(&root).expect("a store");
    let job = random_job();
    let dir = JobDir::create(
        store.layout(),
        job.clone(), // BORROW: the directory owns its job id
        &job_record(&job, JobState::Running),
    )
    .expect("a job directory");
    let token = canary();
    let marker = run_key().to_string();
    let mut cleanup = Cleanup {
        child: None,
        rig: Rig::new(&dir),
    };

    // The child test's name, as libtest lists it: this module's path without the crate's name.
    let module = module_path!().split_once("::").map_or("", |(_, rest)| rest);
    let entry = if module.is_empty() {
        String::from("supervisor_child")
    } else {
        format!("{module}::supervisor_child")
    };
    let mut command = Command::new(std::env::current_exe().expect("this binary"));
    command
        .args(["--exact", &entry, "--nocapture"])
        .env_clear()
        .envs(client_env())
        .env(ENV_JOB, job.to_string())
        .env(ENV_STORE, &root)
        .env(ENV_WORK, &work)
        .env(ENV_SIDECAR, &sidecar)
        .env(ENV_TOKEN, &token)
        .env("XDG_STATE_HOME", tmp.path().join("state"))
        .stdin(Stdio::null())
        .stdout(std::fs::File::create(logs.join("child.out")).expect("a log"))
        .stderr(std::fs::File::create(logs.join("child.err")).expect("a log"));
    cleanup.child = Some(command.spawn().expect("the supervisor"));

    // The upstream's network does not exist until the gateway creates it: start the fake beside.
    let fake = {
        let (job, certs, token) = (job.clone(), certs.clone(), token.clone()); // BORROW: owned copies move to the thread
        std::thread::spawn(move || run_fake(&job, &certs, &token))
    };
    let child = cleanup.child.as_mut().expect("the supervisor");
    let agent_client = wait_ready(child, &work, &logs);
    let fake = fake.join().expect("the fake's thread");

    // The job is mid-run: started, its three containers up and its agent's client marked.
    assert!(has_started(&dir).expect("the log"), "no job.started");
    // The agent's client was spawned just before `ready`: its container may still be starting.
    let until = Instant::now() + Duration::from_secs(30);
    while running(&job).len() < 3 {
        assert!(
            Instant::now() < until,
            "the job's containers are not running: {:?}",
            running(&job)
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    let before = labelled(&job);
    assert!(
        before.len() >= 3 && before.contains(&fake),
        "expected the sidecar, the fake and the agent, found {before:?}"
    );
    assert!(
        before
            .iter()
            .any(|cid| label_of(cid, "candler.test").as_deref() == Some("supervision_gates")),
        "no container carries {TEST_LABEL}"
    );
    let known = vec![(agent_client, process_start(agent_client))];
    let alive = marked(&marker, &known).expect("the process listing");
    assert!(
        alive.contains(&agent_client),
        "the agent's client {agent_client} is not marked: {alive:?}"
    );
    cleanup.rig.allow_job();

    // The supervisor dies without a word: SIGKILL, to the process this test spawned.
    let child = cleanup.child.as_mut().expect("the supervisor");
    child.kill().expect("the supervisor takes a kill");
    let status = child.wait().expect("a status");
    assert_eq!(status.signal(), Some(9), "the supervisor was not killed");
    cleanup.child = None;

    // Nothing has cleaned up yet: the job is lost, its containers and process still there.
    assert_eq!(
        classify(&dir).expect("the lock"),
        JobLiveness::Lost,
        "the killed supervisor's job is not lost"
    );
    assert_eq!(
        labelled(&job),
        before,
        "the containers went with the supervisor"
    );
    assert!(
        marked(&marker, &known)
            .expect("the process listing")
            .contains(&agent_client),
        "the agent's client went with the supervisor"
    );

    let report = cleanup.rig.recover().await;
    assert_eq!(
        report.outcome,
        RecoverOutcome::Recovered,
        "recovery: {report:?}"
    );
    assert!(report.refused.is_empty() && !report.unknown, "{report:?}");

    // I19, I22: nothing is labelled for the job and its networks are gone.
    assert!(
        labelled(&job).is_empty(),
        "containers of the job remain: {:?}",
        labelled(&job)
    );
    for cid in &before {
        assert!(label_of(cid, "candler.job").is_none(), "{cid} still exists");
    }
    for net in [
        NetworkName::for_job(&job),
        NetworkName::egress_for_job(&job),
    ] {
        assert!(!network_exists(&net), "{} remains", net.as_str()); // BORROW: .as_str() names the network
    }
    // I20: no host process is marked for the job's run.
    let until = Instant::now() + GONE_LIMIT;
    while !marked(&marker, &known)
        .expect("the process listing")
        .is_empty()
    {
        assert!(
            Instant::now() < until,
            "marked processes remain: {:?}",
            marked(&marker, &known)
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        classify(&dir).expect("the lock"),
        JobLiveness::Ended(JobEnd::Failed)
    );

    // I15: the canary is in no file the job, its store or its supervisor wrote.
    assert_clean(&root, &token);
    assert_clean(&logs, &token);
}
