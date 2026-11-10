#![cfg(test)]
//! Guards for the record of a run's agent container: the isolation writes it from the run's
//! cidfile through the job's recorder, and every destroy path then finds the container recorded.
//! The docker half is the recording fake of `test-support`; no test reaches docker.

mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use candler_core::isolation::container::DockerIsolation;
use candler_core::isolation::container::guard::ContainerGuard;
use candler_core::isolation::driver::DockerDriver;
use candler_core::isolation::gateway::ChildRecorder;
use candler_core::isolation::model::{
    ContainerPlan, GatewayHandle, ImageId, ImageRef, Inspect, InspectState, Labels, Limits,
    NetworkName, UserSpec,
};
use candler_core::isolation::testing::{RecordingDestroyer, RecordingDocker};
use candler_core::isolation::{Isolation as _, IsolationError};
use candler_core::jobs::dir::JobDir;
use candler_schemas::execution::JobState;
use candler_schemas::primitives::{ContainerId, JobId, RunKey, Timestamp};
use serde_json::Value;
use support::container_stop::{cid, container_ref, digest, inspect, job, labels, run_key};
use support::job_store::job_record;
use support::run_plan::fresh_store;

/// The sha256 digest the scripted inspect gives as the agent's image id.
const IMAGE_ID: &str = "sha256:5555555555555555555555555555555555555555555555555555555555555555";

/// The creation time the scripted inspect gives, as docker prints it.
const CREATED: &str = "2026-03-04T05:06:07.123456789Z";

/// The directory of this crate.
fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A job directory in a fresh store.
fn job_dir(job: &JobId) -> (tempfile::TempDir, JobDir) {
    let (dir, store, _lock) = fresh_store();
    let created = JobDir::create(
        store.layout(),
        job.clone(), // BORROW: the directory owns a copy of the id
        &job_record(job, JobState::Running),
    )
    .expect("a job directory");
    (dir, created)
}

/// The labels docker reports for an agent container of `job` and the run `key`.
fn agent_labels(job: &JobId, key: &RunKey) -> Labels {
    labels(job, "agent", Some(key)).with("candler.experiment", &digest(9).to_string())
}

/// What docker answers for the agent `id` carrying `labels`.
fn agent_inspect(id: &ContainerId, labels: Labels) -> Inspect {
    let mut answer = inspect(id, labels);
    answer.image = ImageId(IMAGE_ID.to_owned()); // BORROW: .to_owned() gives the answer its own text
    answer.created = CREATED.parse::<Timestamp>().expect("a time");
    answer.state = InspectState::new(
        "running".to_owned(), // BORROW: .to_owned() gives the state its own text
        true,
        0,
        false,
        None,
        None,
    );
    answer
}

/// The plan of a run of `job`, with its cidfile under `dir`.
fn plan_in(dir: &Path, job: &JobId) -> ContainerPlan {
    ContainerPlan {
        image: ImageRef {
            id: ImageId(IMAGE_ID.to_owned()), // BORROW: .to_owned() gives the image its own text
            cc_version: None,
        },
        labels: Labels::new()
            .with("candler.job", job.as_str()) // BORROW: explicit .as_str()
            .with("candler.role", "agent"),
        mounts: Vec::new(),
        env: Vec::new(),
        network: NetworkName::for_job(job),
        egress: None,
        limits: Limits {
            pids_limit: 64,
            memory: String::from("1g"),
            cpus: 1.0,
        },
        cidfile: dir.join("agent.cid"),
        user: UserSpec::Engine,
        workdir: String::from("/work"),
        home: String::from("/home/agent"),
        expiry: Duration::from_secs(10),
        project_root: dir.to_path_buf(),
        store_root: dir.to_path_buf(),
    }
}

/// An isolation over the recording `docker` that records through `dir`.
fn isolation_over(docker: RecordingDocker, dir: &JobDir, job: &JobId) -> DockerIsolation {
    let read: Arc<dyn DockerDriver> = Arc::new(docker); // TRAIT_OBJECT: the isolation reads through the driver trait, a recording fake here
    let guard = ContainerGuard::new(
        Arc::new(RecordingDestroyer::new()),
        Arc::clone(&read),
        Arc::new(dir.clone()), // BORROW: the guard's view owns its directory handle
    );
    let gateway = GatewayHandle::new(
        container_ref(&inspect(&cid(0xfff), labels(job, "gateway", None))),
        "http://127.0.0.1:1".parse().expect("a url"),
        NetworkName::for_job(job),
        NetworkName::egress_for_job(job),
        String::from("dummy"),
    )
    .expect("a handle");
    let recorder: Arc<dyn ChildRecorder> = Arc::new(dir.clone()); // BORROW: the recorder owns its directory handle; TRAIT_OBJECT: the isolation takes the recorder as a trait object
    DockerIsolation::new(gateway, read, Arc::new(guard)).with_recorder(recorder)
}

/// The files of `children/` of `dir`, by name.
fn children_of(dir: &JobDir) -> BTreeSet<String> {
    fs::read_dir(dir.children_dir())
        .expect("the children directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| Path::new(name).extension().is_some_and(|ext| ext == "json"))
        .collect()
}

/// Guards: R01 §Process supervision — the agent's container is recorded in the job's `children/`
/// as a `container-record/1` from its own inspect: its role, creation time, image and labels.
/// Oracle: the scripted inspect's values as literals, and the `container-record/1` schema file's
/// member names and version marker.
/// Bite: set the role of the built record to `ContainerRole::Gateway` in
/// `DockerIsolation::agent_record`.
#[tokio::test]
async fn record_container_writes_the_agents_record() {
    let current = job(1);
    let key = run_key(3);
    let id = cid(0xabc);
    let (_keep, dir) = job_dir(&current);
    let work = tempfile::tempdir().expect("a temp dir");
    let plan = plan_in(work.path(), &current);
    fs::write(&plan.cidfile, format!("{}\n", id.as_str())).expect("a cidfile"); // BORROW: explicit .as_str()
    let docker = RecordingDocker::new().with_inspect(
        id.as_str(), // BORROW: explicit .as_str()
        agent_inspect(&id, agent_labels(&current, &key)),
    );
    let isolation = isolation_over(docker, &dir, &current);

    let recorded = isolation
        .record_container(&plan, &key)
        .await
        .expect("a record");
    assert_eq!(recorded, Some(id.clone())); // BORROW: .clone() keeps the id for the path

    let short = &id.as_str()[..12]; // INDEX: the id is 64 hex digits
    let path = dir.children_dir().join(format!("container-{short}.json"));
    let written: Value =
        serde_json::from_slice(&fs::read(&path).expect("the record file")).expect("record JSON");
    let schema: Value = serde_json::from_slice(
        &fs::read(here().join("../candler-schemas/schema/container-record.schema.json"))
            .expect("the schema file"),
    )
    .expect("schema JSON");
    let members = |value: &Value| -> BTreeSet<String> {
        value
            .as_object()
            .expect("an object")
            .keys()
            .cloned() // BORROW: the set owns its names
            .collect()
    };
    assert_eq!(members(&written), members(&schema["properties"]));
    assert_eq!(written["schema"], schema["properties"]["schema"]["const"]);
    assert_eq!(written["role"], "agent");
    assert_eq!(written["cid"], id.as_str()); // BORROW: explicit .as_str()
    assert_eq!(written["created_at"], CREATED);
    assert_eq!(written["image_id"], IMAGE_ID);
    assert_eq!(written["labels"]["job"], current.as_str()); // BORROW: explicit .as_str()
    assert_eq!(written["labels"]["run"], key.to_string());
    assert_eq!(written["labels"]["experiment"], digest(9).to_string());
    assert_eq!(children_of(&dir).len(), 1);
}

/// Guards: R01 §Process supervision — recording is idempotent: a second call leaves the record
/// file byte-identical and returns the same id.
/// Oracle: the file's bytes before and after the second call; the spawn time is read from the
/// clock at each build, so a second write would differ.
/// Bite: drop the `recorded` set check at the top of `DockerIsolation::record_container`.
#[tokio::test]
async fn record_container_twice_leaves_one_identical_file() {
    let current = job(1);
    let key = run_key(3);
    let id = cid(0xabc);
    let (_keep, dir) = job_dir(&current);
    let work = tempfile::tempdir().expect("a temp dir");
    let plan = plan_in(work.path(), &current);
    fs::write(&plan.cidfile, id.as_str()).expect("a cidfile"); // BORROW: explicit .as_str()
    let docker = RecordingDocker::new().with_inspect(
        id.as_str(), // BORROW: explicit .as_str()
        agent_inspect(&id, agent_labels(&current, &key)),
    );
    let isolation = isolation_over(docker, &dir, &current);
    let short = &id.as_str()[..12]; // INDEX: the id is 64 hex digits
    let path = dir.children_dir().join(format!("container-{short}.json"));

    let first = isolation.record_container(&plan, &key).await;
    let before = fs::read(&path).expect("the record file");
    std::thread::sleep(Duration::from_millis(20));
    let second = isolation.record_container(&plan, &key).await;
    let after = fs::read(&path).expect("the record file");

    assert_eq!(first, Ok(Some(id.clone()))); // BORROW: .clone() compares with the id
    assert_eq!(second, first);
    assert_eq!(after, before, "the second call rewrote the record");
    assert_eq!(children_of(&dir).len(), 1);
}

/// Guards: R01 §Process supervision — a cidfile that is absent, or that docker cannot inspect
/// yet, records nothing and answers `Ok(None)`; a container of another job or run is refused.
/// Oracle: the children directory's listing, and the error variant.
/// Bite: return `Ok(Some(cid))` before the inspect in `DockerIsolation::record_container`.
#[tokio::test]
async fn record_container_writes_nothing_without_a_container() {
    let current = job(1);
    let key = run_key(3);
    let id = cid(0xabc);
    let (_keep, dir) = job_dir(&current);
    let work = tempfile::tempdir().expect("a temp dir");
    let plan = plan_in(work.path(), &current);

    // No cidfile: the client has not written it.
    let isolation = isolation_over(RecordingDocker::new(), &dir, &current);
    assert_eq!(isolation.record_container(&plan, &key).await, Ok(None));
    assert!(children_of(&dir).is_empty());

    // A cidfile whose container docker cannot inspect.
    fs::write(&plan.cidfile, id.as_str()).expect("a cidfile"); // BORROW: explicit .as_str()
    assert_eq!(isolation.record_container(&plan, &key).await, Ok(None));
    assert!(children_of(&dir).is_empty());

    // A container of another job, and of another run, is refused and recorded nowhere.
    for (case, labels) in [
        ("another job", agent_labels(&job(2), &key)),
        ("another run", agent_labels(&current, &run_key(4))),
    ] {
        let docker = RecordingDocker::new().with_inspect(
            id.as_str(), // BORROW: explicit .as_str()
            agent_inspect(&id, labels),
        );
        let refusing = isolation_over(docker, &dir, &current);
        let outcome = refusing.record_container(&plan, &key).await;
        assert!(
            matches!(outcome, Err(IsolationError::Plan { .. })),
            "{case}: {outcome:?}"
        );
        assert!(children_of(&dir).is_empty(), "{case}");
    }
}
