//! Development tests of the job verbs; deleted before the verbs land.
#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::JobEvents;
use candler_core::jobs::lock::JobLock;
use candler_core::store::Store;
use candler_core::store::layout::StoreLayout;
use candler_schemas::execution::{EventPayload, JobEnd, JobRecord, JobState, SupervisorIdentity};
use candler_schemas::primitives::{Digest, JobId};
use candler_schemas::process::ProcessStart;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "candler-scaffold-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn job(store: &Path, n: u8) -> JobDir {
    let id: JobId = format!("01890000-0000-7000-8000-0000000000{n:02x}")
        .parse()
        .unwrap();
    let record = JobRecord::builder()
        .id(id.clone())
        .supervisor(SupervisorIdentity::new(1, ProcessStart::darwin(1, 0)).unwrap())
        .spec_digest(
            format!("sha256:{}", "0".repeat(64))
                .parse::<Digest>()
                .unwrap(),
        )
        .state(JobState::Running)
        .build()
        .unwrap();
    JobDir::create(&StoreLayout::new(store), id, &record).unwrap()
}

/// A directory that holds a `docker` that logs its arguments and prints nothing.
fn fake_docker(dir: &Path) -> PathBuf {
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let script = bin.join("docker");
    fs::write(&script, "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"${0%/*}/docker.log\"\ncase \"$1\" in network|image) echo 'Error: No such object' >&2; exit 1;; esac\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

fn candler(args: &[&str], bin: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .args(args)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn ps_shows_running_ended_and_lost() {
    let root = scratch("ps");
    let store = root.join("store");
    Store::init(&store).unwrap();
    let running = job(&store, 1);
    let _held = JobLock::acquire(&running).unwrap();
    let ended = job(&store, 2);
    JobEvents::open(&ended)
        .unwrap()
        .append(None, EventPayload::job_ended(JobEnd::Completed))
        .unwrap();
    let _lost = job(&store, 3);
    let bin = fake_docker(&root);
    let out = candler(&["ps", "--store", store.to_str().unwrap()], &bin);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text} / {err}");
    // INDEX: the length was asserted above
    assert!(lines[0].contains("running"), "{text}");
    // INDEX: the length was asserted above
    assert!(lines[1].contains("ended (completed)"), "{text}");
    // INDEX: the length was asserted above
    assert!(lines[2].contains("ended (failed)"), "{text} / {err}");
    assert!(out.status.success(), "{err}");
}

fn with_stdin(args: &[&str], bin: &Path, input: &str) -> std::process::Output {
    use std::io::Write as _;
    let mut child = Command::new(env!("CARGO_BIN_EXE_candler"))
        .args(args)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn kill_asks_and_a_lost_job_is_recovered() {
    let root = scratch("kill");
    let store = root.join("store");
    Store::init(&store).unwrap();
    let lost = job(&store, 3);
    let bin = fake_docker(&root);
    let path = store.to_str().unwrap();
    let log = root.join("bin").join("docker.log");
    for input in ["", "\n", "maybe\n"] {
        let out = with_stdin(&["kill", "--all", "--store", path], &bin, input);
        assert_eq!(out.status.code(), Some(2), "{input:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("refused: pass -y to confirm"));
        assert!(!log.exists());
    }
    let out = with_stdin(&["kill", "--all", "--store", path], &bin, "yes\n");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(log.exists());
    let ended = candler_core::jobs::lost::classify(&lost).unwrap();
    assert!(matches!(
        ended,
        candler_core::jobs::lost::JobLiveness::Ended(_)
    ));
    let again = with_stdin(&["kill", "--all", "-y", "--store", path], &bin, "");
    assert!(again.status.success());
}
