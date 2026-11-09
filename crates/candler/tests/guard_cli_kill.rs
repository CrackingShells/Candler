//! `candler kill --all` without confirmation makes no docker call.
#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::JobEvents;
use candler_core::store::Store;
use candler_core::store::layout::StoreLayout;
use candler_schemas::execution::{EventPayload, JobRecord, JobState, SupervisorIdentity};
use candler_schemas::primitives::{Digest, JobId};
use candler_schemas::process::ProcessStart;

/// The id of the lost job the test makes: a lowercase hyphenated `UUIDv7`.
const JOB: &str = "01890000-0000-7000-8000-000000000003";

/// The fake client, as the fixture file holds it.
const FAKE: &str = include_str!("fixtures/cli/docker");

/// A fresh directory with a store holding one lost job, a `bin/` holding the fake client first
/// on the path, and an empty call log; returns the directory.
fn world() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "candler-guard-kill-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let fake = bin.join("docker");
    fs::write(&fake, FAKE).unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(bin.join("docker.log"), "").unwrap();
    let store = root.join("store");
    Store::init(&store).unwrap();
    let id: JobId = JOB.parse().unwrap();
    // A supervisor that never held the lock: the job is lost and has no live record.
    let record = JobRecord::builder()
        .id(id.clone()) // BORROW: the record owns its id
        .supervisor(SupervisorIdentity::new(1, ProcessStart::darwin(1, 0)).unwrap())
        .spec_digest(
            format!("sha256:{}", "0".repeat(64))
                .parse::<Digest>()
                .unwrap(),
        )
        .state(JobState::Running)
        .build()
        .unwrap();
    let dir = JobDir::create(&StoreLayout::new(&store), id, &record).unwrap();
    // It had begun: a job whose log has no `job.started` is still starting, not lost.
    JobEvents::open(&dir)
        .unwrap()
        .append(None, EventPayload::job_started())
        .unwrap();
    root
}

/// Runs the real binary on the world with stdin closed.
fn candler(root: &Path, args: &[&str]) -> Output {
    let store = root.join("store");
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .args(args)
        .arg("--store")
        .arg(&store)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", root.join("bin").display()),
        )
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// What the fake client was asked, one call per line.
fn calls(root: &Path) -> String {
    fs::read_to_string(root.join("bin").join("docker.log")).unwrap()
}

/// Guards: B31 — `kill --all` without `-y` and with stdin closed refuses before any docker call.
///
/// Oracle: the fake client's call log, read after the run, and the exit status (2). With `-y`
/// (the control) the exit status is 0 and the log holds the call that lists the job's labelled
/// containers. No argument is taken from the implementation.
///
/// Bite: the `yes` default of `kill::Args` becomes true, so the refusal never happens.
#[test]
fn kill_all_without_confirmation_makes_no_docker_call() {
    let refused = world();
    let out = candler(&refused, &["kill", "--all"]);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("refused"), "{stderr}");
    assert_eq!(calls(&refused), "", "a refused kill made a docker call");

    let control = world();
    let out = candler(&control, &["kill", "--all", "-y"]);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    let log = calls(&control);
    let wanted = format!("--filter label=candler.job={JOB}");
    assert!(
        log.lines()
            .any(|line| line.starts_with("container ls") && line.contains(wanted.as_str())), // BORROW: contains takes the id as text
        "the confirmed kill did not list the job's containers: {log:?}"
    );
}
