//! `candler kill`: no docker call without confirmation, and the store every verb shares.
#![cfg(test)]

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
const FAKE: &str = include_str!("../fixtures/cli/docker");

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

/// Guards: B42 — `candler kill <job>` with stdin not a terminal and no `-y` prints nothing on
/// stdout, names the confirm flag on stderr, and reaches no docker client, so the job stays as it
/// was. On a terminal, or with `-y`, the confirmation is unchanged and the stop proceeds.
///
/// Oracle: the fake client's call log (empty), the exit status (2), and the absent stdout. The
/// command prints nothing while it is refused, so a runner that cannot see a prompt is not left
/// waiting on one.
///
/// Bite: `kill` writes the prompt to stdout and reads stdin, so off a terminal the prompt reaches
/// the pipe unchanged.
#[test]
fn kill_off_a_terminal_refuses_before_prompting() {
    let root = world();
    let out = candler(&root, &["kill", JOB]);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(2), "{stdout}{stderr}");
    assert_eq!(
        stdout, "",
        "stdout carried the prompt or the plan: {stdout:?}"
    );
    assert!(
        stderr.contains("-y"),
        "stderr does not name the confirm flag: {stderr}"
    );
    assert!(
        stderr.contains("candler ps"),
        "stderr does not name the command that lists the jobs: {stderr}"
    );
    assert_eq!(calls(&root), "", "a refused kill made a docker call");

    let control = world();
    let out = candler(&control, &["kill", JOB, "-y"]);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(0), "{stderr}");
}

/// Guards: a store that cannot be read is reported as such, whether or not stdin is a terminal:
/// `candler kill --all --store <a regular file>` with stdin closed and no `-y` gives the store's
/// failure and not the "not confirmed" refusal.
///
/// Oracle: the exit status and stderr of the real binary; the control run on the same line with
/// the store that exists gives the "not confirmed" refusal, so the two texts differ by cause.
///
/// Bite: the terminal test runs before the store and the targets are resolved, so the refusal to
/// confirm hides the store's.
#[test]
fn kill_off_a_terminal_reports_the_store_before_the_confirmation() {
    let root = world();
    let missing = root.join("a-file");
    fs::write(&missing, "").unwrap();
    let run = |store: &Path| {
        Command::new(env!("CARGO_BIN_EXE_candler"))
            .args(["kill", "--all", "--store"])
            .arg(store)
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", root.join("bin").display()),
            )
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let out = run(&missing);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(!out.status.success(), "{stderr}");
    assert!(
        !stderr.contains("pass -y"),
        "the store's failure was hidden behind the confirmation: {stderr}"
    );
    let control = run(&root.join("store"));
    let stderr = String::from_utf8_lossy(&control.stderr).into_owned();
    assert!(stderr.contains("pass -y"), "{stderr}");
}

/// Runs the real binary on the world, naming its store as `how` says: `CANDLER_STORE` alone,
/// or `--store` before the verb. The working directory and the state home name no store.
fn candler_naming(root: &Path, how: &str, args: &[&str]) -> Output {
    let store = root.join("store");
    let mut command = Command::new(env!("CARGO_BIN_EXE_candler"));
    command
        .current_dir(root)
        .env("HOME", root)
        .env("XDG_STATE_HOME", root.join("state"))
        .env_remove("CANDLER_STORE");
    match how {
        "env" => {
            command.env("CANDLER_STORE", &store);
        }
        "flag before the verb" => {
            command.arg("--store").arg(&store);
        }
        other => panic!("no such way to name the store: {other}"),
    }
    command
        .args(args)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", root.join("bin").display()),
        )
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// Guards: A36 — every verb reads one store root: `--store` anywhere on the line, else
/// `CANDLER_STORE`, else the configuration. Before A36, `run` and `supervise` asked the
/// configuration only and never saw the global value.
///
/// Oracle: the A36 ruling text, and the fake client's call log: a `kill --all -y` that found the
/// store lists the lost job's labelled containers; one that did not lists nothing, since the
/// working directory and the state home name no store.
///
/// Bite: `kill` calls `common::store_root("kill", None)`, as `run` did before A36 (recorded in
/// the M2a rulings).
#[test]
fn a_job_verb_finds_the_store_named_by_the_env_or_a_leading_flag() {
    for how in ["env", "flag before the verb"] {
        let root = world();
        let out = candler_naming(&root, how, &["kill", "--all", "-y"]);
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(out.status.code(), Some(0), "{how}: {stderr}");
        let log = calls(&root);
        let wanted = format!("--filter label=candler.job={JOB}");
        assert!(
            log.lines()
                .any(|line| line.starts_with("container ls") && line.contains(wanted.as_str())), // BORROW: contains takes the id as text
            "{how}: kill did not find the store: {log:?}"
        );
    }
}
