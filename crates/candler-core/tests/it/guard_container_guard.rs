//! Guards for the container guard and its callers, against recording fakes. No test here reaches
//! docker: the read driver is scripted and the destroy driver is the recording one from
//! `test-support`, so a mutated guard destroys nothing.
#![cfg(test)]

use crate::support;

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use candler_core::isolation::container::guard::{
    ContainerGuard, Destroyed, GuardError, GuardRefusal, LostProof,
};
use candler_core::isolation::container::stop::ContainerStopHook;
use candler_core::isolation::container::sweeper::{LabelSweeper, UnreadableRecord};
use candler_core::isolation::docker::{ClientEnv, DockerCli, DockerError};
use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
use candler_core::isolation::model::{ContainerRef, SweepScope};
use candler_core::isolation::model::{Labels, NetworkInfo, NetworkName};
use candler_core::isolation::testing::{FakeJobView, RecordingDestroyer, RecordingDocker};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::JobEvents;
use candler_core::jobs::lock::JobLock;
use candler_core::jobs::view::ChildEntry;
use candler_core::supervise::hook::{HookError, StopHook, StopHookCtx};
use candler_core::supervise::signal::Signaller;
use candler_core::supervise::spawn::SpawnRequest;
use candler_core::supervise::stop::Ended;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::spec::{StopSignal, StopSpec};
use candler_schemas::container::ContainerRole;
use candler_schemas::execution::{EventPayload, JobEnd, StopStep};
use candler_schemas::primitives::{ContainerId, JobId, RunKey};
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
/// Oracle: the recording destroy driver's call list is empty for each refused case, and the
/// positive control's list is exactly `stop` then `rm` of the recorded id after one inspect
/// (the verb order of the design's I22 text, with the second inspect dropped by D7).
/// Bite: skip the `job == current job` comparison in `ContainerGuard::check_job`; or drop the
/// `check_job` call before `rm` in `ContainerGuard::destroy_verified`; or replace the
/// `check_job` call in `ContainerGuard::destroy_network_with` by `false && ..`.
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
    assert_eq!(docker.calls(), vec![format!("inspect_many {full}")]);
}

/// Guards: R01 §Process supervision (D7) — a destroy inspects the container once, before its
/// verbs: `stop` and `rm` follow that one verification, and a container whose facts the caller
/// claims wrongly is refused before any verb.
/// Oracle: the one ordered log of inspects and verbs, derived by hand from D7's protocol
/// (`inspect, stop, rm`); the forged claim is a reference whose labels docker does not show.
/// Bite: inspect again before `rm` in `ContainerGuard::destroy_verified`, or skip `claims_hold`.
#[tokio::test]
async fn b11_one_inspect_before_the_verbs() {
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

    let (guard, log) = run_case(vec![first.clone()]); // BORROW: .clone() gives the script its own answer
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
            format!("rm {full}"),
        ]
    );

    let mut claimed = container_ref(&first);
    claimed.labels = labels(&job(2), "agent", Some(&run));
    let (guard, log) = run_case(vec![first.clone()]); // BORROW: .clone() gives the script its own answer
    assert_eq!(
        guard.destroy(&claimed, &scope).await,
        Err(GuardError::Refused(GuardRefusal::Mismatch))
    );
    assert_eq!(
        log.lines(),
        vec![format!("inspect {full}")],
        "a verb followed a refusal"
    );
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

/// A sink that keeps no event.
struct Quiet;

impl EventSink for Quiet {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// Guards: R01 §Process supervision (one docker client) — several containers are read in one
/// docker call: an id docker does not have is `NotFound` for that id alone, the others keep their
/// facts in the order asked, an id that is not a full id is refused without a call, and the
/// recording fake answers the same way in one recorded call.
/// Oracle: the output of `docker container inspect --format '{{json .}}' A MISSING B C` on Docker
/// Engine 29.9.2, recorded in `tests/fixtures/docker_inspect_many` (stdout of three containers,
/// stderr naming the fourth, exit 1), and the one call a fake binary logs.
/// Bite: a client that spawns once per id, one that maps the exit 1 to a failure of every id, one
/// that answers the found containers in the order of docker's lines against the wrong ids, or an
/// unknown id the fake answers as `Failed`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inspect_many_maps_each_id() {
    use std::os::unix::fs::PermissionsExt as _;

    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("docker_inspect_many");
    let stdout = std::fs::read_to_string(fixtures.join("three_ids_one_missing.stdout"))
        .expect("the recorded stdout");
    let found: Vec<String> = stdout
        .lines()
        .map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).expect("one JSON object");
            value["Id"].as_str().expect("an Id").to_owned()
        })
        .collect();
    assert_eq!(found.len(), 3, "the fixture holds three containers");
    let missing = "f".repeat(64);

    let work = tempfile::tempdir().expect("a temp dir");
    let log = work.path().join("spawns.log");
    let script = work.path().join("docker");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{log}'\ncat '{out}'\ncat '{err}' >&2\nexit 1\n",
            log = log.display(),
            out = fixtures.join("three_ids_one_missing.stdout").display(),
            err = fixtures.join("three_ids_one_missing.stderr").display(),
        ),
    )
    .expect("a fake docker");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("executable");
    let records = work.path().join("records");
    std::fs::create_dir_all(&records).expect("a record dir");
    let sink: Arc<dyn EventSink> = Arc::new(Quiet);
    let cli = DockerCli::with_binary(
        Arc::new(Supervisor::new(sink)),
        script,
        ClientEnv::default(),
        records,
    );

    let asked: Vec<String> = vec![
        found[0].clone(), // BORROW: .clone() gives the request its own ids
        missing.clone(),  // BORROW: .clone() gives the request its own ids
        found[1].clone(), // BORROW: .clone() gives the request its own ids
        found[2].clone(), // BORROW: .clone() gives the request its own ids
        String::from("not-an-id"),
    ];
    let answers = cli.inspect_many(&asked).await;
    assert_eq!(answers.len(), asked.len(), "one answer per id asked");
    for (position, id) in [(0, &found[0]), (2, &found[1]), (3, &found[2])] {
        let inspected = answers[position].as_ref().expect("a found container");
        assert_eq!(
            inspected.id.as_str(),
            id.as_str(),
            "answer {position} is its own id"
        );
    }
    assert!(
        matches!(&answers[1], Err(DockerError::NotFound { detail }) if detail.contains(&missing)),
        "the missing id alone is NotFound: {:?}",
        answers[1]
    );
    assert!(
        matches!(&answers[4], Err(DockerError::NotAFullId { .. })),
        "a short id is refused: {:?}",
        answers[4]
    );
    let spawns = std::fs::read_to_string(&log).expect("the spawn log");
    assert_eq!(
        spawns.lines().count(),
        1,
        "one docker process for all the ids: {spawns}"
    );
    let line = spawns.lines().next().expect("one line");
    assert!(line.starts_with("container inspect "), "the verb: {line}");
    assert!(
        !line.contains("not-an-id"),
        "the short id never reaches docker: {line}"
    );
    for id in [&found[0], &missing, &found[1], &found[2]] {
        assert!(line.contains(id.as_str()), "{id} is asked: {line}");
    }

    let fake = RecordingDocker::new().with_inspect(
        &found[0],
        inspect(
            &ContainerId::from_str(&found[0]).expect("a full id"),
            Labels::new(),
        ),
    );
    let two = vec![found[0].clone(), missing.clone()]; // BORROW: .clone() gives the request its own ids
    let fake_answers = fake.inspect_many(&two).await;
    assert!(fake_answers[0].is_ok(), "the known id is answered");
    assert!(
        matches!(fake_answers[1], Err(DockerError::NotFound { .. })),
        "the unknown id is NotFound: {:?}",
        fake_answers[1]
    );
    assert_eq!(
        fake.calls().len(),
        1,
        "one recorded call: {:?}",
        fake.calls()
    );
}

/// Two containers whose full ids share their first 63 digits, one of this job's run and one of
/// another job, for the rows that tell facts of one from facts of the other.
fn twins() -> (ContainerId, ContainerId) {
    (cid(0x10), cid(0x11))
}

/// Guards: R01 §Process supervision (D7) — facts verified for one container are never applied to
/// another: a decision over two containers whose ids share a long prefix destroys the one that
/// passes the rule, refuses the other by its own facts, and sends no verb for the refused id.
/// Oracle: derived by hand: the twin of another job must answer `other_job`, whatever its
/// neighbour's facts say; the destroyer's list is the passing twin's `stop` and `rm` alone.
/// Bite: key the verified facts by the first 63 digits of the id in `verify_many`.
#[tokio::test]
async fn reuse_never_crosses_containers() {
    let current = job(1);
    let run = run_key(1);
    let (ours, theirs) = twins();
    let mine = inspect(&ours, labels(&current, "agent", Some(&run)));
    let foreign = inspect(&theirs, labels(&job(2), "agent", Some(&run)));
    let (guard, docker, destroyer) = rig(
        FakeJobView::new(current.clone()), // BORROW: .clone() gives the view its own job
        vec![mine.clone(), foreign.clone()], // BORROW: .clone() gives the rig its own answers
    );
    let asked = [container_ref(&mine), container_ref(&foreign)];
    let answers = guard
        .destroy_many(&asked, &SweepScope::Run(run.clone())) // BORROW: .clone() gives the scope its own key
        .await;
    assert_eq!(
        answers,
        vec![
            Ok(Destroyed {
                recorded: false,
                absent: false
            }),
            Err(GuardError::Refused(GuardRefusal::OtherJob)),
        ]
    );
    assert_eq!(
        destroyer.calls(),
        vec![
            format!("stop {}", ours.as_str()), // BORROW: .as_str() renders the id
            format!("rm {}", ours.as_str()),   // BORROW: .as_str() renders the id
        ]
    );
    assert_eq!(
        docker.calls().len(),
        1,
        "one inspect for both: {:?}",
        docker.calls()
    );
}

/// Guards: R01 §Process supervision (D7) — a container gone between the list and the inspect is
/// skipped and never destroyed: the sweep names it neither destroyed nor refused, sends it no
/// verb, and still destroys the container beside it.
/// Oracle: derived by hand: docker lists three ids and knows two; the recording destroyer must
/// show `stop` and `rm` of the two and nothing for the third.
/// Bite: treat `NotFound` in `verify_many` as an inspect that passed, or as a failed sweep.
#[tokio::test]
async fn vanished_between_list_and_inspect() {
    let current = job(1);
    let run = run_key(1);
    let (first, second, gone) = (cid(0x21), cid(0x22), cid(0x23));
    let made = |id: &ContainerId| inspect(id, labels(&current, "agent", Some(&run)));
    let listed = vec![
        container_ref(&made(&first)),
        container_ref(&made(&gone)),
        container_ref(&made(&second)),
    ];
    let docker = Arc::new(
        RecordingDocker::new()
            .with_inspect(first.as_str(), made(&first)) // BORROW: .as_str() keys the answer
            .with_inspect(second.as_str(), made(&second)) // BORROW: .as_str() keys the answer
            .with_listed(listed),
    );
    let destroyer = Arc::new(RecordingDestroyer::new());
    let view = FakeJobView::new(current.clone()); // BORROW: .clone() gives the view its own job
    let read: Arc<dyn DockerDriver> = docker.clone(); // BORROW: .clone() shares the fake; TRAIT_OBJECT: the guard holds a trait object
    let kill: Arc<dyn DestroyDriver> = destroyer.clone(); // BORROW: .clone() shares the fake; TRAIT_OBJECT: the guard holds a trait object
    let guard = Arc::new(ContainerGuard::new(
        kill,
        Arc::clone(&read),
        view.clone().shared(),
    )); // BORROW: .clone() gives the guard its own view
    let sweeper = LabelSweeper::new(read, guard, view.shared(), Duration::from_secs(5));
    let report = sweeper
        .sweep_report(&SweepScope::Run(run.clone())) // BORROW: .clone() gives the scope its own key
        .await
        .expect("a sweep");
    assert!(
        !report.sweep.unknown,
        "a vanished container is not an unknown"
    );
    assert!(
        report.sweep.refused.is_empty(),
        "{:?}",
        report.sweep.refused
    );
    assert_eq!(report.sweep.destroyed.len(), 2);
    let verbs: Vec<String> = [&first, &second]
        .iter()
        .flat_map(|id| {
            [
                format!("stop {}", id.as_str()),
                format!("rm {}", id.as_str()),
            ]
        }) // BORROW: .as_str() renders the id
        .collect();
    assert_eq!(destroyer.calls(), verbs);
    assert!(
        destroyer
            .calls()
            .iter()
            .all(|call| !call.contains(gone.as_str())), // BORROW: .as_str() renders the id
        "a verb named the vanished container: {:?}",
        destroyer.calls()
    );
}

/// A PATH-fake docker over a state directory, and the log of its calls.
///
/// Each container is a file `<id>.json` holding the answer of `container inspect`; `container ls`
/// prints the ids whose file holds the filtered label, `container inspect` prints the files of the
/// ids it is asked and names the others on stderr with exit 1, `container rm` deletes the file,
/// and every call appends its second word to the log.
struct Fakery {
    dir: tempfile::TempDir,
    cli: DockerCli,
    view: FakeJobView,
    job: JobId,
}

impl Fakery {
    fn new(current: &JobId) -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().expect("a temp dir");
        for sub in ["state", "records"] {
            std::fs::create_dir_all(dir.path().join(sub)).expect("a directory");
        }
        let script = dir.path().join("docker");
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
state='{state}'
echo "$2" >> '{log}'
case "$2" in
  ls)
    for a in "$@"; do case "$a" in label=*) kv="${{a#label=}}";; esac; done
    k="${{kv%%=*}}"; v="${{kv#*=}}"
    for f in "$state"/*.json; do
      [ -f "$f" ] && grep -q "\"$k\":\"$v\"" "$f" && basename "$f" .json
    done
    exit 0;;
  inspect)
    code=0
    for a in "$@"; do
      case "$a" in
        container|inspect|--format|'{{{{json .}}}}') ;;
        *) if [ -f "$state/$a.json" ]; then cat "$state/$a.json"; echo; else echo "Error: No such container: $a" >&2; code=1; fi;;
      esac
    done
    exit $code;;
  rm) for a in "$@"; do [ -f "$state/$a.json" ] && mv "${{state:?}}/${{a:?}}.json" "${{state:?}}/${{a:?}}.gone"; done; exit 0;;
  *) exit 0;;
esac
"#,
                state = dir.path().join("state").display(),
                log = dir.path().join("docker.log").display(),
            ),
        )
        .expect("a fake docker");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("executable");
        let sink: Arc<dyn EventSink> = Arc::new(Quiet);
        let cli = DockerCli::with_binary(
            Arc::new(Supervisor::new(sink)),
            script,
            ClientEnv::default(),
            dir.path().join("records"),
        );
        Self {
            dir,
            cli,
            view: FakeJobView::new(current.clone()), // BORROW: .clone() gives the view its own job
            job: current.clone(),                    // BORROW: .clone() gives the rig its own job
        }
    }

    /// Adds a container of the view's job in `role`, of `run` when given.
    fn add(&self, n: u32, role: &str, run: Option<&RunKey>) -> ContainerId {
        let id = cid(n);
        let owner = self.job.clone(); // BORROW: .clone() gives the labels their own job
        let mut pairs = format!(r#""candler.job":"{owner}","candler.role":"{role}""#);
        if let Some(key) = run {
            pairs = format!(r#"{pairs},"candler.run":"{key}""#);
        }
        let text = format!(
            r#"{{"Id":"{id}","Created":"2026-01-02T03:04:05.123456789Z","Image":"sha256:aa","Config":{{"Labels":{{{pairs}}}}},"State":{{"Status":"running","Running":true,"ExitCode":0,"OOMKilled":false,"StartedAt":"2026-01-02T03:04:05.123456789Z","FinishedAt":"0001-01-01T00:00:00Z"}}}}"#,
            id = id.as_str() // BORROW: .as_str() renders the id
        );
        std::fs::write(
            self.dir
                .path()
                .join("state")
                .join(format!("{}.json", id.as_str())), // BORROW: .as_str() names the file
            text,
        )
        .expect("a container");
        id
    }

    /// The hook of this job over the fake.
    fn hook(&self) -> ContainerStopHook {
        let guard = Arc::new(ContainerGuard::over_docker(
            &self.cli,
            self.view.clone().shared(),
        )); // BORROW: .clone() gives the guard its own view
        let read: Arc<dyn DockerDriver> = Arc::new(self.cli.clone()); // BORROW: .clone() gives the sweeper its own client; TRAIT_OBJECT: the sweeper holds a trait object
        let sweeper = Arc::new(LabelSweeper::new(
            read,
            Arc::clone(&guard),
            self.view.clone().shared(), // BORROW: .clone() gives the sweeper its own view
            Duration::from_secs(30),
        ));
        ContainerStopHook::new(guard, sweeper, self.job.clone()) // BORROW: .clone() gives the hook its own job
    }

    /// How many calls of the counted verbs the fake saw.
    fn spawns(&self) -> usize {
        std::fs::read_to_string(self.dir.path().join("docker.log"))
            .unwrap_or_default()
            .lines()
            .filter(|verb| matches!(*verb, "ls" | "inspect" | "stop" | "rm"))
            .count()
    }

    /// How many containers the fake still has.
    fn remaining(&self) -> usize {
        std::fs::read_dir(self.dir.path().join("state")).map_or(0, |entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
                .count()
        })
    }
}

/// A job of 9 containers. A run is the cell's agent container (the gateway carries no run key);
/// `own` agents belong to `run` and the rest of the 9 are one gateway and agents of other runs.
fn nine(fake: &Fakery, run: &RunKey, own: u32) {
    let mut made = 0;
    for n in 0..own {
        fake.add(0x31 + n, "agent", Some(run));
        made += 1;
    }
    fake.add(0x50, "gateway", None);
    made += 1;
    for n in 0..(9 - made) {
        fake.add(0x60 + n, "agent", Some(&run_key(2 + n)));
    }
}

/// Guards: R01 §Process supervision (D7, A1) — a destroy decision costs one list, one inspect,
/// then a stop and an rm per target: a stopped run in a job of 9 containers spawns at most the
/// hand-derived number of docker calls on each path, and the run's own containers alone are gone.
/// Oracle: counts derived by hand from the protocol, before any was measured, from the log of a
/// PATH-fake docker (so a loop hidden in `inspect_many` or in the list is counted). The list is
/// L = 2 (one `ls`, one `inspect_many` of what it printed); a verification is 1 `inspect_many`;
/// a target costs a stop and an rm (2). One run (N = 1 agent) in a job of 9: sweep = L + 1 + 2 =
/// 5. Normal exit = the hook's list L (2), the agent's last inspect (1) and the sweep (5) = 8.
/// Ladder stop = the pre-stop (list 2, verify 1, stop 1 = 4) and the normal exit (8) = 12.
/// Exec pre-stop = list 2 and one inspect = 3. A run of N = 7 (7 of the 9): sweep = 2 + 1 + 14 = 17.
/// Bite: `inspect_many` spawning one docker process per id, or `ps_by_label` inspecting per id.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_budget_paths() {
    let current = job(1);
    let run = run_key(1);
    let ctx = |ended| StopHookCtx::new(Some(run.clone()), "m".parse().expect("a marker"), ended); // BORROW: .clone() gives the context its own key
    let rows: [(&str, u32, usize); 5] = [
        ("sweep", 1, 5),
        ("normal exit", 1, 8),
        ("ladder stop", 1, 12),
        ("exec pre-stop", 1, 3),
        ("sweep of 7", 7, 17),
    ];
    for (path, own, bound) in rows {
        let fake = Fakery::new(&current);
        nine(&fake, &run, own);
        let hook = fake.hook().with_observer(Arc::new(|| {
            candler_harness::spec::Observed::new(vec!["sess-1".parse().expect("a session id")])
        }));
        match path {
            "sweep" | "sweep of 7" => {
                let sweeper = {
                    let guard = hook.guard();
                    let read: Arc<dyn DockerDriver> = Arc::new(fake.cli.clone()); // BORROW: .clone() gives the sweeper its own client; TRAIT_OBJECT: the sweeper holds a trait object
                    LabelSweeper::new(
                        read,
                        guard,
                        fake.view.clone().shared(),
                        Duration::from_secs(30),
                    ) // BORROW: .clone() gives the sweeper its own view
                };
                let report = sweeper
                    .sweep_report(&SweepScope::Run(run.clone())) // BORROW: .clone() gives the scope its own key
                    .await
                    .expect("a sweep");
                assert_eq!(
                    report.sweep.destroyed.len(),
                    usize::try_from(own).expect("a small count"),
                    "{path}"
                );
            }
            "normal exit" | "ladder stop" => {
                if path == "ladder stop" {
                    hook.pre_stop(&ctx(Ended::Stopped))
                        .await
                        .expect("a pre-stop");
                }
                let swept = hook.destroy(&ctx(Ended::Stopped)).await.expect("a destroy");
                assert_eq!(swept.destroyed.len(), 1, "{path}");
            }
            _ => {
                let spec = candler_harness::spec::CommandSpec::builder()
                    .argv(vec!["true".to_owned()]) // BORROW: .to_owned() gives the command its own text
                    .build()
                    .expect("a command");
                let said = hook
                    .exec(&ctx(Ended::Stopped), &spec, Duration::from_secs(5))
                    .await
                    .expect("an exec");
                assert_eq!(said, "exec done", "{path}");
            }
        }
        let spawned = fake.spawns();
        assert!(
            spawned <= bound,
            "{path}: {spawned} spawns, at most {bound}"
        );
        if path != "exec pre-stop" {
            assert_eq!(
                fake.remaining(),
                9 - usize::try_from(own).expect("a small count"),
                "{path}: only the run's containers are gone"
            );
        }
    }
}
