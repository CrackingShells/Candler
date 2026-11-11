//! `candler evaluate` as a job: the hidden `supervise` verb runs an evaluate spec, and the verb
//! starts that job, follows it and prints its report. The binary is the real one; the world is the
//! one of `candler run` (a PATH-fake docker client), with the `pool` plug-in of the engine guards.
#![cfg(test)]
#![cfg(unix)]

#[expect(
    dead_code,
    reason = "this guard uses the world and its launch, not every helper of the shared module"
)]
#[path = "support/cli_run.rs"]
mod cli_run;
#[expect(
    dead_code,
    reason = "this guard uses the helper's environment and the job's wait, not every helper"
)]
#[path = "support/seam.rs"]
mod seam;

use std::collections::BTreeSet;
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use candler_core::evaluation::engine::RunSelection;
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::{JobEvents, TailPos};
use candler_core::jobs::lock::{JobLock, LockState};
use candler_core::jobs::model::EvaluateSpec;
use candler_core::jobs::runtime::launch::{mint_job_id, own_identity};
use candler_core::store::layout::StoreLayout;
use candler_schemas::execution::{EventPayload, JobEnd, JobRecord, JobState};
use candler_schemas::primitives::{Digest, JobId, RunKey};
use serde_json::{Value, json};

use cli_run::{World, run_keys, wait_for, world};
use seam::{entries_under, finish, shasum, sync_environment};

/// How long a job that has all it needs may take to end.
const JOB_LIMIT: Duration = Duration::from_secs(120);

/// The fixtures of the engine guards, read in place.
fn engine_fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../candler-core/tests/fixtures/engine")
}

/// The helper's `uv` project, from which the fixture plug-in runs.
fn helper() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../python/candler-evaluator")
}

/// The directory of the `pool` plug-in below the world: its manifest, built from the engine
/// fixture's template, and a link to the helper's project.
fn pool_plugin(world: &World) -> PathBuf {
    let dir = world.root.join("plugin");
    fs::create_dir_all(&dir).unwrap();
    symlink(helper(), dir.join("env")).unwrap();
    let manifest = fs::read_to_string(engine_fixtures().join("plugin_pool.toml"))
        .unwrap()
        .replace(
            "@EXAMPLES@",
            &helper().join("examples").display().to_string(),
        )
        .replace(
            "@KINDS@",
            &engine_fixtures().join("kinds").display().to_string(),
        );
    fs::write(dir.join("plugin.toml"), manifest).unwrap();
    dir
}

/// The fixture suite `name` with every `(token, text)` of `fills` replaced, written below the
/// world; returns its path.
fn suite_of(world: &World, name: &str, fills: &[(&str, String)]) -> PathBuf {
    let mut text = fs::read_to_string(engine_fixtures().join(name)).unwrap();
    for (token, fill) in fills {
        text = text.replace(token, fill);
    }
    let dir = world.root.join("suites");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, text).unwrap();
    path
}

/// A project directory below the world, with a `candler.toml` that lists `dirs` as the plug-in
/// directories; returns it.
fn project(world: &World, name: &str, dirs: &[&Path]) -> PathBuf {
    let dir = world.root.join(name);
    fs::create_dir_all(&dir).unwrap();
    let listed: Vec<String> = dirs
        .iter()
        .map(|dir| format!("\"{}\"", dir.display()))
        .collect();
    fs::write(
        dir.join("candler.toml"),
        format!("[plugins]\ndirs = [{}]\n", listed.join(", ")),
    )
    .unwrap();
    dir
}

/// Writes the user configuration of the world: `text` as its `config.toml`.
fn user_config(world: &World, text: &str) {
    let dir = world.root.join("config").join("candler");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("config.toml"), text).unwrap();
}

/// A `candler` command over the world's store, run from `cwd`, with the environment of the world
/// and nothing else of the caller's but the path to the interpreter's tools.
fn candler(world: &World, cwd: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_candler"));
    command
        .arg("--store")
        .arg(world.store())
        .current_dir(cwd)
        .env_remove("CANDLER_STORE")
        .env("HOME", world.root.join("home"))
        .env("XDG_CONFIG_HOME", world.root.join("config"))
        .env("XDG_STATE_HOME", &world.state)
        .stdin(Stdio::null());
    command
}

/// Runs a world up to a sealed run: the plug-in's environment built, one `candler run` finished.
fn sealed_world() -> World {
    let world = world();
    sync_environment(&world);
    let launched = world.launch();
    finish(&world, &launched);
    world.assert_sealed(1);
    world
}

/// The events of the job `id`, in log order.
fn events_of(world: &World, id: &JobId) -> Vec<EventPayload> {
    let dir = JobDir::open(&StoreLayout::new(&world.store()), id).unwrap();
    JobEvents::open(&dir)
        .unwrap()
        .tail(TailPos(0))
        .unwrap()
        .events
        .iter()
        .map(|envelope| envelope.event().clone())
        .collect()
}

/// The run keys the store holds an outcome directory for, from the directory names.
fn graded(world: &World) -> BTreeSet<String> {
    let dir = StoreLayout::new(&world.store()).outcomes_dir();
    fs::read_dir(dir).map_or_else(
        |_| BTreeSet::new(),
        |read| {
            read.flatten()
                .filter(|entry| entry.path().is_dir())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        },
    )
}

/// The run keys the fake docker was handed, from its call log: the `candler.run=` labels that are
/// run keys (the gateway's container carries one that is not).
fn handed_runs(world: &World) -> BTreeSet<String> {
    run_keys(&world.calls())
        .into_iter()
        .filter(|key| key.parse::<RunKey>().is_ok())
        .collect()
}

/// The outcome files the store holds, at any depth.
fn outcome_files(world: &World) -> BTreeSet<PathBuf> {
    entries_under(&StoreLayout::new(&world.store()).outcomes_dir())
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect()
}

/// Guards: E1 (the binary half), the hidden verb — `candler supervise --job <id>` runs an
/// evaluate spec that a test wrote by hand: it builds no Claude driver, and reads the spec as the
/// kind of job it is. The test seals one run with `candler run`, writes the spec and the job's
/// directory itself, and starts the supervisor with the environment `evaluate` gives it.
///
/// Oracle: the job's `events.jsonl` ends in `job.ended` with `completed`, `report.json` exists,
/// and the store holds an outcome directory for every run the fake docker was handed and for no
/// other.
///
/// Bite: the `Evaluate` arm missing from the verb (it reads the file as a run job's spec, which
/// an evaluate spec is not, and the job never starts).
#[test]
fn e1_supervise_runs_an_evaluate_spec_written_by_hand() {
    let world = sealed_world();
    let plugin = pool_plugin(&world);
    let suite = suite_of(&world, "suite_pool_ok.toml", &[]);
    let spec = EvaluateSpec {
        suite: suite.clone(),
        suite_digest: Digest::from_hex(&shasum(&fs::read(&suite).unwrap())).unwrap(),
        plugin_dirs: vec![plugin],
        env_root: world.state.join("candler").join("environments"),
        selection: RunSelection::All,
        request_deadline_secs: None,
    };
    let bytes = serde_json::to_vec(&spec).unwrap();
    let digest = Digest::from_hex(&shasum(&bytes)).unwrap();
    let id = mint_job_id().unwrap();
    let record = JobRecord::builder()
        .id(id.clone())
        .supervisor(own_identity().unwrap())
        .spec_digest(digest)
        .state(JobState::Running)
        .build()
        .unwrap();
    let layout = StoreLayout::new(&world.store());
    let dir = JobDir::create(&layout, id.clone(), &record).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_candler"))
        .args(["supervise", "--job", &id.to_string()])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", world.root.join("home"))
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CONFIG_HOME", world.root.join("config"))
        .env("CANDLER_STORE_ROOT", world.store())
        .stdin(Stdio::piped())
        .stdout(fs::File::create(world.work.join("supervise.out")).unwrap())
        .stderr(fs::File::create(world.work.join("supervise.err")).unwrap())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&bytes).unwrap();
    // The supervisor exiting ends the wait too: one that refused its spec never writes `job.ended`.
    let ended = wait_for(JOB_LIMIT, || {
        child.try_wait().unwrap().is_some()
            || (events_of(&world, &id)
                .iter()
                .any(|event| matches!(event, EventPayload::JobEnded { .. }))
                && JobLock::probe(&dir).unwrap() == LockState::Free)
    });
    let status = child.wait().unwrap();
    let err = fs::read_to_string(world.work.join("supervise.err")).unwrap_or_default();
    assert!(ended, "the job ends\nstderr: {err}");
    assert!(
        status.success(),
        "the supervisor exits 0: {status:?}\n{err}"
    );

    let events = events_of(&world, &id);
    assert!(
        matches!(
            events.last(),
            Some(EventPayload::JobEnded {
                reason: JobEnd::Completed,
                ..
            })
        ),
        "the log ends in job.ended(completed): {events:?}"
    );
    assert!(dir.path().join("report.json").is_file(), "the report");
    let handed = handed_runs(&world);
    assert_eq!(graded(&world), handed, "an outcome directory per run");
    assert!(!handed.is_empty(), "the run was handed to the fake docker");
    world.remove();
}
