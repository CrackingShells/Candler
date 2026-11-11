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
/// directories, as written; returns it.
fn project(world: &World, name: &str, dirs: &[&str]) -> PathBuf {
    let dir = world.root.join(name);
    fs::create_dir_all(&dir).unwrap();
    let listed: Vec<String> = dirs.iter().map(|dir| format!("\"{dir}\"")).collect();
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
/// Returns the world and the id of the run job.
fn sealed_world() -> (World, JobId) {
    let world = world();
    sync_environment(&world);
    let launched = world.launch();
    let job = finish(&world, &launched);
    world.assert_sealed(1);
    (world, job)
}

/// Adds a level to the world's factor and runs the experiment again: a second job whose runs are
/// none of the first job's. Returns the job.
fn second_job(world: &World) -> JobId {
    let text = fs::read_to_string(&world.experiment).unwrap();
    let before = "levels = [\"fixture-model\"]";
    assert!(text.contains(before), "the experiment names its level");
    fs::write(
        &world.experiment,
        text.replace(before, "levels = [\"fixture-model\", \"second-model\"]"),
    )
    .unwrap();
    let launched = world.launch();
    finish(world, &launched)
}

/// The text of a command's standard error.
fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The outcome files of each run directory of the store, by run key.
fn files_by_run(world: &World) -> std::collections::BTreeMap<String, BTreeSet<PathBuf>> {
    let mut by_run: std::collections::BTreeMap<String, BTreeSet<PathBuf>> = graded(world)
        .into_iter()
        .map(|key| (key, BTreeSet::new()))
        .collect();
    let base = StoreLayout::new(&world.store()).outcomes_dir();
    for file in outcome_files(world) {
        let key = file
            .strip_prefix(&base)
            .unwrap()
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_string_lossy()
            .into_owned();
        by_run.entry(key).or_default().insert(file);
    }
    by_run
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
    let (world, _) = sealed_world();
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

/// Guards: E4 — `evaluate <suite>`, with no `--plugin`, takes the plug-ins from the project's
/// `plugins.dirs` and grades by default the runs with no complete outcome set. Two jobs of
/// `candler run` leave three sealed runs; `evaluate` runs from `project/`, whose `candler.toml`
/// names `../plugin`. Rows: (1) the report has no failed request and the store holds an outcome
/// directory for exactly the run keys the fake docker was handed; (2) a second `evaluate` of the
/// same suite writes no file and reports no request; (3) `evaluate <suite> <job-id>` grades the
/// runs of that job and no other, `--runs '<glob>'` grades the runs the glob names and no other,
/// and the two forms together are refused by the parser with exit 2.
///
/// Oracle: the run keys of the fake docker's call log (never the engine's), taken per job as the
/// keys the log gained; the store's outcome files, listed by run before and after.
///
/// Bite: a default selection of every run (row 2 grades again), of none (row 1 finds nothing), or
/// a `<job-id>` that selects every run (row 3 grades the other job's runs).
#[test]
fn e4_evaluate_with_no_plugin_option_grades_exactly_the_runs_just_run() {
    let (world, first) = sealed_world();
    let first_runs = handed_runs(&world);
    let second = second_job(&world);
    let handed = handed_runs(&world);
    let second_runs: BTreeSet<String> = handed.difference(&first_runs).cloned().collect();
    assert!(!second_runs.is_empty(), "the second job ran something new");
    let plugin = pool_plugin(&world);
    let suite = suite_of(&world, "suite_pool_ok.toml", &[]);
    let project = project(&world, "project", &["../plugin"]);
    let run = |extra: &[&str]| {
        candler(&world, &project)
            .arg("evaluate")
            .arg(&suite)
            .args(extra)
            .output()
            .unwrap()
    };
    assert!(plugin.is_dir());

    // Row 1: every run just run, and no other.
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr_of(&out));
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["failed_requests"], json!([]), "{report}");
    assert_eq!(
        graded(&world),
        handed,
        "an outcome directory per run, and no other"
    );
    let after_first = files_by_run(&world);

    // Row 2: nothing is pending any more.
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr_of(&out));
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["request_ids"], json!([]), "{report}");
    assert_eq!(report["counts"], json!({}), "{report}");
    assert_eq!(files_by_run(&world), after_first, "no file was written");

    // Row 3: a job's runs, a glob's runs, and not both.
    let out = run(&[&first.to_string()]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr_of(&out));
    let regraded = files_by_run(&world);
    for (key, files) in &regraded {
        let grew = files.len() > after_first[key].len();
        assert_eq!(
            grew,
            first_runs.contains(key),
            "{key}: graded again exactly when the first job started it"
        );
    }
    let prefix = second_runs.iter().next().unwrap()[..12].to_owned();
    let out = run(&["--runs", &format!("{prefix}*")]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr_of(&out));
    for (key, files) in &files_by_run(&world) {
        let grew = files.len() > regraded[key].len();
        assert_eq!(
            grew,
            key.starts_with(&prefix),
            "{key}: graded again exactly when the glob names it"
        );
    }
    let out = run(&[&second.to_string(), "--runs", "*"]);
    assert_eq!(out.status.code(), Some(2), "both forms are refused");
    assert!(
        stderr_of(&out).contains("cannot be used with"),
        "{}",
        stderr_of(&out)
    );
    world.remove();
}

/// Guards: E1 (the binary half) — `evaluate` prints the job's id on stderr and its report on
/// stdout. The first line of stderr is a job id, stdout holds one JSON document with the four
/// keys of the report and nothing else, the job's log holds one `evaluation.outcome` per record
/// the store's outcome files hold and ends in `job.ended` with `completed`, and `candler ps`
/// shows the job `ended (completed)`.
///
/// Oracle: the store's outcome files, counted by lines; the job's `events.jsonl`; `ps`.
///
/// Bite: the id printed on stdout (the report no longer parses as one document), or the report
/// printed while the job ended `failed` (the last event is not `completed`).
#[test]
fn e1_evaluate_prints_the_job_id_on_stderr_and_the_report_on_stdout() {
    let (world, _) = sealed_world();
    pool_plugin(&world);
    let suite = suite_of(&world, "suite_pool_ok.toml", &[]);
    let project = project(&world, "project", &["../plugin"]);
    let out = candler(&world, &project)
        .arg("evaluate")
        .arg(&suite)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr_of(&out));
    let stderr = stderr_of(&out);
    let id: JobId = stderr.lines().next().unwrap().parse().unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(stdout.lines().count(), 1, "one line on stdout: {stdout}");
    let report: Value = serde_json::from_str(&stdout).unwrap();
    let mut keys: Vec<&str> = report
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["counts", "failed_requests", "outcome_files", "request_ids"]
    );

    let records: usize = outcome_files(&world)
        .iter()
        .map(|file| fs::read_to_string(file).unwrap().lines().count())
        .sum();
    assert!(records > 0, "the store holds outcome records");
    let events = events_of(&world, &id);
    let logged = events
        .iter()
        .filter(|event| matches!(event, EventPayload::EvaluationOutcome { .. }))
        .count();
    assert_eq!(logged, records, "one evaluation.outcome per stored record");
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
    let ps = candler(&world, &project).arg("ps").output().unwrap();
    let listing = String::from_utf8_lossy(&ps.stdout).into_owned();
    let row = listing
        .lines()
        .find(|line| line.starts_with(&id.to_string()))
        .unwrap();
    assert_eq!(
        row.split('\t').nth(2),
        Some("ended (completed)"),
        "{listing}"
    );
    world.remove();
}

/// Guards: E1 (the binary half) — `evaluate` with no plug-in directory anywhere is refused before
/// a job exists: exit 2, a message that names `plugins.dirs`, and no job directory in the store.
///
/// Oracle: the exit status, stderr, and the store's `jobs/` directory.
///
/// Bite: the job started and failing later (a job directory exists, or the message is the
/// supervisor's).
#[test]
fn e1_evaluate_refuses_without_a_plugin_directory() {
    let (world, _) = sealed_world();
    let before = fs::read_dir(world.store().join("jobs")).unwrap().count();
    let suite = suite_of(&world, "suite_pool_ok.toml", &[]);
    let out = candler(&world, &world.work)
        .arg("evaluate")
        .arg(&suite)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", stderr_of(&out));
    assert!(
        stderr_of(&out).contains("plugins.dirs"),
        "{}",
        stderr_of(&out)
    );
    assert!(out.stdout.is_empty(), "nothing on stdout");
    let after = fs::read_dir(world.store().join("jobs")).unwrap().count();
    assert_eq!(before, after, "no job directory was created");
    world.remove();
}
