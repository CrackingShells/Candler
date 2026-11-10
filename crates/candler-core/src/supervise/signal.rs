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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use candler_schemas::primitives::Marker;
use candler_schemas::process::ProcessStart;
use rustix::io::Errno;
use rustix::process::{
    Pid, Signal, getpgrp, getpid, kill_process, kill_process_group, test_kill_process,
    test_kill_process_group,
};

#[cfg(target_os = "linux")]
use super::platform::process_start;
use super::platform::{alive, pid_of};

/// Delivers a signal to a process group. Implemented by `OsSignaller` for production and by a
/// recording fake in tests; no other type sends a signal.
pub trait Signaller: Send + Sync {
    /// Sends `signal` to every member of the process group `group`.
    ///
    /// # Errors
    /// The system's error, `ESRCH` when the group has no member.
    fn signal_group(&self, group: Pid, signal: Signal) -> Result<(), Errno>;

    /// Sends `signal` to the one process `pid`, never to its group. The default delivers
    /// nothing and answers `ENOSYS`, so a signaller that does not override it cannot signal a
    /// process by accident.
    ///
    /// # Errors
    /// The system's error, `ESRCH` when the process does not exist.
    fn signal_process(&self, pid: Pid, signal: Signal) -> Result<(), Errno> {
        let _ = (pid, signal);
        Err(Errno::NOSYS)
    }
}

/// The signaller of production: `killpg`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct OsSignaller;

impl Signaller for OsSignaller {
    fn signal_group(&self, group: Pid, signal: Signal) -> Result<(), Errno> {
        kill_process_group(group, signal)
    }

    fn signal_process(&self, pid: Pid, signal: Signal) -> Result<(), Errno> {
        kill_process(pid, signal)
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

/// What the operating system says about one process right now: the facts the rules of
/// [`Guard::signal_marked`] and [`Guard::signal_adopted`] are decided from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcFacts {
    /// When the process started.
    pub start: ProcessStart,
    /// The process id of its parent.
    pub parent: u32,
    /// Its process group.
    pub group: u32,
    /// The user that owns it.
    pub uid: u32,
    /// The value of `CANDLER_RUN` in its environment, if it has one and it can be read.
    pub marker: Option<String>,
    /// Its state letter, as `/proc/<pid>/stat` gives it: `Z` for a process that has exited and
    /// has not been waited for.
    pub state: char,
}

/// The marker of one control call, such as one `docker` client run: `control-<pid>-<n>`, with
/// the process id of the caller and a number that no other call of the process shares. A marker
/// shared by two live calls would let the sweep after one of them end the other.
#[must_use]
pub fn control_marker() -> Marker {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    loop {
        let seq = NEXT.fetch_add(1, Ordering::Relaxed);
        let text = format!("control-{}-{seq}", std::process::id());
        // EXPLICIT: the text always fits the grammar of a marker; the loop ends on its first pass
        if let Ok(marker) = text.parse::<Marker>() {
            return marker;
        }
    }
}

/// Where the guard reads the facts of a process from, when a test replaces the operating system.
#[cfg(feature = "test-support")]
// TRAIT_OBJECT: the inspector is the operating system's reader, or a closure in tests
pub type Inspector = Arc<dyn Fn(u32) -> Option<ProcFacts> + Send + Sync>;

/// The name of the variable that carries a run's marker.
#[cfg(target_os = "linux")]
const MARKER_VARIABLE: &str = "CANDLER_RUN";

/// The value of `CANDLER_RUN` in the text of `/proc/<pid>/environ`: entries are separated by NUL
/// and the first entry of the name wins, as `getenv` reads it.
#[cfg(target_os = "linux")]
fn marker_in(environ: &[u8]) -> Option<String> {
    environ.split(|byte| *byte == 0).find_map(|entry| {
        let text = std::str::from_utf8(entry).ok()?;
        let (name, value) = text.split_once('=')?;
        // BORROW: .to_owned() gives the facts their own text
        (name == MARKER_VARIABLE).then(|| value.to_owned())
    })
}

/// The state, the parent and the group of a process, read from the text of its
/// `/proc/<pid>/stat`: fields 3 to 5, which follow the command name that ends at the last `)`.
#[cfg(target_os = "linux")]
fn state_parent_group(stat: &str) -> Option<(char, u32, u32)> {
    let rest = stat.get(stat.rfind(')')?.checked_add(1)?..)?;
    let mut fields = rest.split_ascii_whitespace();
    let state = fields.next()?.chars().next()?;
    let parent = fields.next()?.parse().ok()?;
    let group = fields.next()?.parse().ok()?;
    Some((state, parent, group))
}

/// Reads the facts of `pid` from `/proc`; `None` when the process is gone or not readable.
#[cfg(target_os = "linux")]
pub(crate) fn read_facts(pid: u32) -> Option<ProcFacts> {
    use std::os::unix::fs::MetadataExt;
    let line = std::fs::read(format!("/proc/{pid}/stat")).ok()?;
    let (state, parent, group) = state_parent_group(&String::from_utf8_lossy(&line))?;
    let began = process_start(pid);
    let uid = std::fs::metadata(format!("/proc/{pid}")).ok()?.uid();
    let marker = std::fs::read(format!("/proc/{pid}/environ"))
        .ok()
        .and_then(|environ| marker_in(&environ));
    // The process may have been replaced while it was read: the start is read last again.
    (process_start(pid) == began).then_some(ProcFacts {
        start: began,
        parent,
        group,
        uid,
        marker,
        state,
    })
}

/// Without `/proc` no process can be inspected, so every signal to a process this supervisor did
/// not spawn is refused.
#[cfg(not(target_os = "linux"))]
pub(crate) fn read_facts(pid: u32) -> Option<ProcFacts> {
    let _ = pid;
    None
}

/// Whether `later` is a start after `earlier` of the same host and boot; two starts that cannot
/// be compared are not ordered. The variants are read through their wire form, which is the one
/// place their members are public.
fn started_after(later: &ProcessStart, earlier: &ProcessStart) -> bool {
    let (Ok(later), Ok(earlier)) = (serde_json::to_value(later), serde_json::to_value(earlier))
    else {
        return false;
    };
    let number =
        |value: &serde_json::Value, key: &str| value.get(key).and_then(serde_json::Value::as_u64);
    match (
        later.get("kind").and_then(|kind| kind.as_str()), // BORROW: the kind is matched as text
        earlier.get("kind").and_then(|kind| kind.as_str()), // BORROW: the kind is matched as text
    ) {
        (Some("linux"), Some("linux")) => {
            later.get("boot_id") == earlier.get("boot_id")
                && number(&later, "ticks") > number(&earlier, "ticks")
                && number(&later, "ticks").is_some()
        }
        (Some("darwin"), Some("darwin")) => {
            let key = |value: &serde_json::Value| (number(value, "sec"), number(value, "usec"));
            key(&later).0.is_some() && key(&later) > key(&earlier)
        }
        // Two starts of different kinds, or of a kind this comparison does not know, do not order:
        // a start is never later than one it cannot be compared with.
        _ => false,
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
    /// What answers "what is this process", when a test replaces the operating system's answer.
    #[cfg(feature = "test-support")]
    inspector: Option<Inspector>,
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
            #[cfg(feature = "test-support")]
            inspector: None,
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

    /// The same guard reading the facts of a process from `inspector` instead of `/proc`, so that
    /// a test can show each rule of [`Guard::signal_marked`] and [`Guard::signal_adopted`] on any
    /// host, and change what a process is between two calls. For tests only.
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn with_inspector(mut self, inspector: Inspector) -> Self {
        self.inspector = Some(inspector);
        self
    }

    /// What the operating system says about `pid` now.
    #[cfg_attr(
        not(feature = "test-support"),
        expect(
            clippy::unused_self,
            reason = "without the feature no field is read: the answer is the operating system's"
        )
    )]
    pub(crate) fn facts(&self, pid: u32) -> Option<ProcFacts> {
        #[cfg(feature = "test-support")]
        if let Some(inspector) = &self.inspector {
            return inspector(pid);
        }
        read_facts(pid)
    }

    /// Whether the process `pid` is the one that was spawned with `start`.
    #[cfg_attr(
        not(feature = "test-support"),
        expect(
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
    pub(crate) fn own(&self) -> (u32, u32) {
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

    /// The process id `pid` as a target a stray can be, refusing 0, 1, a number that is no process
    /// id, the caller's own process id and the caller's own process group.
    fn stray_target(&self, pid: u32) -> Result<(Pid, u32, u32), SignalError> {
        let refused = |detail| Err(SignalError::Refused { detail });
        let Some(target) = pid_of(pid).filter(|_| pid > 1) else {
            return refused("not a process id a stray can have");
        };
        let (own_group, own_pid) = self.own();
        if pid == own_pid {
            return refused("the caller's own process id");
        }
        if pid == own_group {
            return refused("the caller's own process group");
        }
        Ok((target, own_group, own_pid))
    }

    /// Sends `signal` to the one process `pid`, for a process that left its group, if its
    /// environment carries exactly `marker` in `CANDLER_RUN`. Everything is read again from the
    /// operating system right before the signal: the process's start time must be `start`, its
    /// marker must be `marker`, and its user must be the caller's. A marker found by an earlier
    /// scan is not enough.
    ///
    /// # Errors
    /// [`SignalError::Refused`] when a rule refuses, with nothing sent; [`SignalError::Gone`] when
    /// the process does not exist; [`SignalError::Failed`] for any other system error.
    pub fn signal_marked(
        &self,
        pid: u32,
        start: &ProcessStart,
        marker: &Marker,
        signal: Signal,
    ) -> Result<(), SignalError> {
        let refused = |detail| Err(SignalError::Refused { detail });
        let (target, _, own_pid) = self.stray_target(pid)?;
        let Some(facts) = self.facts(pid) else {
            return refused("the process cannot be inspected");
        };
        if matches!(start, ProcessStart::Unavailable) || facts.start != *start {
            return refused("the process id now names another process");
        }
        // BORROW: .as_str() compares the marker with the text read from the process
        if facts.marker.as_deref() != Some(marker.as_str()) {
            return refused("the process does not carry the marker");
        }
        let Some(own) = self.facts(own_pid) else {
            return refused("the caller cannot be inspected");
        };
        if facts.uid != own.uid {
            return refused("the process belongs to another user");
        }
        self.deliver_process(target, signal)
    }

    /// Whether `facts` is an orphan this process adopted: its parent is the caller, it started
    /// after the caller, it carries a well-formed marker and it belongs to the caller's user.
    fn adopted(facts: &ProcFacts, own_pid: u32, own: &ProcFacts) -> Result<(), &'static str> {
        if facts.parent != own_pid {
            return Err("the process is not a child of the caller");
        }
        if !started_after(&facts.start, &own.start) {
            return Err("the process did not start after the caller");
        }
        // BORROW: .as_str() reads the marker text for the parse
        if facts
            .marker
            .as_deref()
            .is_none_or(|text| text.parse::<Marker>().is_err())
        {
            return Err("the process carries no marker");
        }
        if facts.uid != own.uid {
            return Err("the process belongs to another user");
        }
        Ok(())
    }

    /// Sends `signal` to the one process `pid` that this process adopted as a subreaper, and to
    /// its process group only when the group's leader passes the same checks. Right before the
    /// signal the process is read again: its start time must be `start` and later than the
    /// caller's own, its parent must be the caller, its environment must carry a well-formed
    /// `CANDLER_RUN`, and its user must be the caller's. A group is signalled only for a leader
    /// that passes, never because one member did. The caller reaps with `waitpid(pid)`.
    ///
    /// # Errors
    /// [`SignalError::Refused`] when a rule refuses, with nothing sent; [`SignalError::Gone`] when
    /// the process does not exist; [`SignalError::Failed`] for any other system error.
    pub fn signal_adopted(
        &self,
        pid: u32,
        start: &ProcessStart,
        signal: Signal,
    ) -> Result<(), SignalError> {
        let refused = |detail| Err(SignalError::Refused { detail });
        let (target, own_group, own_pid) = self.stray_target(pid)?;
        let Some(facts) = self.facts(pid) else {
            return refused("the process cannot be inspected");
        };
        if matches!(start, ProcessStart::Unavailable) || facts.start != *start {
            return refused("the process id now names another process");
        }
        let Some(own) = self.facts(own_pid) else {
            return refused("the caller cannot be inspected");
        };
        if let Err(detail) = Self::adopted(&facts, own_pid, &own) {
            return refused(detail);
        }
        // The group is decided before anything is sent: its leader is read and checked now.
        let group = Some(facts.group)
            .filter(|group| *group > 1 && *group != own_group && *group != own_pid)
            .filter(|group| {
                *group == pid
                    || self
                        .facts(*group)
                        .is_some_and(|leader| Self::adopted(&leader, own_pid, &own).is_ok())
            })
            .and_then(pid_of);
        self.deliver_process(target, signal)?;
        if let Some(group) = group {
            // EXPLICIT: the leader can exit between the check and this signal; the group's
            // members keep its id reserved, and a group that is already empty is not an error
            let _ = self.signaller.signal_group(group, signal);
        }
        Ok(())
    }

    /// Delivers `signal` to the one process `target`, reading the system's answer.
    fn deliver_process(&self, target: Pid, signal: Signal) -> Result<(), SignalError> {
        match self.signaller.signal_process(target, signal) {
            Ok(()) => Ok(()),
            Err(error) if error == Errno::SRCH => Err(SignalError::Gone),
            Err(error) => Err(SignalError::Failed {
                detail: error.to_string(),
            }),
        }
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
