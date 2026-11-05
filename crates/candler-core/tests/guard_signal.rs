//! Guards for the signal guard: what it refuses, what it lets through and what it reports. This
//! binary never spawns a process: no test here calls `Command` or `Supervisor::spawn`, and the
//! only signaller is a recording fake, so a mutated guard cannot signal anything. Mutations of the
//! guard or of a process or group number are run against this binary alone.
#![cfg(test)]

use std::sync::{Arc, Mutex, PoisonError};

use candler_core::supervise::platform::process_start;
use candler_core::supervise::signal::{Guard, SignalError, Signaller};
use candler_schemas::process::ProcessStart;
use rustix::io::Errno;
use rustix::process::{Pid, Signal, getpgrp, getppid, setpgid};

/// Records each group it is asked to signal, signals nothing and answers `reply`.
struct Fake {
    calls: Mutex<Vec<(i32, i32)>>,
    reply: Result<(), Errno>,
}

impl Signaller for Fake {
    fn signal_group(&self, group: Pid, signal: Signal) -> Result<(), Errno> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((group.as_raw_nonzero().get(), signal.as_raw()));
        self.reply
    }
}

impl Fake {
    fn calls(&self) -> Vec<(i32, i32)> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone() // BORROW: .clone() copies the log out of the lock
    }
}

/// The parent of the test process: a process that exists and that nothing here spawned. Its
/// start is read, so it stands in for a leader whose record is true.
fn parent() -> (u32, ProcessStart) {
    let pid = u32::try_from(getppid().map_or(0, |pid| pid.as_raw_nonzero().get()))
        .expect("a positive pid");
    assert!(
        pid > 1,
        "the test process has no parent to stand in for a leader"
    );
    (pid, process_start(pid))
}

/// A guard over a fresh fake whose caller is group `own_group`, pid `own_pid`.
fn identified_guard(own_group: u32, own_pid: u32) -> (Guard, Arc<Fake>) {
    replying_guard(own_group, own_pid, Ok(()))
}

/// A guard over a fresh fake that answers `reply` to every signal.
fn replying_guard(own_group: u32, own_pid: u32, reply: Result<(), Errno>) -> (Guard, Arc<Fake>) {
    let fake = Arc::new(Fake {
        calls: Mutex::new(Vec::new()),
        reply,
    });
    // TRAIT_OBJECT: the guard takes its signaller as a trait object
    let signaller: Arc<dyn Signaller> = fake.clone(); // BORROW: .clone() shares the fake with the test
    (
        Guard::new(signaller).with_identity(own_group, own_pid),
        fake,
    )
}

/// Guards: I22 — the core never signals a process group or pid it did not spawn and re-verify. Oracle: the fake's call log against the spawn table; POSIX kill(2) for pid <= 1 (`kill(-1, sig)` signals every process the user owns). Nothing here sends a signal.
#[test]
fn the_guard_refuses_what_it_did_not_spawn_and_signals_nothing() {
    let (leader, true_start) = parent();
    let me = std::process::id();
    let my_start = process_start(me);
    let refused = |got: &Result<(), SignalError>| matches!(got, Err(SignalError::Refused { .. }));

    // `register` keeps such numbers out of the table.
    let (guard, fake) = identified_guard(me, leader);
    for (label, number) in [
        ("zero", 0),
        ("init", 1),
        ("minus one as a raw u32", u32::MAX),
        ("a negative number as a raw u32", 0xFFFF_FFF0),
        ("own group", me),
        ("own pid", leader),
    ] {
        let got = guard.register(number, process_start(1));
        assert!(refused(&got), "register accepted {label}: {got:?}");
        assert!(!guard.holds(number), "the table holds {label}");
    }

    // `signal_group` refuses each of them by its own rule, although the table holds them
    // with a start that is true, so that no other rule can be what refuses.
    for (label, number, start) in [
        ("init", 1, my_start.clone()), // BORROW: .clone() seeds one start per row
        ("own group", me, my_start.clone()), // BORROW: .clone() seeds one start per row
        ("own pid", leader, true_start.clone()), // BORROW: .clone() seeds one start per row
    ] {
        // The leader check is told "yes", so that only the rule under test can refuse.
        let (guard, fake) = identified_guard(me, leader);
        let guard = guard.with_verifier(|_, _| true);
        guard.seed_unchecked(number, start);
        let got = guard.signal_group(number, Signal::KILL);
        assert!(refused(&got), "{label}: {got:?}");
        assert_eq!(
            fake.calls(),
            Vec::<(i32, i32)>::new(),
            "{label} was signalled"
        );
    }

    // The raw values that name no group are refused with nothing held at all.
    for (label, number) in [
        ("zero", 0),
        ("minus one as a raw u32", u32::MAX),
        ("a negative number as a raw u32", 0xFFFF_FFF0),
    ] {
        let got = guard.signal_group(number, Signal::KILL);
        assert!(refused(&got), "{label}: {got:?}");
    }
    assert_eq!(fake.calls(), Vec::<(i32, i32)>::new());

    // A group the table never held, and a leader whose pid now names another process.
    let got = guard.signal_group(7_000_003, Signal::KILL);
    assert!(refused(&got), "never held: {got:?}");
    let (guard, fake) = identified_guard(me, 7_000_002);
    guard.seed_unchecked(leader, other_start(&true_start));
    let got = guard.signal_group(leader, Signal::KILL);
    assert!(refused(&got), "recycled pid, different start: {got:?}");
    assert_eq!(fake.calls(), Vec::<(i32, i32)>::new());

    // Another supervisor's table is another table: a group only it spawned, with a true
    // start, is unknown here.
    let (stranger, stranger_fake) = identified_guard(me, 7_000_002);
    let got = stranger.signal_group(leader, Signal::KILL);
    assert!(refused(&got), "{got:?}");
    assert_eq!(stranger_fake.calls(), Vec::<(i32, i32)>::new());
    assert_eq!(
        fake.calls(),
        Vec::<(i32, i32)>::new(),
        "a refused group was signalled"
    );

    // The positive control: a group the table holds with a true start is signalled, once,
    // with exactly that number.
    let (guard, fake) = identified_guard(me, 7_000_002);
    assert_eq!(guard.register(leader, true_start), Ok(()));
    let got = guard.signal_group(leader, Signal::TERM);
    assert_eq!(got, Ok(()));
    assert_eq!(
        fake.calls(),
        vec![(i32::try_from(leader).expect("fits"), Signal::TERM.as_raw())]
    );
}

/// A start that is not the start of the process it was read from.
fn other_start(start: &ProcessStart) -> ProcessStart {
    if matches!(start, ProcessStart::Unavailable) {
        return ProcessStart::Unavailable;
    }
    let mut json = serde_json::to_value(start).expect("JSON");
    let field = if json.get("usec").is_some() {
        "usec"
    } else {
        "ticks"
    };
    let value = json
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .expect("a number");
    if let Some(map) = json.as_object_mut() {
        map.insert(field.to_owned(), serde_json::json!(value.wrapping_add(1)));
    }
    serde_json::from_value(json).expect("a start")
}

/// Guards: I22 — a group with no member reads `Gone` and nothing is sent to it; a signal the system answers `ESRCH` reads `Gone`; any other system error is `Failed`, never `Gone` or a refusal. Oracle: the fake's call log and its scripted answers; POSIX kill(2) for the meaning of `ESRCH` (no such process) and `EPERM`. A process id above the largest the kernels allow (4194304 on Linux) names no process.
#[test]
fn a_group_that_is_gone_reads_gone_and_a_system_error_reads_failed() {
    let (leader, true_start) = parent();
    let me = std::process::id();

    // No process has this id, so the table's group has no member.
    let (guard, fake) = identified_guard(me, 7_000_002);
    assert_eq!(guard.register(7_000_004, true_start.clone()), Ok(())); // BORROW: .clone() seeds one start per row
    assert_eq!(
        guard.signal_group(7_000_004, Signal::KILL),
        Err(SignalError::Gone)
    );
    assert_eq!(
        fake.calls(),
        Vec::<(i32, i32)>::new(),
        "a gone group was signalled"
    );

    // The leader is true and the system says the group vanished between the check and the signal.
    let (guard, fake) = replying_guard(me, 7_000_002, Err(Errno::SRCH));
    assert_eq!(guard.register(leader, true_start.clone()), Ok(())); // BORROW: .clone() seeds one start per row
    assert_eq!(
        guard.signal_group(leader, Signal::TERM),
        Err(SignalError::Gone)
    );
    assert_eq!(fake.calls().len(), 1, "the signal was not attempted");

    // Any other error is the system's, and says so.
    let (guard, fake) = replying_guard(me, 7_000_002, Err(Errno::PERM));
    assert_eq!(guard.register(leader, true_start), Ok(()));
    let got = guard.signal_group(leader, Signal::TERM);
    assert!(
        matches!(&got, Err(SignalError::Failed { detail }) if !detail.is_empty()),
        "{got:?}"
    );
    assert_eq!(fake.calls().len(), 1);
}

/// Guards: I22 and PID reuse — a leader whose process id now names another live process is refused even when a group with that number has members. Oracle: the fake's call log; the test process, made the leader of its own group so that its id is both a live process and a live group, stands in for the other process, and the table holds a start that is not its own.
#[test]
fn a_leader_whose_pid_names_another_live_process_is_refused_though_the_group_lives() {
    let me = std::process::id();
    // A group leader is a live process whose id is also a live group. EPERM means the process
    // already leads a session, and so a group.
    let _ = setpgid(None, None);
    assert_eq!(
        u32::try_from(getpgrp().as_raw_nonzero().get()).expect("a positive group"),
        me,
        "the test process does not lead its own group"
    );
    // The guard's caller is told to be someone else, so that no rule about the caller refuses.
    let (guard, fake) = identified_guard(7_000_001, 7_000_002);
    assert_eq!(guard.register(me, other_start(&process_start(me))), Ok(()));
    let got = guard.signal_group(me, Signal::KILL);
    assert!(
        matches!(got, Err(SignalError::Refused { .. })),
        "a recycled id with a living group: {got:?}"
    );
    assert_eq!(fake.calls(), Vec::<(i32, i32)>::new());
}

/// Guards: I22 — a group the table has forgotten is refused afterwards, and a group still held is not. Oracle: the fake's call log, and the table's own answer to `holds`.
#[test]
fn a_forgotten_group_is_refused_and_a_held_one_is_not() {
    let (leader, start) = parent();
    let me = std::process::id();
    let (guard, fake) = identified_guard(me, 7_000_002);
    assert_eq!(guard.register(leader, start), Ok(()));
    assert!(guard.holds(leader));
    assert_eq!(guard.signal_group(leader, Signal::TERM), Ok(()));
    assert_eq!(fake.calls().len(), 1);

    guard.forget(leader);
    assert!(!guard.holds(leader));
    let got = guard.signal_group(leader, Signal::KILL);
    assert!(
        matches!(got, Err(SignalError::Refused { .. })),
        "a forgotten group: {got:?}"
    );
    assert_eq!(fake.calls().len(), 1, "a forgotten group was signalled");
}
