//! Guards for the signal guard: what it refuses, what it lets through and what it reports. This
//! binary never spawns a process: no test here calls `Command` or `Supervisor::spawn`, and the
//! only signaller is a recording fake, so a mutated guard cannot signal anything. Mutations of the
//! guard or of a process or group number are run against this binary alone.
#![cfg(test)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use candler_core::jobs::runtime::sweep::ProcMarkerSweeper;
use candler_core::supervise::platform::process_start;
use candler_core::supervise::signal::{
    Guard, Inspector, ProcFacts, SignalError, Signaller, control_marker,
};
use candler_core::supervise::stop::StraySweeper;
use candler_schemas::primitives::Marker;
use candler_schemas::process::ProcessStart;
use rustix::io::Errno;
use rustix::process::{Pid, Signal, getppid};

/// Records each group it is asked to signal, signals nothing and answers `reply`.
struct Fake {
    calls: Mutex<Vec<(i32, i32)>>,
    singles: Mutex<Vec<(i32, i32)>>,
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

    fn signal_process(&self, pid: Pid, signal: Signal) -> Result<(), Errno> {
        self.singles
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((pid.as_raw_nonzero().get(), signal.as_raw()));
        self.reply
    }
}

impl Fake {
    fn singles(&self) -> Vec<(i32, i32)> {
        self.singles
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone() // BORROW: .clone() copies the log out of the lock
    }

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
        singles: Mutex::new(Vec::new()),
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
        // BORROW: .to_owned() makes the owned key the map holds
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

/// The verifier of a leader that is not the process that was spawned: whatever the system says.
fn not_the_spawned_process(_pid: u32, _start: &ProcessStart) -> bool {
    false
}

/// Guards: I22 and PID reuse — a leader whose process id now names another live process is refused. Oracle: the fake's call log, empty; the parent of the test process stands in for the other live process (it exists, nothing here spawned it), and the verifier says it is not the process that was spawned. The test process's own group is left as it is.
#[test]
fn a_leader_whose_pid_names_another_live_process_is_refused_though_the_group_lives() {
    let (other, start) = parent();
    // The guard's caller is told to be someone else, so that no rule about the caller refuses.
    let (guard, fake) = identified_guard(7_000_001, 7_000_002);
    let guard = guard.with_verifier(not_the_spawned_process);
    guard.seed_unchecked(other, start);
    let got = guard.signal_group(other, Signal::KILL);
    assert!(
        matches!(got, Err(SignalError::Refused { .. })),
        "a recycled id naming a live process: {got:?}"
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

/// The caller of the rows below, as the fixed identity of a guard.
const OWN_GROUP: u32 = 7_100_001;
/// The caller's process id.
const OWN_PID: u32 = 7_100_002;
/// The caller's user.
const OWN_UID: u32 = 1000;
/// A stray, and a second process that is a member of the stray's group.
const STRAY: u32 = 7_100_010;
/// The leader of a group a stray is a member of.
const LEADER: u32 = 7_100_011;

/// A start on a made-up host, `sec` seconds after its epoch.
fn at(sec: u64) -> ProcessStart {
    ProcessStart::darwin(sec, 0)
}

/// The facts of a process.
fn facts(start: u64, parent: u32, group: u32, uid: u32, marker: Option<&str>) -> ProcFacts {
    ProcFacts {
        start: at(start),
        parent,
        group,
        uid,
        marker: marker.map(str::to_owned), // BORROW: .to_owned() gives the facts their own text
        state: 'S',
    }
}

/// What the made-up operating system holds, which a row changes between two calls.
type Table = Arc<Mutex<BTreeMap<u32, ProcFacts>>>;

/// A guard over a fresh fake whose inspector answers from the returned table, which starts with
/// the caller (started at 100) and nothing else.
fn inspected() -> (Guard, Arc<Fake>, Table) {
    let (guard, fake) = identified_guard(OWN_GROUP, OWN_PID);
    let table: Table = Arc::new(Mutex::new(BTreeMap::new()));
    set(
        &table,
        OWN_PID,
        Some(facts(100, 1, OWN_GROUP, OWN_UID, None)),
    );
    let read = Arc::clone(&table);
    let inspector: Inspector = Arc::new(move |pid| {
        read.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&pid)
            .cloned() // BORROW: .cloned() copies the facts out of the lock
    });
    (guard.with_inspector(inspector), fake, table)
}

/// Puts the facts of `pid` in the table, or removes the process.
fn set(table: &Table, pid: u32, held: Option<ProcFacts>) {
    let mut table = table.lock().unwrap_or_else(PoisonError::into_inner);
    match held {
        Some(held) => {
            table.insert(pid, held);
        }
        None => {
            table.remove(&pid);
        }
    }
}

/// The one refusal reason of a result, or a failed assertion.
fn reason(got: &Result<(), SignalError>) -> &'static str {
    match got {
        Err(SignalError::Refused { detail }) => detail,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// `signal_marked` refuses every process but the verified one, and signals that one alone.
///
/// Guards: I22 — pid, start, marker and user are re-read right before the signal.
/// Oracle: the fake's single and group call logs against a made-up process table.
/// Bite: delete the marker comparison in `signal_marked`.
#[test]
fn signal_marked_refuses_all_but_the_verified_process() {
    let marker: Marker = "run-aaa".parse().expect("a marker");
    let stray_facts = || facts(200, 1, STRAY, OWN_UID, Some("run-aaa"));

    // Every refusal leaves both logs empty.
    let (guard, fake, table) = inspected();
    set(&table, STRAY, Some(stray_facts()));
    for (label, pid, start) in [
        ("zero", 0, at(200)),
        ("init", 1, at(200)),
        ("minus one as a raw u32", u32::MAX, at(200)),
        ("own pid", OWN_PID, at(100)),
        ("own group", OWN_GROUP, at(100)),
        ("nothing at that pid", 7_100_099, at(200)),
        ("a start of another process", STRAY, at(201)),
        ("an unavailable start", STRAY, ProcessStart::Unavailable),
    ] {
        let got = guard.signal_marked(pid, &start, &marker, Signal::KILL);
        assert!(
            matches!(got, Err(SignalError::Refused { .. })),
            "{label}: {got:?}"
        );
    }
    // Forged marker: a prefix, a longer text, another run, none at all.
    for (label, held) in [
        ("prefix of the marker", Some("run-aa")),
        ("the marker with a tail", Some("run-aaab")),
        ("another run", Some("run-bbb")),
        ("no marker", None),
    ] {
        set(&table, STRAY, Some(facts(200, 1, STRAY, OWN_UID, held)));
        let got = guard.signal_marked(STRAY, &at(200), &marker, Signal::KILL);
        assert_eq!(
            reason(&got),
            "the process does not carry the marker",
            "{label}"
        );
    }
    // Another user's process that carries the exact marker.
    set(
        &table,
        STRAY,
        Some(facts(200, 1, STRAY, 0, Some("run-aaa"))),
    );
    let got = guard.signal_marked(STRAY, &at(200), &marker, Signal::KILL);
    assert_eq!(reason(&got), "the process belongs to another user");
    assert_eq!(fake.singles(), Vec::<(i32, i32)>::new());
    assert_eq!(fake.calls(), Vec::<(i32, i32)>::new());

    // Pid reuse: the scan saw start 200, and the id now names a process that started at 300.
    set(
        &table,
        STRAY,
        Some(facts(300, 1, STRAY, OWN_UID, Some("run-aaa"))),
    );
    let got = guard.signal_marked(STRAY, &at(200), &marker, Signal::KILL);
    assert_eq!(reason(&got), "the process id now names another process");

    // The marker vanished between the scan and the signal.
    set(&table, STRAY, Some(stray_facts()));
    assert_eq!(
        guard.signal_marked(STRAY, &at(200), &marker, Signal::TERM),
        Ok(())
    );
    set(&table, STRAY, Some(facts(200, 1, STRAY, OWN_UID, None)));
    let got = guard.signal_marked(STRAY, &at(200), &marker, Signal::KILL);
    assert_eq!(reason(&got), "the process does not carry the marker");

    // The positive control: one signal, to one process, and no group.
    assert_eq!(
        fake.singles(),
        vec![(i32::try_from(STRAY).expect("fits"), Signal::TERM.as_raw())]
    );
    assert_eq!(
        fake.calls(),
        Vec::<(i32, i32)>::new(),
        "a group was signalled"
    );

    // A process that is gone when the system is asked reads `Gone`.
    let (guard, _fake, table) = {
        let (guard, fake) = replying_guard(OWN_GROUP, OWN_PID, Err(Errno::SRCH));
        let (inspecting, _, table) = inspected();
        drop(inspecting);
        let read = Arc::clone(&table);
        let inspector: Inspector = Arc::new(move |pid| {
            read.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&pid)
                .cloned() // BORROW: .cloned() copies the facts out of the lock
        });
        (guard.with_inspector(inspector), fake, table)
    };
    set(&table, STRAY, Some(stray_facts()));
    assert_eq!(
        guard.signal_marked(STRAY, &at(200), &marker, Signal::KILL),
        Err(SignalError::Gone)
    );
}

/// `signal_adopted` refuses every orphan but a verified one and signals its group only when
/// the group's leader passes the same checks.
///
/// Guards: I22 — parent, start, marker and user are re-read right before the signal.
/// Oracle: the fake's single and group call logs against a made-up process table.
/// Bite: drop the parent check in `signal_adopted`.
#[test]
fn signal_adopted_refuses_all_but_a_verified_orphan_and_its_verified_group() {
    let orphan = |start: u64, group: u32| facts(start, OWN_PID, group, OWN_UID, Some("run-aaa"));
    let (guard, fake, table) = inspected();

    for (label, pid, start) in [
        ("zero", 0, at(200)),
        ("init", 1, at(200)),
        ("minus one as a raw u32", u32::MAX, at(200)),
        ("own pid", OWN_PID, at(100)),
        ("own group", OWN_GROUP, at(100)),
        ("nothing at that pid", 7_100_099, at(200)),
    ] {
        let got = guard.signal_adopted(pid, &start, Signal::KILL);
        assert!(
            matches!(got, Err(SignalError::Refused { .. })),
            "{label}: {got:?}"
        );
    }

    // (label, what the process is now, the start the caller read earlier, the reason)
    let rows: [(&str, ProcFacts, ProcessStart, &str); 8] = [
        (
            "pid reuse: the start changed",
            orphan(300, STRAY),
            at(200),
            "the process id now names another process",
        ),
        (
            "an unavailable start",
            orphan(200, STRAY),
            ProcessStart::Unavailable,
            "the process id now names another process",
        ),
        (
            "a parent that is not the caller",
            facts(200, 4242, STRAY, OWN_UID, Some("run-aaa")),
            at(200),
            "the process is not a child of the caller",
        ),
        (
            "started before the caller",
            orphan(100, STRAY),
            at(100),
            "the process did not start after the caller",
        ),
        (
            "started before the caller, by a later clock",
            orphan(50, STRAY),
            at(50),
            "the process did not start after the caller",
        ),
        (
            "reparented without the marker",
            facts(200, OWN_PID, STRAY, OWN_UID, None),
            at(200),
            "the process carries no marker",
        ),
        (
            "a marker that is not one",
            facts(200, OWN_PID, STRAY, OWN_UID, Some("not a marker!")),
            at(200),
            "the process carries no marker",
        ),
        (
            "another user's orphan",
            facts(200, OWN_PID, STRAY, 0, Some("run-aaa")),
            at(200),
            "the process belongs to another user",
        ),
    ];
    for (label, held, start, why) in rows {
        set(&table, STRAY, Some(held));
        let got = guard.signal_adopted(STRAY, &start, Signal::KILL);
        assert_eq!(reason(&got), why, "{label}");
    }
    assert_eq!(fake.singles(), Vec::<(i32, i32)>::new());
    assert_eq!(fake.calls(), Vec::<(i32, i32)>::new());

    // A member that passes, in a group whose leader is not a child of the caller: the member is
    // signalled alone.
    set(&table, STRAY, Some(orphan(200, LEADER)));
    set(
        &table,
        LEADER,
        Some(facts(150, 4242, LEADER, OWN_UID, Some("run-aaa"))),
    );
    assert_eq!(guard.signal_adopted(STRAY, &at(200), Signal::KILL), Ok(()));
    let stray = i32::try_from(STRAY).expect("fits");
    assert_eq!(fake.singles(), vec![(stray, Signal::KILL.as_raw())]);
    assert_eq!(
        fake.calls(),
        Vec::<(i32, i32)>::new(),
        "a member's pass signalled a group"
    );

    // The same group with a leader that fails on the marker, then one that is gone.
    set(
        &table,
        LEADER,
        Some(facts(150, OWN_PID, LEADER, OWN_UID, None)),
    );
    assert_eq!(guard.signal_adopted(STRAY, &at(200), Signal::KILL), Ok(()));
    set(&table, LEADER, None);
    assert_eq!(guard.signal_adopted(STRAY, &at(200), Signal::KILL), Ok(()));
    assert_eq!(
        fake.calls(),
        Vec::<(i32, i32)>::new(),
        "a failed leader's group was signalled"
    );

    // A group in the caller's own group is never signalled, though its leader would pass every
    // other check: only the exclusion refuses it.
    set(&table, STRAY, Some(orphan(200, OWN_GROUP)));
    set(&table, OWN_GROUP, Some(orphan(150, OWN_GROUP)));
    assert_eq!(guard.signal_adopted(STRAY, &at(200), Signal::KILL), Ok(()));
    assert_eq!(
        fake.calls(),
        Vec::<(i32, i32)>::new(),
        "the caller's own group was signalled"
    );
    set(&table, OWN_GROUP, None);

    // The same for a group numbered like the caller's own pid, whose leader reads as a passing
    // orphan the second time it is read.
    set(&table, STRAY, Some(orphan(200, OWN_PID)));
    let first = Arc::new(Mutex::new(true));
    let again = Arc::clone(&table);
    let inspector: Inspector = Arc::new(move |pid| {
        let held = again
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&pid)
            .cloned(); // BORROW: .cloned() copies the facts out of the lock
        if pid == OWN_PID {
            let mut fresh = first.lock().unwrap_or_else(PoisonError::into_inner);
            if !*fresh {
                return Some(orphan(150, OWN_PID));
            }
            *fresh = false;
        }
        held
    });
    let (twice, fake_twice) = identified_guard(OWN_GROUP, OWN_PID);
    let twice = twice.with_inspector(inspector);
    assert_eq!(twice.signal_adopted(STRAY, &at(200), Signal::KILL), Ok(()));
    assert_eq!(
        fake_twice.calls(),
        Vec::<(i32, i32)>::new(),
        "the caller's own pid was signalled as a group"
    );

    // The positive control: the leader passes the same checks, so the group is signalled too.
    set(&table, STRAY, Some(orphan(200, LEADER)));
    set(&table, LEADER, Some(orphan(150, LEADER)));
    assert_eq!(guard.signal_adopted(STRAY, &at(200), Signal::KILL), Ok(()));
    assert_eq!(
        fake.calls(),
        vec![(i32::try_from(LEADER).expect("fits"), Signal::KILL.as_raw())]
    );

    // A leader that is the orphan itself passes by the orphan's own check.
    set(&table, STRAY, Some(orphan(200, STRAY)));
    assert_eq!(guard.signal_adopted(STRAY, &at(200), Signal::TERM), Ok(()));
    assert_eq!(fake.calls().last(), Some(&(stray, Signal::TERM.as_raw())));
}

/// A start on a made-up Linux host: `ticks` since the boot `boot`.
fn linux_at(boot: &str, ticks: u64) -> ProcessStart {
    ProcessStart::linux(boot.parse().expect("a boot id"), ticks)
}

/// An orphan that started before the caller on a Linux host is refused, and one that started
/// after it, in the same boot, is signalled; a start of another boot cannot be ordered.
///
/// Guards: I22 — the order of two Linux starts reads the boot and the ticks.
/// Oracle: the fake's call log against made-up boot ids and ticks.
/// Bite: compare the ticks the wrong way round in `started_after`.
#[test]
fn signal_adopted_orders_linux_starts_by_boot_and_ticks() {
    const BOOT: &str = "5b0fd5a2-1c1e-4d3c-9a38-6f1d0c9e2a11";
    const OTHER: &str = "0a0fd5a2-1c1e-4d3c-9a38-6f1d0c9e2a22";
    let (guard, fake, table) = inspected();
    let own = |start: ProcessStart| ProcFacts {
        start,
        ..facts(100, 1, OWN_GROUP, OWN_UID, None)
    };
    let orphan = |start: ProcessStart| ProcFacts {
        start,
        ..facts(0, OWN_PID, STRAY, OWN_UID, Some("run-aaa"))
    };
    set(&table, OWN_PID, Some(own(linux_at(BOOT, 1_000))));
    for (label, start, ordered) in [
        ("fewer ticks", linux_at(BOOT, 900), false),
        ("the same ticks", linux_at(BOOT, 1_000), false),
        ("another boot", linux_at(OTHER, 5_000), false),
        ("more ticks", linux_at(BOOT, 1_001), true),
    ] {
        set(&table, STRAY, Some(orphan(start.clone()))); // BORROW: .clone() keeps the start for the call
        let got = guard.signal_adopted(STRAY, &start, Signal::KILL);
        if ordered {
            assert_eq!(got, Ok(()), "{label}");
        } else {
            assert_eq!(
                reason(&got),
                "the process did not start after the caller",
                "{label}"
            );
        }
    }
    let stray = i32::try_from(STRAY).expect("fits");
    assert_eq!(
        fake.singles(),
        vec![(stray, Signal::KILL.as_raw())],
        "only the later start was signalled"
    );
}

/// Two live children of the caller, a stray and nothing else: the processes the sweep lists.
const FIRST: u32 = 7_100_020;
/// The second live child, in a group of its own.
const SECOND: u32 = 7_100_021;
/// A process that left every group of the caller and carries the marker of the sweep.
const OUTLAW: u32 = 7_100_022;

/// The processes of the made-up host.
fn listing() -> Vec<u32> {
    vec![FIRST, SECOND, OUTLAW]
}

/// When a control call exits, the sweep that follows it by its own marker signals nothing of
/// another live call of the same supervisor: not because the markers differ, and not when they
/// are the same, because the group of a live child is held.
///
/// Guards: I22 — a sweep never reaches a live child of the caller, and no two control calls share
/// a marker.
/// Oracle: the fake's single and group call logs against a made-up process table.
/// Bite: give `control_marker` a fixed value, or drop the held-group skip in the sweep.
#[test]
fn a_sweep_never_signals_a_live_child_of_the_caller() {
    let (guard, fake, table) = inspected();
    let guard = Arc::new(guard);
    let (first, second) = (control_marker(), control_marker());
    assert_ne!(first, second, "two control calls share a marker");
    assert!(first.as_str().starts_with("control-"));
    let child =
        |pid: u32, marker: &Marker| facts(150, OWN_PID, pid, OWN_UID, Some(marker.as_str()));
    guard.seed_unchecked(FIRST, at(150));
    guard.seed_unchecked(SECOND, at(150));
    let sweeper = ProcMarkerSweeper::new(Arc::clone(&guard)).with_listing(listing);

    // The first call has exited; the second is alive with its own marker.
    set(&table, SECOND, Some(child(SECOND, &second)));
    assert_eq!(sweeper.sweep(&first).len(), 0);

    // The same, with a marker the two calls share: the held group keeps the second safe.
    set(&table, SECOND, Some(child(SECOND, &first)));
    assert_eq!(sweeper.sweep(&first).len(), 0);
    assert_eq!(fake.singles(), Vec::<(i32, i32)>::new());
    assert_eq!(fake.calls(), Vec::<(i32, i32)>::new());

    // The control: a process of no held group that carries the marker is killed, and only it.
    set(
        &table,
        OUTLAW,
        Some(facts(160, 1, OUTLAW, OWN_UID, Some(first.as_str()))),
    );
    let swept = sweeper.sweep(&first);
    assert_eq!(swept.len(), 1);
    let outlaw = i32::try_from(OUTLAW).expect("fits");
    assert_eq!(fake.singles(), vec![(outlaw, Signal::KILL.as_raw())]);
    assert_eq!(fake.calls(), Vec::<(i32, i32)>::new());
}

/// The `.rs` files under `dir`.
fn rust_files(dir: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a directory lists").flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

/// The places in `text` that parse a marker from a string literal starting with `control`:
/// `Marker::from_str("control…")`, `"control…".parse`, or a `&str` constant of that value that
/// `Marker::from_str` or `.parse` takes.
fn control_literals(text: &str) -> Vec<String> {
    let direct =
        regex::Regex::new(r#"from_str\(\s*"control|"control[^"]*"\s*\.parse"#).expect("a pattern");
    let constant =
        regex::Regex::new(r#"const\s+(\w+)\s*:\s*&str\s*=\s*"control"#).expect("a pattern");
    let mut found: Vec<String> = direct
        .find_iter(text)
        .map(|m| m.as_str().to_owned())
        .collect(); // BORROW: .to_owned() keeps the text
    for caps in constant.captures_iter(text) {
        let name = &caps[1];
        let taken = regex::Regex::new(&format!(
            r"from_str\(\s*{name}\b|\b{name}\s*\.parse|parse::<Marker>\(\s*{name}\b"
        ))
        .expect("a pattern");
        if taken.is_match(text) {
            found.push(name.to_owned()); // BORROW: .to_owned() keeps the name
        }
    }
    found
}

/// The control calls of the core carry the marker `control_marker` gives them, never a literal
/// they share.
///
/// Guards: I22 — no marker is parsed from a literal starting with `control` outside `control_marker`.
/// Oracle: the text of the source of the core, and a planted literal that must be found.
/// Bite: put `Marker::from_str(CONTROL_MARKER)` over a `"control"` constant back at one site.
#[test]
fn no_control_call_parses_a_shared_marker_literal() {
    let planted =
        "const CONTROL_MARKER: &str = \"control\";\nlet m = Marker::from_str(CONTROL_MARKER);";
    assert_eq!(
        control_literals(planted).len(),
        1,
        "the scan cannot see a planted literal"
    );
    assert_eq!(
        control_literals("let m = \"control\".parse::<Marker>();").len(),
        1
    );
    assert_eq!(
        control_literals("const CONTROL_ROLE: &str = \"control\";").len(),
        0
    );

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    assert!(files.len() > 50, "the scan read {} files", files.len());
    let mut offending = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).expect("a source file");
        for hit in control_literals(&text) {
            offending.push(format!("{}: {hit}", file.display()));
        }
    }
    assert_eq!(offending, Vec::<String>::new());
}
