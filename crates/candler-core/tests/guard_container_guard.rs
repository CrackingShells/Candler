//! Guards for the container guard and its callers, against recording fakes. No test here reaches
//! docker: the read driver is scripted and the destroy driver is the recording one from
//! `test-support`, so a mutated guard destroys nothing.
#![cfg(test)]

mod support;

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use candler_core::isolation::Isolation as _;
use candler_core::isolation::container::DockerIsolation;
use candler_core::isolation::container::guard::{
    ContainerGuard, Destroyed, GuardError, GuardRefusal, LostProof,
};
use candler_core::isolation::container::sweeper::{LabelSweeper, UnreadableRecord};
use candler_core::isolation::docker::DockerError;
use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
use candler_core::isolation::model::{
    ContainerPlan, ContainerRef, GatewayHandle, ImageId, ImageRef, Limits, SweepScope, UserSpec,
};
use candler_core::isolation::model::{Labels, NetworkInfo, NetworkName};
use candler_core::isolation::testing::{FakeJobView, RecordingDestroyer, RecordingDocker};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::JobEvents;
use candler_core::jobs::lock::JobLock;
use candler_core::jobs::view::ChildEntry;
use candler_core::supervise::hook::{HookError, StopHook, StopHookCtx};
use candler_core::supervise::signal::Signaller;
use candler_core::supervise::spawn::SpawnRequest;
use candler_core::supervise::stop::{Ended, Exit};
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::spec::Workspace;
use candler_harness::spec::{StopSignal, StopSpec};
use candler_schemas::container::ContainerRole;
use candler_schemas::execution::{EventPayload, JobEnd, StopStep};
use candler_schemas::primitives::{ContainerId, JobId, RunKey};
use candler_schemas::process::ExitStatus;
use futures::future::BoxFuture;
use rustix::io::Errno;
use rustix::process::{Pid, Signal};
use support::container_stop::{
    Log, LoggingDestroyer, ScriptedDocker, cid, container_ref, inspect, job, labels, record,
    run_key,
};

/// A guard over a recording docker that answers `inspects`, a recording destroyer and `view`.
fn rig(
    view: FakeJobView,
    inspects: Vec<candler_core::isolation::model::Inspect>,
) -> (
    ContainerGuard,
    Arc<RecordingDocker>,
    Arc<RecordingDestroyer>,
) {
    let docker = inspects
        .into_iter()
        .fold(RecordingDocker::new(), |docker, one| {
            let id = one.id.as_str().to_owned(); // BORROW: .as_str() and .to_owned() key the answer
            docker.with_inspect(&id, one)
        });
    let docker = Arc::new(docker);
    let destroyer = Arc::new(RecordingDestroyer::new());
    let read: Arc<dyn DockerDriver> = docker.clone(); // BORROW: .clone() shares the fake; TRAIT_OBJECT: the guard holds a trait object
    let kill: Arc<dyn DestroyDriver> = destroyer.clone(); // BORROW: .clone() shares the fake; TRAIT_OBJECT: the guard holds a trait object
    (
        ContainerGuard::new(kill, read, view.shared()),
        docker,
        destroyer,
    )
}

/// The directory of the lost job `job` under `root`, with its lock held: the lock a recovery
/// holds, and the proof it gives.
fn locked(root: &std::path::Path, job: &JobId) -> (JobDir, JobLock, LostProof) {
    let path = root.join(job.to_string());
    std::fs::create_dir_all(&path).expect("a job directory");
    let dir = JobDir::new(job.clone(), path); // BORROW: .clone() gives the directory its own job
    let lock = JobLock::acquire(&dir).expect("the lock of a lost job");
    let proof = LostProof::held(&lock);
    (dir, lock, proof)
}

/// Appends a `job.ended` to the log of `dir`.
fn end_job(dir: &JobDir) {
    JobEvents::open(dir)
        .expect("the log")
        .append(None, EventPayload::job_ended(JobEnd::Failed))
        .expect("an event");
}

/// A destroy driver that ends the job of `dir` when asked to stop, as a supervisor that comes
/// back while its container is stopping would.
struct EndingDestroyer {
    dir: JobDir,
    inner: Arc<RecordingDestroyer>,
}

impl DestroyDriver for EndingDestroyer {
    fn stop<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        end_job(&self.dir);
        self.inner.stop(id)
    }

    fn rm<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        self.inner.rm(id)
    }

    fn network_rm<'a>(&'a self, name: &'a NetworkName) -> BoxFuture<'a, Result<(), DockerError>> {
        self.inner.network_rm(name)
    }
}

/// Asserts that the guard refused `claimed` in `scope` with `refusal` and recorded no verb.
async fn assert_refused(
    case: &str,
    view: FakeJobView,
    answer: candler_core::isolation::model::Inspect,
    claimed: &ContainerRef,
    scope: &SweepScope,
    refusal: GuardRefusal,
) {
    let (guard, _docker, destroyer) = rig(view, vec![answer]);
    let result = guard.destroy(claimed, scope).await;
    assert_eq!(
        result,
        Err(GuardError::Refused(refusal)),
        "{case}: wrong answer"
    );
    assert!(
        destroyer.calls().is_empty(),
        "{case}: the guard destroyed {:?}",
        destroyer.calls()
    );
}

/// Guards: B10 — the guard refuses by name, pattern, "all", empty, unlabelled, other job, other
/// run, label mismatch, control role, unrecorded, a lost job without the proof of its held lock,
/// a job that has ended and the networks of another job, with no stop, rm or network rm recorded.
/// `collect_and_destroy` scopes its destroy to the run its caller names: a cidfile that names a
/// container of another run of the job gives no stop and no rm, and the run's own container does.
/// Oracle: the recording destroy driver's call list is empty for each refused case, and the
/// positive control's list is exactly `stop` then `rm` of the recorded id after two inspects
/// (the verb order of the design's I22 text).
/// Bite: skip the `job == current job` comparison in `ContainerGuard::check_job`; or drop the
/// `check_job` call before `rm` in `ContainerGuard::destroy_with`; or replace the
/// `check_job` call in `ContainerGuard::destroy_network_with` by `false && ..`; or derive the
/// scope of `DockerIsolation::collect_and_destroy` from the target's own `candler.run` label.
#[tokio::test]
async fn b10_guard_refusals() {
    // A name, a pattern, "all" and the empty string are not container ids at all.
    for text in ["web", "web-*", "all", "", "9dcd"] {
        assert!(
            ContainerId::from_str(text).is_err(),
            "`{text}` was taken for a container id"
        );
    }
    let current = job(1);
    let other = job(2);
    let run = run_key(1);
    let other_run = run_key(2);
    let id = cid(0xabc);
    let view = || FakeJobView::new(current.clone()); // BORROW: .clone() gives each view its own job
    let good = |role: &str, run: Option<&candler_schemas::primitives::RunKey>| {
        inspect(&id, labels(&current, role, run))
    };

    // A twelve digit prefix is a valid id type, but the guard wants the full id and asks docker
    // nothing.
    let short = ContainerId::from_str("0123456789ab").expect("12 hex digits");
    let short_ref = ContainerRef::new(
        short,
        labels(&current, "agent", None),
        support::container_stop::created(),
    );
    let (guard, docker, destroyer) = rig(view(), Vec::new());
    assert_eq!(
        guard.destroy(&short_ref, &SweepScope::CurrentJob).await,
        Err(GuardError::Refused(GuardRefusal::NotFullId))
    );
    assert!(docker.calls().is_empty() && destroyer.calls().is_empty());

    // Unlabelled: docker says the container carries no candler label.
    let bare = inspect(&id, candler_core::isolation::model::Labels::new());
    assert_refused(
        "unlabelled",
        view(),
        bare.clone(), // BORROW: .clone() gives the helper its own answer
        &container_ref(&bare),
        &SweepScope::CurrentJob,
        GuardRefusal::Unlabelled,
    )
    .await;

    // A role that is neither agent nor gateway.
    let odd = good("agent2", None);
    assert_refused(
        "unknown role",
        view(),
        odd.clone(), // BORROW: .clone() gives the helper its own answer
        &container_ref(&odd),
        &SweepScope::CurrentJob,
        GuardRefusal::Unlabelled,
    )
    .await;

    // The control role is never destroyed, even when a record names it.
    let control = good("control", None);
    assert_refused(
        "control",
        view().with_children(vec![ChildEntry::Container(record(
            &id,
            &current,
            &run,
            ContainerRole::Agent,
        ))]),
        control.clone(), // BORROW: .clone() gives the helper its own answer
        &container_ref(&control),
        &SweepScope::CurrentJob,
        GuardRefusal::Control,
    )
    .await;

    // The label names another job.
    let foreign = inspect(&id, labels(&other, "agent", None));
    assert_refused(
        "other job",
        view(),
        foreign.clone(), // BORROW: .clone() gives the helper its own answer
        &container_ref(&foreign),
        &SweepScope::CurrentJob,
        GuardRefusal::OtherJob,
    )
    .await;

    // A lost scope reaches a job the view does not call lost, and never the current job.
    assert_refused(
        "lost scope, job not lost",
        view(),
        foreign.clone(), // BORROW: .clone() gives the helper its own answer
        &container_ref(&foreign),
        &SweepScope::Lost(other.clone()), // BORROW: .clone() gives the scope its own job
        GuardRefusal::OtherJob,
    )
    .await;
    let mine = good("agent", None);
    assert_refused(
        "lost scope names the current job",
        view().with_lost(vec![current.clone()]), // BORROW: .clone() gives the view its own job
        mine.clone(),                            // BORROW: .clone() gives the helper its own answer
        &container_ref(&mine),
        &SweepScope::Lost(current.clone()), // BORROW: .clone() gives the scope its own job
        GuardRefusal::OtherJob,
    )
    .await;

    // The label names another run of the job.
    let stray = good("agent", Some(&other_run));
    assert_refused(
        "other run",
        view(),
        stray.clone(), // BORROW: .clone() gives the helper its own answer
        &container_ref(&stray),
        &SweepScope::Run(run.clone()), // BORROW: .clone() gives the scope its own key
        GuardRefusal::OtherRun,
    )
    .await;

    // What the caller claims differs from what docker says now.
    let claimed = container_ref(&good("agent", Some(&run)));
    let changed = inspect(&id, labels(&other, "agent", Some(&run)));
    assert_refused(
        "inspect label mismatch",
        view(),
        changed,
        &claimed,
        &SweepScope::Run(run.clone()), // BORROW: .clone() gives the scope its own key
        GuardRefusal::Mismatch,
    )
    .await;

    // A record that disagrees with the labels.
    let recorded_elsewhere = good("agent", Some(&run));
    assert_refused(
        "record role mismatch",
        view().with_children(vec![ChildEntry::Container(record(
            &id,
            &current,
            &run,
            ContainerRole::Gateway,
        ))]),
        recorded_elsewhere.clone(), // BORROW: .clone() gives the helper its own answer
        &container_ref(&recorded_elsewhere),
        &SweepScope::CurrentJob,
        GuardRefusal::Mismatch,
    )
    .await;

    // The current job's container that no record names, and whose record file is unreadable.
    let unrecorded = good("agent", None);
    assert_refused(
        "unrecorded",
        view().with_children(vec![ChildEntry::Unreadable {
            path: "children/container-abc.json".into(),
            error: "truncated".to_owned(), // BORROW: .to_owned() gives the entry its own text
        }]),
        unrecorded.clone(), // BORROW: .clone() gives the helper its own answer
        &container_ref(&unrecorded),
        &SweepScope::CurrentJob,
        GuardRefusal::Unrecorded,
    )
    .await;

    // A lost job is reached only through the proof of its held lock.
    let root = tempfile::tempdir().expect("a temp dir");
    let theirs = inspect(&id, labels(&other, "agent", None));
    let lost_rig = |view: FakeJobView| {
        let docker = Arc::new(RecordingDocker::new().with_inspect(id.as_str(), theirs.clone())); // BORROW: .as_str() keys the answer; .clone() gives it its own copy
        let destroyer = Arc::new(RecordingDestroyer::new());
        let read: Arc<dyn DockerDriver> = docker;
        let kill: Arc<dyn DestroyDriver> = destroyer.clone(); // BORROW: .clone() shares the fake; TRAIT_OBJECT: the guard holds a trait object
        (ContainerGuard::new(kill, read, view.shared()), destroyer)
    };
    let full = id.as_str(); // BORROW: explicit .as_str() instead of Deref coercion
    let (dir, _lock, proof) = locked(root.path(), &other);
    let (guard, destroyer) = lost_rig(view());
    assert_eq!(
        guard.destroy_lost(&container_ref(&theirs), &proof).await,
        Ok(Destroyed {
            recorded: false,
            absent: false
        }),
        "a lost job under its held lock"
    );
    assert_eq!(
        destroyer.calls(),
        [format!("stop {full}"), format!("rm {full}")]
    );
    // A proof for another job does not reach this one.
    let third = job(3);
    let (_third_dir, _third_lock, third_proof) = locked(root.path(), &third);
    let (guard, destroyer) = lost_rig(view());
    assert_eq!(
        guard
            .destroy_lost(&container_ref(&theirs), &third_proof)
            .await,
        Err(GuardError::Refused(GuardRefusal::OtherJob))
    );
    // Without a proof the scope reaches nothing, whatever the view says.
    assert_eq!(
        guard
            .destroy(
                &container_ref(&theirs),
                &SweepScope::Lost(other.clone()) // BORROW: .clone() gives the scope its own job
            )
            .await,
        Err(GuardError::Refused(GuardRefusal::OtherJob))
    );
    assert!(destroyer.calls().is_empty(), "{:?}", destroyer.calls());
    // A job that has ended is not lost.
    end_job(&dir);
    let (guard, destroyer) = lost_rig(view());
    assert_eq!(
        guard.destroy_lost(&container_ref(&theirs), &proof).await,
        Err(GuardError::Refused(GuardRefusal::OtherJob))
    );
    assert!(destroyer.calls().is_empty(), "{:?}", destroyer.calls());
    // A job that ends while its container stops is asked about again before the rm.
    let (dir_late, _lock_late, proof_late) = locked(root.path(), &job(4));
    let late = inspect(&id, labels(&job(4), "agent", None));
    let docker = Arc::new(RecordingDocker::new().with_inspect(id.as_str(), late.clone())); // BORROW: .as_str() keys the answer; .clone() gives it its own copy
    let recorded_calls = Arc::new(RecordingDestroyer::new());
    let ending: Arc<dyn DestroyDriver> = Arc::new(EndingDestroyer {
        dir: dir_late,
        inner: Arc::clone(&recorded_calls),
    });
    let guard = ContainerGuard::new(ending, docker, view().shared());
    assert_eq!(
        guard.destroy_lost(&container_ref(&late), &proof_late).await,
        Err(GuardError::Refused(GuardRefusal::OtherJob))
    );
    assert_eq!(
        recorded_calls.calls(),
        [format!("stop {full}")],
        "the guard removed a container of a job that had ended"
    );

    // Networks follow the job rule of containers.
    let net_of = |job: &JobId, egress: bool| {
        let name = if egress {
            NetworkName::egress_for_job(job)
        } else {
            NetworkName::for_job(job)
        };
        let info = NetworkInfo {
            name: name.clone(), // BORROW: .clone() gives the answer its own name
            internal: !egress,
            labels: Labels::new().with("candler.job", job.as_str()), // BORROW: explicit .as_str() instead of Deref coercion
        };
        (name, info)
    };
    let fifth = job(5);
    let (_dir5, _lock5, proof5) = locked(root.path(), &fifth);
    for egress in [false, true] {
        let (own, own_info) = net_of(&current, egress);
        let (theirs_net, theirs_info) = net_of(&fifth, egress);
        let log = Log::new();
        let destroyer = Arc::new(RecordingDestroyer::new());
        let docker = ScriptedDocker::new(&log)
            .network(&own, own_info)
            .network(&theirs_net, theirs_info);
        let kill: Arc<dyn DestroyDriver> = Arc::new(LoggingDestroyer::new(&log, destroyer));
        let guard = ContainerGuard::new(kill, Arc::new(docker), view().shared());
        assert_eq!(
            guard
                .destroy_network(&theirs_net, &SweepScope::CurrentJob)
                .await,
            Err(GuardError::Refused(GuardRefusal::OtherJob)),
            "another job's network (egress {egress})"
        );
        assert_eq!(
            guard
                .destroy_network(&theirs_net, &SweepScope::Lost(fifth.clone())) // BORROW: .clone() gives the scope its own job
                .await,
            Err(GuardError::Refused(GuardRefusal::OtherJob)),
            "a lost job's network without a proof (egress {egress})"
        );
        assert!(
            !log.lines()
                .iter()
                .any(|line| line.starts_with("network rm")),
            "{:?}",
            log.lines()
        );
        assert_eq!(
            guard.destroy_network(&own, &SweepScope::CurrentJob).await,
            Ok(()),
            "the current job's own network (egress {egress})"
        );
        assert_eq!(
            guard.destroy_network_lost(&theirs_net, &proof5).await,
            Ok(()),
            "a lost job's network under its proof (egress {egress})"
        );
        let removed: Vec<String> = log
            .lines()
            .into_iter()
            .filter(|line| line.starts_with("network rm"))
            .collect();
        assert_eq!(
            removed,
            [
                format!("network rm {}", own.as_str()), // BORROW: explicit .as_str()
                format!("network rm {}", theirs_net.as_str()), // BORROW: explicit .as_str()
            ],
            "egress {egress}"
        );
    }

    // Positive control: a recorded container whose labels match is stopped and removed.
    let recorded = good("agent", Some(&run));
    let (guard, docker, destroyer) = rig(
        view().with_children(vec![ChildEntry::Container(record(
            &id,
            &current,
            &run,
            ContainerRole::Agent,
        ))]),
        vec![recorded.clone()], // BORROW: .clone() gives the rig its own answer
    );
    let done = guard
        .destroy(&container_ref(&recorded), &SweepScope::CurrentJob)
        .await;
    assert_eq!(
        done,
        Ok(Destroyed {
            recorded: true,
            absent: false
        })
    );
    let full = id.as_str(); // BORROW: explicit .as_str() instead of Deref coercion
    assert_eq!(
        destroyer.calls(),
        vec![format!("stop {full}"), format!("rm {full}")]
    );
    assert_eq!(
        docker.calls(),
        vec![format!("inspect {full}"), format!("inspect {full}")]
    );

    // collect_and_destroy: the scope is the caller's run, never the target's own label. A cidfile
    // that names a container of another run of the same job destroys nothing; the run's own
    // container is destroyed.
    let work = tempfile::tempdir().expect("a temp dir");
    let plan = ContainerPlan {
        image: ImageRef {
            id: ImageId(String::from("sha256:aa")),
            harness_version: None,
        },
        labels: Labels::new().with("candler.job", current.as_str()), // BORROW: explicit .as_str()
        mounts: Vec::new(),
        env: Vec::new(),
        gateway: None,
        state: Vec::new(),
        network: NetworkName::for_job(&current),
        egress: None,
        limits: Limits {
            pids_limit: 64,
            memory: String::from("1g"),
            cpus: 1.0,
        },
        cidfile: work.path().join("agent.cid"),
        user: UserSpec::Engine,
        workdir: String::from("/work"),
        home: String::from("/home/agent"),
        expiry: Duration::from_secs(10),
        project_root: work.path().to_path_buf(),
        store_root: work.path().to_path_buf(),
    };
    std::fs::write(&plan.cidfile, id.as_str()).expect("a cidfile"); // BORROW: explicit .as_str()
    let exit = Exit::new(
        ExitStatus::Code { code: 0 },
        Ended::Exited,
        Vec::new(),
        true,
        Vec::new(),
        Vec::new(),
    );
    let workspace = Workspace::new(work.path());
    let gateway = GatewayHandle::new(
        container_ref(&inspect(&cid(0xfff), labels(&current, "gateway", None))),
        "http://127.0.0.1:1".parse().expect("a url"),
        NetworkName::for_job(&current),
        NetworkName::egress_for_job(&current),
        String::from("dummy"),
    )
    .expect("a handle");
    for (case, label_run, own) in [
        ("another run's container", &other_run, false),
        ("the run's own", &run, true),
    ] {
        let target = inspect(&id, labels(&current, "agent", Some(label_run)));
        let (guard, docker, destroyer) = rig(
            view().with_children(vec![ChildEntry::Container(record(
                &id,
                &current,
                label_run,
                ContainerRole::Agent,
            ))]),
            vec![target],
        );
        let read: Arc<dyn DockerDriver> = docker; // TRAIT_OBJECT: the isolation reads through the driver trait, a scripted fake here
        let isolation = DockerIsolation::new(gateway.clone(), read, Arc::new(guard)); // BORROW: .clone() gives each isolation its own handle
        let outcome = isolation
            .collect_and_destroy(&plan, &workspace, &exit, &run)
            .await
            .expect("an outcome");
        if own {
            assert_eq!(
                destroyer.calls(),
                vec![format!("stop {full}"), format!("rm {full}")],
                "{case}"
            );
            assert_eq!(outcome.sweep.destroyed.len(), 1, "{case}");
        } else {
            assert!(
                destroyer.calls().is_empty(),
                "{case}: {:?}",
                destroyer.calls()
            );
            assert!(outcome.sweep.destroyed.is_empty(), "{case}");
            assert_eq!(outcome.sweep.refused.len(), 1, "{case}");
        }
    }

    // A container of another job, named by the cidfile, is refused as a mismatch of the job.
    let foreign_target = inspect(&id, labels(&other, "agent", Some(&run)));
    let (guard, docker, destroyer) = rig(view(), vec![foreign_target]);
    let read: Arc<dyn DockerDriver> = docker; // TRAIT_OBJECT: the isolation reads through the driver trait, a scripted fake here
    let isolation = DockerIsolation::new(gateway, read, Arc::new(guard));
    let outcome = isolation
        .collect_and_destroy(&plan, &workspace, &exit, &run)
        .await
        .expect("an outcome");
    assert!(destroyer.calls().is_empty(), "{:?}", destroyer.calls());
    assert_eq!(outcome.sweep.refused.len(), 1);
    // INDEX: the length is asserted to be 1 on the line above
    assert_eq!(outcome.sweep.refused[0].reason, "job_mismatch");
}

/// Guards: B11 — every destroy verb is preceded by an inspect of the same id, and a second
/// inspect that differs from the first refuses the `rm`.
/// Oracle: the one ordered log of inspects and verbs; the verb list when the second scripted
/// answer has another job, another creation time or another id.
/// Bite: reuse the first inspect result in `ContainerGuard::destroy` instead of inspecting again.
#[tokio::test]
async fn b11_inspect_before_every_verb() {
    let current = job(1);
    let run = run_key(1);
    let id = cid(0xabc);
    let full = id.as_str(); // BORROW: explicit .as_str() instead of Deref coercion
    let first = inspect(&id, labels(&current, "agent", Some(&run)));
    let scope = SweepScope::Run(run.clone()); // BORROW: .clone() gives the scope its own key

    let run_case = |answers: Vec<candler_core::isolation::model::Inspect>| {
        let log = Log::new();
        let docker: Arc<dyn DockerDriver> =
            Arc::new(ScriptedDocker::new(&log).script(&id, answers)); // TRAIT_OBJECT: the guard holds a trait object
        let recording = Arc::new(RecordingDestroyer::new());
        let destroyer: Arc<dyn DestroyDriver> = Arc::new(LoggingDestroyer::new(&log, recording)); // TRAIT_OBJECT: the guard holds a trait object
        let view = FakeJobView::new(current.clone()).shared(); // BORROW: .clone() gives the view its own job
        (ContainerGuard::new(destroyer, docker, view), log)
    };

    // Positive control: the same answer twice runs all four verbs, each verb after an inspect.
    let (guard, log) = run_case(vec![first.clone(), first.clone()]); // BORROW: .clone() gives the script its own answers
    let result = guard.destroy(&container_ref(&first), &scope).await;
    assert_eq!(
        result,
        Ok(Destroyed {
            recorded: false,
            absent: false
        })
    );
    assert_eq!(
        log.lines(),
        vec![
            format!("inspect {full}"),
            format!("stop {full}"),
            format!("inspect {full}"),
            format!("rm {full}"),
        ]
    );

    // The second inspect answers with the id's labels changed, as if the id had been reused.
    let other_job = inspect(&id, labels(&job(2), "agent", Some(&run)));
    let mut later = first.clone(); // BORROW: .clone() gives the case its own answer
    later.created = "2026-02-02T03:04:05Z".parse().expect("a timestamp");
    for (case, second) in [
        ("labels differ", other_job),
        ("creation time differs", later),
    ] {
        let (guard, log) = run_case(vec![first.clone(), second]); // BORROW: .clone() gives the script its own answer
        let result = guard.destroy(&container_ref(&first), &scope).await;
        assert_eq!(
            result,
            Err(GuardError::Refused(GuardRefusal::Mismatch)),
            "{case}"
        );
        assert_eq!(
            log.lines(),
            vec![
                format!("inspect {full}"),
                format!("stop {full}"),
                format!("inspect {full}"),
            ],
            "{case}: a verb followed the second inspect"
        );
    }
}

/// Records each group it is asked to signal and signals nothing.
#[derive(Default)]
struct RecordingSignaller {
    calls: Mutex<Vec<(i32, i32)>>,
}

impl Signaller for RecordingSignaller {
    fn signal_group(&self, group: Pid, signal: Signal) -> Result<(), Errno> {
        self.calls
            .lock()
            .expect("a lock")
            .push((group.as_raw_nonzero().get(), signal.as_raw()));
        Ok(())
    }
}

/// Keeps the stop steps the supervisor reports, in order, with their outcomes.
#[derive(Default)]
struct StepLog {
    steps: Mutex<Vec<(StopStep, Option<String>)>>,
}

impl EventSink for StepLog {
    fn emit(&self, _run: Option<&RunKey>, payload: EventPayload) {
        if let EventPayload::ProcessStopStep { step, outcome, .. } = payload {
            self.steps.lock().expect("a lock").push((step, outcome));
        }
    }
}

/// A hook that fails where it is told to and notes that it ran.
struct FailingHook {
    fail_pre_stop: bool,
    fail_destroy: bool,
    ran: Mutex<Vec<&'static str>>,
}

impl StopHook for FailingHook {
    fn pre_stop<'a>(&'a self, _ctx: &'a StopHookCtx) -> BoxFuture<'a, Result<(), HookError>> {
        self.ran.lock().expect("a lock").push("pre_stop");
        Box::pin(async move {
            if self.fail_pre_stop {
                Err(HookError::Failed {
                    detail: "boom-pre".to_owned(), // BORROW: .to_owned() gives the error its own text
                })
            } else {
                Ok(())
            }
        })
    }

    fn destroy<'a>(
        &'a self,
        _ctx: &'a StopHookCtx,
    ) -> BoxFuture<'a, Result<candler_core::isolation::model::ContainerSweep, HookError>> {
        self.ran.lock().expect("a lock").push("destroy");
        Box::pin(async move {
            if self.fail_destroy {
                Err(HookError::Refused {
                    detail: "boom-destroy".to_owned(), // BORROW: .to_owned() gives the error its own text
                })
            } else {
                Ok(candler_core::isolation::model::ContainerSweep::default())
            }
        })
    }
}

/// Stops a `sleep` child whose hook fails as told, and returns the steps the ladder reported, the
/// groups it signalled and the pid of the child.
async fn ladder(
    fail_pre_stop: bool,
    fail_destroy: bool,
) -> (
    Vec<(StopStep, Option<String>)>,
    Vec<(i32, i32)>,
    i32,
    Vec<&'static str>,
) {
    let signals = Arc::new(RecordingSignaller::default());
    let steps = Arc::new(StepLog::default());
    let signaller: Arc<dyn Signaller> = signals.clone(); // BORROW: .clone() shares the log; TRAIT_OBJECT: the supervisor takes a trait object
    let sink: Arc<dyn EventSink> = steps.clone(); // BORROW: .clone() shares the log; TRAIT_OBJECT: the supervisor takes a trait object
    let supervisor = Supervisor::new(sink).with_signaller(signaller);
    let hook = Arc::new(FailingHook {
        fail_pre_stop,
        fail_destroy,
        ran: Mutex::new(Vec::new()),
    });
    let attached: Arc<dyn StopHook> = hook.clone(); // BORROW: .clone() shares the hook; TRAIT_OBJECT: the request takes a trait object
    let records = tempfile::tempdir().expect("a temp dir");
    let request = SpawnRequest::builder()
        .argv(vec!["sleep".to_owned(), "1".to_owned()]) // BORROW: .to_owned() gives the argument its own text
        .env(vec![("PATH".to_owned(), "/usr/bin:/bin".to_owned())]) // BORROW: .to_owned() gives the variable its own text
        .marker("b14-stop-hook".parse().expect("a marker"))
        .run(run_key(5))
        .stop(StopSpec::new(
            None,
            StopSignal::Term,
            Duration::from_millis(200),
        ))
        .stop_hook(attached)
        .record_dir(records.path().to_path_buf())
        .build()
        .expect("a request");
    let child = supervisor.spawn(request).expect("a child");
    let pid = i32::try_from(child.record().pid()).expect("a pid");
    // The recording signaller delivers nothing, so the wait may end in an error; the steps and
    // the signals are what is checked.
    let _ = child.stop().await;
    let ran = hook.ran.lock().expect("a lock").clone(); // BORROW: .clone() copies the list out of the lock
    let seen = steps.steps.lock().expect("a lock").clone(); // BORROW: .clone() copies the log out of the lock
    let calls = signals.calls.lock().expect("a lock").clone(); // BORROW: .clone() copies the log out of the lock
    (seen, calls, pid, ran)
}

/// Guards: B14 — a hook that fails in `pre_stop` or `destroy` is logged as that step's outcome
/// and the group signals, the sweep and the other hook step still run.
/// Oracle: the stop steps the supervisor reports and the groups the recording signaller was asked
/// to signal; the positive control is a hook that fails nowhere, which runs all four steps.
/// Bite: return early on `Err` from the hook in `supervise/stop.rs`, applied transiently.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b14_hook_error_logged_ladder_continues() {
    let order = |steps: &[(StopStep, Option<String>)]| -> Vec<StopStep> {
        steps.iter().map(|(step, _)| *step).collect()
    };
    let outcome_of = |steps: &[(StopStep, Option<String>)], wanted: StopStep, nth: usize| {
        steps
            .iter()
            .filter(|(step, _)| *step == wanted)
            .nth(nth)
            .and_then(|(_, outcome)| outcome.clone()) // BORROW: .clone() copies the outcome out of the log
    };
    for (label, fail_pre_stop, fail_destroy) in [
        ("healthy hook", false, false),
        ("pre_stop fails", true, false),
        ("destroy fails", false, true),
    ] {
        let (steps, calls, pid, ran) = ladder(fail_pre_stop, fail_destroy).await;
        let (term, kill) = (Signal::TERM.as_raw(), Signal::KILL.as_raw());
        assert_eq!(ran, ["pre_stop", "destroy"], "{label}: {steps:?}");
        assert_eq!(
            calls,
            [(pid, term), (pid, term), (pid, kill)],
            "{label}: the group signals changed"
        );
        let seen = order(&steps);
        let position = |wanted: StopStep| seen.iter().position(|step| *step == wanted);
        let (pre, terminate, sweep, destroy) = (
            position(StopStep::PreStop),
            position(StopStep::TerminateGroup),
            position(StopStep::Sweep),
            position(StopStep::DestroyContainer),
        );
        assert!(
            pre < terminate && terminate < sweep && sweep < destroy && destroy.is_some(),
            "{label}: the four steps did not run in order: {seen:?}"
        );
        let pre_outcome = outcome_of(&steps, StopStep::PreStop, 0).unwrap_or_default();
        let destroy_outcome = outcome_of(&steps, StopStep::DestroyContainer, 0).unwrap_or_default();
        if fail_pre_stop {
            assert!(pre_outcome.contains("boom-pre"), "{label}: {pre_outcome}");
        } else {
            assert!(!pre_outcome.contains("boom"), "{label}: {pre_outcome}");
        }
        if fail_destroy {
            assert!(
                destroy_outcome.contains("boom-destroy"),
                "{label}: {destroy_outcome}"
            );
        } else {
            assert!(
                !destroy_outcome.contains("boom"),
                "{label}: {destroy_outcome}"
            );
        }
    }
}

/// Guards: B15 — a record the job directory could not read is reported by the sweeper and never
/// read as "no container"; the valid record beside it is still handled.
/// Oracle: the sweep's `unknown` flag, the unreadable records it lists and the recording
/// destroyer's verbs for the valid sibling.
/// Bite: `ChildEntry::Unreadable { .. } => {}` in the sweeper's record reader.
#[tokio::test]
async fn b15_unreadable_child_reported() {
    let current = job(1);
    let run = run_key(1);
    let id = cid(0xabc);
    let answer = inspect(&id, labels(&current, "agent", Some(&run)));
    let broken = ChildEntry::Unreadable {
        path: PathBuf::from("children/container-abcabcabcabc.json"),
        error: "expected value at line 1".to_owned(), // BORROW: .to_owned() gives the entry its own text
    };
    let valid = ChildEntry::Container(record(&id, &current, &run, ContainerRole::Agent));
    for (label, children, siblings) in [
        ("broken beside valid", vec![broken.clone(), valid], 1), // BORROW: .clone() gives the case its own entry
        ("broken alone", vec![broken.clone()], 0), // BORROW: .clone() gives the case its own entry
    ] {
        let docker = Arc::new(
            RecordingDocker::new()
                .with_inspect(id.as_str(), answer.clone()) // BORROW: .as_str() keys the answer; .clone() gives it its own copy
                .with_listed(if siblings == 1 {
                    vec![container_ref(&answer)]
                } else {
                    Vec::new()
                }),
        );
        let destroyer = Arc::new(RecordingDestroyer::new());
        let view = FakeJobView::new(current.clone()).with_children(children); // BORROW: .clone() gives the view its own job
        let read: Arc<dyn DockerDriver> = docker.clone(); // BORROW: .clone() shares the fake; TRAIT_OBJECT: the guard holds a trait object
        let kill: Arc<dyn DestroyDriver> = destroyer.clone(); // BORROW: .clone() shares the fake; TRAIT_OBJECT: the guard holds a trait object
        let guard = Arc::new(ContainerGuard::new(
            kill,
            Arc::clone(&read),
            view.clone().shared(),
        )); // BORROW: .clone() gives the guard its own view
        let sweeper = LabelSweeper::new(read, guard, view.shared(), Duration::from_secs(5));

        let report = sweeper
            .sweep_report(&SweepScope::CurrentJob)
            .await
            .expect("a sweep");
        assert!(
            report.sweep.unknown,
            "{label}: an unreadable record was read as nothing"
        );
        assert_eq!(
            report.unreadable,
            [UnreadableRecord {
                path: PathBuf::from("children/container-abcabcabcabc.json"),
                error: "expected value at line 1".to_owned(), // BORROW: .to_owned() gives the record its own text
            }],
            "{label}"
        );
        let full = id.as_str(); // BORROW: explicit .as_str() instead of Deref coercion
        if siblings == 1 {
            assert_eq!(report.sweep.destroyed, [container_ref(&answer)], "{label}");
            assert_eq!(
                destroyer.calls(),
                [format!("stop {full}"), format!("rm {full}")],
                "{label}"
            );
        } else {
            assert!(report.sweep.destroyed.is_empty(), "{label}");
            assert!(destroyer.calls().is_empty(), "{label}");
        }
    }
}
