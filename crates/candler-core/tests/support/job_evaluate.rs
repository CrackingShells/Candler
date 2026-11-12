//! Helpers for the guards of the evaluate job: the rig (a store, a plug-in directory and a built
//! plug-in environment), the supervisor child the guards start, and the entries that child runs.
//! Builders and fixtures, never expected values.
#![allow(dead_code, reason = "each guard binary uses a subset of the helpers")]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use candler_core::evaluation::engine::RunSelection;
use candler_core::evaluation::registry::EnvironmentKey;
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::{JobEvents, TailPos};
use candler_core::jobs::model::EvaluateSpec;
use candler_core::jobs::runtime::evaluate::{EvaluateEntry, supervise_evaluate};
use candler_core::jobs::runtime::session::recover_job_dir;
use candler_core::store::Store;
use candler_schemas::execution::{EventPayload, JobEnd, JobState};
use candler_schemas::primitives::{Digest, JobId};
use serde_json::{Value, json};
use sha2::Digest as _;

use crate::legacy_import::{import_with_reader, mini};
use crate::support::job_runtime::HeldChild;
use crate::support::job_store::{job_id, job_record};

/// The job the supervisor child runs.
pub const ENV_JOB: &str = "CANDLER_TEST_EVALUATE_JOB";
/// The store the child opens.
pub const ENV_STORE: &str = "CANDLER_TEST_STORE";
/// The file that holds the spec the child reads, as JSON.
pub const ENV_SPEC: &str = "CANDLER_TEST_EVALUATE_SPEC";
/// The directory the child writes what it saw in.
pub const ENV_WORK: &str = "CANDLER_TEST_WORK";
/// The job the recovery child recovers.
pub const ENV_RECOVER: &str = "CANDLER_TEST_RECOVER_JOB";

/// The root of the repository.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The helper's project, from which every plug-in runs.
pub fn helper() -> PathBuf {
    repo().join("python/candler-evaluator")
}

/// The fixtures of the engine guards, which these guards reuse.
pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine")
}

/// The digest of `bytes`, taken here.
pub fn digest_of(bytes: &[u8]) -> Digest {
    let mut hex = String::new();
    for byte in sha2::Sha256::digest(bytes) {
        hex.extend(char::from_digit(u32::from(byte >> 4), 16));
        hex.extend(char::from_digit(u32::from(byte & 0xf), 16));
    }
    Digest::from_hex(&hex).expect("a digest")
}

/// The entry of the supervisor child: it runs `supervise_evaluate` over the job, the store and the
/// spec its variables name, and writes what came back to `returned.json` in its work directory:
/// `{"end": …, "document": …}`, or `{"error": …}` when the supervisor returned an error. It does
/// nothing unless `CANDLER_TEST_EVALUATE_JOB` is set.
pub fn evaluate_entry() {
    let Ok(job) = std::env::var(ENV_JOB) else {
        return;
    };
    let root = std::env::var(ENV_STORE).expect("a store");
    let spec_path = std::env::var(ENV_SPEC).expect("a spec file");
    let work = PathBuf::from(std::env::var(ENV_WORK).expect("a work directory"));
    let spec: EvaluateSpec =
        serde_json::from_slice(&fs::read(&spec_path).expect("the spec file")).expect("a spec");
    let entry = EvaluateEntry {
        store: Store::open(Path::new(&root)).expect("a store"),
        job: job.parse().expect("a job id"),
        spec,
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let returned = match runtime.block_on(supervise_evaluate(entry)) {
        Ok(outcome) => json!({
            "end": serde_json::to_value(outcome.end).expect("an end"),
            "document": outcome.report.to_document(),
        }),
        Err(error) => json!({"error": error.to_string()}),
    };
    fs::write(work.join("returned.json"), returned.to_string()).expect("the result file");
}

/// The entry of the recovery child: it recovers the lost job its variables name, the way the
/// supervisor's sweep of lost jobs does, and writes the report to `recovered.txt`. It runs in a
/// child whose `PATH` holds a fake `docker`, because the recovery looks for one.
pub fn recover_entry() {
    let Ok(job) = std::env::var(ENV_RECOVER) else {
        return;
    };
    let root = std::env::var(ENV_STORE).expect("a store");
    let work = PathBuf::from(std::env::var(ENV_WORK).expect("a work directory"));
    let store = Store::open(Path::new(&root)).expect("a store");
    let job: JobId = job.parse().expect("a job id");
    let dir = JobDir::open(store.layout(), &job).expect("the job directory");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let text = match runtime.block_on(recover_job_dir(&dir)) {
        Ok(report) => format!("{report:?}"),
        Err(reason) => format!("error {reason}"),
    };
    fs::write(work.join("recovered.txt"), text).expect("the result file");
}

/// A store with the mini-corpus, a plug-in directory for the `pool` plug-in, its built environment
/// and a directory to work in.
pub struct EvalRig {
    /// Keeps the scratch directory alive.
    pub tmp: tempfile::TempDir,
    /// The store root.
    pub store_dir: PathBuf,
    /// The store.
    pub store: Store,
    /// The directory that holds the plug-in's manifest.
    pub plugin_dir: PathBuf,
    /// The root of the plug-in environments.
    pub env_root: PathBuf,
    /// Where suites, spec files and what the child writes go.
    pub work: PathBuf,
}

impl EvalRig {
    /// Builds the rig: the import of the mini-corpus, the manifest of `pool` with its paths
    /// filled in, and the environment of the helper, synced by `uv` and marked `ready`.
    pub async fn new() -> Self {
        let tmp = tempfile::tempdir().expect("a scratch directory");
        let store_dir = tmp.path().join("store");
        fs::create_dir(&store_dir).expect("a store directory");
        let store = Store::init(&store_dir).expect("the store initialises");
        import_with_reader(&mini(), &store).expect("the corpus imports");

        let plugin_dir = tmp.path().join("plugin");
        fs::create_dir(&plugin_dir).expect("a plug-in directory");
        std::os::unix::fs::symlink(helper(), plugin_dir.join("env")).expect("a link to the helper");
        let manifest = fs::read_to_string(fixtures().join("plugin_pool.toml"))
            .expect("the manifest template reads")
            .replace(
                "@EXAMPLES@",
                &helper().join("examples").display().to_string(),
            )
            .replace("@KINDS@", &fixtures().join("kinds").display().to_string());
        fs::write(plugin_dir.join("plugin.toml"), manifest).expect("the manifest writes");

        let env_root = tmp.path().join("envs");
        let key =
            EnvironmentKey::of_files(&helper().join("uv.lock"), &helper().join("pyproject.toml"))
                .await
                .expect("the environment key");
        let dir = key.dir(&env_root);
        fs::create_dir_all(&dir).expect("an environment directory");
        let synced = Command::new("uv")
            .args(["sync", "--frozen"])
            .env("UV_PROJECT_ENVIRONMENT", dir.join(".venv"))
            .current_dir(helper())
            .status()
            .expect("uv runs");
        assert!(synced.success(), "uv sync built the helper's environment");
        fs::write(dir.join("ready"), key.digest()).expect("the ready marker writes");

        let work = tmp.path().join("work");
        fs::create_dir(&work).expect("a work directory");
        Self {
            tmp,
            store_dir,
            store,
            plugin_dir,
            env_root,
            work,
        }
    }

    /// Writes the fixture suite `name` with every `(token, text)` of `fills` replaced and returns
    /// its path.
    pub fn suite(&self, name: &str, fills: &[(&str, String)]) -> PathBuf {
        let mut text = fs::read_to_string(fixtures().join(name)).expect("the suite reads");
        for (token, fill) in fills {
            text = text.replace(token, fill);
        }
        let path = self.work.join(name);
        fs::write(&path, text).expect("the suite writes");
        path
    }

    /// The spec of a job that grades `selection` with the suite file `suite`.
    pub fn spec(&self, suite: &Path, selection: RunSelection) -> EvaluateSpec {
        EvaluateSpec {
            suite: suite.to_path_buf(),
            suite_digest: digest_of(&fs::read(suite).expect("the suite reads")),
            plugin_dirs: vec![self.plugin_dir.clone()],
            env_root: self.env_root.clone(),
            selection,
            request_deadline_secs: Some(600),
        }
    }

    /// A job directory for job `n`, with a running record, in the rig's store.
    pub fn job(&self, n: u64) -> (JobId, JobDir) {
        let job = job_id(n);
        let dir = JobDir::create(
            self.store.layout(),
            job.clone(),
            &job_record(&job, JobState::Running),
        )
        .expect("a job directory");
        (job, dir)
    }

    /// The user configuration directory of the child, with `config` as its `config.toml` when
    /// there is one.
    pub fn config_home(&self, config: Option<&str>) -> PathBuf {
        let home = self.tmp.path().join("config");
        fs::create_dir_all(home.join("candler")).expect("a config directory");
        if let Some(text) = config {
            fs::write(home.join("candler").join("config.toml"), text).expect("a config file");
        }
        home
    }

    /// Starts the supervisor child of `job` with `spec`: this test binary again, running
    /// `entry`, with the state, config and work directories the rig names. Its error output goes
    /// to `child.err` in the work directory.
    pub fn start(
        &self,
        job: &JobId,
        spec: &EvaluateSpec,
        entry: &str,
        config: Option<&str>,
    ) -> HeldChild {
        let spec_file = self.work.join(format!("{job}.spec.json"));
        fs::write(&spec_file, serde_json::to_vec(spec).expect("a spec")).expect("the spec file");
        let child = Command::new(std::env::current_exe().expect("this binary"))
            .args(["--exact", entry, "--nocapture"])
            .env_clear()
            .env(ENV_JOB, job.to_string())
            .env(ENV_STORE, &self.store_dir)
            .env(ENV_SPEC, &spec_file)
            .env(ENV_WORK, &self.work)
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", std::env::var_os("HOME").unwrap_or_default())
            .env("XDG_STATE_HOME", self.tmp.path().join("state"))
            .env("XDG_CONFIG_HOME", self.config_home(config))
            .stdin(Stdio::null())
            .stdout(fs::File::create(self.work.join("child.out")).expect("a log"))
            .stderr(fs::File::create(self.work.join("child.err")).expect("a log"))
            .spawn()
            .expect("the supervisor");
        HeldChild::new(child)
    }

    /// Writes a `docker` that holds nothing, as a daemon with no container, network or image
    /// does: `network` and `image` calls say there is no such object, every other call prints
    /// nothing. Returns its directory.
    fn fake_docker(&self) -> PathBuf {
        let bin = self.tmp.path().join("bin");
        fs::create_dir_all(&bin).expect("a directory");
        let client = bin.join("docker");
        fs::write(
            &client,
            "#!/bin/sh\ncase \"$1\" in\nnetwork | image)\n    echo 'Error: No such object' >&2\n    exit 1\n    ;;\nesac\n",
        )
        .expect("a client");
        fs::set_permissions(&client, fs::Permissions::from_mode(0o755)).expect("a mode");
        bin
    }

    /// Starts the recovery child for `job`, with a fake `docker` first on its `PATH`, and waits
    /// for it. Returns the text it wrote.
    pub fn recover(&self, job: &JobId, entry: &str) -> String {
        let bin = self.fake_docker();
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut child = HeldChild::new(
            Command::new(std::env::current_exe().expect("this binary"))
                .args(["--exact", entry, "--nocapture"])
                .env_clear()
                .env(ENV_RECOVER, job.to_string())
                .env(ENV_STORE, &self.store_dir)
                .env(ENV_WORK, &self.work)
                .env("PATH", path)
                .env("HOME", std::env::var_os("HOME").unwrap_or_default())
                .stdin(Stdio::null())
                .stdout(fs::File::create(self.work.join("recover.out")).expect("a log"))
                .stderr(fs::File::create(self.work.join("recover.err")).expect("a log"))
                .spawn()
                .expect("the recovery"),
        );
        let status = wait_exit(&mut child, Duration::from_secs(120)).expect("the recovery ends");
        assert!(
            status.success(),
            "the recovery ended {status:?}: {}",
            fs::read_to_string(self.work.join("recover.err")).unwrap_or_default()
        );
        fs::read_to_string(self.work.join("recovered.txt")).expect("what the recovery wrote")
    }

    /// What the supervisor child wrote to `returned.json`.
    pub fn returned(&self) -> Value {
        serde_json::from_slice(&fs::read(self.work.join("returned.json")).expect("returned.json"))
            .expect("JSON")
    }

    /// The error output of the supervisor child.
    pub fn child_err(&self) -> String {
        fs::read_to_string(self.work.join("child.err")).unwrap_or_default()
    }
}

/// Waits up to `limit` for `child` to exit.
pub fn wait_exit(child: &mut HeldChild, limit: Duration) -> Option<std::process::ExitStatus> {
    let until = Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait().expect("a wait") {
            return Some(status);
        }
        if Instant::now() >= until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The events of the job in `dir`: their kinds, and each event's payload.
pub fn events_of(dir: &JobDir) -> Vec<EventPayload> {
    JobEvents::open(dir)
        .expect("the log")
        .tail(TailPos(0))
        .expect("the events")
        .events
        .iter()
        .map(|envelope| envelope.event().clone()) // BORROW: the list owns the payloads
        .collect()
}

/// The reason of the last `job.ended` among `events`.
pub fn end_of(events: &[EventPayload]) -> Option<JobEnd> {
    events.iter().rev().find_map(|event| {
        if let EventPayload::JobEnded { reason, .. } = event {
            Some(*reason)
        } else {
            None
        }
    })
}
