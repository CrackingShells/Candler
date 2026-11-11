//! Guards for the one reader of `/proc/<pid>/stat` and `/proc/<pid>/status`: which fields it reads
//! whatever the command name holds, and when it refuses a process whose two sources disagree.
//! This binary is separate from `guard_signal`, which promises never to spawn: one row here runs
//! a real process, so that the kernel, and not the test, writes the line.
#![cfg(test)]

use std::cell::Cell;

use candler_core::supervise::platform::{
    CROSS_CHECK_READS, ProcFile, ProcStat, ProcStatus, read_proc_checked,
};

/// The 20 fields that follow the command name (fields 3 to 22), from the state to `start`.
fn tail(state: &str, parent: u32, group: u32, start: u64) -> String {
    format!(
        "{state} {parent} {group} 4242 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 {start} 1048576 256 18446744073709551615"
    )
}

/// Guards: I22 — the parent, the group and the state of a process are read from the right
/// fields whatever its command name holds. Oracle: literal lines, each row's pid, state, parent,
/// group and start ticks chosen before the line was built.
/// Bite: `find(')')` in place of `rfind(')')`; a field index off by one.
#[test]
fn s4_proc_stat_reads_every_field_for_any_command_name() {
    let names: [(&str, &str); 7] = [
        ("plain", "sleep"),
        ("close then fields", "a) S 1 1 (b"),
        ("spaces", "tmux: server"),
        ("open parens", "(("),
        ("close parens", "))"),
        ("fifteen bytes", "abcdefghijklmno"),
        ("empty", ""),
    ];
    // (state, parent, group, start): distinct numbers, so a field read from the wrong place shows.
    let facts: [(char, u32, u32, u64); 3] = [
        ('S', 4_111, 4_222, 123_456),
        ('R', 7, 9, 0),
        ('Z', 1, 1, 18_446_744_073_709_551_615),
    ];
    for (label, name) in names {
        for (state, parent, group, start) in facts {
            let line = format!(
                "4242 ({name}) {}",
                tail(&state.to_string(), parent, group, start)
            );
            let stat = ProcStat::parse(&line).unwrap_or_else(|error| panic!("{label}: {error}"));
            assert_eq!(
                stat,
                ProcStat {
                    pid: 4242,
                    state,
                    parent,
                    group,
                    start_ticks: start
                },
                "{label}: {line}"
            );
        }
    }
    // A command name that holds a newline: the kernel escapes few bytes, so it is read as given.
    let line = format!("4242 (a\nb) {}", tail("S", 11, 12, 13));
    let stat = ProcStat::parse(&line).expect("a name with a newline");
    assert_eq!((stat.parent, stat.group, stat.start_ticks), (11, 12, 13));
}

/// Guards: I22 — a line that is not a `stat` line is refused, never read as a process. Oracle:
/// literal lines, each missing or breaking one part.
/// Bite: drop the pid check; read a line of fewer than 22 fields; skip the number parse of the parent.
#[test]
fn s4_proc_stat_refuses_a_line_that_is_not_a_stat_line() {
    let table: [(&str, String); 9] = [
        ("empty", String::new()),
        ("no open", format!("4242 sleep) {}", tail("S", 1, 1, 5))),
        ("no close", "4242 (sleep S 1 1".to_owned()),
        (
            "close before open",
            format!("4242 )sleep( {}", tail("S", 1, 1, 5)),
        ),
        ("no pid", format!("(sleep) {}", tail("S", 1, 1, 5))),
        (
            "parent not a number",
            format!(
                "4242 (sleep) {}",
                tail("S", 1, 1, 5).replacen(" 1 ", " x ", 1)
            ),
        ),
        (
            "state not a letter",
            format!("4242 (sleep) {}", tail("5", 1, 1, 5)),
        ),
        ("short", "4242 (sleep) S 1 1 4242".to_owned()),
        (
            "start not a number",
            format!(
                "4242 (sleep) {}",
                tail("S", 1, 1, 5).replace(" 5 1048576", " x5 1048576")
            ),
        ),
    ];
    for (label, line) in table {
        assert!(ProcStat::parse(&line).is_err(), "{label}: {line}");
    }
}

/// Guards: I22 — `status` is read by key, so no command name shifts a field. Oracle: literal
/// `key:\tvalue` texts.
/// Bite: take the last number of `NSpgid`; match a key by `contains`; treat a missing `NSpgid` as 0.
#[test]
fn s4_proc_status_reads_ppid_and_the_leftmost_nspgid_by_key() {
    let read = |text: &str| ProcStatus::parse(text);
    assert_eq!(
        read("Name:\tsleep\nPid:\t4242\nPPid:\t77\nNSpgid:\t88\t3\t1\n"),
        Some(ProcStatus {
            pid: 4242,
            parent: 77,
            group: Some(88)
        }),
        "two values: the leftmost"
    );
    assert_eq!(
        read("Name:\tsleep\nPid:\t4242\nPPid:\t77\n"),
        Some(ProcStatus {
            pid: 4242,
            parent: 77,
            group: None
        }),
        "no NSpgid: no group, and the parent is still read"
    );
    assert_eq!(
        read("Name:\tPPid:\t5\nPid:\t4242\nPPid:\t77\nNSpgid:\t88\n"),
        Some(ProcStatus {
            pid: 4242,
            parent: 77,
            group: Some(88)
        }),
        "a Name value that holds a key is not the key"
    );
    assert_eq!(
        read("Name:\tx\nPid:\t4242\nXPPid:\t5\nNSpgid:\t88\n"),
        None,
        "only the exact key counts, so there is no PPid at all"
    );
    assert_eq!(read(""), None, "empty");
    assert_eq!(
        read("Pid:\t4242\nPPid:\tx\n"),
        None,
        "a parent that is not a number"
    );
}

/// A reader of pairs for the cross-check: pair `n` is `script[n]` as (`stat` parent, `status`
/// parent); it counts the files asked for and fails the test past `limit` pairs.
struct Pairs {
    script: Vec<(u32, u32)>,
    asked: Cell<u32>,
    limit: u32,
}

impl Pairs {
    fn read(&self, pid: u32, file: ProcFile) -> String {
        let asked = self.asked.get();
        self.asked.set(asked + 1);
        let pair = usize::try_from(asked / 2).expect("a small index");
        assert!(
            asked / 2 < self.limit,
            "the reader was asked past {} pairs",
            self.limit
        );
        let (stat_parent, status_parent) = *self
            .script
            .get(pair)
            .or_else(|| self.script.last())
            .expect("a pair");
        match file {
            ProcFile::Stat => format!("{pid} (sleep) {}", tail("S", stat_parent, 4242, 5)),
            ProcFile::Status => format!("Pid:\t{pid}\nPPid:\t{status_parent}\nNSpgid:\t4242\n"),
        }
    }
}

/// Guards: I22 — a parent that `stat` and `status` do not agree on is refused, after a bounded
/// number of re-reads. Oracle: a reader whose `stat` says parent 5 and `status` says 6 on every read.
/// Bite: skip the cross-check; cut the loop to one read (the clearing row goes red); leave the loop
/// unbounded (this row panics past its limit).
#[test]
fn s4_cross_check_refuses_when_stat_and_status_disagree_on_every_read() {
    let reader = Pairs {
        script: vec![(5, 6)],
        asked: Cell::new(0),
        limit: CROSS_CHECK_READS + 5,
    };
    let answer = read_proc_checked(4242, &|pid, file| Some(reader.read(pid, file)));
    assert_eq!(answer, None);
    assert_eq!(
        reader.asked.get(),
        2 * CROSS_CHECK_READS,
        "exactly CROSS_CHECK_READS pairs"
    );
}

/// Guards: I22 — an orphan reparented between the two reads is not refused for it. Oracle: a reader
/// whose first pair disagrees (parent 5, then 6) and whose second agrees on 6.
/// Bite: cut the re-read loop to one read (the disagreement is refused).
#[test]
fn s4_cross_check_accepts_a_disagreement_that_clears_on_a_reread() {
    let reader = Pairs {
        script: vec![(5, 6), (6, 6)],
        asked: Cell::new(0),
        limit: CROSS_CHECK_READS + 5,
    };
    let answer = read_proc_checked(4242, &|pid, file| Some(reader.read(pid, file)));
    assert_eq!(answer.map(|stat| stat.parent), Some(6));
    assert_eq!(reader.asked.get(), 4, "two pairs");
}

/// The two files of a process as `read_proc_checked` asks for them: `stat` of process `stat_pid`
/// with its parent and group, and `status` of process `status_pid` with its parent and group.
fn pair(
    (stat_pid, stat_group): (u32, u32),
    (status_pid, status_group): (u32, u32),
    parent: u32,
    file: ProcFile,
) -> String {
    match file {
        ProcFile::Stat => format!("{stat_pid} (sleep) {}", tail("S", parent, stat_group, 5)),
        ProcFile::Status => {
            format!("Pid:\t{status_pid}\nPPid:\t{parent}\nNSpgid:\t{status_group}\n")
        }
    }
}

/// Guards: I22 — a group that `stat` and `status` do not agree on is refused although the parent
/// agrees, after a bounded number of re-reads. Oracle: a reader whose `stat` says group 4242 and
/// `status` says 9999 on every read, both saying parent 5.
/// Bite: delete the group term (`status.group.is_none_or(..)`) of the cross-check.
#[test]
fn s4_cross_check_refuses_a_group_that_stat_and_status_disagree_on() {
    let asked = Cell::new(0_u32);
    let answer = read_proc_checked(4242, &|pid, file| {
        asked.set(asked.get() + 1);
        Some(pair((pid, 4242), (pid, 9999), 5, file))
    });
    assert_eq!(answer, None);
    assert_eq!(
        asked.get(),
        2 * CROSS_CHECK_READS,
        "exactly CROSS_CHECK_READS pairs"
    );
}

/// Guards: I22 — an orphan whose group changes between the two reads is not refused for it.
/// Oracle: a reader whose first pair disagrees on the group (4242, then 9999) and whose second
/// agrees on 9999, the parent being 5 throughout.
/// Bite: delete the group term of the cross-check (the first pair is accepted with group 4242).
#[test]
fn s4_cross_check_accepts_a_group_disagreement_that_clears_on_a_reread() {
    let asked = Cell::new(0_u32);
    let answer = read_proc_checked(4242, &|pid, file| {
        let n = asked.get();
        asked.set(n + 1);
        // The first pair: stat 4242, status 9999. After it: both 9999.
        let stat_group = if n < 2 { 4242 } else { 9999 };
        Some(pair((pid, stat_group), (pid, 9999), 5, file))
    });
    assert_eq!(
        answer.map(|stat| (stat.parent, stat.group)),
        Some((5, 9999))
    );
    assert_eq!(asked.get(), 4, "two pairs");
}

/// Guards: I22 — files that name another process than the one asked for are refused, not read.
/// Oracle: readers whose `stat` names pid 4243, or whose `status` does, for the pid 4242 asked,
/// every other field agreeing.
/// Bite: replace the pid check (`stat.pid != pid || status.pid != pid`) with `if false`.
#[test]
fn s4_cross_check_refuses_files_that_name_another_pid() {
    let wrong_stat = read_proc_checked(4242, &|_, file| {
        Some(pair((4243, 4242), (4242, 4242), 5, file))
    });
    assert_eq!(wrong_stat, None, "stat of another pid");
    let wrong_status = read_proc_checked(4242, &|_, file| {
        Some(pair((4242, 4242), (4243, 4242), 5, file))
    });
    assert_eq!(wrong_status, None, "status of another pid");
    let right = read_proc_checked(4242, &|_, file| {
        Some(pair((4242, 4242), (4242, 4242), 5, file))
    });
    assert_eq!(
        right.map(|stat| stat.pid),
        Some(4242),
        "the control: both name 4242"
    );
}

/// Guards: I22 — a process whose `status` cannot be read, or is not a status text, is refused.
/// Oracle: readers whose `status` is absent, is empty or holds no `PPid:`, beside a good `stat`.
/// Bite: answer from `stat` alone when `status` cannot be read or parsed.
#[test]
fn s4_cross_check_refuses_when_status_cannot_be_read_or_parsed() {
    let with_status = |status: Option<&'static str>| {
        read_proc_checked(4242, &|_, file| match file {
            ProcFile::Stat => Some(pair((4242, 4242), (4242, 4242), 5, file)),
            ProcFile::Status => status.map(str::to_owned), // BORROW: .to_owned() makes the owned text
        })
    };
    assert_eq!(with_status(None), None, "status cannot be read");
    assert_eq!(with_status(Some("")), None, "status is empty");
    assert_eq!(
        with_status(Some("Pid:\t4242\nNSpgid:\t4242\n")),
        None,
        "no PPid line"
    );
    assert_eq!(
        with_status(Some("Pid:\t4242\nPPid:\tx\n")),
        None,
        "PPid is not a number"
    );
    assert!(
        with_status(Some("Pid:\t4242\nPPid:\t5\nNSpgid:\t4242\n")).is_some(),
        "the control: a good status is accepted"
    );
}

/// Guards: I22 — the parent and the group of a real process are read right whatever its command
/// name holds, the kernel writing the line. Oracle: this test's pid and group, which are the
/// child's true parent and group.
/// Bite: `find(')')` in place of `rfind(')')`; a field index off by one; the `status` cross-check
/// skipped does not show here (both sources agree on a live process).
#[cfg(target_os = "linux")]
#[test]
fn s4_a_real_process_with_a_hostile_name_reads_back_its_true_parent() {
    use std::os::unix::fs::symlink;
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    use candler_core::supervise::platform::read_proc_stat;

    let dir = tempfile::tempdir().expect("a temp dir");
    let hostile = dir.path().join("x) S 1 1 (y");
    symlink("/bin/sleep", &hostile).expect("a symlink");
    let mut child = Command::new(&hostile)
        .arg0("sleep")
        .arg("1")
        .spawn()
        .expect("a child");
    let pid = child.id();
    let stat = read_proc_stat(pid).expect("the child's stat");
    let own_group = rustix::process::getpgrp().as_raw_nonzero().get();
    assert_eq!(stat.pid, pid);
    assert_eq!(stat.parent, std::process::id());
    assert_eq!(i32::try_from(stat.group).expect("a pid"), own_group);
    child.wait().expect("the child ends by itself");
}
