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
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use candler_core::evaluation::engine::RunSelection;
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::{JobEvents, TailPos};
use candler_core::jobs::lock::{JobLock, LockState};
use candler_core::jobs::model::EvaluateSpec;
use candler_core::jobs::runtime::TEARDOWN_BOUND;
use candler_core::jobs::runtime::launch::{mint_job_id, own_identity};
use candler_core::store::layout::StoreLayout;
use candler_core::supervise::platform::process_start;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::spec::{StopSignal, StopSpec};
use candler_schemas::execution::{EventPayload, JobEnd, JobRecord, JobState};
use candler_schemas::primitives::{Digest, JobId, Marker, RunKey, Timestamp};
use candler_schemas::process::ChildRecord;
use serde_json::{Value, json};

use cli_run::{World, ps_with_environment, run_keys, wait_for, world};
use seam::{entries_under, finish, occurrences, shasum, sync_environment};

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

/// An event sink that keeps nothing: the test's own supervisor reports nowhere.
struct Quiet;

impl EventSink for Quiet {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// Sends SIGINT to `pid`, a process the test spawned as the leader of its own group, through the
/// guarded stop (`Supervisor::reap`, which checks the identity against the operating system
/// before it signals). The stop waits for the process to exit, so it runs on a thread of its own.
fn interrupt(pid: u32, job: &JobId) -> std::thread::JoinHandle<()> {
    let record = ChildRecord::builder()
        .pid(pid)
        .pgid(pid)
        .start(process_start(pid))
        .marker(job.as_str().parse::<Marker>().unwrap())
        .spawned_at("1970-01-01T00:00:00Z".parse::<Timestamp>().unwrap())
        .build()
        .unwrap();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let stop = StopSpec::new(None, StopSignal::Int, TEARDOWN_BOUND);
        // TRAIT_OBJECT: the supervisor takes its sink as a trait object
        let sink: Arc<dyn EventSink> = Arc::new(Quiet);
        let reaped = runtime.block_on(Supervisor::new(sink).reap(None, &record, &stop));
        assert!(reaped.is_ok(), "the guarded stop ran: {reaped:?}");
    })
}

/// Guards: E1 (the binary half) — Ctrl-C on the waiting `evaluate` stops the job through the
/// guarded `kill` path and leaves it `killed`, with no process of it alive. The `interval` kind
/// of the `pool` plug-in sleeps 60 seconds, so a request is in flight; the test starts
/// `evaluate` as a child it owns, waits for the plug-in's `start` line, and sends SIGINT to the
/// child through the guarded stop.
///
/// Oracle: the command's exit status (2) within `TEARDOWN_BOUND`, its message (`cancelled`), the
/// job's one `job.ended` (`killed`), the job's lock (free), and the evaluator's pid
/// from the plug-in's own file, whose start time is no longer the one it had.
///
/// Bite: the job left running when the command exits (the lock stays held and no `job.ended` is
/// written).
#[test]
fn e1_ctrl_c_ends_the_evaluate_job_killed() {
    let (world, _) = sealed_world();
    pool_plugin(&world);
    let marks = world.root.join("interval.log");
    let suite = suite_of(
        &world,
        "suite_pool_interval.toml",
        &[
            ("@FILE@", marks.display().to_string()),
            ("@SLEEP@", "60".to_owned()),
        ],
    );
    let project = project(&world, "project", &["../plugin"]);
    let err_path = world.work.join("evaluate.err");
    let mut child = candler(&world, &project)
        .arg("evaluate")
        .arg(&suite)
        .process_group(0)
        .stdout(fs::File::create(world.work.join("evaluate.out")).unwrap())
        .stderr(fs::File::create(&err_path).unwrap())
        .spawn()
        .unwrap();
    let started = wait_for(JOB_LIMIT, || {
        child.try_wait().unwrap().is_some()
            || fs::read_to_string(&marks).is_ok_and(|text| text.contains(" start "))
    });
    let err = fs::read_to_string(&err_path).unwrap_or_default();
    assert!(
        started && child.try_wait().unwrap().is_none(),
        "a request is in flight\n{err}"
    );
    let evaluator: u32 = fs::read_to_string(&marks)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let evaluator_start = process_start(evaluator);
    let job: JobId = err.lines().next().unwrap().parse().unwrap();
    let dir = JobDir::open(&StoreLayout::new(&world.store()), &job).unwrap();
    assert_eq!(
        JobLock::probe(&dir).unwrap(),
        LockState::Held,
        "the job is running"
    );

    let stopper = interrupt(child.id(), &job);
    let limit = Instant::now() + TEARDOWN_BOUND + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < limit,
            "the command ends within the teardown bound"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    stopper.join().unwrap();
    let err = fs::read_to_string(&err_path).unwrap();
    assert_eq!(status.code(), Some(2), "{status:?}\n{err}");
    assert!(err.contains("cancelled"), "{err}");
    // The stop's own ladder steps follow `job.ended` in the log, so the end is looked for, not
    // the last line.
    let ends: Vec<JobEnd> = events_of(&world, &job)
        .into_iter()
        .filter_map(|event| {
            if let EventPayload::JobEnded { reason, .. } = event {
                Some(reason)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(ends, [JobEnd::Killed], "one job.ended, and it says killed");
    assert_eq!(
        JobLock::probe(&dir).unwrap(),
        LockState::Free,
        "the lock is free"
    );
    assert_ne!(
        process_start(evaluator),
        evaluator_start,
        "the evaluator is gone, or its pid is another process's"
    );
    world.remove();
}

/// Guards: E1 (the binary half) — a job that ends without a report is not printed as one: with a
/// suite that names an evaluator no plug-in provides the supervisor cannot run the evaluation and
/// the job ends `failed`; the command prints nothing on stdout, exits 2 and says why, quoting the end of
/// the job's `supervisor.log`, and `candler ps` shows the job `ended (failed)`.
///
/// Oracle: the exit status, both streams, `ps`.
///
/// Bite: the report read from a job that ended `failed` (stdout is not empty, or the exit is 0).
#[test]
fn e1_evaluate_of_a_failed_job_prints_no_report_and_says_why() {
    let (world, _) = sealed_world();
    pool_plugin(&world);
    let suite = suite_of(&world, "suite_pool_ok.toml", &[]);
    let text = fs::read_to_string(&suite).unwrap();
    fs::write(&suite, text.replace("name = \"pool\"", "name = \"absent\"")).unwrap();
    let project = project(&world, "project", &["../plugin"]);
    let out = candler(&world, &project)
        .arg("evaluate")
        .arg(&suite)
        .output()
        .unwrap();
    let stderr = stderr_of(&out);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(out.stdout.is_empty(), "no report on stdout");
    assert!(stderr.contains("absent"), "{stderr}");
    let id: JobId = stderr.lines().next().unwrap().parse().unwrap();
    let ps = candler(&world, &project).arg("ps").output().unwrap();
    let listing = String::from_utf8_lossy(&ps.stdout).into_owned();
    let row = listing
        .lines()
        .find(|line| line.starts_with(&id.to_string()))
        .unwrap();
    assert_eq!(row.split('\t').nth(2), Some("ended (failed)"), "{listing}");
    world.remove();
}

/// The variable the environment guards plant in the caller.
const SENTINEL: &str = "CANDLER_TEST_SENTINEL";

/// A value no run of the guards has seen before.
fn sentinel_value() -> String {
    format!(
        "sentinel-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

/// A plug-in of one kind that reports the value of `CANDLER_TEST_SENTINEL` in its evidence, set up
/// beside `world` with a link to the helper's project. Returns the plug-in's directory and a suite
/// that asks it once.
fn env_plugin(world: &World) -> (PathBuf, PathBuf) {
    let dir = world.root.join("env-plugin");
    let kinds = dir.join("kinds");
    fs::create_dir_all(&kinds).unwrap();
    symlink(helper(), dir.join("env")).unwrap();
    fs::write(
        kinds.join("env_kind.py"),
        "import os\n\nNAME = \"seam\"\nVERSION = \"0.1.0\"\n\
         KIND_SPECS = {\"check\": {\"params_schema\": {\"type\": \"object\", \
         \"additionalProperties\": False}}}\n\n\n\
         def check(params, context, prior):\n    value = os.environ.get(\"CANDLER_TEST_SENTINEL\", \"unset\")\n    \
         return {\"status\": \"pass\", \"evidence\": [{\"note\": value}]}\n\n\n\
         KINDS = {\"check\": check}\n",
    )
    .unwrap();
    fs::write(
        dir.join("plugin.toml"),
        format!(
            "schema = \"plugin-manifest/1\"\nname = \"seam\"\nversion = \"0.1.0\"\n\
             command = [\"env\", \"PYTHONPATH={}\", \"uv\", \"run\", \"--frozen\", \"python\", \
             \"-m\", \"candler_evaluator\", \"env_kind\"]\n\n[protocol]\nmajor = 1\nminor = 0\n\n\
             [environment]\nkind = \"uv\"\nproject = \"env\"\nlock = \"env/uv.lock\"\n",
            kinds.display()
        ),
    )
    .unwrap();
    let suite = world.root.join("suites").join("env-suite.toml");
    fs::create_dir_all(suite.parent().unwrap()).unwrap();
    fs::write(
        &suite,
        "schema = \"suite/1\"\n\n[[expectations]]\nid = \"R1\"\nrole = \"gate\"\nkind = \"check\"\n\
         depends_on = []\n\n[expectations.evaluator]\nname = \"seam\"\nrange = \"^0.1\"\n\n\
         [expectations.params]\n",
    )
    .unwrap();
    (dir, suite)
}

/// Guards: the environment of an evaluate job — a variable planted in the caller's environment
/// reaches the evaluator process of an evaluate job. The plug-in's kind reports the value of the
/// variable in its evidence, which the store keeps.
///
/// Oracle: the planted value, unique to the run, read back from the store's outcome files.
///
/// Bite: the evaluators given `PATH` and `HOME` only, as before the ruling (the evidence says
/// `unset`).
#[test]
fn e5_a_variable_planted_in_the_caller_reaches_the_evaluator() {
    let (world, _) = sealed_world();
    let (plugin, suite) = env_plugin(&world);
    let project = project(&world, "project", &[plugin.to_str().unwrap()]);
    let value = sentinel_value();
    let out = candler(&world, &project)
        .env(SENTINEL, &value)
        .arg("evaluate")
        .arg(&suite)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr_of(&out));
    let stored: String = outcome_files(&world)
        .iter()
        .map(|file| fs::read_to_string(file).unwrap())
        .collect();
    assert!(
        stored.contains(&value),
        "the evaluator saw the variable: {stored}"
    );
    world.remove();
}

/// The environments of the supervisors running now, as text: from `/proc/<pid>/environ` where
/// there is a `/proc`, else from `ps eww`.
fn supervisor_environments() -> Vec<String> {
    let mut found = Vec::new();
    if let Ok(read) = fs::read_dir("/proc") {
        for entry in read.flatten() {
            let Ok(command) = fs::read(entry.path().join("cmdline")) else {
                continue;
            };
            let command = String::from_utf8_lossy(&command).replace('\0', " ");
            if command.contains("supervise --job")
                && let Ok(environment) = fs::read(entry.path().join("environ"))
            {
                found.push(String::from_utf8_lossy(&environment).replace('\0', " "));
            }
        }
        return found;
    }
    ps_with_environment()
        .lines()
        .filter(|line| line.contains("supervise --job"))
        .map(str::to_owned)
        .collect()
}

/// Guards: the environment of an agent job — a variable planted in the caller's environment does
/// not reach the supervisor of a `run` job. The test lists the processes of the machine with
/// their environments while the job runs; the supervisor shows `CANDLER_STORE_ROOT`, which proves
/// the listing reads its environment, and never the planted value.
///
/// Oracle: the supervisor's environment as the operating system reports it (`/proc` or `ps eww`),
/// read while the job's log has no `job.ended`.
///
/// Bite: the allowlist of `run` replaced by the whole environment (the supervisor's line holds the
/// value).
#[test]
fn e5_a_variable_planted_in_the_caller_does_not_reach_a_run_job() {
    let world = world();
    let value = sentinel_value();
    let launched = Command::new(env!("CARGO_BIN_EXE_candler"))
        .arg("run")
        .arg(&world.experiment)
        .arg("--set")
        .arg("isolation.build.lock=env.lock")
        .arg("--set")
        .arg(format!("model.token_secret={}", cli_run::SECRET_NAME))
        .current_dir(&world.work)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", world.bin.display()))
        .env("HOME", world.root.join("home"))
        .env("XDG_STATE_HOME", &world.state)
        .env(cli_run::SECRET_NAME, "fixture-token-0123456789")
        .env(SENTINEL, &value)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(launched.status.success(), "{}", stderr_of(&launched));
    let id: JobId = String::from_utf8_lossy(&launched.stdout)
        .trim()
        .parse()
        .unwrap();
    let mut seen_supervisor = false;
    let ended = wait_for(JOB_LIMIT, || {
        for environment in supervisor_environments() {
            seen_supervisor |= environment.contains("CANDLER_STORE_ROOT=");
            assert!(
                !environment.contains(&value),
                "the run job's supervisor holds the variable"
            );
        }
        events_of(&world, &id)
            .iter()
            .any(|event| matches!(event, EventPayload::JobEnded { .. }))
    });
    assert!(ended, "the job ends");
    assert!(
        seen_supervisor,
        "the listing showed the supervisor's environment"
    );
    world.remove();
}

/// Guards: the environment of an evaluate job is written nowhere — a sentinel planted in the
/// caller appears in no file of the store (the jobs' specs, records, logs and reports among them),
/// in nothing the command printed, in the error of a job that failed, and in the output of `ps`,
/// `watch` and `inspect` over either job. One job completes and one fails (`slots.evaluators = 0`),
/// so that the report and the failure's text are both covered.
///
/// Oracle: the planted value, searched by name and bytes under the store and in each output.
///
/// Bite: the supervisor writing its environment to its log (the sentinel is found in
/// `supervisor.log`).
#[test]
fn e5_the_callers_environment_is_written_nowhere() {
    let (world, _) = sealed_world();
    pool_plugin(&world);
    let suite = suite_of(&world, "suite_pool_ok.toml", &[]);
    let project = project(&world, "project", &["../plugin"]);
    let value = sentinel_value();
    let run = |args: &[&str]| {
        candler(&world, &project)
            .env(SENTINEL, &value)
            .args(args)
            .output()
            .unwrap()
    };
    let suite_arg = suite.to_str().unwrap();
    let mut printed = Vec::new();
    let mut ids = Vec::new();
    let done = run(&["evaluate", suite_arg, "--runs", "*"]);
    assert_eq!(done.status.code(), Some(0), "{}", stderr_of(&done));
    user_config(&world, "[slots]\nevaluators = 0\n");
    let failed = run(&["evaluate", suite_arg, "--runs", "*"]);
    assert_eq!(failed.status.code(), Some(2), "{}", stderr_of(&failed));
    for out in [&done, &failed] {
        ids.push(stderr_of(out).lines().next().unwrap().to_owned());
        printed.push(out.clone());
    }
    printed.push(run(&["ps"]));
    for id in &ids {
        printed.push(run(&["watch", id]));
        printed.push(run(&["inspect", id]));
    }
    for out in &printed {
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), stderr_of(out));
        assert!(!text.contains(&value), "the variable was printed: {text}");
    }
    assert_eq!(
        occurrences(&world.store(), &value),
        Vec::<String>::new(),
        "the variable is in a file of the store"
    );
    world.remove();
}

/// Guards: R2 — a suite that changed after the job was started prints as the record of
/// `evaluation.suite_changed`, side `suite`, under `--error-format json`. The test writes the job
/// by hand with the digest of the suite as it was, changes the suite, and starts the hidden
/// `supervise` verb; the job ends `failed`.
///
/// Oracle: a record written by hand from the contract, compared field by field.
///
/// Bite: the refusal left as text only (the record has the fallback code `candler.internal` and
/// the side `candler`).
#[test]
fn r2_a_suite_changed_after_the_job_started_prints_as_a_suite_record() {
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
    fs::write(&suite, "# edited after the launcher read it\n").unwrap();
    let bytes = serde_json::to_vec(&spec).unwrap();
    let id = mint_job_id().unwrap();
    let record = JobRecord::builder()
        .id(id.clone())
        .supervisor(own_identity().unwrap())
        .spec_digest(Digest::from_hex(&shasum(&bytes)).unwrap())
        .state(JobState::Running)
        .build()
        .unwrap();
    JobDir::create(&StoreLayout::new(&world.store()), id.clone(), &record).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_candler"))
        .args([
            "--error-format",
            "json",
            "supervise",
            "--job",
            &id.to_string(),
        ])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", world.root.join("home"))
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CONFIG_HOME", world.root.join("config"))
        .env("CANDLER_STORE_ROOT", world.store())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&bytes).unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", stderr_of(&out));
    let stderr = stderr_of(&out);
    let last = stderr.lines().last().unwrap();
    let record: Value = serde_json::from_str(last).unwrap();
    assert_eq!(record["schema"], "refusal/1", "{record}");
    assert_eq!(record["code"], "evaluation.suite_changed", "{record}");
    assert_eq!(record["side"], "suite", "{record}");
    assert!(
        matches!(
            events_of(&world, &id).last(),
            Some(EventPayload::JobEnded {
                reason: JobEnd::Failed,
                ..
            })
        ),
        "the job ended failed"
    );
    world.remove();
}

/// Starts `evaluate` over the world with `suite` and `--error-format json`, from the project, and
/// returns the output with its stderr cut to the last line, where the record is: the preflight
/// and the job's id come before it.
fn evaluate_json(world: &World, project: &Path, suite: &Path) -> Output {
    let out = candler(world, project)
        .args(["--error-format", "json", "evaluate"])
        .arg(suite)
        .output()
        .unwrap();
    let stderr = stderr_of(&out);
    let last = stderr.lines().last().unwrap_or_default().to_owned();
    Output {
        stderr: format!("{last}\n").into_bytes(),
        ..out
    }
}

/// Guards: R2 — a refusal the engine raises inside an evaluate job keeps its code, side and
/// pointer through the verb: a suite entry that names an evaluator no plug-in provides is
/// `suite.evaluator_unknown`, side `suite`, though the job and not the command found it.
///
/// Oracle: a record written by hand from the contract.
///
/// Bite: the verb not reading the job's `refusal.json` (the record has the fallback code
/// `candler.internal` and the side `candler`); the verb's own message put in the record in place
/// of the recorded one (the message quotes the log and repeats its cause).
#[test]
fn r2_a_refusal_raised_in_the_job_keeps_its_code_through_the_verb() {
    let (world, _) = sealed_world();
    pool_plugin(&world);
    let suite = suite_of(&world, "suite_pool_ok.toml", &[]);
    let text = fs::read_to_string(&suite).unwrap();
    assert!(
        text.contains("name = \"pool\""),
        "the suite names its evaluator"
    );
    fs::write(&suite, text.replace("name = \"pool\"", "name = \"absent\"")).unwrap();
    let project = project(&world, "project", &["../plugin"]);
    let out = evaluate_json(&world, &project, &suite);
    assert_eq!(out.status.code(), Some(2));
    let record: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(record["schema"], "refusal/1", "{record}");
    assert_eq!(record["code"], "suite.evaluator_unknown", "{record}");
    assert_eq!(record["side"], "suite", "{record}");
    // The record is the job's own, as written: its message is the engine's and its causes are the
    // engine's, not the words of the verb that quotes the log.
    let written: Vec<Value> = fs::read_dir(world.store().join("jobs"))
        .unwrap()
        .flatten()
        .filter_map(|entry| fs::read(entry.path().join("refusal.json")).ok())
        .map(|bytes| serde_json::from_slice(&bytes).unwrap())
        .collect();
    assert_eq!(written.len(), 1, "the job wrote one refusal.json");
    assert_eq!(record["message"], written[0]["message"], "{record}");
    assert_eq!(record["causes"], written[0]["causes"], "{record}");
    let message = record["message"].as_str().unwrap();
    assert!(!message.contains("ended without a report"), "{message}");
    assert!(!message.contains("supervise failed"), "{message}");
    world.remove();
}

/// Guards: R2 — a suite that changed after the verb read it prints as `evaluation.suite_changed`,
/// side `suite`, through the verb. The plug-in's environment is not built, so the preflight runs
/// `uv sync` and reports it on stderr; the test edits the suite on the first such line, which is
/// after the verb read it and before the supervisor does.
///
/// Oracle: a record written by hand from the contract.
///
/// Bite: the verb not reading the job's `refusal.json` (the record has the fallback code).
#[test]
fn r2_a_changed_suite_keeps_its_code_through_the_verb() {
    use std::io::{BufRead as _, BufReader, Read as _};

    let world = world();
    let launched = world.launch();
    finish(&world, &launched);
    pool_plugin(&world);
    let suite = suite_of(&world, "suite_pool_ok.toml", &[]);
    let project = project(&world, "project", &["../plugin"]);
    let mut child = candler(&world, &project)
        .args(["--error-format", "json", "evaluate"])
        .arg(&suite)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut first = String::new();
    stderr.read_line(&mut first).unwrap();
    assert!(
        first.contains("event"),
        "the preflight reported a spawn: {first:?}"
    );
    fs::write(&suite, "# edited after the verb read it\n").unwrap();
    let mut rest = String::new();
    stderr.read_to_string(&mut rest).unwrap();
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(2), "{rest}");
    let record: Value = serde_json::from_str(rest.lines().last().unwrap()).unwrap();
    assert_eq!(record["code"], "evaluation.suite_changed", "{record}");
    assert_eq!(record["side"], "suite", "{record}");
    world.remove();
}

/// Guards: the recovery of a lost evaluate job needs no docker. The command and the supervisor of
/// a job with an evaluator in flight are killed (SIGKILL), which leaves the job lost; `kill -y`
/// then runs with a `PATH` that holds no `docker`. An evaluate job owns no container, so its
/// recovery builds no docker client and sweeps no label.
///
/// Oracle: the exit status and text of `kill`, the evaluator's pid and start time (the process
/// is gone, or the pid is another process's), the job's log (one `job.ended`, `failed`), and the
/// job's lock (free), then `ps` over the same `PATH` says `ended (failed)`.
///
/// Bite: `recover_job_dir_with` building the `DockerCli` for an evaluate job as it does for a
/// `run` job (`kill` fails with `recover failed: docker call is not available`, and the job
/// stays lost).
#[test]
fn e1_a_lost_evaluate_job_is_recovered_with_no_docker_on_the_path() {
    let (world, _) = sealed_world();
    pool_plugin(&world);
    let marks = world.root.join("interval.log");
    let suite = suite_of(
        &world,
        "suite_pool_interval.toml",
        &[
            ("@FILE@", marks.display().to_string()),
            ("@SLEEP@", "60".to_owned()),
        ],
    );
    let project = project(&world, "project", &["../plugin"]);
    let err_path = world.work.join("evaluate.err");
    let mut child = candler(&world, &project)
        .arg("evaluate")
        .arg(&suite)
        .process_group(0)
        .stdout(fs::File::create(world.work.join("evaluate.out")).unwrap())
        .stderr(fs::File::create(&err_path).unwrap())
        .spawn()
        .unwrap();
    let started = wait_for(JOB_LIMIT, || {
        child.try_wait().unwrap().is_some()
            || fs::read_to_string(&marks).is_ok_and(|text| text.contains(" start "))
    });
    let err = fs::read_to_string(&err_path).unwrap_or_default();
    assert!(
        started && child.try_wait().unwrap().is_none(),
        "a request is in flight\n{err}"
    );
    let evaluator: u32 = fs::read_to_string(&marks)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let evaluator_start = process_start(evaluator);
    let job: JobId = err.lines().next().unwrap().parse().unwrap();
    let dir = JobDir::open(&StoreLayout::new(&world.store()), &job).unwrap();
    let supervisor = dir.record().unwrap().supervisor().pid();

    // The supervisor first, so that it cannot answer the command's death; then the command.
    let killed = Command::new("kill")
        .args(["-KILL", &supervisor.to_string()])
        .status()
        .unwrap();
    assert!(killed.success(), "the supervisor was killed");
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(
        wait_for(Duration::from_secs(30), || {
            JobLock::probe(&dir).unwrap() == LockState::Free
        }),
        "the job is lost: nobody holds its lock"
    );
    assert_eq!(
        process_start(evaluator),
        evaluator_start,
        "the evaluator is still alive before the recovery"
    );

    let no_docker = world.root.join("no-docker");
    fs::create_dir_all(&no_docker).unwrap();
    let out = candler(&world, &project)
        .env("PATH", &no_docker)
        .args(["kill", job.as_str(), "-y"])
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        stderr_of(&out)
    );
    assert!(
        out.status.success() && text.contains("recovered"),
        "kill recovered the job without docker: {text}"
    );
    assert!(
        !text.contains("docker call is not available"),
        "no docker was asked for: {text}"
    );
    assert!(
        wait_for(Duration::from_secs(30), || {
            process_start(evaluator) != evaluator_start
        }),
        "the evaluator is gone, or its pid is another process's"
    );
    let ends = events_of(&world, &job)
        .into_iter()
        .filter(|event| matches!(event, EventPayload::JobEnded { .. }))
        .count();
    assert_eq!(ends, 1, "one job.ended");
    assert!(
        events_of(&world, &job).into_iter().any(|event| matches!(
            event,
            EventPayload::JobEnded {
                reason: JobEnd::Failed,
                ..
            }
        )),
        "the job ended failed"
    );
    assert_eq!(
        JobLock::probe(&dir).unwrap(),
        LockState::Free,
        "the lock is free"
    );
    let ps = candler(&world, &project)
        .env("PATH", &no_docker)
        .arg("ps")
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&ps.stdout).contains("ended (failed)"),
        "ps: {}{}",
        String::from_utf8_lossy(&ps.stdout),
        stderr_of(&ps)
    );
    world.remove();
}

/// The number `i4_violations` holds, wherever the document nests it.
fn violations(value: &Value) -> Option<u64> {
    match value {
        Value::Object(map) => map
            .get("i4_violations")
            .and_then(Value::as_u64)
            .or_else(|| map.values().find_map(violations)),
        Value::Array(items) => items.iter().find_map(violations),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
    }
}

/// Guards: a run graded twice counts once. Two `evaluate` calls over the same runs, one after the
/// other as two at once would leave them, hold two complete sets of outcomes for one evaluator
/// version; `show` and `analyze` read the current record under each I4 key, so their output is
/// the same with the second set as without it, and the pending selection treats the run as
/// complete either way.
///
/// Oracle: every run's outcome directory holds two files after the second call, the output of
/// `show --format json` and `analyze --format json` is byte-equal before and after, the I4
/// violations of `analyze` are none (the evaluator is deterministic), and a third `evaluate`
/// with the default selection writes no file.
///
/// Bite: the I4 key of `select` carrying the request id, so that each set is current under its own
/// key (`show` differs after the second call).
#[test]
fn e1_a_run_graded_twice_counts_once_for_every_reader() {
    let (world, _) = sealed_world();
    pool_plugin(&world);
    let suite = suite_of(&world, "suite_pool_ok.toml", &[]);
    let project = project(&world, "project", &["../plugin"]);
    let evaluate = |extra: &[&str]| {
        let out = candler(&world, &project)
            .arg("evaluate")
            .arg(&suite)
            .args(extra)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr_of(&out));
    };
    let read = |verb: &str| {
        let out = candler(&world, &project)
            .args([verb, "--format", "json"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{verb}: {}", stderr_of(&out));
        out.stdout
    };

    evaluate(&[]);
    let once = files_by_run(&world);
    let shown = read("show");
    let analysed = read("analyze");
    evaluate(&["--runs", "*"]);
    for (key, files) in &files_by_run(&world) {
        assert_eq!(files.len(), 2 * once[key].len(), "{key}: a second set");
    }
    assert_eq!(read("show"), shown, "show counts each run once");
    assert_eq!(read("analyze"), analysed, "analyze counts each run once");
    let report: Value = serde_json::from_slice(&analysed).unwrap();
    assert_eq!(violations(&report), Some(0), "no I4 violation: {report}");
    let after_two = files_by_run(&world);
    evaluate(&[]);
    assert_eq!(files_by_run(&world), after_two, "nothing is pending");
    world.remove();
}

/// Guards: R2 — a configuration `evaluate` cannot work with is refused before any job starts, with
/// a `config` record: `slots.evaluators = 0` is `config.no_evaluator_slots`, and a project with an
/// empty `plugins.dirs`, or none at all, is `config.no_plugin_dirs`.
///
/// Oracle: records written by hand from the contract, and the job directories of the store, which
/// no row adds to.
///
/// Bite: the refusals raised as `CliError::Refused` (the code is `candler.internal`, the side
/// `candler`).
#[test]
fn r2_a_configuration_evaluate_cannot_use_is_a_config_record() {
    let (world, _) = sealed_world();
    pool_plugin(&world);
    let suite = suite_of(&world, "suite_pool_ok.toml", &[]);
    let jobs = || fs::read_dir(world.store().join("jobs")).unwrap().count();
    let before = jobs();
    let expect = |out: &Output, code: &str| {
        assert_eq!(out.status.code(), Some(2), "{}", stderr_of(out));
        let record: Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(record["schema"], "refusal/1", "{record}");
        assert_eq!(record["code"], code, "{record}");
        assert_eq!(record["side"], "config", "{record}");
    };
    let empty = project(&world, "empty", &[]);
    expect(
        &evaluate_json(&world, &empty, &suite),
        "config.no_plugin_dirs",
    );
    expect(
        &evaluate_json(&world, &world.work, &suite),
        "config.no_plugin_dirs",
    );
    user_config(&world, "[slots]\nevaluators = 0\n");
    let project = project(&world, "project", &["../plugin"]);
    let out = evaluate_json(&world, &project, &suite);
    expect(&out, "config.no_evaluator_slots");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("slots.evaluators is 0"),
        "{}",
        stderr_of(&out)
    );
    assert_eq!(jobs(), before, "no job directory was created");
    world.remove();
}

/// Guards: a job found lost by the command is said to be lost. The supervisor of an evaluate job
/// with an evaluator in flight is killed (SIGKILL) while the command waits; the command recovers
/// the job, exits 2 and says that the job was lost and what the recovery did, and the job ended
/// `failed`.
///
/// Oracle: the exit status and the text on stderr, the job's one `job.ended`, the evaluator's
/// pid and start time.
///
/// Bite: the words of a job that failed (`its supervisor.log is empty`) for a lost one.
#[test]
fn e1_evaluate_says_when_its_job_was_lost() {
    let (world, _) = sealed_world();
    pool_plugin(&world);
    let marks = world.root.join("interval.log");
    let suite = suite_of(
        &world,
        "suite_pool_interval.toml",
        &[
            ("@FILE@", marks.display().to_string()),
            ("@SLEEP@", "60".to_owned()),
        ],
    );
    let project = project(&world, "project", &["../plugin"]);
    let err_path = world.work.join("evaluate.err");
    let mut child = candler(&world, &project)
        .arg("evaluate")
        .arg(&suite)
        .process_group(0)
        .stdout(fs::File::create(world.work.join("evaluate.out")).unwrap())
        .stderr(fs::File::create(&err_path).unwrap())
        .spawn()
        .unwrap();
    let started = wait_for(JOB_LIMIT, || {
        child.try_wait().unwrap().is_some()
            || fs::read_to_string(&marks).is_ok_and(|text| text.contains(" start "))
    });
    let err = fs::read_to_string(&err_path).unwrap_or_default();
    assert!(
        started && child.try_wait().unwrap().is_none(),
        "a request is in flight\n{err}"
    );
    let evaluator: u32 = fs::read_to_string(&marks)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let evaluator_start = process_start(evaluator);
    let job: JobId = err.lines().next().unwrap().parse().unwrap();
    let dir = JobDir::open(&StoreLayout::new(&world.store()), &job).unwrap();
    let supervisor = dir.record().unwrap().supervisor().pid();
    let killed = Command::new("kill")
        .args(["-KILL", &supervisor.to_string()])
        .status()
        .unwrap();
    assert!(killed.success(), "the supervisor was killed");
    let limit = Instant::now() + TEARDOWN_BOUND + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < limit,
            "the command ends once it has recovered the job"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let err = fs::read_to_string(&err_path).unwrap();
    assert_eq!(status.code(), Some(2), "{err}");
    assert!(err.contains("it was lost"), "{err}");
    assert!(err.contains("recovery stopped"), "{err}");
    assert!(!err.contains("supervisor.log is empty"), "{err}");
    let ends = events_of(&world, &job)
        .into_iter()
        .filter(|event| matches!(event, EventPayload::JobEnded { .. }))
        .count();
    assert_eq!(ends, 1, "one job.ended");
    assert_ne!(
        process_start(evaluator),
        evaluator_start,
        "the evaluator is gone, or its pid is another process's"
    );
    world.remove();
}
