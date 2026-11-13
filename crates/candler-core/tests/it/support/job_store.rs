//! Helpers for the guards of the job store: builders and fixtures, never expected values.
use candler_schemas::execution::{JobRecord, JobState};
use candler_schemas::primitives::JobId;
use serde_json::json;

/// The text of a digest made of the character `c`.
pub fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

/// A job id: a version 7 UUID numbered by `n`.
pub fn job_id(n: u64) -> JobId {
    format!("018f2b3c-4d5e-7a6b-8c7d-{n:012x}")
        .parse()
        .expect("a job id")
}

/// A job record of `job` in `state`, written by hand.
pub fn job_record(job: &JobId, state: JobState) -> JobRecord {
    let state = if state == JobState::Running {
        "running"
    } else {
        "ended"
    };
    serde_json::from_value(json!({
        "schema": "job-record/1",
        "id": job.to_string(),
        "supervisor": {"pid": 4242, "start": {"kind": "unavailable"}},
        "spec_digest": digest_text('b'),
        "state": state,
    }))
    .expect("a job record")
}
