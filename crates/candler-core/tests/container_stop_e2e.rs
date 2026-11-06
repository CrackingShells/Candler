//! The real-Docker guards of the `container_stop` leaf.
//!
//! Each test makes its own jobs, each with its own `candler.job` label, and its own containers
//! from the testagent image. The guard is given a destroy driver wrapped in an allowlist that
//! panics on any container the test did not create, and the oracle is `docker ps -a` filtered by
//! the test's own job label. Nothing is removed but through the guard.
#![cfg(test)]

use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use candler_core::isolation::EndFacts;
use candler_core::isolation::container::guard::{ContainerGuard, LostProof};
use candler_core::isolation::container::stop::ContainerStopHook;
use candler_core::isolation::container::sweeper::LabelSweeper;
use candler_core::isolation::docker::{ClientEnv, DockerCli, DockerDestroyer, DockerError};
use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
use candler_core::isolation::model::{Labels, NetworkName, SweepScope};
use candler_core::isolation::testing::{AllowlistDriver, FakeJobView};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::lock::JobLock;
use candler_core::jobs::view::JobView;
use candler_core::supervise::hook::{StopHook, StopHookCtx};
use candler_core::supervise::spawn::SpawnRequest;
use candler_core::supervise::stop::Ended;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{JobId, Marker, RunKey};
use candler_schemas::process::ExitStatus;
use futures::FutureExt as _;
use rustix::process::Signal;

#[path = "support/container_stop.rs"]
mod stop_support;

use stop_support::{job, run_key};

/// The image the testagent scaffold builds; it is not pulled.
const IMAGE: &str = "candler-testagent:test";

/// How long a sweep may take in these tests.
const SWEEP_LIMIT: Duration = Duration::from_secs(90);

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// The real client, its allowlisted destroy half, and the directory its calls leave records in.
struct Rig {
    cli: Arc<DockerCli>,
    destroyer: Arc<AllowlistDriver<DockerDestroyer>>,
    supervisor: Arc<Supervisor>,
    records: tempfile::TempDir,
}

impl Rig {
    fn new() -> Self {
        // TRAIT_OBJECT: the supervisor takes its sink as a trait object
        let sink: Arc<dyn EventSink> = Arc::new(Silent);
        let supervisor = Arc::new(Supervisor::new(sink));
        let records = tempfile::tempdir().expect("a record directory");
        std::fs::create_dir_all(records.path().join("control")).expect("a control directory");
        std::fs::create_dir_all(records.path().join("run")).expect("a run directory");
        let cli = DockerCli::new(
            Arc::clone(&supervisor),
            ClientEnv::from_process(),
            records.path().join("control"),
        )
        .expect("a docker client");
        let destroyer = Arc::new(AllowlistDriver::new(cli.destroyer_for_tests()));
        Self {
            cli: Arc::new(cli),
            destroyer,
            supervisor,
            records,
        }
    }

    /// A guard over this rig's drivers that decides from `view`.
    fn guard(&self, view: Arc<dyn JobView>) -> Arc<ContainerGuard> {
        // TRAIT_OBJECT: the guard holds its drivers as trait objects
        let destroy: Arc<dyn DestroyDriver> = self.destroyer.clone(); // BORROW: .clone() shares the allowlist
        // TRAIT_OBJECT: the guard holds its drivers as trait objects
        let docker: Arc<dyn DockerDriver> = self.cli.clone(); // BORROW: .clone() shares the client
        Arc::new(ContainerGuard::new(destroy, docker, view))
    }

    /// A sweeper over a guard that decides from `view`.
    fn sweeper(&self, view: &Arc<dyn JobView>) -> Arc<LabelSweeper> {
        // TRAIT_OBJECT: the sweeper holds the client as a trait object
        let docker: Arc<dyn DockerDriver> = self.cli.clone(); // BORROW: .clone() shares the client
        Arc::new(LabelSweeper::new(
            docker,
            self.guard(Arc::clone(view)),
            Arc::clone(view),
            SWEEP_LIMIT,
        ))
    }
}

/// A job id that no other test uses: the process id and the clock make it.
fn unique_job(salt: u32) -> JobId {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos());
    job(nanos ^ std::process::id().rotate_left(16) ^ salt.wrapping_mul(0x9e37_79b1))
}

/// The labels the container of `job` in `role` carries, and a label that names this leaf.
fn label_args(job: &JobId, role: &str, run: Option<&RunKey>) -> Vec<String> {
    let mut args = Vec::new();
    for label in [
        format!("candler.job={job}"),
        format!("candler.role={role}"),
        "candler.test=container_stop".to_owned(), // BORROW: .to_owned() gives the argument its own text
    ] {
        args.push("--label".to_owned()); // BORROW: .to_owned() gives the argument its own text
        args.push(label);
    }
    if let Some(key) = run {
        args.push("--label".to_owned()); // BORROW: .to_owned() gives the argument its own text
        args.push(format!("candler.run={key}"));
    }
    args
}

/// Starts a detached testagent container of `job` and returns its full id.
fn start(
    rig: &Rig,
    job: &JobId,
    role: &str,
    run: Option<&RunKey>,
    network: Option<&NetworkName>,
) -> String {
    let mut command = Command::new(rig.cli.binary());
    command.args(["run", "--detach", "--stop-timeout", "1"]);
    command.args(label_args(job, role, run));
    if let Some(name) = network {
        // BORROW: explicit .as_str() instead of Deref coercion
        command.args(["--network", name.as_str()]);
    }
    command.args([IMAGE, "sleep-forever"]);
    let output = command.output().expect("docker runs");
    assert!(
        output.status.success(),
        "docker run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let id = String::from_utf8_lossy(&output.stdout).trim().to_owned(); // BORROW: .to_owned() gives the id its own text
    assert_eq!(id.len(), 64, "docker run printed a full id: {id:?}");
    rig.destroyer.allow_container(&id);
    id
}

/// The ids `docker ps -a` lists under the label `candler.job=<job>`: the oracle.
fn listed(rig: &Rig, job: &JobId) -> Vec<String> {
    let output = Command::new(rig.cli.binary())
        .args(["ps", "-a", "--no-trunc", "--quiet", "--filter"])
        .arg(format!("label=candler.job={job}"))
        .output()
        .expect("docker runs");
    assert!(output.status.success(), "docker ps failed");
    let mut ids: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned) // BORROW: .to_owned() gives each id its own text
        .collect();
    ids.sort();
    ids
}

/// The lock of the lost job `job`, taken on a directory of its own under the rig's records, and
/// the proof it gives. The lock is held as long as the first value is.
fn lost_lock(rig: &Rig, job: &JobId) -> (JobLock, LostProof) {
    let path = rig.records.path().join(job.to_string());
    std::fs::create_dir_all(&path).expect("a job directory");
    let lock = JobLock::acquire(&JobDir::new(job.clone(), path)).expect("the lock of a lost job"); // BORROW: .clone() gives the directory its own job
    let proof = LostProof::held(&lock);
    (lock, proof)
}

/// Destroys what a failed test left under `job`, through the guard, so that a failure leaks
/// nothing. It does nothing to a clean run.
async fn clean(rig: &Rig, job: &JobId, runs: &[RunKey], lost: &[JobId]) {
    let view = FakeJobView::new(job.clone()) // BORROW: .clone() gives the view its own job
        .with_lost(lost.to_vec()) // BORROW: .to_vec() gives the view its own list
        .shared();
    let sweeper = rig.sweeper(&view);
    for key in runs {
        let scope = SweepScope::Run(key.clone()); // BORROW: .clone() gives the scope its own key
        let _ = sweeper.sweep_report(&scope).await;
    }
    for gone in lost {
        let (_lock, proof) = lost_lock(rig, gone);
        let _ = sweeper.sweep_lost_report(&proof).await;
    }
}

/// What a test made: a job, the runs of its containers, and the lost jobs it made.
struct Made {
    job: JobId,
    runs: Vec<RunKey>,
    lost: Vec<JobId>,
}

/// Runs `body`, cleans up whatever happened, and then lets a panic of the body go on.
async fn guarded<F>(rig: &Rig, made: &[Made], body: F)
where
    F: std::future::Future<Output = ()>,
{
    let outcome = std::panic::AssertUnwindSafe(body).catch_unwind().await;
    for one in made {
        clean(rig, &one.job, &one.runs, &one.lost).await;
    }
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    for one in made {
        assert!(
            listed(rig, &one.job).is_empty(),
            "the job {} left a container",
            one.job
        );
    }
}

/// Waits until `docker ps -a` lists a container of `job`.
fn wait_for_container(rig: &Rig, job: &JobId) -> String {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(id) = listed(rig, job).into_iter().next() {
            rig.destroyer.allow_container(&id);
            return id;
        }
        assert!(Instant::now() < until, "no container of {job} appeared");
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Whether docker still has the network `name`.
async fn network_exists(rig: &Rig, name: &NetworkName) -> bool {
    match rig.cli.network_inspect(name).await {
        Ok(_) => true,
        Err(DockerError::NotFound { .. }) => false,
        Err(error) => panic!("network inspect failed: {error}"),
    }
}

/// Guards: B12 — the sweep is scoped by the job label.
/// Oracle: docker ps -a --filter label=candler.job=<id> before and after, inside the allowlist wrapper.
/// Bite: list by `candler.test` instead of `candler.job` in the sweeper (every container is then a candidate; the allowlist wrapper only permits this test's).
#[tokio::test]
#[ignore = "needs a docker daemon and the testagent image; run by the container-e2e group"]
async fn b12_sweep_scope() {
    let rig = Rig::new();
    let live = unique_job(1);
    let lost = unique_job(2);
    let current = unique_job(3);
    let (live_run, lost_run, current_run) = (run_key(11), run_key(12), run_key(13));
    let made = [
        Made {
            job: live.clone(),            // BORROW: .clone() gives the record its own job
            runs: vec![live_run.clone()], // BORROW: .clone() gives the record its own run
            lost: Vec::new(),
        },
        Made {
            job: current.clone(),            // BORROW: .clone() gives the record its own job
            runs: vec![current_run.clone()], // BORROW: .clone() gives the record its own run
            lost: vec![lost.clone()],        // BORROW: .clone() gives the record its own job
        },
    ];
    guarded(&rig, &made, async {
        let network = NetworkName::for_job(&lost);
        // BORROW: explicit .as_str() instead of Deref coercion
        let net_labels = Labels::new().with("candler.job", lost.as_str());
        rig.cli
            .network_create(&network, true, &net_labels)
            .await
            .expect("a network");
        rig.destroyer.allow_network(network.as_str()); // BORROW: explicit .as_str()

        let live_id = start(&rig, &live, "agent", Some(&live_run), None);
        let lost_id = start(&rig, &lost, "agent", Some(&lost_run), Some(&network));
        let current_id = start(&rig, &current, "agent", Some(&current_run), None);
        assert_eq!(listed(&rig, &live), [live_id.as_str()]); // BORROW: .as_str() compares the id
        assert_eq!(listed(&rig, &lost), [lost_id.as_str()]); // BORROW: .as_str() compares the id
        assert_eq!(listed(&rig, &current), [current_id.as_str()]); // BORROW: .as_str() compares the id
        assert!(network_exists(&rig, &network).await);

        // The view is of the current job; no record names any of the three containers.
        let view = FakeJobView::new(current.clone()) // BORROW: .clone() gives the view its own job
            .with_lost(vec![lost.clone()]) // BORROW: .clone() gives the view its own job
            .shared();
        let sweeper = rig.sweeper(&view);

        // A live sibling has no proof, and its supervisor holds its lock: its container survives.
        let sibling = sweeper
            .sweep_report(&SweepScope::Lost(live.clone())) // BORROW: .clone() gives the scope its own job
            .await
            .expect("a sweep");
        assert!(sibling.sweep.destroyed.is_empty(), "{sibling:?}");
        assert_eq!(listed(&rig, &live), [live_id.as_str()]); // BORROW: .as_str() compares the id

        // The lost job's unrecorded container and its network go.
        let (lock, proof) = lost_lock(&rig, &lost);
        let swept = sweeper.sweep_lost_report(&proof).await.expect("a sweep");
        drop(lock);
        assert_eq!(swept.sweep.destroyed.len(), 1, "{swept:?}");
        assert!(!swept.sweep.unknown, "{swept:?}");
        assert_eq!(swept.sweep.refused, Vec::new(), "{swept:?}");
        assert_eq!(listed(&rig, &lost), Vec::<String>::new());
        assert!(!network_exists(&rig, &network).await);
        assert_eq!(listed(&rig, &live), [live_id.as_str()]); // BORROW: .as_str() compares the id
        assert_eq!(listed(&rig, &current), [current_id.as_str()]); // BORROW: .as_str() compares the id

        // An unrecorded labelled container of the live current job is refused, not destroyed.
        let live_job = sweeper
            .sweep_report(&SweepScope::CurrentJob)
            .await
            .expect("a sweep");
        assert!(live_job.sweep.destroyed.is_empty(), "{live_job:?}");
        assert_eq!(live_job.sweep.refused.len(), 1, "{live_job:?}");
        assert_eq!(listed(&rig, &current), [current_id.as_str()]); // BORROW: .as_str() compares the id
        assert_eq!(listed(&rig, &live), [live_id.as_str()]); // BORROW: .as_str() compares the id
    })
    .await;
}

/// Guards: B13 — a client death never leaves the agent container.
/// Oracle: no container labelled for the job after the ladder finishes.
/// Bite: make `ContainerStopHook::destroy` return an empty sweep without sweeping.
#[tokio::test]
#[ignore = "needs a docker daemon and the testagent image; run by the container-e2e group"]
async fn b13_client_killed_container_gone() {
    let rig = Rig::new();
    let current = unique_job(4);
    let run = run_key(21);
    let made = [Made {
        job: current.clone(),    // BORROW: .clone() gives the record its own job
        runs: vec![run.clone()], // BORROW: .clone() gives the record its own run
        lost: Vec::new(),
    }];
    guarded(&rig, &made, async {
        let view = FakeJobView::new(current.clone()).shared(); // BORROW: .clone() gives the view its own job
        let guard = rig.guard(Arc::clone(&view));
        let sweeper = rig.sweeper(&view);
        let hook = Arc::new(ContainerStopHook::new(guard, sweeper, current.clone())); // BORROW: .clone() gives the hook its own job

        let mut argv = vec![
            // BORROW: .into_owned() gives the argument its own text
            rig.cli.binary().to_string_lossy().into_owned(),
            "run".to_owned(), // BORROW: .to_owned() gives the argument its own text
            "--stop-timeout".to_owned(), // BORROW: .to_owned() gives the argument its own text
            "1".to_owned(),   // BORROW: .to_owned() gives the argument its own text
        ];
        argv.extend(label_args(&current, "agent", Some(&run)));
        argv.push(IMAGE.to_owned()); // BORROW: .to_owned() gives the argument its own text
        argv.push("ignore-term".to_owned()); // BORROW: .to_owned() gives the argument its own text
        let marker = Marker::for_run(&run);
        // TRAIT_OBJECT: the request takes its hook as a trait object
        let attached: Arc<dyn StopHook> = hook.clone(); // BORROW: .clone() shares the hook
        let request = SpawnRequest::builder()
            .argv(argv)
            .env(ClientEnv::from_process().pairs().to_vec()) // BORROW: .to_vec() gives the request its own env
            .marker(marker.clone()) // BORROW: .clone() gives the request its own marker
            .run(run.clone()) // BORROW: .clone() gives the request its own run
            .stop_hook(attached)
            .record_dir(rig.records.path().join("run"))
            .build()
            .expect("a request");
        let child = rig
            .supervisor
            .spawn(request)
            .expect("the docker client starts");
        let container = wait_for_container(&rig, &current);

        // The client dies; the signal guard is the only thing that signals it.
        rig.supervisor
            .guard()
            .signal_group(child.record().pgid(), Signal::KILL)
            .expect("the guard signals the client's group");
        let exit = child.wait().await.expect("the client is reaped");
        assert_eq!(exit.ended, Ended::Exited);
        assert!(
            matches!(exit.status, ExitStatus::Signal { .. }),
            "the client did not die by a signal: {exit:?}"
        );
        // The premise: the container outlived its client.
        assert_eq!(listed(&rig, &current), [container.as_str()]); // BORROW: .as_str() compares the id

        // The ladder does not run the hook for a child that exited, so the run's caller does.
        let ctx = StopHookCtx::new(Some(run.clone()), marker, Ended::Exited); // BORROW: .clone() gives the context its own run
        hook.pre_stop(&ctx)
            .await
            .expect("the container is asked to stop");
        let swept = hook.destroy(&ctx).await.expect("the sweep runs");
        assert_eq!(swept.destroyed.len(), 1, "{swept:?}");
        assert!(!swept.unknown, "{swept:?}");
        assert_eq!(listed(&rig, &current), Vec::<String>::new());
        let facts = hook
            .final_inspect()
            .expect("the hook took the final inspect");
        let inspected = facts.inspect.expect("the final inspect");
        assert_eq!(inspected.id.as_str(), container.as_str()); // BORROW: .as_str() compares the id
    })
    .await;
}
