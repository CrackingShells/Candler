//! Guards for the spec file of a job: a run job's file is its `JobSpec` and nothing around it.
#![cfg(test)]

use crate::support;

use candler_core::jobs::model::{JobSpec, JobSpecFile};
use support::job_orchestrate::spec;
use support::run_plan::Facts;

/// Guards: P0 — `JobSpecFile::Run` writes the bytes `JobSpec` writes, so no spec digest moves, and
/// reads them back. Oracle: the bytes of `serde_json::to_vec` of the bare `JobSpec`, the form
/// `candler run` wrote before the enum existed. Bite: an externally tagged enum writes
/// `{"Run": …}` and fails the byte comparison.
#[test]
fn p0_run_spec_file_bytes_equal_the_job_spec() {
    let bare: JobSpec = spec(&["m"], &Facts::base(), 3600, 2);
    let expected = serde_json::to_vec(&bare).expect("the bare spec serialises");
    let file = JobSpecFile::from(bare.clone()); // BORROW: the file owns its spec
    let written = serde_json::to_vec(&file).expect("the file serialises");
    assert_eq!(written, expected, "the file's bytes are the spec's bytes");
    let read: JobSpecFile = serde_json::from_slice(&expected).expect("the spec's bytes read back");
    assert_eq!(
        read,
        JobSpecFile::Run(bare),
        "the round trip keeps the spec"
    );
}
