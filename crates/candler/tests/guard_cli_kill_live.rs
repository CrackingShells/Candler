//! `candler kill` on running jobs: the supervisors are real, the signal is the CLI's own.
//!
//! The test binary re-executes itself as two supervisors, each a real `supervise_entry` over a
//! body that starts no container and waits for its cancel. The test holds a `Child` of each.
#![cfg(test)]
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod kill_live {
    use std::fs;
    use std::future::Future;
    use std::os::unix::fs::PermissionsExt as _;
    use std::os::unix::process::ExitStatusExt as _;
    use std::path::{Path, PathBuf};
    use std::pin::Pin;
    use std::process::{Child, Command, ExitStatus, Stdio};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use candler_core::jobs::dir::JobDir;
    use candler_core::jobs::events::{JobEvents, TailPos};
    use candler_core::jobs::lock::{JobLock, LockState};
    use candler_core::jobs::lost::{JobLiveness, classify};
    use candler_core::jobs::model::{JobCause, JobControl, JobOutcome, JobSpec};
    use candler_core::jobs::runtime::launch::own_identity;
    use candler_core::jobs::runtime::{JobEntry, JobServices, supervise_entry};
    use candler_core::resolve::layers::Resolution;
    use candler_core::runner::Runtime;
    use candler_core::runner::evidence::provenance::EvalInfra;
    use candler_core::runner::exec::SkillTree;
    use candler_core::store::Store;
    use candler_core::supervise::platform::process_start;
    use candler_harness::driver::DriverError;
    use candler_harness::spec::{CommandSpec, PreparedFile};
    use candler_harness_claude_code::driver::ClaudeCodeDriver;
    use candler_schemas::execution::{
        EventPayload, ExperimentFile, JobEnd, JobRecord, TreatmentRef,
    };
    use candler_schemas::primitives::JobId;
    use candler_schemas::process::ProcessStart;

    /// The job a re-executed supervisor serves.
    const ENV_JOB: &str = "CANDLER_TEST_KILL_LIVE_JOB";
    /// The store the supervisor opens.
    const ENV_STORE: &str = "CANDLER_TEST_KILL_LIVE_STORE";
    /// The directory the supervisor's body writes `ready` in.
    const ENV_WORK: &str = "CANDLER_TEST_KILL_LIVE_WORK";

    /// The two jobs the test runs: lowercase hyphenated `UUIDv7`s.
    const JOBS: [&str; 2] = [
        "01890000-0000-7000-8000-000000000041",
        "01890000-0000-7000-8000-000000000042",
    ];

    /// How long the body of a supervisor waits for its cancel before it returns by itself, so
    /// that no supervisor outlives a test that died without cleaning up.
    const BODY_LIMIT: Duration = Duration::from_mins(5);
    /// How long a helper process (`ps`, the dry run) may take.
    const HELPER_LIMIT: Duration = Duration::from_secs(30);
    /// How long a supervisor may take to hold its lock and say it is ready.
    const READY_LIMIT: Duration = Duration::from_secs(60);
    /// How long the whole `kill` may take.
    const KILL_LIMIT: Duration = Duration::from_secs(120);

    /// The shim: it fails closed. Every call is logged; an argument that starts with `-H`,
    /// `--host` or `--context` anywhere, every destroy verb and every unknown form is refused.
    const SHIM: &str = r#"#!/bin/sh
log="$(dirname "$0")/docker.log"
echo "$*" >> "$log"
for a in "$@"; do
  case "$a" in
    -H*|--host*|--context*) echo "REFUSED $*" >> "$log"; exit 1 ;;
  esac
done
case "$1 $2" in
  "ps "*|"inspect "*|"version "*|"info "*|"container ls"|"container inspect"|"network ls"|"network inspect") exit 0 ;;
esac
echo "REFUSED $*" >> "$log"
exit 1
"#;

    /// A skill tree the supervisor never asks for anything.
    struct NoSkills;

    impl SkillTree for NoSkills {
        fn archive_command(
            &self,
            _treatment: &TreatmentRef,
            _source: &Path,
        ) -> Result<CommandSpec, DriverError> {
            Err(DriverError::VersionOutput {
                // BORROW: the error owns its detail
                detail: "the guard reads no skill tree".to_owned(),
            })
        }

        fn files(
            &self,
            _treatment: &TreatmentRef,
            _archive: &[u8],
        ) -> Result<Vec<PreparedFile>, DriverError> {
            Ok(Vec::new())
        }
    }

    /// A spec of no runs.
    fn spec() -> JobSpec {
        let file = ExperimentFile::from_toml_str(
            "schema = \"experiment/1\"\nreplicates = 1\n\n[[factors]]\npath = \"model.id\"\nlevels = [\"m\"]\n\n[budget.per_run]\nwall_s = 3600\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"htn\"\n",
        )
        .expect("an experiment file");
        let resolution: Resolution = serde_json::from_value(serde_json::json!({
            "values": [], "settings": [], "sources": [], "preflight": [],
        }))
        .expect("a resolution");
        JobSpec {
            budget: file.budget().clone(), // BORROW: the spec owns its budget
            experiment: file,
            resolution,
            plans: Vec::new(),
            force: false,
            inputs: Vec::new(),
        }
    }

    /// The body of a supervisor: it writes its pid to `ready`, then waits for its cancel and
    /// returns. The runtime decides how the job ends.
    // TRAIT_OBJECT: the body type of a job returns a boxed future
    fn waiting_body(
        _rt: Runtime,
        _spec: JobSpec,
        control: JobControl,
    ) -> Pin<Box<dyn Future<Output = JobOutcome> + Send + 'static>> {
        Box::pin(async move {
            let work = PathBuf::from(std::env::var_os(ENV_WORK).expect("a work directory"));
            // The test reads `ready` as soon as it exists: it appears whole, by a rename.
            let part = work.join("ready.part");
            fs::write(&part, std::process::id().to_string()).expect("the ready file");
            fs::rename(&part, work.join("ready")).expect("the ready file in place");
            let mut cancel = control.cancel.subscribe();
            let waiting = async {
                loop {
                    if matches!(*cancel.borrow_and_update(), Some(JobCause::Killed)) {
                        break;
                    }
                    if cancel.changed().await.is_err() {
                        break;
                    }
                }
            };
            // A test that dies leaves no supervisor behind: the wait ends by itself.
            drop(tokio::time::timeout(BODY_LIMIT, waiting).await);
            JobOutcome {
                end: JobEnd::Completed,
                runs: Vec::new(),
            }
        })
    }

    /// The re-exec entry of the supervisors that B41 starts; it guards nothing itself. It does
    /// nothing unless `CANDLER_TEST_KILL_LIVE_JOB` names a job: run on its own it is an empty
    /// test.
    ///
    /// Guards: none (re-exec entry of B41).
    /// Oracle: none.
    /// Bite: none.
    #[test]
    fn supervisor_child() {
        let Ok(job) = std::env::var(ENV_JOB) else {
            return;
        };
        let root = std::env::var(ENV_STORE).expect("a store");
        let store = Store::open(Path::new(&root)).expect("a store");
        let job: JobId = job.parse().expect("a job id");
        let dir = JobDir::open(store.layout(), &job).expect("the job directory");
        // The supervisor records who it is before it serves, as `candler supervise` does.
        dir.set_supervisor(own_identity().expect("an identity"))
            .expect("the record");
        let entry = JobEntry {
            store,
            job,
            spec: spec(),
            services: JobServices {
                // TRAIT_OBJECT: the driver is the harness's real one, which the body never calls
                driver: Arc::new(
                    ClaudeCodeDriver::new().expect("the driver builds from constants"),
                ),
                // TRAIT_OBJECT: the tree is a stub that holds no file
                skills: Arc::new(NoSkills),
                eval_infra: EvalInfra::default(),
                planner_package: None,
            },
        };
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("a runtime");
        let outcome = runtime
            .block_on(supervise_entry(entry, waiting_body))
            .expect("the job ends in order");
        assert_eq!(outcome.end, JobEnd::Killed);
    }

    /// A supervisor the test spawned and holds. When the test ends with it still running, even
    /// by a failed assertion, the drop ends it with the one signal a `Child` sends itself
    /// (SIGKILL to its own pid) and reaps it, so that no supervisor outlives its test.
    struct Held(Child);

    impl Drop for Held {
        fn drop(&mut self) {
            // EXPLICIT: Child::kill signals only the process this handle spawned; one already
            // reaped is a no-op
            drop(self.0.kill());
            drop(self.0.wait());
        }
    }

    /// The text of a file, or nothing.
    fn text_of(path: &Path) -> String {
        fs::read_to_string(path).unwrap_or_default()
    }

    /// A job's world: its directory in the shared store and the files of its supervisor.
    struct Rig {
        id: JobId,
        dir: JobDir,
        work: PathBuf,
    }

    /// The record of a job whose supervisor has not yet said who it is.
    fn record(id: &JobId) -> JobRecord {
        serde_json::from_value(serde_json::json!({
            "schema": "job-record/1",
            "id": id.to_string(),
            "supervisor": {"pid": 4242, "start": {"kind": "unavailable"}},
            "spec_digest": format!("sha256:{}", "b".repeat(64)),
            "state": "running",
        }))
        .expect("a job record")
    }

    /// Writes a `docker` into `dir` that fails closed with an empty allowlist: it logs every call
    /// to `docker.log` beside it, answers the read verbs the job's own setup may use, and
    /// refuses with status 1, and a `REFUSED` line, every other form, every destroy verb and
    /// every `-H`, `--host` or `--context` flag. It never runs a real docker.
    fn shim_docker(dir: &Path) {
        fs::create_dir_all(dir).expect("a shim directory");
        let path = dir.join("docker");
        fs::write(&path, SHIM).expect("a shim");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("a mode");
    }

    /// Runs `command` with its output taken and a deadline: one that outlives it is killed, by
    /// the handle held here, and fails the test.
    fn bounded(command: &mut Command) -> (ExitStatus, String) {
        let mut held = Held(
            command
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .expect("a helper"),
        );
        let mut stdout = held.0.stdout.take().expect("stdout");
        let reader = std::thread::spawn(move || {
            use std::io::Read as _;
            let mut text = String::new();
            drop(stdout.read_to_string(&mut text));
            text
        });
        let until = Instant::now() + HELPER_LIMIT;
        let status = loop {
            if let Some(status) = held.0.try_wait().expect("a helper's status") {
                break status;
            }
            assert!(Instant::now() < until, "a helper did not finish");
            std::thread::sleep(Duration::from_millis(10));
        };
        (status, reader.join().expect("the reader"))
    }

    /// The pids of the processes of the group `leader`, from the process table.
    fn group_members(leader: u32) -> Vec<u32> {
        let (_, table) = bounded(Command::new("/bin/ps").args(["-A", "-o", "pid=,pgid="]));
        table
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                let pid: u32 = fields.next()?.parse().ok()?;
                let group: u32 = fields.next()?.parse().ok()?;
                (group == leader).then_some(pid)
            })
            .collect()
    }

    /// The environment of the binary: everything inside `tmp`, and no store but `--store`.
    fn confine(command: &mut Command, tmp: &Path, path: &str) {
        command
            .env_clear()
            .env("PATH", path)
            .env("HOME", tmp)
            .env("XDG_CONFIG_HOME", tmp.join("xdg-config"))
            .env("XDG_DATA_HOME", tmp.join("xdg-data"))
            .env("XDG_STATE_HOME", tmp.join("state-cli"))
            .env("XDG_CACHE_HOME", tmp.join("xdg-cache"))
            .stdin(Stdio::null());
    }

    /// Starts the supervisor of `rig` by re-executing this binary.
    fn spawn_supervisor(rig: &Rig, root: &Path, tmp: &Path, path: &str) -> Held {
        // The child test's name, as libtest lists it: this module's path without the crate's.
        let module = module_path!().split_once("::").map_or("", |(_, rest)| rest);
        let entry = format!("{module}::supervisor_child");
        let home = rig.work.join("home");
        let child = Command::new(std::env::current_exe().expect("this binary"))
            .args(["--exact", &entry, "--nocapture"])
            .env_clear()
            .env(ENV_JOB, rig.id.to_string())
            .env(ENV_STORE, root)
            .env(ENV_WORK, &rig.work)
            .env("PATH", path)
            .env("HOME", &home)
            .env("XDG_STATE_HOME", tmp.join(format!("state-{}", rig.id)))
            .stdin(Stdio::null())
            .stdout(fs::File::create(rig.work.join("child.out")).expect("a log"))
            .stderr(fs::File::create(rig.work.join("child.err")).expect("a log"))
            .spawn()
            .expect("the supervisor");
        Held(child)
    }

    /// Waits until the supervisor of `rig` says it is ready and returns the pid it names, or
    /// panics with what it said.
    fn wait_ready(held: &mut Held, rig: &Rig) -> u32 {
        let until = Instant::now() + READY_LIMIT;
        loop {
            if let Ok(text) = fs::read_to_string(rig.work.join("ready")) {
                return text.trim().parse().expect("a pid");
            }
            let exited = held.0.try_wait().expect("the supervisor's status");
            assert!(
                exited.is_none() && Instant::now() < until,
                "the supervisor of {} did not become ready (exited: {exited:?}): {}",
                rig.id,
                text_of(&rig.work.join("child.err"))
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The kinds of the events of the job in `dir`, in order, and the reason of its `job.ended`
    /// events.
    fn events_of(dir: &JobDir) -> (Vec<String>, Vec<JobEnd>) {
        let tail = JobEvents::open(dir)
            .expect("the log")
            .tail(TailPos(0))
            .expect("the events");
        let kinds = tail
            .events
            .iter()
            // BORROW: the list owns the kinds
            .map(|envelope| envelope.event().kind().to_owned())
            .collect();
        let ends = tail
            .events
            .iter()
            .filter_map(|envelope| {
                if let EventPayload::JobEnded { reason, .. } = envelope.event() {
                    Some(*reason)
                } else {
                    None
                }
            })
            .collect();
        (kinds, ends)
    }

    /// Guards: B41 — `candler kill --all -y` on two running jobs reaps each supervisor from the
    /// identity `job.json` records, with SIGTERM through the process guard, and each job ends
    /// `killed` by its own teardown and not by a recovery.
    /// Oracle: the real binary's exit status and lines; the status of each supervisor, read from
    /// the `Child` the test holds (a normal exit, not SIGKILL); each job's event log
    /// (`job.started`, one `job.ended` of `killed`, then only the reap's `process.stop_step`); the
    /// jobs' locks; and the start time of each supervisor's pid. The supervisors start no process
    /// of their own, so no job marker exists to scan: the pid and start time stand in for it.
    /// Bite: the running branch of `kill` skips the reap and falls to the recovery.
    ///
    /// The test dies safely: each supervisor's body ends the job by itself after five minutes
    /// (`BODY_LIMIT`), whether or not anything signals it, and every helper wait is bounded.
    #[test]
    fn b41_kill_all_reaps_running_supervisors() {
        let tmp = tempfile::tempdir().expect("a temporary directory");
        let root = tmp.path().join("store");
        let store = Store::init(&root).expect("a store");
        let bin = tmp.path().join("bin");
        shim_docker(&bin);
        let path = format!("{}:/usr/bin:/bin", bin.display());

        let rigs: Vec<Rig> = JOBS
            .iter()
            .map(|text| {
                let id: JobId = text.parse().expect("a job id");
                let dir = JobDir::create(store.layout(), id.clone(), &record(&id)) // BORROW: the directory owns its job id
                    .expect("a job directory");
                let work = tmp.path().join(format!("work-{id}"));
                fs::create_dir_all(&work).expect("a work directory");
                Rig { id, dir, work }
            })
            .collect();
        let mut held: Vec<Held> = rigs
            .iter()
            .map(|rig| spawn_supervisor(rig, &root, tmp.path(), &path))
            .collect();
        let mut pids = Vec::new();
        for (held, rig) in held.iter_mut().zip(&rigs) {
            pids.push(wait_ready(held, rig));
        }
        let starts: Vec<(u32, ProcessStart)> =
            pids.iter().map(|pid| (*pid, process_start(*pid))).collect();
        for ((rig, held), (pid, _)) in rigs.iter().zip(&held).zip(&starts) {
            assert_eq!(held.0.id(), *pid, "the ready pid is not the child's");
            // The control: the supervisor holds the lock and the log says only that it began.
            assert_eq!(JobLock::probe(&rig.dir).expect("a probe"), LockState::Held);
            assert_eq!(
                classify(&rig.dir).expect("a class"),
                JobLiveness::Running,
                "{} is not running",
                rig.id
            );
            assert_eq!(events_of(&rig.dir).0, ["job.started"]);
        }

        // The signal can only reach what the test spawned: the record names the child, the child
        // leads its group, and nothing else is in that group.
        for ((rig, held), (pid, _)) in rigs.iter().zip(&held).zip(&starts) {
            let recorded = rig.dir.record().expect("the record").supervisor().pid();
            assert_eq!(
                recorded,
                held.0.id(),
                "{}: job.json names another pid",
                rig.id
            );
            assert_eq!(
                group_members(*pid),
                [*pid],
                "{}: the supervisor's group is not only itself",
                rig.id
            );
            let (_, pgid) =
                bounded(Command::new("/bin/ps").args(["-o", "pgid=", "-p", &pid.to_string()]));
            assert_eq!(
                pgid.trim(),
                pid.to_string(),
                "{}: the supervisor does not lead its group",
                rig.id
            );
        }

        // The dry run: without `-y` and with stdin closed the binary refuses, and what it would
        // stop is exactly the test's own jobs.
        let mut dry = Command::new(env!("CARGO_BIN_EXE_candler"));
        dry.args(["kill", "--all", "--store"]).arg(&root);
        confine(&mut dry, tmp.path(), &path);
        let (dry, said) = bounded(&mut dry);
        assert_eq!(dry.code(), Some(2), "the dry run was not refused: {said}");
        let mut wanted: Vec<String> = rigs.iter().map(|rig| rig.id.to_string()).collect();
        wanted.sort();
        assert_eq!(
            said.lines().next().map(str::to_owned),
            Some(format!("would stop: {}", wanted.join(" "))),
            "the dry run would stop other jobs than the test's own: {said}"
        );

        // The real binary, stdin closed. The test reaps its own supervisors while it waits, as
        // their parent would: a zombie is not a process that is gone.
        let mut command = Command::new(env!("CARGO_BIN_EXE_candler"));
        command.args(["kill", "--all", "-y", "--store"]).arg(&root);
        confine(&mut command, tmp.path(), &path);
        let mut cli = Held(
            command
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("the cli"),
        );
        let mut statuses: Vec<Option<ExitStatus>> = vec![None; held.len()];
        let until = Instant::now() + KILL_LIMIT;
        let exit = loop {
            for (slot, held) in statuses.iter_mut().zip(held.iter_mut()) {
                if slot.is_none() {
                    *slot = held.0.try_wait().expect("a supervisor's status");
                }
            }
            if let Some(exit) = cli.0.try_wait().expect("the cli's status") {
                break exit;
            }
            assert!(Instant::now() < until, "kill did not finish");
            std::thread::sleep(Duration::from_millis(20));
        };
        let (mut out, mut err) = (String::new(), String::new());
        {
            use std::io::Read as _;
            cli.0
                .stdout
                .take()
                .expect("stdout")
                .read_to_string(&mut out)
                .expect("stdout text");
            cli.0
                .stderr
                .take()
                .expect("stderr")
                .read_to_string(&mut err)
                .expect("stderr text");
        }
        assert_eq!(exit.code(), Some(0), "stdout: {out}\nstderr: {err}");
        for rig in &rigs {
            let line = format!("{}: ended (killed)", rig.id);
            assert_eq!(
                out.lines().filter(|l| *l == line).count(),
                1,
                "no single `{line}` line: {out}"
            );
        }
        assert_eq!(out.lines().count(), rigs.len(), "{out}");

        // Each supervisor ended by itself, through its teardown: a normal exit, not a SIGKILL.
        for ((slot, held), rig) in statuses.iter().zip(held.iter_mut()).zip(&rigs) {
            let status = slot.unwrap_or_else(|| held.0.wait().expect("a status"));
            assert_eq!(status.signal(), None, "{} was killed by a signal", rig.id);
            assert!(
                status.success(),
                "the supervisor of {} ended {status:?}: {}",
                rig.id,
                text_of(&rig.work.join("child.err"))
            );
        }
        for (pid, before) in &starts {
            assert_ne!(
                process_start(*pid),
                *before,
                "supervisor {pid} is still there"
            );
        }
        for rig in &rigs {
            assert_eq!(
                classify(&rig.dir).expect("a class"),
                JobLiveness::Ended(JobEnd::Killed),
                "{}",
                rig.id
            );
            // The end is the supervisor's own: its log holds the two events it writes and the
            // steps of the reap that `kill` records, and nothing a recovery writes.
            let (kinds, ends) = events_of(&rig.dir);
            assert!(kinds.len() >= 2, "{}: {kinds:?}", rig.id);
            // INDEX: the log holds at least `job.started` and `job.ended`, asserted just above
            assert_eq!(kinds[..2], ["job.started", "job.ended"], "{}", rig.id);
            assert!(
                kinds[2..].iter().all(|kind| kind == "process.stop_step"),
                "{}: {kinds:?}",
                rig.id
            );
            assert_eq!(ends, [JobEnd::Killed], "{}", rig.id);
            assert_eq!(JobLock::probe(&rig.dir).expect("a probe"), LockState::Free);
        }

        // No destroy verb reached the shim, and nothing it refused.
        let calls = text_of(&bin.join("docker.log"));
        assert!(
            !calls.contains("REFUSED")
                && calls.lines().all(|line| !line
                    .split_whitespace()
                    .any(|w| matches!(w, "rm" | "stop" | "kill" | "prune" | "rmi" | "restart"))),
            "a destroy verb or a refused call reached docker: {calls}"
        );
    }

    /// Guards: B41's shim — the docker that B41 puts first on `PATH` refuses every destroy verb
    /// and every host or context form, wherever the flag stands, and lets the read verbs pass.
    /// Oracle: the shim's exit status, run by `env -i` over a matrix of argument lists; the
    /// read verbs are the positive controls. The shim forwards nothing, so no docker is run.
    /// Bite: the shim answers a destroy verb or an attached `-H` form with status 0.
    #[test]
    fn the_shim_refuses_destroys_and_hosts() {
        let tmp = tempfile::tempdir().expect("a temporary directory");
        shim_docker(tmp.path());
        let shim = tmp.path().join("docker");
        let run = |args: &[&str]| {
            let (status, _) = bounded(Command::new("/usr/bin/env").arg("-i").arg(&shim).args(args));
            status.code()
        };
        let refused: [&[&str]; 15] = [
            &["rm", "x"],
            &["rm", "-f", "x"],
            &["stop", "x"],
            &["kill", "x"],
            &["rmi", "x"],
            &["container", "rm", "x"],
            &["network", "rm", "x"],
            &["system", "prune"],
            &["-H", "tcp://x", "ps"],
            &["ps", "-Hx"],
            &["ps", "-H=x"],
            &["ps", "-a", "-Htcp://x"],
            &["--host", "x", "ps"],
            &["--host=x", "ps"],
            &["ps", "--context", "x"],
        ];
        for args in refused {
            assert_eq!(run(args), Some(1), "{args:?} was not refused");
        }
        assert_eq!(run(&["--context=x", "ps"]), Some(1), "--context=x");
        let allowed: [&[&str]; 5] = [
            &["ps"],
            &["inspect", "x"],
            &["version"],
            &["info"],
            &["container", "ls"],
        ];
        for args in allowed {
            assert_eq!(run(args), Some(0), "{args:?} was refused");
        }
    }
}
