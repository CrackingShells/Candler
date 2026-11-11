//! Guards for the verbs that observe: `resolve` prints every value with its source from a real
//! preflight and starts nothing, `ls` lists the experiments of a project, `watch` prints JSON lines
//! by default and progress with `--live`, and `ps` shows what each job runs. The binary is the
//! real one, run from a directory outside the project with the PATH-fake docker client of
//! `support/cli_run.rs`.
#![cfg(test)]

#[expect(
    dead_code,
    reason = "this guard uses the world and its launch, not every helper of the shared module"
)]
#[path = "support/cli_run.rs"]
mod cli_run;

// The terminal rules are pure and use nothing of the binary, so the guard reads the file itself:
// the binary's modules are not reachable from a test crate.
#[expect(
    dead_code,
    reason = "this guard reads the rules, not the painter every verb uses"
)]
#[path = "../src/commands/term.rs"]
mod term;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::JobEvents;
use candler_core::jobs::lock::{JobLock, LockState};
use candler_core::jobs::model::JobSpec;
use candler_core::resolve::layers::Resolution;
use candler_core::runner::plan::experiment_of;
use candler_core::store::Store;
use candler_core::store::layout::StoreLayout;
use candler_core::store::run::{Existing, RunWriter};
use candler_schemas::execution::{
    EventPayload, ExperimentFile, JobEnd, JobRecord, JobState, RunManifest, SupervisorIdentity,
};
use candler_schemas::primitives::{Digest, JobId};
use candler_schemas::process::ProcessStart;
use serde_json::{Value, json};
use tempfile::TempDir;

use cli_run::{SECRET_NAME, World, wait_for, world};

/// The value the fixture secret holds in the worlds of this file.
const TOKEN: &str = "fixture-token-0123456789";

/// A stand-in for the endpoint's model list: it answers `200` to the right bearer token and `401`
/// to any other, and keeps the request line and verdict of every request it saw.
struct Models {
    port: u16,
    seen: Arc<Mutex<Vec<(String, bool)>>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Models {
    fn start(token: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen: Arc<Mutex<Vec<(String, bool)>>> = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let want = format!("Bearer {token}");
        let (log, halt) = (Arc::clone(&seen), Arc::clone(&stop));
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if halt.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(stream) = stream else { continue };
                Self::answer(stream, &want, &log);
            }
        });
        Self {
            port,
            seen,
            stop,
            handle: Some(handle),
        }
    }

    fn answer(stream: TcpStream, want: &str, log: &Mutex<Vec<(String, bool)>>) {
        let mut reader = BufReader::new(stream);
        let mut first = String::new();
        if reader.read_line(&mut first).is_err() {
            return;
        }
        let mut ok = false;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case("authorization")
            {
                ok = value.trim() == want;
            }
        }
        log.lock().unwrap().push((first.trim().to_owned(), ok));
        let reply = if ok {
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
        } else {
            "HTTP/1.1 401 Unauthorized\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
        };
        drop(reader.get_mut().write_all(reply.as_bytes()));
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn requests(&self) -> Vec<(String, bool)> {
        self.seen.lock().unwrap().clone()
    }
}

impl Drop for Models {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        drop(TcpStream::connect(("127.0.0.1", self.port)));
        if let Some(handle) = self.handle.take() {
            drop(handle.join());
        }
    }
}

/// `candler resolve` on the world's experiment, from the world's working directory, with the
/// configuration home inside the world.
fn resolve(world: &World, endpoint: &str, extra: &[&str], secret_value: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_candler"));
    command
        .arg("resolve")
        .arg(&world.experiment)
        .arg("--set")
        .arg("isolation.build.lock=env.lock")
        .arg("--set")
        .arg(format!("model.endpoint={endpoint}"))
        .arg("--set")
        .arg(format!("model.token_secret={SECRET_NAME}"))
        .args(extra)
        .current_dir(&world.work)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", world.bin.display()))
        .env("HOME", world.root.join("home"))
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CONFIG_HOME", world.root.join("cfg"))
        .stdin(Stdio::null());
    if let Some(value) = secret_value {
        command.env(SECRET_NAME, value);
    }
    command.output().unwrap()
}

/// The lines of `out` split at tabs, by their first column.
fn table(out: &Output) -> BTreeMap<String, Vec<String>> {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|line| {
            let cols: Vec<String> = line.split('\t').map(str::to_owned).collect();
            (cols[0].clone(), cols)
        })
        .collect()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Writes `text` to `path`, making the directory first.
fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// The docker calls of `world` that create or start a container.
fn container_calls(world: &World) -> Vec<String> {
    world
        .calls()
        .lines()
        .filter(|line| {
            line.starts_with("create ")
                || line.starts_with("start ")
                || line.starts_with("network create")
                || line.contains("--cidfile")
        })
        .map(str::to_owned)
        .collect()
}

/// Guards: O1 — `candler resolve` prints every value and setting with its source from a real
/// preflight, from a directory outside the project, and starts no job and no agent. A project
/// `candler.toml` gives `plugins.dirs`, a user configuration gives `slots.count`, `--set` gives
/// one value, the experiment gives its own, the fake docker's image id is derived and the store
/// root is the core's default; a second run with `store.root` in the project shows that source.
/// The model list stand-in sees one `GET /v1/models` with the right token and nothing else.
///
/// Oracle: hand-written expectations (the source of each path, as the layers' order gives it);
/// the fake docker's call log and its list of handed-out containers; the store's directory.
///
/// Bite: a value printed with no source; a job directory written; an agent container created.
#[test]
fn o1_resolve_prints_every_value_with_its_source_and_starts_no_job() {
    let world = world();
    let models = Models::start(TOKEN);
    fs::create_dir_all(world.work.join("plugins")).unwrap();
    write(
        &world.work.join("candler.toml"),
        "[plugins]\ndirs = [\"plugins\"]\n",
    );
    write(
        &world.root.join("cfg").join("candler").join("config.toml"),
        "[slots]\ncount = 2\n",
    );
    let out = resolve(
        &world,
        &models.url(),
        &["--set", "prompt.template=\"overridden\""],
        Some(TOKEN),
    );
    let stderr = text(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout: {}\nstderr: {stderr}\ncalls:\n{}",
        text(&out.stdout),
        world.calls()
    );
    let lines = table(&out);
    for (path, cols) in &lines {
        assert_eq!(cols.len(), 3, "{path}: path, value and source: {cols:?}");
        assert!(
            !cols[1].is_empty() && !cols[2].is_empty(),
            "{path}: {cols:?}"
        );
    }
    let sorted: Vec<&String> = lines.keys().collect();
    let printed: Vec<String> = text(&out.stdout)
        .lines()
        .map(|l| l.split('\t').next().unwrap().to_owned())
        .collect();
    assert_eq!(
        printed.iter().collect::<Vec<_>>(),
        sorted,
        "the lines are sorted by path"
    );
    let source = |path: &str| {
        lines
            .get(path)
            .unwrap_or_else(|| panic!("no line for {path}: {lines:?}"))[2]
            .clone()
    };
    assert_eq!(source("prompt.template"), "explicit_argument");
    assert_eq!(source("permission_mode"), "experiment_file");
    assert_eq!(source("plugins.dirs"), "project_config");
    assert_eq!(source("slots.count"), "user_config");
    assert_eq!(source("store.root"), "core_default");
    assert!(
        source("isolation.image").starts_with("derived image_id@1"),
        "{}",
        source("isolation.image")
    );
    assert_eq!(lines["prompt.template"][1], "\"overridden\"");
    assert_eq!(lines["slots.count"][1], "2");

    assert_eq!(container_calls(&world), Vec::<String>::new());
    assert!(
        world.handed_out().is_empty(),
        "an agent container was created"
    );
    assert!(
        !world
            .work
            .join(".candler")
            .join("store")
            .join("jobs")
            .exists(),
        "a job directory was written"
    );
    assert_eq!(
        models.requests(),
        [("GET /v1/models HTTP/1.1".to_owned(), true)],
        "one authenticated read of the model list and nothing else"
    );

    // The project names the store: its source is the project configuration.
    write(
        &world.work.join("candler.toml"),
        "[store]\nroot = \"mystore\"\n",
    );
    let out = resolve(&world, &models.url(), &[], Some(TOKEN));
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(table(&out)["store.root"][2], "project_config");
    assert!(!world.work.join("mystore").join("jobs").exists());
    world.remove();
}

/// Guards: O1 — `candler resolve` reports every failure at once: with two declared secrets unset,
/// an unreachable endpoint and a `plugins.dirs` entry that is not a directory, one run prints a
/// line for each with its check id, in the same output, and exits 2. A refused token is named as
/// refused, not as unreachable.
///
/// Oracle: hand-written expectations (the check ids and the two secret names).
///
/// Bite: the verb stopping at the first failure.
#[test]
fn o1_resolve_reports_every_failure_at_once_with_codes() {
    let world = world();
    let text_of = fs::read_to_string(&world.experiment).unwrap();
    let declared = format!("secrets = [\"{SECRET_NAME}\"]");
    assert!(text_of.contains(&declared));
    fs::write(
        &world.experiment,
        text_of.replace(&declared, "secrets = [\"ALPHA_SECRET\", \"BETA_SECRET\"]"),
    )
    .unwrap();
    write(
        &world.work.join("candler.toml"),
        "[plugins]\ndirs = [\"not_a_directory\"]\n",
    );
    let out = resolve(
        &world,
        "http://127.0.0.1:9",
        &["--set", "model.token_secret=ALPHA_SECRET"],
        None,
    );
    let stdout = text(&out.stdout);
    let stderr = text(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stdout}\n{stderr}");
    for id in [
        "secrets.secret_ALPHA_SECRET",
        "secrets.secret_BETA_SECRET",
        "plugins.dir_missing",
        "gateway.gateway_token",
    ] {
        assert!(
            stdout.lines().any(|line| line.contains(id)),
            "no line for {id}:\n{stdout}"
        );
    }
    assert!(stderr.contains("refused"), "{stderr}");
    assert!(!stdout.contains(TOKEN) && !stderr.contains(TOKEN));

    // A token the endpoint refuses is a refusal, and the same run still names the other failures.
    let models = Models::start("some-other-token");
    let out = resolve(
        &world,
        &models.url(),
        &["--set", "model.token_secret=ALPHA_SECRET"],
        None,
    );
    assert_eq!(out.status.code(), Some(2));
    drop(models);
    world.remove();
}

/// Guards: O1 — the gateway check reads the endpoint's model list with the secret as a bearer
/// token: the right token passes, a wrong one fails as refused (not as unreachable), and neither
/// the token nor the model list's body reaches the output. Oracle: a stand-in endpoint that
/// answers 200 only to the right token. Bite: a 401 read as a pass; the token printed.
#[test]
fn o1_resolve_checks_the_gateway_token_by_a_model_list_read() {
    let world = world();
    let models = Models::start("the-right-token");
    let out = resolve(&world, &models.url(), &[], Some(TOKEN));
    let stdout = text(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(2),
        "{stdout}\n{}",
        text(&out.stderr)
    );
    let refused: Vec<&str> = stdout
        .lines()
        .filter(|line| line.contains("gateway.gateway_token"))
        .collect();
    assert_eq!(refused.len(), 1, "{stdout}");
    assert!(
        refused[0].contains("the token was refused"),
        "{}",
        refused[0]
    );
    assert!(!stdout.contains(TOKEN) && !text(&out.stderr).contains(TOKEN));
    assert_eq!(
        models.requests(),
        [("GET /v1/models HTTP/1.1".to_owned(), false)]
    );
    world.remove();
}

/// Copies the tree `from` to `to`.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("observe")
}

/// `candler ls` with `args` from `cwd`, the home and the state inside `home`.
fn ls(cwd: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .arg("--store")
        .arg(home.join("store"))
        .arg("ls")
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", home)
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_CONFIG_HOME", home.join("cfg"))
        .output()
        .unwrap()
}

/// The names of the files `ls` listed as experiments, with their cells and runs.
fn listed(out: &Output) -> BTreeMap<String, (String, String)> {
    text(&out.stdout)
        .lines()
        .filter_map(|line| {
            let cols: Vec<&str> = line.split('\t').collect();
            let name = Path::new(cols[0])
                .file_name()?
                .to_string_lossy()
                .into_owned();
            (cols.get(1) != Some(&"skipped"))
                .then(|| (name, (cols[1].to_owned(), cols[2].to_owned())))
        })
        .collect()
}

/// One sealed run in the store at `root` whose recorded values are those of the first cell of
/// `experiments/a.toml` of the fixture: `model.id` m1, `permission_mode` plan, the prompt
/// "say hello" and the cap of 30 seconds.
fn seed_run_of_a(root: &Path) {
    let store = Store::init(root).unwrap();
    let digest = |c: char| format!("sha256:{}", String::from(c).repeat(64));
    let body = |cell: &str| {
        json!({
            "schema": "run-manifest/1",
            "run": {"cell": cell, "replicate": 1, "attempt": 1},
            "experiment": digest('0'),
            "factors": [],
            "comparable": {
                "values": [
                    {"path": "budget.per_run.wall_s", "value": 30},
                    {"path": "model.id", "value": "m1"},
                    {"path": "permission_mode", "value": "plan"},
                    {"path": "prompt.template", "value": "say hello"}
                ],
                "harness": {"name": "h", "version": "1.0.0"},
                "treatment": {"name": "t"},
                "snapshots": [],
                "inputs": [],
                "environments": []
            },
            "sources": [
                {"path": "budget.per_run.wall_s", "source": {"kind": "experiment_file"}},
                {"path": "model.id", "source": {"kind": "experiment_file"}},
                {"path": "permission_mode", "source": {"kind": "experiment_file"}},
                {"path": "prompt.template", "source": {"kind": "experiment_file"}},
                {"path": "harness", "source": {"kind": "experiment_file"}},
                {"path": "treatment", "source": {"kind": "experiment_file"}}
            ],
            "secrets": [],
            "measured": [],
            "deviations": [],
            "integrity": {"kind": "valid"}
        })
    };
    let probe: RunManifest = serde_json::from_value(body(&digest('f'))).unwrap();
    let cell = probe.cell_digest().to_string();
    let manifest: RunManifest = serde_json::from_value(body(cell.as_str())).unwrap();
    RunWriter::create(&store, &manifest, Existing::Refuse)
        .unwrap()
        .done()
        .unwrap();
}

/// Guards: O2 — `candler ls` lists the experiments under `<project root>/experiments/` and the
/// paths it is given, from a directory inside the project but not at its root, each with its
/// declared cells and the stored runs that match it. The project's `experiments/` has `a` (two
/// cells) and `sub/b`; `elsewhere/c` and `other/d` and the working directory's own `e` are not
/// listed unless a path names them; a path that is a file lists that file. A stored run that
/// records the values of one cell of `a` counts for `a` and not for `b`.
///
/// Oracle: a fixture tree (`fixtures/observe/ls`), with the cells and runs counted by hand.
///
/// Bite: a scan of the whole project (it finds `d`); a scan of the working directory (it finds `e`).
#[test]
fn o2_ls_lists_experiments_under_experiments_and_the_paths_given() {
    let scratch = TempDir::new().unwrap();
    let project = scratch.path().join("project");
    copy_tree(&fixtures().join("ls"), &project);
    let work = project.join("work");

    let out = ls(&work, scratch.path(), &[]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let found = listed(&out);
    assert_eq!(
        found.keys().map(String::as_str).collect::<Vec<_>>(),
        ["a.toml", "b.toml"],
        "{}",
        text(&out.stdout)
    );
    assert_eq!(found["a.toml"], ("2".to_owned(), "0".to_owned()));
    assert_eq!(found["b.toml"], ("1".to_owned(), "0".to_owned()));

    let out = ls(&work, scratch.path(), &["../elsewhere"]);
    let found = listed(&out);
    assert_eq!(
        found.keys().map(String::as_str).collect::<Vec<_>>(),
        ["a.toml", "b.toml", "c.toml"]
    );
    let out = ls(&work, scratch.path(), &["../other/d.toml"]);
    assert!(listed(&out).contains_key("d.toml"), "{}", text(&out.stdout));
    assert!(!listed(&out).contains_key("c.toml"));

    seed_run_of_a(&scratch.path().join("store"));
    let found = listed(&ls(&work, scratch.path(), &[]));
    assert_eq!(found["a.toml"].1, "1", "the run matches a cell of a");
    assert_eq!(found["b.toml"].1, "0", "and does not match b");
}

/// Guards: O2 — `candler ls` lists a TOML file that is no experiment as `skipped` with the reason
/// the parser gives, never silently; with no project it looks at `./experiments/` of the working
/// directory and nothing else, and a missing `experiments/` is an empty listing with exit 0.
///
/// Oracle: a fixture tree whose `notes.toml` is not an experiment; hand-written expectations.
///
/// Bite: the skipped file dropped from the listing; the reason lost.
#[test]
fn o2_ls_skips_other_toml_with_its_reason_and_never_scans_the_cwd() {
    let scratch = TempDir::new().unwrap();
    let project = scratch.path().join("project");
    copy_tree(&fixtures().join("ls"), &project);
    let out = ls(&project.join("work"), scratch.path(), &[]);
    let stdout = text(&out.stdout);
    let skipped: Vec<Vec<&str>> = stdout
        .lines()
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .filter(|cols| cols.get(1) == Some(&"skipped"))
        .collect();
    assert_eq!(skipped.len(), 1, "{stdout}");
    assert!(skipped[0][0].ends_with("notes.toml"));
    assert!(
        skipped[0].get(2).is_some_and(|reason| reason.len() > 10),
        "the reason is the parser's words: {:?}",
        skipped[0]
    );

    // No project above the working directory: only `./experiments/`.
    let lone = scratch.path().join("lone");
    copy_tree(
        &fixtures().join("ls").join("elsewhere"),
        &lone.join("experiments"),
    );
    copy_tree(&fixtures().join("ls").join("other"), &lone);
    let out = ls(&lone, scratch.path(), &[]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(
        listed(&out).keys().map(String::as_str).collect::<Vec<_>>(),
        ["c.toml"],
        "the working directory's own d.toml is not listed"
    );
    let empty = scratch.path().join("empty");
    fs::create_dir_all(&empty).unwrap();
    let out = ls(&empty, scratch.path(), &[]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty(), "{}", text(&out.stdout));
}

