//! Reaping what the supervisor killed: a wait on one process id at a time.

use std::time::{Duration, Instant};

use candler_schemas::execution::StrayProcess;
use rustix::io::Errno;
use rustix::process::{WaitOptions, waitpid};

use crate::supervise::platform::{pid_of, process_start};

/// The longest a killed process is waited for.
pub const REAP_BOUND: Duration = Duration::from_secs(2);
/// The pause between two looks at a process that has not yet exited.
const POLL: Duration = Duration::from_millis(10);

/// Waits, without blocking, for each of `pids` that is a child of this process, so that a killed
/// stray never stays a zombie. Every wait names one process id, never "any child". A process
/// whose id now names another start is not waited for, and a process that is not this process's
/// child is left to its parent. Returns how many were reaped.
#[must_use]
pub fn reap_by_pid(pids: &[StrayProcess]) -> usize {
    let started = Instant::now();
    let mut reaped = 0;
    for stray in pids {
        let Some(pid) = pid_of(stray.pid()) else {
            continue;
        };
        loop {
            let now = process_start(stray.pid());
            if &now != stray.start()
                && !matches!(now, candler_schemas::process::ProcessStart::Unavailable)
            {
                break;
            }
            match waitpid(Some(pid), WaitOptions::NOHANG) {
                Ok(Some(_)) => {
                    reaped += 1;
                    break;
                }
                // EXPLICIT: the child has not exited yet; the loop polls again until the bound
                Ok(None) => {}
                Err(Errno::INTR) => continue,
                Err(_) => break,
            }
            if started.elapsed() >= REAP_BOUND {
                break;
            }
            std::thread::sleep(POLL);
        }
    }
    reaped
}

/// Waits, without blocking, for the one process `pid`, which must be a child of this process that
/// has exited. Returns whether it was reaped.
#[must_use]
pub fn reap_one(pid: u32) -> bool {
    let Some(pid) = pid_of(pid) else {
        return false;
    };
    loop {
        match waitpid(Some(pid), WaitOptions::NOHANG) {
            Ok(Some(_)) => return true,
            // EXPLICIT: an interrupted wait is retried by the loop
            Err(Errno::INTR) => {}
            Ok(None) | Err(_) => return false,
        }
    }
}
