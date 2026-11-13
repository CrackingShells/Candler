//! What the operating system says about a process: when it started, and whether it is alive.
//!
//! A process id alone does not name a process, because the system reuses ids. A process is named
//! by its id and its start time. The start time is read from `/proc/<pid>/stat` and the boot
//! identifier on Linux, and from the BSD process information of `libproc` on macOS. Elsewhere, and
//! whenever the system does not answer, the start time is [`ProcessStart::Unavailable`], and a
//! record with no start time never reads as alive.
//!
//! On Linux the start time counts clock ticks (`CLK_TCK`, 100 a second), so two processes that
//! start in the same tick share it. That does not weaken the name against reuse: the kernel hands
//! out ids in turn and gives an id back only after it has gone through the others, which takes
//! far longer than a tick. The start time tells a reused id apart; it does not tell a forged
//! record apart, since a record that copies a live process's true start reads as alive.

use candler_schemas::process::{ChildRecord, ProcessStart};
use rustix::process::Pid;

use super::SuperviseError;
use super::signal::process_exists;

/// Whether the process a record names is still running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[expect(
    clippy::exhaustive_enums,
    reason = "EXHAUSTIVE: a process is running or it is not, and in no third state"
)]
pub enum Liveness {
    /// The process id exists and its start time is the record's.
    Alive,
    /// The process id does not exist, belongs to a later process, or cannot be told apart.
    Gone,
}

/// The process id `raw` as the operating-system type, or `None` for 0 and for a number that does
/// not fit a process id.
pub(crate) fn pid_of(raw: u32) -> Option<Pid> {
    i32::try_from(raw).ok().and_then(Pid::from_raw)
}

/// Reads the start time of the process `pid`: [`ProcessStart::Unavailable`] when the process is
/// gone or the system does not report it.
#[must_use]
pub fn process_start(pid: u32) -> ProcessStart {
    read_start(pid).unwrap_or(ProcessStart::Unavailable)
}

/// Whether the process the record names is still running: its id exists and its start time is
/// the record's. A record or a process without a start time reads as [`Liveness::Gone`].
///
/// A zombie leader reads as alive: its parent has not reaped it, so its process group id is
/// still reserved and no other process can have taken the id.
#[must_use]
pub fn liveness(record: &ChildRecord) -> Liveness {
    if alive(record.pid(), record.start()) {
        Liveness::Alive
    } else {
        Liveness::Gone
    }
}

/// Whether the process `pid` exists and started at `start`. A start of
/// [`ProcessStart::Unavailable`] never matches.
pub(crate) fn alive(pid: u32, start: &ProcessStart) -> bool {
    if matches!(start, ProcessStart::Unavailable) || !process_exists(pid) {
        return false;
    }
    process_start(pid) == *start
}

#[cfg(target_os = "linux")]
fn read_start(pid: u32) -> Option<ProcessStart> {
    let bytes = std::fs::read(format!("/proc/{pid}/stat")).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let ticks = parse_proc_stat(&text).ok()?;
    Some(ProcessStart::linux(boot_id()?, ticks))
}

/// The id of this boot, kept from its first successful read: it holds for the life of the
/// process. A failed read is not kept, so the next start tries again.
#[cfg(target_os = "linux")]
fn boot_id() -> Option<candler_schemas::primitives::BootId> {
    use candler_schemas::primitives::BootId;
    static BOOT_ID: std::sync::OnceLock<BootId> = std::sync::OnceLock::new();
    if let Some(id) = BOOT_ID.get() {
        return Some(id.clone()); // BORROW: each start owns its boot id
    }
    let id: BootId = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(BOOT_ID.get_or_init(|| id).clone()) // BORROW: each start owns its boot id
}

#[cfg(target_os = "macos")]
fn read_start(pid: u32) -> Option<ProcessStart> {
    use libproc::bsd_info::BSDInfo;
    use libproc::proc_pid::pidinfo;
    let raw = i32::try_from(pid).ok()?;
    let info = pidinfo::<BSDInfo>(raw, 0).ok()?;
    let usec = u32::try_from(info.pbi_start_tvusec).ok()?;
    Some(ProcessStart::darwin(info.pbi_start_tvsec, usec))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn read_start(_pid: u32) -> Option<ProcessStart> {
    None
}

/// The fields of a process that Candler reads from `/proc/<pid>/stat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_structs,
    reason = "EXHAUSTIVE: these five fields are the ones the line is read for; a field added later is a new reader"
)]
pub struct ProcStat {
    /// Field 1, the process id.
    pub pid: u32,
    /// Field 3, the state letter (`R`, `S`, `Z`, ...).
    pub state: char,
    /// Field 4, the parent's process id.
    pub parent: u32,
    /// Field 5, the process group id.
    pub group: u32,
    /// Field 22, the start time in clock ticks since boot.
    pub start_ticks: u64,
}

impl ProcStat {
    /// Reads the process id, state, parent, group and start time from the text of
    /// `/proc/<pid>/stat`.
    ///
    /// The second field, the command name, sits between parentheses and may itself hold spaces and
    /// parentheses, so the fields that follow it start after the last `)` of the line; every one
    /// of them is a number or a letter.
    ///
    /// # Errors
    /// Returns [`SuperviseError::ProcStat`] if the line has no process id before the command name,
    /// no `(` or `)` around it, fewer than 22 fields, a state that is not one letter, or a
    /// parent, group or start time that is not a number.
    pub fn parse(line: &str) -> Result<Self, SuperviseError> {
        let malformed = |detail: &str| SuperviseError::ProcStat {
            // BORROW: .to_owned() keeps the description in the error
            detail: detail.to_owned(),
        };
        let open = line
            .find('(')
            .ok_or_else(|| malformed("no `(` opens the command name"))?;
        let close = line
            .rfind(')')
            .ok_or_else(|| malformed("no `)` closes the command name"))?;
        if close < open {
            return Err(malformed("the command name is closed before it opens"));
        }
        let leading = line
            .get(..open)
            .ok_or_else(|| malformed("the line does not split at the command name"))?;
        let pid = leading
            .trim()
            .parse::<u32>()
            .map_err(|_| malformed("the process id before the command name is not a number"))?;
        let rest = line
            .get(close.saturating_add(1)..)
            .ok_or_else(|| malformed("the line does not split after the command name"))?;
        // Field 3 is the first after the command name, so field 22 is the 20th (index 19).
        let mut fields = rest.split_ascii_whitespace();
        let mut letters = fields
            .next()
            .ok_or_else(|| malformed("the line has fewer than 22 fields"))?
            .chars();
        let state = letters
            .next()
            .filter(|letter| letter.is_ascii_alphabetic() && letters.next().is_none())
            .ok_or_else(|| malformed("the state is not one letter"))?;
        let mut number = |what: &str| -> Result<u64, SuperviseError> {
            fields
                .next()
                .ok_or_else(|| malformed("the line has fewer than 22 fields"))?
                .parse::<u64>()
                .map_err(|_| malformed(what))
        };
        let parent = u32::try_from(number("field 4 (the parent) is not a number")?)
            .map_err(|_| malformed("field 4 (the parent) is not a process id"))?;
        let group = u32::try_from(number("field 5 (the group) is not a number")?)
            .map_err(|_| malformed("field 5 (the group) is not a process id"))?;
        // Fields 6 to 21 sit between the group and the start time.
        let start_ticks = fields
            .nth(16)
            .ok_or_else(|| malformed("the line has fewer than 22 fields"))?
            .parse::<u64>()
            .map_err(|_| malformed("field 22 is not a number"))?;
        Ok(Self {
            pid,
            state,
            parent,
            group,
            start_ticks,
        })
    }
}

/// Reads field 22 (`starttime`, in clock ticks since boot) from the text of `/proc/<pid>/stat`.
///
/// # Errors
/// Returns [`SuperviseError::ProcStat`] as [`ProcStat::parse`] does.
pub fn parse_proc_stat(line: &str) -> Result<u64, SuperviseError> {
    ProcStat::parse(line).map(|stat| stat.start_ticks)
}

/// The fields of a process that Candler reads from `/proc/<pid>/status`, a file of `key:\tvalue`
/// lines that no command name can shift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_structs,
    reason = "EXHAUSTIVE: these three keys are the ones the file is read for"
)]
pub struct ProcStatus {
    /// The `Pid:` line.
    pub pid: u32,
    /// The `PPid:` line.
    pub parent: u32,
    /// The leftmost number of the `NSpgid:` line, which is the group in the namespace `/proc` was
    /// mounted in; `None` on a kernel older than 4.1, which has no such line.
    pub group: Option<u32>,
}

impl ProcStatus {
    /// Reads the process id, the parent and the group from the text of `/proc/<pid>/status`: a
    /// key is the text before the first `:` of a line and must be `Pid`, `PPid` or `NSpgid`
    /// exactly, and the first line of a key wins. `None` when `Pid:` or `PPid:` is missing or
    /// holds no number, or when `NSpgid:` holds no number.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (mut pid, mut parent, mut group) = (None, None, None);
        for line in text.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let first = value.split_ascii_whitespace().next();
            match key {
                "Pid" if pid.is_none() => pid = Some(first?.parse::<u32>().ok()?),
                "PPid" if parent.is_none() => parent = Some(first?.parse::<u32>().ok()?),
                "NSpgid" if group.is_none() => group = Some(first?.parse::<u32>().ok()?),
                // EXPLICIT: every other line, and a repeated key, carries nothing read here
                _ => {}
            }
        }
        Some(Self {
            pid: pid?,
            parent: parent?,
            group,
        })
    }
}

/// The two files of `/proc/<pid>` that [`read_proc_checked`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_enums,
    reason = "EXHAUSTIVE: the cross-check reads these two files and no third"
)]
pub enum ProcFile {
    /// `/proc/<pid>/stat`.
    Stat,
    /// `/proc/<pid>/status`.
    Status,
}

/// How many pairs of `stat` and `status` are read before a disagreement stands. An orphan
/// reparented between the two reads of a pair disagrees with itself once; the next pair agrees.
pub const CROSS_CHECK_READS: u32 = 3;

/// Reads `pid` through `read`, and answers only when `/proc/<pid>/stat` and `/proc/<pid>/status`
/// say the same about it: both name `pid`, and the parent agrees and, where `status` has one, the
/// group agrees. On a disagreement the pair is read again, up to [`CROSS_CHECK_READS`] pairs;
/// `None` when the last pair still disagrees, when a file cannot be read, or when a file is not
/// what it should be.
#[must_use]
// TRAIT_OBJECT: the reader is `/proc`, or a script of lines in tests
pub fn read_proc_checked(
    pid: u32,
    read: &dyn Fn(u32, ProcFile) -> Option<String>,
) -> Option<ProcStat> {
    for _ in 0..CROSS_CHECK_READS {
        let stat = ProcStat::parse(&read(pid, ProcFile::Stat)?).ok()?;
        let status = ProcStatus::parse(&read(pid, ProcFile::Status)?)?;
        if stat.pid != pid || status.pid != pid {
            return None;
        }
        if stat.parent == status.parent && status.group.is_none_or(|group| group == stat.group) {
            return Some(stat);
        }
    }
    None
}

/// Reads the process, group, parent and state of `pid` from `/proc`, accepted only when `stat`
/// and `status` agree (see [`read_proc_checked`]); `None` when the process is gone, cannot be
/// read, or its two sources disagree. `None` on a host without `/proc`.
#[must_use]
pub fn read_proc_stat(pid: u32) -> Option<ProcStat> {
    #[cfg(target_os = "linux")]
    {
        read_proc_checked(pid, &|pid, file| {
            let name = match file {
                ProcFile::Stat => "stat",
                ProcFile::Status => "status",
            };
            let bytes = std::fs::read(format!("/proc/{pid}/{name}")).ok()?;
            Some(String::from_utf8_lossy(&bytes).into_owned())
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

#[cfg(test)]
mod guards {
    use super::parse_proc_stat;

    /// The 20 fields that follow the command name, with `start` as the last (field 22).
    fn tail(start: &str) -> String {
        format!(
            "S 1 4242 4242 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 {start} 1048576 256 18446744073709551615"
        )
    }

    /// Guards: PID reuse (the start time of a Linux record) — field 22 is read after the last `)`, whatever `comm` holds. Oracle: literal lines written by hand, each with its start time chosen before the line was built.
    #[test]
    fn proc_stat_reads_field_22_after_the_last_paren() {
        let table: [(&str, &str, u64); 5] = [
            ("plain", "4242 (sleep) ", 123_456),
            ("space", "4242 (tmux: server) ", 7),
            ("paren pair", "4242 (a) (b) ", 99),
            (
                "fake tail",
                "4242 (x) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19) ",
                18_446_744_073_709_551_615,
            ),
            ("final paren", "4242 (weird)) ", 0),
        ];
        for (label, head, start) in table {
            let line = format!("{head}{}", tail(&start.to_string()));
            assert_eq!(parse_proc_stat(&line).ok(), Some(start), "{label}: {line}");
        }
    }

    /// Guards: PID reuse (the start time of a Linux record) — a line without a command name, a process id, 22 fields or a numeric start time is refused. Oracle: literal lines, each missing one part.
    #[test]
    fn proc_stat_refuses_a_line_that_is_not_a_stat_line() {
        let table: [(&str, String); 6] = [
            ("empty", String::new()),
            ("no paren", "4242 sleep S 1".to_owned()),
            ("no close", "4242 (sleep S 1".to_owned()),
            ("no pid", format!("(sleep) {}", tail("5"))),
            ("short", "4242 (sleep) S 1 2 3".to_owned()),
            ("not a number", format!("4242 (sleep) {}", tail("x5"))),
        ];
        for (label, line) in table {
            assert!(parse_proc_stat(&line).is_err(), "{label}: {line}");
        }
    }
}
