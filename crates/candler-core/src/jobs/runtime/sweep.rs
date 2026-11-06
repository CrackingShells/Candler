//! Finding and killing the processes a run left behind: by the marker in their environment, and,
//! after the job, by having been reparented to this subreaper. Every signal goes through the
//! guard, one process id at a time.

use std::sync::Arc;

use candler_schemas::execution::{EventPayload, StopStep, StrayProcess};
use candler_schemas::primitives::Marker;
use candler_schemas::process::ProcessStart;
use rustix::process::Signal;

use super::reap::{reap_by_pid, reap_one};
use crate::supervise::signal::{Guard, SignalError};
use crate::supervise::stop::StraySweeper;
use crate::supervise::{EventSink, Supervisor, now};

/// The process ids that `/proc` lists; none where there is no `/proc`.
fn listed_pids() -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .collect()
}

/// Records the sweep step of `pid`, with what came of it as its outcome.
// TRAIT_OBJECT: the sink is the job's event log, or a fake in tests
fn record_step(sink: &dyn EventSink, pid: u32, start: &ProcessStart, outcome: String) {
    let Ok(at) = now() else {
        return;
    };
    // BORROW: .clone() gives the event its own start
    let step =
        EventPayload::process_stop_step(pid, start.clone(), StopStep::Sweep, at, Some(outcome));
    if let Ok(payload) = step {
        sink.emit(None, payload);
    }
}

/// Whether the signal killed the process. A process already gone was not killed by it, and a
/// refusal is recorded to `sink` and kills nothing.
fn killed(
    // TRAIT_OBJECT: the sink is the job's event log, or a fake in tests
    sink: Option<&dyn EventSink>,
    pid: u32,
    start: &ProcessStart,
    sent: Result<(), SignalError>,
) -> bool {
    match sent {
        Ok(()) => true,
        Err(SignalError::Gone) => false,
        Err(error) => {
            if let Some(sink) = sink {
                record_step(sink, pid, start, error.to_string());
            }
            false
        }
    }
}

/// The sweeper a job's supervisor installs: on Linux it reads the environment of every process
/// of the same user and kills the ones whose `CANDLER_RUN` is the marker exactly. Elsewhere it
/// finds nothing.
pub struct ProcMarkerSweeper {
    guard: Arc<Guard>,
    // TRAIT_OBJECT: the sink is the job's log, or none
    events: Option<Arc<dyn EventSink>>,
    /// Where the process ids come from: `/proc`, unless a test names them.
    listing: fn() -> Vec<u32>,
}

impl ProcMarkerSweeper {
    /// A sweeper that kills through `guard`.
    #[must_use]
    pub const fn new(guard: Arc<Guard>) -> Self {
        Self {
            guard,
            events: None,
            listing: listed_pids,
        }
    }

    /// The same sweeper listing the processes with `listing` instead of reading `/proc`, so that
    /// a test can show a sweep on any host. For tests only.
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn with_listing(mut self, listing: fn() -> Vec<u32>) -> Self {
        self.listing = listing;
        self
    }

    /// The same sweeper, recording each refusal of the guard to `events`.
    #[must_use]
    // TRAIT_OBJECT: the sink is the job's log
    pub fn with_events(mut self, events: Arc<dyn EventSink>) -> Self {
        self.events = Some(events);
        self
    }
}

impl StraySweeper for ProcMarkerSweeper {
    fn sweep(&self, marker: &Marker) -> Vec<StrayProcess> {
        let (own_group, own_pid) = self.guard.own();
        let Some(uid) = self.guard.facts(own_pid).map(|facts| facts.uid) else {
            return Vec::new();
        };
        let mut dead = Vec::new();
        for pid in (self.listing)() {
            if pid <= 1 || pid == own_pid {
                continue;
            }
            let Some(facts) = self.guard.facts(pid) else {
                continue;
            };
            // BORROW: .as_str() compares the marker exactly, never by prefix
            let carries = facts.marker.as_deref() == Some(marker.as_str());
            // A group this supervisor holds is a live child of its own: the group signal of its
            // ladder ends it, and the tokio runtime that owns it waits for it.
            if !carries
                || facts.uid != uid
                || facts.group == own_group
                || self.guard.holds(facts.group)
            {
                continue;
            }
            let sent = self
                .guard
                .signal_marked(pid, &facts.start, marker, Signal::KILL);
            let sink = self.events.as_deref();
            if killed(sink, pid, &facts.start, sent)
                && let Ok(stray) = StrayProcess::new(pid, facts.start)
            {
                dead.push(stray);
            }
        }
        let _reaped = reap_by_pid(&dead);
        dead
    }
}

/// The processes that have exited, that this process is the parent of, and that are not a leader
/// this supervisor spawned: such a leader belongs to the runtime that spawned it, which waits
/// for it.
fn zombies(guard: &Guard) -> Vec<(u32, ProcessStart)> {
    let (_, own_pid) = guard.own();
    let mut found = Vec::new();
    for pid in listed_pids() {
        if pid <= 1 || pid == own_pid {
            continue;
        }
        let Some(facts) = guard.facts(pid) else {
            continue;
        };
        let held_leader = facts.group == pid && guard.holds(pid);
        if facts.parent == own_pid && facts.state == 'Z' && !held_leader {
            found.push((pid, facts.start));
        }
    }
    found
}

/// Waits, without blocking, for every process that `zombies` finds, once: for a moment when no
/// spawn is in flight. Each wait names one process id. Reaping sends no signal. Returns how many
/// were reaped.
#[must_use]
pub fn reap_zombies(guard: &Guard) -> usize {
    zombies(guard)
        .into_iter()
        .filter(|(pid, _)| reap_one(*pid))
        .count()
}

/// Waits for the processes that `zombies` finds now and found, with the same start, in
/// `previous`, and leaves `previous` holding the ones seen now and not reaped. A child that
/// exits between its spawn and its entry in the table is either in the table a tick later, and
/// then not found, or already waited for by the runtime that spawned it; it is never taken in
/// the tick that first sees it. Returns how many were reaped.
#[must_use]
pub fn reap_confirmed(guard: &Guard, previous: &mut Vec<(u32, ProcessStart)>) -> usize {
    let now = zombies(guard);
    let mut reaped = 0;
    let mut kept = Vec::new();
    for seen in now {
        if previous.contains(&seen) && reap_one(seen.0) {
            reaped += 1;
        } else {
            kept.push(seen);
        }
    }
    *previous = kept;
    reaped
}

/// Kills, once the last run is sealed, every process that was reparented to this subreaper and
/// that no group of the supervisor holds, each through `Guard::signal_adopted`, and reaps it
/// by process id. A process the guard refuses, such as one without the marker, is recorded and
/// left alive. Returns the processes killed.
#[must_use]
// TRAIT_OBJECT: the sink is the job's event log, or a fake in tests
pub fn sweep_orphans(supervisor: &Supervisor, sink: &dyn EventSink) -> Vec<StrayProcess> {
    let guard = supervisor.guard();
    let (_, own_pid) = guard.own();
    let mut dead = Vec::new();
    for pid in listed_pids() {
        if pid <= 1 || pid == own_pid {
            continue;
        }
        let Some(facts) = guard.facts(pid) else {
            continue;
        };
        if facts.parent != own_pid || guard.holds(facts.group) {
            continue;
        }
        let sent = guard.signal_adopted(pid, &facts.start, Signal::KILL);
        if killed(Some(sink), pid, &facts.start, sent) {
            // BORROW: the step owns its outcome
            record_step(sink, pid, &facts.start, "killed as an orphan".to_owned());
            if let Ok(stray) = StrayProcess::new(pid, facts.start) {
                dead.push(stray);
            }
        }
    }
    let _reaped = reap_by_pid(&dead);
    let _zombies = reap_zombies(&guard);
    dead
}
