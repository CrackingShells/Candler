//! Helpers for the guards of the job store: builders and fixtures, never expected values.
#![allow(dead_code, reason = "each guard binary uses a subset of the helpers")]

use candler_schemas::container::ContainerRecord;
use candler_schemas::execution::{JobRecord, JobState, RunManifest};
use candler_schemas::primitives::{JobId, RunKey};
use candler_schemas::process::ChildRecord;
use serde_json::{Value, json};

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
    let state = match state {
        JobState::Running => "running",
        _ => "ended",
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

/// A child record of the process `pid` with marker `marker`; its start is unavailable, so it
/// never reads as alive.
pub fn child_record(pid: u32, marker: &str) -> ChildRecord {
    serde_json::from_value(child_json(pid, marker)).expect("a child record")
}

/// The JSON of [`child_record`].
pub fn child_json(pid: u32, marker: &str) -> Value {
    json!({
        "schema": "child-record/1",
        "pid": pid,
        "pgid": pid,
        "start": {"kind": "unavailable"},
        "marker": marker,
        "spawned_at": "2026-11-06T10:00:00Z",
    })
}

/// A container record of `cid` for `job` and the run marker `run`.
pub fn container_record(cid: &str, job: &JobId, run: &str) -> ContainerRecord {
    serde_json::from_value(container_json(cid, job, run)).expect("a container record")
}

/// The JSON of [`container_record`].
pub fn container_json(cid: &str, job: &JobId, run: &str) -> Value {
    json!({
        "schema": "container-record/1",
        "cid": cid,
        "labels": {"job": job.to_string(), "run": run, "experiment": digest_text('0')},
        "created_at": "2026-11-06T10:00:00Z",
        "image_id": digest_text('c'),
        "role": "agent",
        "spawned_at": "2026-11-06T10:00:01Z",
    })
}

/// A manifest of replicate `replicate`, attempt 1, run by `job`, with the extra `measured` values.
pub fn manifest(job: &JobId, replicate: u32, measured: &[Value]) -> RunManifest {
    let body = |cell: &str, replicate: u32| {
        json!({
            "schema": "run-manifest/1",
            "run": {"cell": cell, "replicate": replicate, "attempt": 1},
            "experiment": digest_text('0'),
            "factors": [],
            "comparable": {
                "values": [{"path": "plan.a", "value": 1}],
                "harness": {"name": "h", "version": "1.0.0"},
                "treatment": {"name": "t"},
                "snapshots": [],
                "inputs": [],
                "environments": []
            },
            "sources": [
                {"path": "plan.a", "source": {"kind": "experiment_file"}},
                {"path": "harness", "source": {"kind": "experiment_file"}},
                {"path": "treatment", "source": {"kind": "experiment_file"}}
            ],
            "secrets": [],
            "measured": measured,
            "deviations": [],
            "job": job.to_string(),
            "integrity": {"kind": "valid"}
        })
    };
    let probe: RunManifest =
        serde_json::from_value(body(&digest_text('f'), 1)).expect("a manifest");
    let cell = probe.cell_digest().to_string();
    serde_json::from_value(body(&cell, replicate)).expect("a manifest")
}

/// The key of a manifest.
pub fn key_of(manifest: &RunManifest) -> RunKey {
    manifest.run().clone()
}
