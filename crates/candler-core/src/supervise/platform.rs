//! What the operating system says about a process: when it started, and whether it is alive.
//!
//! A process id alone does not name a process, because the system reuses ids. A process is named
//! by its id and its start time. The start time is read from `/proc/<pid>/stat` and the boot
//! identifier on Linux, and from the BSD process information of `libproc` on macOS. Elsewhere, and
//! whenever the system does not answer, the start time is [`ProcessStart::Unavailable`], and a
//! record with no start time never reads as alive.

use candler_schemas::process::{ChildRecord, ProcessStart};
use rustix::process::Pid;

use super::SuperviseError;
use super::signal::process_exists;

/// Whether the process a record names is still running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(clippy::exhaustive_enums)] // EXHAUSTIVE: a process is running or it is not, and in no third state
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
    use candler_schemas::primitives::BootId;
    let bytes = std::fs::read(format!("/proc/{pid}/stat")).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let ticks = parse_proc_stat(&text).ok()?;
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
    let boot_id = boot.trim().parse::<BootId>().ok()?;
    Some(ProcessStart::linux(boot_id, ticks))
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

/// Reads field 22 (`starttime`, in clock ticks since boot) from the text of `/proc/<pid>/stat`.
///
/// The second field, the command name, sits between parentheses and may itself hold spaces and
/// parentheses, so the fields that follow it start after the last `)` of the line.
///
/// # Errors
/// Returns [`SuperviseError::ProcStat`] if the line has no process id before the command name,
/// no `)`, fewer than 22 fields, or a start time that is not a number.
pub fn parse_proc_stat(line: &str) -> Result<u64, SuperviseError> {
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
    if leading.trim().parse::<u32>().is_err() {
        return Err(malformed(
            "the process id before the command name is not a number",
        ));
    }
    let rest = line
        .get(close.saturating_add(1)..)
        .ok_or_else(|| malformed("the line does not split after the command name"))?;
    // Field 3 is the first after the command name, so field 22 is the 20th (index 19).
    let field = rest
        .split_ascii_whitespace()
        .nth(19)
        .ok_or_else(|| malformed("the line has fewer than 22 fields"))?;
    field
        .parse::<u64>()
        .map_err(|_| malformed("field 22 is not a number"))
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
