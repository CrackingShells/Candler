//! The real-Docker guard of the plan's read-only workspace file: the mount list the core builds
//! for it holds the file at the workspace root, and a write to it from inside the container fails.
//!
//! The container carries the label `candler.test=workspace_file` and a name of its own, and is removed by that
//! name when the test ends.
#![cfg(test)]

use std::process::Command;
use std::time::Duration;

use candler_core::isolation::container::mounts::{MountMode, MountSpec};
use candler_core::isolation::model::{
    ContainerPlan, ImageId, ImageRef, Labels, Limits, NetworkName, UserSpec,
};
use candler_core::runner::evidence::Inputs;
use candler_core::runner::exec::{mount_inputs, mounted_inputs};
use candler_core::store::Store;
use candler_harness::spec::PreparedFile;
use candler_schemas::primitives::{JobId, RelPath};

use crate::support::test_image::{TESTAGENT_CONTEXT, TestImage};

/// The label of the container this test starts.
const LABEL: &str = "candler.test=workspace_file";

/// The text of the workspace file.
const TEXT: &str = "be brief";

/// Runs `docker <args>` and returns its output, which the test reads and never trusts.
fn docker(args: &[String]) -> std::process::Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("the docker CLI runs")
}

/// Guards: the workspace file the core mounts reads inside a real container and refuses a write,
/// and neither the host's placeholder nor the blob changes.
/// Oracle: the shell's own exit status and message for an append to the file, and the bytes of
/// the blob read on the host afterwards.
/// Bite: mount the file read-write, or skip the placeholder the mount point needs (docker then
/// fails with `mountpoint ... is outside of rootfs`).
#[test]
#[ignore = "needs a docker daemon and the testagent image; run by the container-e2e group"]
fn workspace_file_reads_and_refuses_a_write_in_docker() {
    let image = TestImage::new(TESTAGENT_CONTEXT);
    let home = std::env::var("HOME").expect("HOME");
    let scratch = tempfile::tempdir_in(home).expect("a scratch directory under HOME");
    let root = scratch.path().canonicalize().expect("a canonical path");
    let store = Store::init(&root.join("store")).expect("a store");
    let (work, agent_home) = (root.join("work"), root.join("home"));
    for dir in [&work, &agent_home] {
        std::fs::create_dir_all(dir).expect("a directory");
    }
    let job: JobId = "00000000-0000-7000-8000-00000000f065"
        .parse()
        .expect("a job id");
    let mut plan = ContainerPlan {
        image: ImageRef {
            id: ImageId(image.id().to_owned()), // BORROW: the plan owns the id
            harness_version: None,
        },
        labels: Labels::new(),
        mounts: vec![
            MountSpec {
                source: work.clone(), // BORROW: the mount owns its path
                target: String::from("/work"),
                mode: MountMode::ReadWrite,
            },
            MountSpec {
                source: agent_home,
                target: String::from("/home/agent"),
                mode: MountMode::ReadWrite,
            },
        ],
        env: Vec::new(),
        gateway: None,
        state: Vec::new(),
        network: NetworkName::for_job(&job),
        egress: None,
        add_hosts: Vec::new(),
        limits: Limits {
            pids_limit: 128,
            memory: String::from("256m"),
            cpus: 1.0,
        },
        record_dir: root.join("records"),
        user: UserSpec::Engine,
        workdir: String::from("/work"),
        home: String::from("/home/agent"),
        expiry: Duration::from_secs(30),
        project_root: root.clone(), // BORROW: the plan owns its path
        store_root: root.clone(),   // BORROW: the plan owns its path
        interactive: false,
    };
    let file = PreparedFile::new(
        "AGENTS.md".parse::<RelPath>().expect("a path"),
        TEXT.as_bytes().to_vec(),
        false,
    );
    let mounted = mounted_inputs(&store, &Inputs::default(), &[file]).expect("the file is stored");
    mount_inputs(&store, &mut plan, &work, &mounted).expect("the mount list");
    let blob = mounted.digests.first().expect("one mounted file");
    let blob_path = store.layout().blob(&blob.digest);

    let name = format!("candler-workspace-file-{}", std::process::id());
    let mut args: Vec<String> = [
        "run",
        "--rm",
        "--network",
        "none",
        "--label",
        LABEL,
        "--name",
    ]
    .map(String::from)
    .to_vec();
    args.push(name.clone()); // BORROW: the removal below reads the name
    for mount in &plan.mounts {
        args.push(String::from("--mount"));
        args.push(mount.to_arg().expect("a mount argument"));
    }
    // Root is not refused by a file's mode, so only the mount's read-only flag can refuse the
    // write. As the image's own user, Linux refuses by mode first ("Permission denied").
    args.extend(
        [
            "--user",
            "0",
            "-w",
            "/work",
            "--entrypoint",
            "sh",
            image.id(),
            "-c",
        ]
        .map(String::from),
    );
    args.push(String::from(
        "cat AGENTS.md; if echo changed >> AGENTS.md; then echo WRITE-OK; else echo WRITE-DENIED; fi",
    ));
    let ran = docker(&args);
    // The container removes itself; a leftover is removed by its name.
    docker(&["rm".to_owned(), "-f".to_owned(), name]);
    let shown = String::from_utf8_lossy(&ran.stdout);
    assert!(
        ran.status.success(),
        "docker run: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(shown.contains(TEXT), "the file reads inside: {shown}");
    assert!(
        shown.contains("WRITE-DENIED"),
        "the write is refused: {shown}"
    );
    // The write ran as root, so the refusal is the mount's, and the shell says so.
    let said = String::from_utf8_lossy(&ran.stderr);
    assert!(
        said.contains("Read-only file system"),
        "the mount is read-only: {said}"
    );
    assert!(
        !shown.contains("WRITE-OK"),
        "the write went through: {shown}"
    );
    assert_eq!(std::fs::read_to_string(&blob_path).expect("the blob"), TEXT);
    assert_eq!(
        std::fs::metadata(work.join("AGENTS.md"))
            .expect("the placeholder")
            .len(),
        0,
        "the host's placeholder is untouched"
    );
    let left = docker(&["ps", "-aq", "--filter", &format!("label={LABEL}")].map(String::from));
    assert!(left.stdout.is_empty(), "no container of the test is left");
}
