#![cfg(test)]
//! Guards for the spec file the supervisor keeps in the job directory.

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::runtime::session::{SPEC_FILE, read_spec};
use candler_core::store::Store;
use candler_core::store::layout::StoreLayout;
use candler_schemas::execution::JobState;
use candler_schemas::primitives::Digest;

mod support;

use support::job_orchestrate::spec;
use support::job_store::{job_id, job_record};
use support::run_plan::Facts;

/// The variable that tells the re-exec entry which job directory to read the spec for.
const ENV_DIR: &str = "CANDLER_TEST_SPEC_FILE_DIR";

/// The names the secrets of the spec's experiment and settings go by, whose values the child gets.
const SECRET_NAMES: [&str; 2] = ["TEST_SECRET", "API_KEY"];

/// The re-exec entry of the guard below; it guards nothing itself. It does nothing unless
/// `CANDLER_TEST_SPEC_FILE_DIR` names a job directory: run on its own it is an empty test. As the
/// supervisor does, it reads the spec from standard input and checks it against the job's record.
///
/// Guards: none (re-exec entry of O3).
/// Oracle: none.
/// Bite: none.
#[test]
fn spec_file_child() {
    let Ok(dir) = std::env::var(ENV_DIR) else {
        return;
    };
    read_spec(Path::new(&dir), std::io::stdin()).expect("the spec reads and verifies");
}

/// Guards: O3 — the supervisor's reader keeps the verified spec in `<job dir>/spec.json`, and the
/// file holds each secret by name only: the child that reads the spec has a planted, unique value
/// for each secret in its environment, and no byte of the file holds it. The file is the bytes
/// the launcher sent, so its digest is the one the job record keeps.
///
/// Oracle: the planted values; the bytes the test sent; the digest the test computed for them.
///
/// Bite: writing the resolved environment, or a `secrets` map with values, into the file; writing
/// nothing.
#[test]
fn o3_spec_file_holds_no_secret_value() {
    let scratch = tempfile::tempdir().expect("a directory");
    let root = scratch.path().join("store");
    Store::init(&root).expect("a store");
    let layout = StoreLayout::new(&root);
    let id = job_id(0x5bec);
    let bytes = serde_json::to_vec(&spec(&["m"], &Facts::base(), 3600, 2)).expect("a spec");
    let digest = {
        use sha2::Digest as _;
        use std::fmt::Write as _;
        let hex = sha2::Sha256::digest(&bytes)
            .iter()
            .fold(String::new(), |mut text, byte| {
                write!(text, "{byte:02x}").expect("a string takes text");
                text
            });
        Digest::from_hex(&hex).expect("a digest")
    };
    let mut record = serde_json::to_value(job_record(&id, JobState::Running)).expect("a record");
    record["spec_digest"] = serde_json::Value::String(digest.to_string());
    let record = serde_json::from_value(record).expect("a record");
    let dir = JobDir::create(&layout, id, &record).expect("a job directory");
    assert!(
        !dir.path().join(SPEC_FILE).exists(),
        "no spec before the supervisor reads it"
    );

    let planted = format!(
        "planted-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    );
    let mut command = Command::new(std::env::current_exe().expect("this test binary"));
    command
        .args([
            "--exact",
            "spec_file_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ENV_DIR, dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    for name in SECRET_NAMES {
        command.env(name, &planted);
    }
    let mut child = command.spawn().expect("the child starts");
    child
        .stdin
        .take()
        .expect("a stdin")
        .write_all(&bytes)
        .expect("the spec is sent");
    let output = child.wait_with_output().expect("the child ends");
    assert!(
        output.status.success(),
        "the child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let written = fs::read(dir.path().join(SPEC_FILE)).expect("spec.json exists");
    assert_eq!(written, bytes, "the file is the bytes the launcher sent");
    let body = String::from_utf8_lossy(&written);
    assert!(
        !body.contains(planted.as_str()),
        "a secret value is in spec.json"
    );
    assert!(
        SECRET_NAMES.iter().any(|name| body.contains(name)),
        "a secret appears by name"
    );
}
