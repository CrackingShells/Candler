//! The expiry that ends a container whose supervisor is lost.
//!
//! The command inside the container runs under `timeout` with short options, which coreutils and
//! busybox both read. Coreutils exits 124 when it expires a command. The inner limit is the run's wall time plus a margin, so the supervisor's
//! own deadline fires first and the inner limit only matters when the supervisor is gone.

use std::time::Duration;

use candler_harness::spec::{CommandSpec, CommandSpecError};

/// How long `timeout` waits after its `TERM` before it sends `KILL`.
const KILL_AFTER_SECS: u64 = 10;

/// The whole seconds of `limit`, rounded up.
#[must_use]
pub const fn ceil_secs(limit: Duration) -> u64 {
    limit.as_secs() + if limit.subsec_nanos() > 0 { 1 } else { 0 }
}

/// The command run under `timeout -s TERM -k 10 <secs>`; its environment overlay
/// and working directory are unchanged.
///
/// # Errors
/// [`CommandSpecError`] when the wrapped command cannot be built, which `cmd` being a valid
/// command rules out.
pub fn wrap_expiry(cmd: &CommandSpec, secs: u64) -> Result<CommandSpec, CommandSpecError> {
    let mut argv = vec![
        String::from("timeout"),
        String::from("-s"),
        String::from("TERM"),
        String::from("-k"),
        KILL_AFTER_SECS.to_string(),
        secs.to_string(),
    ];
    argv.extend(cmd.argv().iter().cloned());
    let mut builder = CommandSpec::builder().argv(argv);
    for entry in cmd.env_overlay() {
        // BORROW: the entry is copied into the wrapped command
        builder = builder.env(entry.clone());
    }
    if let Some(cwd) = cmd.cwd() {
        // BORROW: the directory is copied into the wrapped command
        builder = builder.cwd(cwd.clone());
    }
    builder.build()
}

#[cfg(test)]
mod scaffold {
    use super::*;

    #[test]
    fn seconds_round_up() {
        for (ms, want) in [(0, 0), (1, 1), (1000, 1), (1001, 2), (61_000, 61)] {
            assert_eq!(ceil_secs(Duration::from_millis(ms)), want, "{ms}");
        }
    }

    #[test]
    fn the_command_follows_the_timeout() {
        let cmd = CommandSpec::builder()
            .argv(vec![String::from("sleep"), String::from("1")])
            .build()
            .expect("cmd");
        let wrapped = wrap_expiry(&cmd, 70).expect("wrapped");
        assert_eq!(
            wrapped.argv(),
            ["timeout", "-s", "TERM", "-k", "10", "70", "sleep", "1"]
        );
    }
}
