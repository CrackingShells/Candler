//! Signal delivery: the one place this crate sends a signal or probes a process with one.
//!
//! A `Guard` signals a process group only if this supervisor spawned it and still holds it in
//! its table, and only after the leader's process id and start time are read again from the
//! operating system and match. Everything else is refused, and a refusal sends nothing. As a
//! second layer the guard refuses group numbers 0 and 1 and numbers that do not fit a process id
//! (`kill(-1, sig)` signals every process the user owns), the caller's own process group, and the
//! caller's own process id.
//!
//! The only code that calls a `kill` function of `rustix` is `OsSignaller` and the two existence
//! probes below. With the `test-support` feature, tests replace the signaller with a recording
//! fake, so that the logic of the guard and of the escalation is checked without a signal being
//! sent. Without the feature this module is private to the crate.

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use candler_schemas::primitives::Marker;
use candler_schemas::process::ProcessStart;
use rustix::io::Errno;
use rustix::process::{
    Pid, Signal, getpgrp, getpid, kill_process_group, test_kill_process, test_kill_process_group,
};

use super::platform::{alive, pid_of};

/// Delivers a signal to a process group. Implemented by `OsSignaller` for production and by a
/// recording fake in tests; no other type sends a signal.
pub trait Signaller: Send + Sync {
    /// Sends `signal` to every member of the process group `group`.
    ///
    /// # Errors
    /// The system's error, `ESRCH` when the group has no member.
    fn signal_group(&self, group: Pid, signal: Signal) -> Result<(), Errno>;
}

/// The signaller of production: `killpg`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct OsSignaller;

impl Signaller for OsSignaller {
    fn signal_group(&self, group: Pid, signal: Signal) -> Result<(), Errno> {
        kill_process_group(group, signal)
    }
}

/// Whether a process with this id exists. A zombie exists until its parent reaps it, and a
/// process of another user exists too: the system answers `EPERM`. Signal 0 is sent; nothing is
/// delivered.
pub(crate) fn process_exists(pid: u32) -> bool {
    let Some(target) = pid_of(pid) else {
        return false;
    };
    // EXPLICIT: signal 0 delivers nothing; the call only asks whether the process exists
    match test_kill_process(target) {
        Ok(()) => true,
        Err(error) => error == Errno::PERM,
    }
}

/// Whether the process group `pgid` has a member. A zombie is a member until its parent reaps
/// it. Signal 0 is sent; nothing is delivered.
pub(crate) fn group_exists(pgid: u32) -> bool {
    let Some(target) = pid_of(pgid) else {
        return false;
    };
    // EXPLICIT: signal 0 delivers nothing; the call only asks whether a member exists
    match test_kill_process_group(target) {
        Ok(()) => true,
        Err(error) => error == Errno::PERM,
    }
}

/// Why a signal was not delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SignalError {
    /// The guard refused the group and sent nothing.
    Refused {
        /// Which rule refused it.
        detail: &'static str,
    },
    /// The group has no member.
    Gone,
    /// The system failed the signal.
    Failed {
        /// The system's error.
        detail: String,
    },
}

impl fmt::Display for SignalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused { detail } => write!(f, "refused: {detail}"),
            Self::Gone => f.write_str("the group has no member"),
            Self::Failed { detail } => write!(f, "failed: {detail}"),
        }
    }
}

/// A child this supervisor spawned: its leader and the leader's start time.
#[derive(Debug, Clone)]
struct Entry {
    pid: u32,
    pgid: u32,
    start: ProcessStart,
}

/// The table of children and the signaller that may signal them.
pub struct Guard {
    // TRAIT_OBJECT: production sends real signals, tests record them
    signaller: Arc<dyn Signaller>,
    table: Mutex<Vec<Entry>>,
    /// The caller's process group and process id, when a test fixes them.
    identity: Option<(u32, u32)>,
    /// What answers "is this leader still the process that was spawned", when a test replaces
    /// the operating system's answer.
    #[cfg(feature = "test-support")]
    verifier: Option<fn(u32, &ProcessStart) -> bool>,
}

impl fmt::Debug for Guard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Guard").finish_non_exhaustive()
    }
}

/// The number of a process id as the unsigned type of this crate, or 0.
fn raw(pid: Pid) -> u32 {
    u32::try_from(pid.as_raw_nonzero().get()).unwrap_or(0)
}

impl Guard {
    /// A guard with an empty table that signals through `signaller`.
    // TRAIT_OBJECT: production sends real signals, tests record them
    pub fn new(signaller: Arc<dyn Signaller>) -> Self {
        Self {
            signaller,
            table: Mutex::new(Vec::new()),
            identity: None,
            #[cfg(feature = "test-support")]
            verifier: None,
        }
    }

    /// The same guard with the caller's process group and process id fixed, so that a test does
    /// not depend on the process it runs in.
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn with_identity(mut self, own_group: u32, own_pid: u32) -> Self {
        self.identity = Some((own_group, own_pid));
        self
    }

    /// The same guard with the answer of the leader check fixed by `verifier`, so that a test can
    /// reach a rule that comes after it, for a process whose start the system will not tell.
    /// For tests only: it replaces the check that a leader is the process that was spawned.
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn with_verifier(mut self, verifier: fn(u32, &ProcessStart) -> bool) -> Self {
        self.verifier = Some(verifier);
        self
    }

    /// Whether the process `pid` is the one that was spawned with `start`.
    #[cfg_attr(
        not(feature = "test-support"),
        allow(
            clippy::unused_self,
            reason = "without the feature no field is read: the answer is the operating system's"
        )
    )]
    fn leader_alive(&self, pid: u32, start: &ProcessStart) -> bool {
        #[cfg(feature = "test-support")]
        if let Some(verifier) = self.verifier {
            return verifier(pid, start);
        }
        alive(pid, start)
    }

    /// The caller's own process group and process id.
    fn own(&self) -> (u32, u32) {
        self.identity
            .unwrap_or_else(|| (raw(getpgrp()), raw(getpid())))
    }

    /// Records a child this supervisor spawned. The child leads the group it was started in, so
    /// its group id is its process id. The table never holds a number that
    /// [`Guard::signal_group`] would refuse on sight: 0, 1, a number that does not fit a process
    /// id, the caller's own process group and the caller's own process id.
    ///
    /// # Errors
    /// [`SignalError::Refused`] for such a number; the table is left as it was.
    pub fn register(&self, pid: u32, start: ProcessStart) -> Result<(), SignalError> {
        if pid_of(pid).is_none() || pid <= 1 {
            return Err(SignalError::Refused {
                detail: "not a process group number a child can lead",
            });
        }
        let (own_group, own_pid) = self.own();
        if pid == own_group || pid == own_pid {
            return Err(SignalError::Refused {
                detail: "the caller's own process group or process id",
            });
        }
        self.insert(Entry {
            pid,
            pgid: pid,
            start,
        });
        Ok(())
    }

    /// Puts `entry` in the table, replacing the one for the same leader.
    fn insert(&self, entry: Entry) {
        let mut table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        table.retain(|held| held.pid != entry.pid);
        table.push(entry);
    }

    /// Puts a leader in the table without the refusals of [`Guard::register`], so that a test can
    /// show each rule of [`Guard::signal_group`] refusing on its own. For tests only: it can put a
    /// number in the table that a real child never has.
    #[cfg(feature = "test-support")]
    pub fn seed_unchecked(&self, pid: u32, start: ProcessStart) {
        self.insert(Entry {
            pid,
            pgid: pid,
            start,
        });
    }

    /// Whether the table holds a group led by `pgid`.
    pub fn holds(&self, pgid: u32) -> bool {
        let table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        table.iter().any(|entry| entry.pgid == pgid)
    }

    /// Sends `signal` to every group in the table, each through [`Guard::signal_group`], and
    /// ignores what comes back: this is how a test cleans up after itself.
    #[cfg(feature = "test-support")]
    pub fn signal_all(&self, signal: Signal) {
        let pgids: Vec<u32> = {
            let table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
            table.iter().map(|entry| entry.pgid).collect()
        };
        for pgid in pgids {
            let _ = self.signal_group(pgid, signal);
        }
    }

    /// Drops a child that has ended.
    pub fn forget(&self, pid: u32) {
        let mut table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        table.retain(|entry| entry.pid != pid);
    }

    /// Sends `signal` to the one process `pid` if its environment carries `marker` in
    /// `CANDLER_RUN`, for a process that left its group. The body refuses everything until the
    /// job runtime fills it, so a missing guard cannot pass for a working one.
    ///
    /// # Errors
    /// [`SignalError::Refused`], always: nothing is sent.
    #[allow(
        clippy::unused_self,
        reason = "the stub refuses without reading the table; the filled body reads it"
    )]
    pub fn signal_marked(
        &self,
        pid: u32,
        start: &ProcessStart,
        marker: &Marker,
        signal: Signal,
    ) -> Result<(), SignalError> {
        // EXPLICIT: the stub reads none of its arguments, since it sends nothing
        let _ = (pid, start, marker, signal);
        Err(SignalError::Refused {
            detail: "signalling a marked process is not available",
        })
    }

    /// Sends `signal` to the one process `pid` that this process adopted as a subreaper, when
    /// its parent is this process and its start time is `start`. The body refuses everything
    /// until the job runtime fills it.
    ///
    /// # Errors
    /// [`SignalError::Refused`], always: nothing is sent.
    #[allow(
        clippy::unused_self,
        reason = "the stub refuses without reading the table; the filled body reads it"
    )]
    pub fn signal_adopted(
        &self,
        pid: u32,
        start: &ProcessStart,
        signal: Signal,
    ) -> Result<(), SignalError> {
        // EXPLICIT: the stub reads none of its arguments, since it sends nothing
        let _ = (pid, start, signal);
        Err(SignalError::Refused {
            detail: "signalling an adopted process is not available",
        })
    }

    /// Sends `signal` to the process group `pgid` if every rule allows it.
    ///
    /// # Errors
    /// [`SignalError::Refused`] when a rule refuses, with nothing sent; [`SignalError::Gone`]
    /// when the group has no member; [`SignalError::Failed`] for any other system error.
    pub fn signal_group(&self, pgid: u32, signal: Signal) -> Result<(), SignalError> {
        let refused = |detail| Err(SignalError::Refused { detail });
        let Some(target) = pid_of(pgid).filter(|_| pgid > 1) else {
            return refused("not a process group number a child can lead");
        };
        let (own_group, own_pid) = self.own();
        if pgid == own_group {
            return refused("the caller's own process group");
        }
        if pgid == own_pid {
            return refused("the caller's own process id");
        }
        let entry = {
            let table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
            table.iter().find(|entry| entry.pgid == pgid).cloned() // BORROW: .cloned() releases the table before the checks
        };
        let Some(entry) = entry else {
            return refused("this supervisor did not spawn the group");
        };
        // The leader is read again: a process id that now names another process is not ours.
        if !self.leader_alive(entry.pid, &entry.start) {
            if process_exists(entry.pid) {
                return refused("the leader's process id now names another process");
            }
            // The leader is gone and reaped. Its id cannot be reused while a member keeps the
            // group, so a group that still has members is the child's.
            // EXPLICIT: one race remains. The last member can exit and the id can be reused
            // between `group_exists` and the signal below; the window is the two calls, and it
            // exists only for a leader that was reaped while its group lived.
            if !group_exists(entry.pgid) {
                return Err(SignalError::Gone);
            }
        }
        match self.signaller.signal_group(target, signal) {
            Ok(()) => Ok(()),
            Err(error) if error == Errno::SRCH => Err(SignalError::Gone),
            Err(error) => Err(SignalError::Failed {
                detail: error.to_string(),
            }),
        }
    }
}
