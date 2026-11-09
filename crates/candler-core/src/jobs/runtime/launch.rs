//! What the launcher needs to start a job: an id for it, the digest of its spec and the identity
//! of a process.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use candler_schemas::execution::SupervisorIdentity;
use candler_schemas::primitives::{Digest, JobId};
use sha2::Digest as _;

use super::RuntimeError;
use crate::supervise::platform::process_start;

/// Numbers the ids one process mints.
static MINTED: AtomicU64 = AtomicU64::new(0);

/// The digest of a job's spec: `sha256:` and the hex of the SHA-256 of its bytes. It is the one
/// the supervisor checks against `job.json` before it reads the bytes as a spec.
///
/// # Errors
/// [`RuntimeError::SpecDigest`] when the digest cannot be written down.
pub fn spec_digest(bytes: &[u8]) -> Result<Digest, RuntimeError> {
    super::session::digest_of(bytes)
}

/// A new job id: a `UUIDv7` whose time is now, in milliseconds, and whose other bits come from the
/// SHA-256 of the clock to the nanosecond, the process id and a count. The job's directory is
/// made with the id and refuses one that is already there.
///
/// # Errors
/// [`RuntimeError::Detach`] when the clock is before 1970 or the id is malformed.
pub fn mint_job_id() -> Result<JobId, RuntimeError> {
    let fail = |detail: String| RuntimeError::Detach { detail };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| fail(e.to_string()))?;
    let count = MINTED.fetch_add(1, Ordering::Relaxed);
    let mut hasher = sha2::Sha256::new();
    hasher.update(now.as_nanos().to_be_bytes());
    hasher.update(std::process::id().to_be_bytes());
    hasher.update(count.to_be_bytes());
    let bits = hasher
        .finalize()
        .iter()
        .take(10)
        .fold(0_u128, |acc, byte| (acc << 8) | u128::from(*byte))
        >> 6;
    let rand_a = (bits >> 62) & 0xfff;
    let rand_b = bits & ((1_u128 << 62) - 1);
    let millis = now.as_millis();
    let uuid = (millis << 80) | (0x7 << 76) | (rand_a << 64) | (0b10 << 62) | rand_b;
    uuid::Uuid::from_u128(uuid)
        .hyphenated()
        .to_string()
        .parse::<JobId>()
        .map_err(|e| fail(e.to_string()))
}

/// The identity of this process: its id and the time it started.
///
/// # Errors
/// [`RuntimeError::Job`] when the identity is refused.
pub fn own_identity() -> Result<SupervisorIdentity, RuntimeError> {
    let pid = std::process::id();
    SupervisorIdentity::new(pid, process_start(pid)).map_err(|e| RuntimeError::Job {
        detail: e.to_string(),
    })
}
