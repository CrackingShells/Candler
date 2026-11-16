//! A run's stop goes through the daemon; the attached client is signalled only as the fallback.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use candler_core::isolation::EndFacts;
use candler_core::isolation::model::ContainerOutcome;
use candler_core::runner::exec::spawn::RunStopper;
use candler_core::supervise::hook::{HookError, StopHook, StopHookCtx};
use candler_core::supervise::signal::Signaller;
use candler_core::supervise::spawn::SpawnRequest;
use candler_core::supervise::stop::Ended;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::spec::{StopSignal, StopSpec};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::RunKey;
use futures::future::BoxFuture;
use rustix::io::Errno;
use rustix::process::{Pid, Signal, kill_process_group};

/// Records each signal and delivers it, so a client the ladder ends really ends.
#[derive(Default)]
struct DeliveringSignaller {
    calls: Mutex<Vec<(i32, i32)>>,
}

impl Signaller for DeliveringSignaller {
    fn signal_group(&self, group: Pid, signal: Signal) -> Result<(), Errno> {
        self.calls
            .lock()
            .expect("a lock")
            .push((group.as_raw_nonzero().get(), signal.as_raw()));
        kill_process_group(group, signal)
    }
}

/// The hook of a run whose `pre_stop` is the daemon's stop, as the container hook is.
struct DaemonHook(Arc<FakeDaemon>);

impl StopHook for DaemonHook {
    fn pre_stop<'a>(&'a self, ctx: &'a StopHookCtx) -> BoxFuture<'a, Result<(), HookError>> {
        Box::pin(async move {
            match &ctx.run {
                Some(run) => self.0.stop_agent(run).await,
                None => Ok(()),
            }
        })
    }
}

struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// How the fake daemon answers `docker stop`.
enum Daemon {
    /// The container stops and the client, which relays and exits, leaves at once.
    Stops,
    /// The container stops but the client stays.
    StopsClientStays,
    /// The daemon cannot be reached.
    Unreachable,
}

/// The run's end facts with a daemon that answers as told; a stop that works makes the client
/// leave by creating the file the client waits for.
struct FakeDaemon {
    answer: Daemon,
    leave: PathBuf,
    asked: Mutex<usize>,
}

impl EndFacts for FakeDaemon {
    fn final_inspect(&self) -> Option<ContainerOutcome> {
        None
    }

    fn stop_agent<'a>(&'a self, _run: &'a RunKey) -> BoxFuture<'a, Result<(), HookError>> {
        *self.asked.lock().expect("a lock") += 1;
        Box::pin(async move {
            match self.answer {
                Daemon::Stops => {
                    std::fs::write(&self.leave, b"").expect("the client's way out");
                    Ok(())
                }
                Daemon::StopsClientStays => Ok(()),
                Daemon::Unreachable => Err(HookError::Failed {
                    detail: String::from("the daemon is unreachable"),
                }),
            }
        })
    }
}

/// Stops a client that waits for `leave` to exist, through a [`RunStopper`] over a daemon that
/// answers as told. Returns how the run ended, the signals the client received, and how often the
/// daemon was asked.
async fn stop_run(answer: Daemon, deadline: bool) -> (Option<Ended>, Vec<(i32, i32)>, usize) {
    let records = tempfile::tempdir().expect("a temp dir");
    let leave = records.path().join("leave");
    let signals = Arc::new(DeliveringSignaller::default());
    let signaller: Arc<dyn Signaller> = signals.clone(); // BORROW: .clone() shares the log; TRAIT_OBJECT: the supervisor takes a trait object
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let supervisor = Supervisor::new(sink).with_signaller(signaller);
    let waits = "while [ ! -e \"$1\" ]; do sleep 0.05; done";
    let facts = Arc::new(FakeDaemon {
        answer,
        leave: leave.clone(), // BORROW: the facts own their path
        asked: Mutex::new(0),
    });
    let mut builder = SpawnRequest::builder()
        .argv(vec![
            "sh".to_owned(),  // BORROW: .to_owned() gives the argument its own text
            "-c".to_owned(),  // BORROW: .to_owned() gives the argument its own text
            waits.to_owned(), // BORROW: .to_owned() gives the argument its own text
            "sh".to_owned(),  // BORROW: .to_owned() gives the argument its own text
            leave.to_string_lossy().into_owned(),
        ])
        .env(vec![("PATH".to_owned(), "/usr/bin:/bin".to_owned())]) // BORROW: .to_owned() gives the variable its own text
        .marker("stop-via-daemon".parse().expect("a marker"))
        .run(crate::support::container_stop::run_key(7))
        .stop(StopSpec::new(
            None,
            StopSignal::Term,
            Duration::from_millis(300),
        ))
        .record_dir(records.path().to_path_buf());
    if deadline {
        let hook: Arc<dyn StopHook> = Arc::new(DaemonHook(facts.clone())); // BORROW: .clone() shares the facts; TRAIT_OBJECT: the request takes a trait object
        builder = builder.stop_hook(hook).deadline(Duration::from_millis(300));
    }
    let request = builder.build().expect("a request");
    let child = supervisor.spawn(request).expect("a child");
    let stopper = RunStopper::new(
        child.stop_handle(),
        facts.clone(), // BORROW: .clone() shares the facts; TRAIT_OBJECT: the stopper takes a trait object
        crate::support::container_stop::run_key(7),
        Duration::from_millis(300),
    );
    if !deadline {
        stopper.request();
    }
    // The wait is bounded: a run that never ends is the failure, not a hang of the suite.
    let ended = tokio::time::timeout(Duration::from_secs(20), child.wait())
        .await
        .ok()
        .and_then(Result::ok)
        .map(|exit| exit.ended);
    let calls = signals.calls.lock().expect("a lock").clone(); // BORROW: .clone() copies the log out of the lock
    let asked = *facts.asked.lock().expect("a lock");
    (ended, calls, asked)
}

/// Guards: the stop path (D8) — a stop asks the daemon to stop the container and leaves the
/// attached client alone while the daemon answers; the supervisor ends the client only when the
/// daemon cannot be asked or the client outlives the container.
/// Oracle: the groups the recording signaller was asked to signal and how the run ended, read
/// from the supervisor, with the three daemon answers the ruling's probe table distinguishes.
/// The deadline rows are the supervisor's own stop, whose hook is the daemon's stop.
/// Bite: the ladder signalling right after `pre_stop` (the deadline and stopper rows with a
/// working daemon are signalled); the fallback removed from `RunStopper::stop` (the handle is never requested): the
/// unreachable-daemon row and the client-stays row end in `None`, a run that never ends.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_asks_the_daemon_and_falls_back_to_the_client() {
    let rows = [
        (
            "the daemon stops the container",
            Daemon::Stops,
            false,
            false,
        ),
        (
            "the daemon is unreachable",
            Daemon::Unreachable,
            true,
            false,
        ),
        (
            "the client outlives the stopped container",
            Daemon::StopsClientStays,
            true,
            false,
        ),
        ("deadline, the daemon stops", Daemon::Stops, false, true),
        (
            "deadline, daemon unreachable",
            Daemon::Unreachable,
            true,
            true,
        ),
        (
            "deadline, the client outlives the container",
            Daemon::StopsClientStays,
            true,
            true,
        ),
    ];
    for (case, answer, signalled, deadline) in rows {
        let (ended, calls, asked) = stop_run(answer, deadline).await;
        let (ended_by, left) = if deadline {
            (Ended::Deadline, Ended::Deadline)
        } else {
            (Ended::Stopped, Ended::Exited)
        };
        assert_eq!(asked, 1, "{case}: the daemon is asked once");
        if signalled {
            assert_eq!(ended, Some(ended_by), "{case}: the run did not end");
            assert!(!calls.is_empty(), "{case}: the client was never signalled");
        } else {
            assert_eq!(ended, Some(left), "{case}: the client was ended");
            assert_eq!(
                calls,
                Vec::<(i32, i32)>::new(),
                "{case}: the client was signalled"
            );
        }
    }
}
