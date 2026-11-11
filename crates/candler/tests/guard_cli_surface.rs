#![cfg(test)]
#![cfg(unix)]
//! Guards for the surface every verb of the `candler` binary shares: each verb has a row in the
//! working-directory matrix, each option's help ends with a stamp that says why the option exists,
//! a copy of the binary runs outside its source tree, and `show` and `diff` take a bare version.
//! The binary is the real one; the verbs and options are read from its `--help`, so a verb or an
//! option added later is found without editing the guards.

#[expect(
    dead_code,
    reason = "this guard uses the world and its fake client, not every helper of the shared module"
)]
#[path = "support/cli_run.rs"]
mod cli_run;

#[expect(
    dead_code,
    reason = "this guard uses the helper's environment and its wait, not every helper"
)]
#[path = "support/seam.rs"]
mod seam;

use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::lock::{JobLock, LockState};
use candler_core::store::Store;
use candler_core::store::layout::StoreLayout;
use candler_core::store::outcome::OutcomeWriter;
use candler_core::store::run::{Existing, RunWriter};
use candler_schemas::execution::RunManifest;
use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::{JobId, NameVersion, RequestId, RunKey};
use serde_json::{Value, json};
use tempfile::TempDir;

use cli_run::{SECRET_NAME, World, wait_for, world};

/// The two stamps an option's help may end with.
const STAMPS: [&str; 2] = ["cannot be derived: ", "not an experiment setting: "];

/// The real binary with `args`.
fn candler(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .args(args)
        .output()
        .expect("the binary runs")
}

/// The text of `candler <args> --help`.
fn help(args: &[&str]) -> String {
    let mut all = args.to_vec(); // BORROW: the call line is extended
    all.push("--help");
    let output = candler(&all);
    assert!(output.status.success(), "{all:?} --help must succeed");
    String::from_utf8(output.stdout).expect("help is text")
}

/// The verbs in the `Commands:` block of `candler --help`, minus `help`.
fn listed_verbs() -> BTreeSet<String> {
    help(&[])
        .lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .filter(|verb| *verb != "help")
        .map(str::to_owned)
        .collect()
}

/// The options of a help text, by long name, with the help of each: the lines of its block
/// joined by a space, without the `[default: …]`, `[env: …]` and `[possible values: …]` notes.
fn options_of(help: &str) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = Vec::new();
    let mut in_options = false;
    for line in help.lines() {
        if !line.starts_with(' ') && !line.is_empty() {
            in_options = line == "Options:";
            continue;
        }
        if !in_options {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let text = line.trim();
        if (2..=6).contains(&indent) && text.starts_with('-') {
            let name = text
                .split([' ', ','])
                .find_map(|word| word.strip_prefix("--"))
                .unwrap_or_default();
            // The short form of the help puts the option's words after the declaration and two
            // spaces; the long form puts them on the lines that follow.
            let inline = text.split_once("  ").map_or("", |(_, rest)| rest.trim());
            found.push((name.to_owned(), inline.to_owned())); // BORROW: the entry owns its text
        } else if let Some((_, body)) = found.last_mut()
            && !text.is_empty()
        {
            body.push(' ');
            body.push_str(text);
        }
    }
    found
        .into_iter()
        .map(|(name, body)| (name, strip_notes(&body)))
        .collect()
}

/// `body` without its bracketed notes and without a list of possible values.
fn strip_notes(body: &str) -> String {
    let body = body.split(" Possible values:").next().unwrap_or_default();
    let mut out = String::new();
    let mut rest = body;
    while let Some(open) = rest.find('[') {
        out.push_str(rest.get(..open).unwrap_or_default());
        rest = rest.get(open..).unwrap_or_default();
        match rest.find(']') {
            Some(close) => rest = rest.get(close + 1..).unwrap_or_default(),
            None => break,
        }
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether the last sentence of `help` starts with a stamp. Clap drops the final full stop of a
/// one-sentence-per-line help, so the sentence is found by the stop before it.
fn ends_with_stamp(help: &str) -> bool {
    let last = help
        .rfind(". ")
        .map_or(help, |at| help.get(at + 2..).unwrap_or_default());
    STAMPS.iter().any(|stamp| last.starts_with(stamp))
}

/// Guards: U2 — the help of every option, global or of a verb, ends with a sentence that starts
/// `cannot be derived: ` or `not an experiment setting: `; `--help`, `--version` and positional
/// arguments are outside the walk.
/// Oracle: the `--help` of the real binary, each option's continuation lines joined.
/// Bite: an option whose stamp is deleted, or whose stamp is followed by another sentence,
/// names the option and fails.
#[test]
fn u2_every_option_help_ends_with_a_stamp() {
    let mut failing: BTreeSet<String> = BTreeSet::new();
    let mut seen = 0_usize;
    let globals = options_of(&help(&[]));
    let skipped = |name: &str| name == "help" || name == "version";
    for (name, text) in globals.iter().filter(|(name, _)| !skipped(name)) {
        seen += 1;
        if !ends_with_stamp(text) {
            failing.insert(format!("--{name}"));
        }
    }
    let global_names: BTreeSet<&str> = globals.iter().map(|(name, _)| name.as_str()).collect();
    for verb in listed_verbs() {
        for (name, text) in options_of(&help(&[verb.as_str()])) {
            if skipped(&name) || global_names.contains(name.as_str()) {
                continue;
            }
            seen += 1;
            if !ends_with_stamp(&text) {
                failing.insert(format!("{verb} --{name}"));
            }
        }
    }
    assert!(seen > 10, "the walk saw {seen} options");
    assert!(
        failing.is_empty(),
        "the options without a stamp: {failing:?}"
    );
}

/// How long a job that has all it needs may take to end.
const JOB_LIMIT: Duration = Duration::from_secs(90);

/// The fake docker client's wrapper: an image looked up by a tag is absent until a build made it,
/// so the builds the binary asks for are made and logged; every call is passed on to the world's
/// fake, whose log is the record of what was asked.
const BUILDS: &str = r#"#!/bin/sh
real="@REAL@"
here="${0%/*}"
mkdir -p "$here/built"
if [ "$1" = image ] && [ "$2" = inspect ]; then
    for a in "$@"; do ref="$a"; done
    case "$ref" in
    sha256:*) ;;
    *)
        key=$(printf '%s' "$ref" | tr '/:' '__')
        if [ ! -f "$here/built/$key" ]; then
            echo "Error: No such image: $ref" >&2
            exit 1
        fi
        ;;
    esac
fi
if [ "$1" = build ]; then
    prev=""
    for a in "$@"; do
        if [ "$prev" = "-t" ]; then
            touch "$here/built/$(printf '%s' "$a" | tr '/:' '__')"
        fi
        prev="$a"
    done
fi
exec "$real" "$@"
"#;

/// A stand-in for the endpoint's model list: it answers `200` to every request, so the gateway
/// check of `resolve` finds the endpoint it is given.
struct Models {
    port: u16,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Models {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
        let port = listener.local_addr().expect("an address").port();
        let stop = Arc::new(AtomicBool::new(false));
        let halt = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if halt.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(stream) = stream else { continue };
                let mut reader = BufReader::new(stream);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                        break;
                    }
                }
                let reply = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}";
                drop(reader.get_mut().write_all(reply.as_bytes()));
            }
        });
        Self {
            port,
            stop,
            handle: Some(handle),
        }
    }

    /// The `--set` that points a call at this stand-in.
    fn setting(&self) -> String {
        format!("model.endpoint=http://127.0.0.1:{}", self.port)
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

/// One trial: the fake-docker world, a binary to run in it, and an experiment file in the
/// working directory.
struct Env {
    world: World,
    exe: PathBuf,
    wrap: PathBuf,
}

impl Env {
    /// The world with `exe` as the binary, and `exp.toml` and `env.lock` in the working directory.
    fn with(exe: &Path) -> Self {
        let world = world();
        let wrap = world.root.join("wrap");
        fs::create_dir_all(&wrap).expect("a directory");
        let client = wrap.join("docker");
        let real = world.bin.join("docker");
        fs::write(
            &client,
            BUILDS.replace("@REAL@", &real.display().to_string()),
        )
        .expect("the wrapper writes");
        fs::set_permissions(&client, fs::Permissions::from_mode(0o755)).expect("a mode");
        fs::copy(&world.experiment, world.work.join("exp.toml")).expect("the experiment copies");
        Self {
            world,
            exe: exe.to_path_buf(), // BORROW: the trial owns its binary's path
            wrap,
        }
    }

    /// The binary `exe` runs with `args`, from the working directory, with the home and the XDG
    /// variables inside the world and the wrapped fake client first on the path, before the tools of
    /// the caller's path that a plug-in's environment needs.
    fn command(&self, args: &[&str]) -> Command {
        let root = &self.world.root;
        let mut command = Command::new(&self.exe);
        command
            .args(args)
            .current_dir(&self.world.work)
            .env_clear()
            .env(
                "PATH",
                format!(
                    "{}:{}:{}",
                    self.wrap.display(),
                    self.world.bin.display(),
                    std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned())
                ),
            )
            .env("HOME", root.join("home"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("XDG_STATE_HOME", &self.world.state)
            .env(SECRET_NAME, "fixture-token-0123456789")
            .stdin(Stdio::null());
        command
    }

    /// `run exp.toml`, the experiment relative to the working directory.
    fn run(&self) -> Output {
        self.command(&[
            "run",
            "exp.toml",
            "--set",
            "isolation.build.lock=env.lock",
            "--set",
            &format!("model.token_secret={SECRET_NAME}"),
        ])
        .output()
        .expect("the binary runs")
    }

    /// The store the verbs use with no `--store`: below the project when the working directory
    /// holds a `candler.toml`, else below the state home.
    fn store(&self) -> PathBuf {
        let work = &self.world.work;
        if work.join("candler.toml").is_file() {
            work.join(".candler").join("store")
        } else {
            self.world.store()
        }
    }

    /// The `build` calls the fake client logged, each as the context it was given: the last word.
    fn build_contexts(&self) -> Vec<PathBuf> {
        self.world
            .calls()
            .lines()
            .filter(|line| line.starts_with("build "))
            .filter_map(|line| line.split_whitespace().last())
            .map(PathBuf::from)
            .collect()
    }

    /// Waits for the job `out` printed to end and release its lock, and returns its id.
    fn settle(&self, out: &Output) -> JobId {
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        assert!(
            out.status.success(),
            "the launcher exits 0: {:?}\nstdout: {stdout}\nstderr: {stderr}\ncalls:\n{}",
            out.status,
            self.world.calls()
        );
        let id: JobId = stdout.trim().parse().expect("a job id on stdout");
        let dir = JobDir::open(&StoreLayout::new(&self.store()), &id).expect("the job");
        let events = dir.path().join("events.jsonl");
        let ended = wait_for(JOB_LIMIT, || {
            fs::read_to_string(&events).is_ok_and(|text| text.contains("job.ended"))
                && JobLock::probe(&dir).is_ok_and(|state| state == LockState::Free)
        });
        assert!(ended, "the job ends\ncalls:\n{}", self.world.calls());
        id
    }
}

/// Every file below `root`, by its path relative to `root`, with its bytes.
fn files_below(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut found = std::collections::BTreeMap::new();
    let mut stack = vec![root.to_path_buf()]; // BORROW: the walk owns its paths
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("a directory lists").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("below the root")
                    .to_path_buf();
                found.insert(relative, fs::read(&path).expect("a file reads"));
            }
        }
    }
    found
}

/// The assets of the workspace this test was built in.
fn tree() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/docker")
        .canonicalize()
        .expect("the tree has assets/docker")
}

/// A copy of the binary in a directory outside the source tree.
fn copied_binary(dir: &TempDir) -> PathBuf {
    let place = dir.path().join("elsewhere");
    fs::create_dir_all(&place).expect("a directory");
    let copy = place.join("candler");
    fs::copy(env!("CARGO_BIN_EXE_candler"), &copy).expect("the binary copies");
    copy
}

/// Guards: U3 — a copy of the binary, outside the source tree, runs `run` against the fake
/// world from an unrelated directory: the job id is printed, and every `docker build` it asks
/// for is given a context below the store root and outside the source tree; the contexts of the
/// base and gateway images are the written assets, below `<store
/// root>/assets/<digest>/docker/`, hold the bytes of the tree's directory of the same name, and
/// are the only contexts that outlive their build.
/// Oracle: the fake client's call log and the tree's files.
/// Bite: asset paths found through `CARGO_MANIFEST_DIR`, so the recorded context sits under the
/// source tree.
#[test]
fn u3_a_copied_binary_runs_run() {
    let elsewhere = TempDir::new().expect("a directory");
    let env = Env::with(&copied_binary(&elsewhere));
    let out = env.run();
    env.settle(&out);
    let store = env.world.store().canonicalize().expect("the store exists");
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the source tree");
    let contexts = env.build_contexts();
    assert!(!contexts.is_empty(), "the world was asked to build");
    let mut written = 0_usize;
    for context in &contexts {
        assert!(
            !context.starts_with(&source),
            "{} is under the source tree",
            context.display()
        );
        let staged = context.to_string_lossy().contains("/images/ctx-");
        if staged {
            continue;
        }
        let here = context
            .canonicalize()
            .expect("a context outlives its build");
        let assets = store.join("assets");
        assert!(
            here.starts_with(&assets),
            "{} is not below {}",
            here.display(),
            assets.display()
        );
        let below = here.strip_prefix(&assets).expect("below the assets");
        let mut parts = below.components();
        let digest = parts
            .next()
            .expect("a digest")
            .as_os_str()
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            digest.len(),
            64,
            "the directory is named by a digest: {digest}"
        );
        assert_eq!(
            parts.next().map(|p| p.as_os_str().to_owned()),
            Some("docker".into())
        );
        let name = here.file_name().expect("a name");
        assert_eq!(
            files_below(&here),
            files_below(&tree().join(name)),
            "{} holds the tree's bytes",
            here.display()
        );
        written += 1;
    }
    assert!(written >= 1, "a context was written: {contexts:?}");
}

/// Guards: U3 — the embedded assets equal the tree: after `resolve` from a copied binary, the
/// directory `<store root>/assets/<digest>/docker` holds every file of `assets/docker` with
/// equal bytes and no other, and a second `resolve` writes nothing.
/// Oracle: the tree, read from `CARGO_MANIFEST_DIR` (test code), and the modification times of
/// the written files.
/// Bite: a file of the tree left out of the embedded table, or an embedded file that differs
/// from the tree.
#[test]
fn u3_the_embedded_assets_equal_the_tree() {
    let elsewhere = TempDir::new().expect("a directory");
    let env = Env::with(&copied_binary(&elsewhere));
    let models = Models::start();
    let resolve = || {
        let out = env
            .command(&[
                "resolve",
                "exp.toml",
                "--set",
                &models.setting(),
                "--set",
                "isolation.build.lock=env.lock",
                "--set",
                &format!("model.token_secret={SECRET_NAME}"),
            ])
            .output()
            .expect("the binary runs");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    resolve();
    let assets = env.world.store().join("assets");
    let entries: Vec<_> = fs::read_dir(&assets)
        .expect("the assets are written")
        .flatten()
        .map(|entry| entry.path())
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "one directory, no staging left: {entries:?}"
    );
    let docker = entries[0].join("docker");
    assert_eq!(
        files_below(&docker),
        files_below(&tree()),
        "the tree's files, byte for byte"
    );
    let stamps = || -> Vec<_> {
        files_below(&docker)
            .keys()
            .map(|file| {
                fs::metadata(docker.join(file))
                    .and_then(|m| m.modified())
                    .expect("a time")
            })
            .collect()
    };
    let before = stamps();
    resolve();
    assert_eq!(stamps(), before, "a second call wrote nothing");
}

fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

/// The manifest of a one-cell run whose one factor is `plan.model`, with the cell's real digest.
fn manifest(replicate: u32) -> RunManifest {
    let body = |cell: &str| {
        json!({
            "schema": "run-manifest/1",
            "run": {"cell": cell, "replicate": replicate, "attempt": 1},
            "experiment": digest_text('0'),
            "factors": ["plan.model"],
            "comparable": {
                "values": [{"path": "plan.model", "value": "alpha"}],
                "harness": {"name": "h", "version": "1.0.0"},
                "treatment": {"name": "t"},
                "snapshots": [],
                "inputs": [],
                "environments": []
            },
            "sources": [
                {"path": "plan.model", "source": {"kind": "experiment_file"}},
                {"path": "harness", "source": {"kind": "experiment_file"}},
                {"path": "treatment", "source": {"kind": "experiment_file"}}
            ],
            "secrets": [],
            "measured": [
                {"path": "provenance.eval_infra.commit", "value": "abc1234", "source": {"kind": "experiment_file"}},
                {"path": "provenance.eval_infra.dirty", "value": false, "source": {"kind": "experiment_file"}},
                {"path": "provenance.sandbox_contract", "value": "candler-container@1", "source": {"kind": "experiment_file"}}
            ],
            "deviations": [],
            "integrity": {"kind": "valid"}
        })
    };
    let probe: RunManifest = serde_json::from_value(body(&digest_text('f'))).expect("a manifest");
    let cell = probe.cell_digest().to_string();
    serde_json::from_value(body(cell.as_str())).expect("a manifest") // BORROW: explicit .as_str()
}

/// One outcome record: `expectation` graded `status` (`pass` or `fail`) by `evaluator`.
fn record(
    run: &RunKey,
    request: &RequestId,
    evaluator: &str,
    expectation: &str,
    status: &str,
) -> OutcomeRecord {
    let num = u32::from(status == "pass");
    serde_json::from_value(json!({
        "run": {"cell": run.cell().to_string(), "replicate": run.replicate(), "attempt": run.attempt()},
        "expectation_id": expectation,
        "role": "gate",
        "evaluator": evaluator,
        "instruments": [],
        "suite_entry_digest": digest_text('1'),
        "context_digest": digest_text('2'),
        "request_id": request.to_string(),
        "status": status,
        "weight": {"num": num, "den": 1},
        "evidence": [],
    }))
    .expect("an outcome record")
}

/// A store of one sealed run at `root`, holding, per evaluator, the rows `(expectation, status)`.
/// Returns the run's key.
fn seed(root: &Path, rows: &[(&str, &[(&str, &str)])]) -> RunKey {
    let store = Store::init(root).expect("the store initialises");
    let manifest = manifest(1);
    let run = manifest.run().clone(); // BORROW: the key is used after the manifest moves
    RunWriter::create(&store, &manifest, Existing::Refuse)
        .expect("the run starts")
        .done()
        .expect("the run seals");
    for (n, (evaluator, graded)) in rows.iter().enumerate() {
        let request: RequestId = format!("01926b3a-7c3e-7a1b-8c4d-{:012x}", n + 1)
            .parse()
            .expect("a request id");
        let name: NameVersion = evaluator.parse().expect("name@version");
        let mut writer = OutcomeWriter::create(&store, &run, &name, &request).expect("a writer");
        for (expectation, status) in *graded {
            writer
                .append(&record(&run, &request, evaluator, expectation, status))
                .expect("a record appends");
        }
        writer.commit().expect("the outcomes commit");
    }
    run
}

/// Adds a second sealed run of the same cell, replicate 2, to the store at `root`: its key starts
/// like the first run's.
fn seed_second_replicate(root: &Path) {
    let store = Store::open(root).expect("the store opens");
    RunWriter::create(&store, &manifest(2), Existing::Refuse)
        .expect("the run starts")
        .done()
        .expect("the run seals");
}

/// A store seeded by hand in a directory of its own, which is also the working directory and the
/// home of the calls made over it.
struct Seeded {
    dir: TempDir,
}

impl Seeded {
    fn with(rows: &[(&str, &[(&str, &str)])]) -> Self {
        let dir = TempDir::new().expect("a directory");
        seed(&dir.path().join("store"), rows);
        Self { dir }
    }

    /// `candler --store store <args>` from the directory, with the home inside it.
    fn candler(&self, args: &[&str]) -> Output {
        let home = self.dir.path();
        Command::new(env!("CARGO_BIN_EXE_candler"))
            .arg("--store")
            .arg("store")
            .args(args)
            .current_dir(home)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_STATE_HOME", home.join("state"))
            .stdin(Stdio::null())
            .output()
            .expect("the binary runs")
    }
}

fn text_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Guards: U5 — `diff --from 1.0.0 --to 2.0.0` equals `--from ev@1.0.0 --to ev@2.0.0` when one
/// evaluator name holds each version, and is refused, naming both holders, when two names hold
/// the bare version.
/// Oracle: the output of the same call with the names written out, and the evaluators written
/// into the store.
/// Bite: the `name@` prefix required again (the bare call exits 2 with no report), or the first
/// holder of an ambiguous version taken.
#[test]
fn u5_diff_takes_bare_versions() {
    let rows: &[(&str, &[(&str, &str)])] = &[
        ("ev@1.0.0", &[("A1", "pass"), ("A2", "fail")]),
        ("ev@2.0.0", &[("A1", "pass"), ("A2", "pass")]),
    ];
    let store = Seeded::with(rows);
    let bare = store.candler(&[
        "diff", "--from", "1.0.0", "--to", "2.0.0", "--format", "json",
    ]);
    let named = store.candler(&[
        "diff", "--from", "ev@1.0.0", "--to", "ev@2.0.0", "--format", "json",
    ]);
    assert_eq!(named.status.code(), Some(1), "{}", text_of(&named.stderr));
    assert_eq!(
        bare.status.code(),
        named.status.code(),
        "{}",
        text_of(&bare.stderr)
    );
    assert_eq!(bare.stdout, named.stdout, "the same report");
    assert!(!bare.stdout.is_empty(), "a report was printed");

    let held_twice: &[(&str, &[(&str, &str)])] = &[
        ("ev@1.0.0", &[("A1", "pass")]),
        ("other@1.0.0", &[("A1", "pass")]),
        ("ev@2.0.0", &[("A1", "pass")]),
    ];
    let store = Seeded::with(held_twice);
    let out = store.candler(&[
        "diff", "--from", "1.0.0", "--to", "2.0.0", "--format", "json",
    ]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = text_of(&out.stderr);
    assert!(
        stderr.contains("ev@1.0.0") && stderr.contains("other@1.0.0"),
        "both holders are named: {stderr}"
    );
    assert!(out.stdout.is_empty(), "no report for a refusal");
}

/// Guards: U5 — `show --rates --format toml` with no `--evaluator` equals the call that names
/// the evaluator when the store holds one evaluator, and is refused, listing the evaluators,
/// when it holds two.
/// Oracle: the output of the same call with the evaluator named, and the evaluators written
/// into the store.
/// Bite: `--rates` requiring `--evaluator` again (the bare call exits 2 with a usage error), or
/// the first of two evaluators taken.
#[test]
fn u5_show_rates_infers_the_only_evaluator() {
    let one: &[(&str, &[(&str, &str)])] = &[("ev@1.0.0", &[("A1", "pass"), ("A2", "fail")])];
    let store = Seeded::with(one);
    let inferred = store.candler(&["show", "--rates", "--format", "toml"]);
    let named = store.candler(&[
        "show",
        "--rates",
        "--evaluator",
        "ev@1.0.0",
        "--format",
        "toml",
    ]);
    assert_eq!(named.status.code(), Some(0), "{}", text_of(&named.stderr));
    assert_eq!(
        inferred.status.code(),
        Some(0),
        "{}",
        text_of(&inferred.stderr)
    );
    assert_eq!(inferred.stdout, named.stdout, "the same rates");
    assert!(!inferred.stdout.is_empty(), "rates were printed");

    let two: &[(&str, &[(&str, &str)])] = &[
        ("ev@1.0.0", &[("A1", "pass")]),
        ("other@2.0.0", &[("A1", "pass")]),
    ];
    let store = Seeded::with(two);
    let out = store.candler(&["show", "--rates", "--format", "toml"]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = text_of(&out.stderr);
    assert!(
        stderr.contains("ev@1.0.0") && stderr.contains("other@2.0.0"),
        "the evaluators are listed: {stderr}"
    );
    assert!(out.stdout.is_empty(), "no rates for a refusal");
}

/// Guards: U5 — `show --evaluator 1.0.0 --format json` equals the call with `ev@1.0.0` when one
/// evaluator holds that version.
/// Oracle: the output of the same call with the name written out.
/// Bite: the `name@` prefix required again (the bare call exits 2 with no counts).
#[test]
fn u5_show_takes_a_bare_version() {
    let rows: &[(&str, &[(&str, &str)])] = &[
        ("ev@1.0.0", &[("A1", "pass"), ("A2", "fail")]),
        ("ev@2.0.0", &[("A1", "pass")]),
    ];
    let store = Seeded::with(rows);
    let bare = store.candler(&["show", "--evaluator", "1.0.0", "--format", "json"]);
    let named = store.candler(&["show", "--evaluator", "ev@1.0.0", "--format", "json"]);
    assert_eq!(named.status.code(), Some(0), "{}", text_of(&named.stderr));
    assert_eq!(bare.status.code(), Some(0), "{}", text_of(&bare.stderr));
    assert_eq!(bare.stdout, named.stdout, "the same counts");
    let counts: Value = serde_json::from_slice(&bare.stdout).expect("JSON counts");
    assert_eq!(
        counts,
        json!({"counts": {"ev@1.0.0": {"pass": 1, "fail": 1}}})
    );
}

/// What a verb's row does in a fresh [`Env`]: it runs the verb from the working directory with
/// relative paths and asserts what the verb answered.
type Check = fn(&Env);

/// A verb and what its row asserts.
struct Row {
    verb: &'static str,
    check: Check,
}

/// The verbs that run from an unrelated directory, one row each.
const MATRIX: &[Row] = &[
    Row {
        verb: "run",
        check: row_run,
    },
    Row {
        verb: "resolve",
        check: row_resolve,
    },
    Row {
        verb: "ls",
        check: row_ls,
    },
    Row {
        verb: "ps",
        check: row_ps,
    },
    Row {
        verb: "kill",
        check: row_kill,
    },
    Row {
        verb: "watch",
        check: row_watch,
    },
    Row {
        verb: "ingest",
        check: row_ingest,
    },
    Row {
        verb: "inspect",
        check: row_inspect,
    },
    Row {
        verb: "evaluate",
        check: row_evaluate,
    },
    Row {
        verb: "show",
        check: row_show,
    },
    Row {
        verb: "diff",
        check: row_diff,
    },
    Row {
        verb: "analyze",
        check: row_analyze,
    },
];

/// The text of a call's standard output.
fn stdout_of(out: &Output) -> String {
    text_of(&out.stdout)
}

/// `out`, after asserting that it exited 0.
fn succeeded(out: Output) -> Output {
    assert!(
        out.status.success(),
        "exit {:?}\nstderr: {}",
        out.status.code(),
        text_of(&out.stderr)
    );
    out
}

/// The names in the working directory of `env`.
fn work_listing(env: &Env) -> BTreeSet<String> {
    fs::read_dir(&env.world.work)
        .expect("the directory lists")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

/// Writes the raw trace directory `traces/` below the working directory: one Claude Code stream.
fn write_traces(env: &Env) {
    let dir = env.world.work.join("traces");
    fs::create_dir_all(&dir).expect("a directory");
    let init = r#"{"type":"system","subtype":"init","session_id":"S","model":"m-a","claude_code_version":"2.1.220"}"#;
    let result = r#"{"type":"result","subtype":"success","is_error":false,"session_id":"S"}"#;
    fs::write(dir.join("a.jsonl"), format!("{init}\n{result}\n")).expect("the stream writes");
}

/// The two evaluators of the matrix's seeded store, graded alike.
const TWO_EVALUATORS: &[(&str, &[(&str, &str)])] = &[
    ("ev@1.0.0", &[("A1", "pass"), ("A2", "fail")]),
    ("ev@2.0.0", &[("A1", "pass"), ("A2", "fail")]),
];

/// Seeds the store the verbs find with no `--store`, below the state home, and returns the key.
fn seed_default_store(env: &Env, rows: &[(&str, &[(&str, &str)])]) -> RunKey {
    seed(&env.world.store(), rows)
}

fn row_run(env: &Env) {
    let out = env.run();
    let id = env.settle(&out);
    let dir = env.world.store().join("jobs").join(id.to_string());
    assert!(
        dir.is_dir(),
        "the job has its directory in the store: {}",
        dir.display()
    );
}

fn row_resolve(env: &Env) {
    let models = Models::start();
    let out = succeeded(
        env.command(&[
            "resolve",
            "exp.toml",
            "--set",
            "isolation.build.lock=env.lock",
            "--set",
            &models.setting(),
            "--set",
            &format!("model.token_secret={SECRET_NAME}"),
        ])
        .output()
        .expect("the binary runs"),
    );
    let lines: Vec<String> = stdout_of(&out).lines().map(str::to_owned).collect();
    assert!(lines.len() > 5, "the values are listed: {lines:?}");
    for line in &lines {
        let columns: Vec<&str> = line.split('\t').collect();
        assert_eq!(columns.len(), 3, "a path, a value and a source: {line}");
        assert!(columns.iter().all(|column| !column.is_empty()), "{line}");
    }
    assert!(
        !env.world.store().join("jobs").exists(),
        "no job directory appears"
    );
}

fn row_ls(env: &Env) {
    let dir = env.world.work.join("experiments");
    fs::create_dir_all(&dir).expect("a directory");
    fs::copy(env.world.work.join("exp.toml"), dir.join("a.toml")).expect("the file copies");
    let out = succeeded(env.command(&["ls"]).output().expect("the binary runs"));
    assert!(
        stdout_of(&out).lines().any(|line| line.contains("a.toml")),
        "a.toml is listed: {}",
        stdout_of(&out)
    );
}

fn row_ps(env: &Env) {
    let launched = env.run();
    let id = text_of(&launched.stdout).trim().to_owned();
    let out = succeeded(env.command(&["ps"]).output().expect("the binary runs"));
    assert!(
        stdout_of(&out).lines().any(|line| line.starts_with(&id)),
        "the job just started is listed: {}",
        stdout_of(&out)
    );
    env.settle(&launched);
}

fn row_kill(env: &Env) {
    succeeded(
        env.command(&["kill", "--all", "--yes"])
            .output()
            .expect("the binary runs"),
    );
}

fn row_watch(env: &Env) {
    let launched = env.run();
    let id = env.settle(&launched);
    let out = succeeded(
        env.command(&["watch", &id.to_string()])
            .output()
            .expect("the binary runs"),
    );
    let text = stdout_of(&out);
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines.len() > 1, "the events are printed: {text}");
    for line in &lines {
        serde_json::from_str::<Value>(line).expect("each event is one JSON document");
    }
    assert!(
        lines.last().is_some_and(|line| line.contains("job.ended")),
        "the last event is job.ended: {text}"
    );
}

fn row_ingest(env: &Env) {
    write_traces(env);
    let before = work_listing(env);
    let out = succeeded(
        env.command(&["ingest", "traces/"])
            .output()
            .expect("the binary runs"),
    );
    let summary: Value = serde_json::from_slice(&out.stdout).expect("a JSON summary");
    assert_eq!(summary["created"], 1, "{summary}");
    let runs = fs::read_dir(env.world.store().join("runs"))
        .expect("the store holds the runs")
        .flatten()
        .count();
    assert_eq!(runs, 1, "the run is below the store");
    assert_eq!(
        work_listing(env),
        before,
        "nothing else is written in the working directory"
    );
}

fn row_inspect(env: &Env) {
    let key = seed(&env.world.work.join("store"), TWO_EVALUATORS);
    let target = format!("store/runs/{key}");
    let out = succeeded(
        env.command(&["inspect", &target])
            .output()
            .expect("the binary runs"),
    );
    assert!(
        stdout_of(&out).contains(&key.to_string()),
        "the run is shown"
    );
}

/// The `pool` plug-in of the engine guards below the working directory, with a `candler.toml`
/// that lists it under `plugins.dirs`, and the suite beside them.
fn plugin_project(env: &Env) {
    let work = &env.world.work;
    let engine =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../candler-core/tests/fixtures/engine");
    let helper = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../python/candler-evaluator")
        .canonicalize()
        .expect("the helper's project");
    let plugin = work.join("plugin");
    fs::create_dir_all(&plugin).expect("a directory");
    std::os::unix::fs::symlink(&helper, plugin.join("env")).expect("the link");
    let manifest = fs::read_to_string(engine.join("plugin_pool.toml"))
        .expect("the manifest reads")
        .replace("@EXAMPLES@", &helper.join("examples").display().to_string())
        .replace("@KINDS@", &engine.join("kinds").display().to_string());
    fs::write(plugin.join("plugin.toml"), manifest).expect("the manifest writes");
    fs::copy(engine.join("suite_pool_ok.toml"), work.join("suite.toml")).expect("the suite copies");
    fs::write(
        work.join("candler.toml"),
        "[plugins]\ndirs = [\"plugin\"]\n",
    )
    .expect("the project");
}

fn row_evaluate(env: &Env) {
    plugin_project(env);
    seam::sync_environment(&env.world);
    env.settle(&env.run());
    let out = succeeded(
        env.command(&["evaluate", "suite.toml"])
            .output()
            .expect("the binary runs"),
    );
    let stderr = text_of(&out.stderr);
    let id: JobId = stderr
        .lines()
        .next()
        .expect("a first line")
        .parse()
        .expect("the job id is on stderr");
    let report: Value = serde_json::from_slice(&out.stdout).expect("the report is on stdout");
    assert_eq!(report["failed_requests"], json!([]), "{report}");
    assert!(
        report["request_ids"]
            .as_array()
            .is_some_and(|ids| !ids.is_empty()),
        "the run just run was graded: {report}"
    );
    let dir = JobDir::open(&StoreLayout::new(&env.store()), &id).expect("the job");
    assert!(
        wait_for(JOB_LIMIT, || JobLock::probe(&dir)
            .is_ok_and(|state| state == LockState::Free)),
        "the job's supervisor has gone"
    );
}

fn row_show(env: &Env) {
    seed_default_store(env, TWO_EVALUATORS);
    let out = succeeded(
        env.command(&["show", "--format", "json"])
            .output()
            .expect("the binary runs"),
    );
    let counts: Value = serde_json::from_slice(&out.stdout).expect("JSON counts");
    assert_eq!(
        counts,
        json!({"counts": {
            "ev@1.0.0": {"pass": 1, "fail": 1},
            "ev@2.0.0": {"pass": 1, "fail": 1},
        }})
    );
}

fn row_diff(env: &Env) {
    seed_default_store(env, TWO_EVALUATORS);
    let out = succeeded(
        env.command(&[
            "diff", "--from", "1.0.0", "--to", "2.0.0", "--format", "json",
        ])
        .output()
        .expect("the binary runs"),
    );
    let report: Value = serde_json::from_slice(&out.stdout).expect("a JSON report");
    assert_eq!(report["compared"], 2, "{report}");
    assert_eq!(report["changed"], 0, "{report}");
}

fn row_analyze(env: &Env) {
    seed_default_store(env, &TWO_EVALUATORS[..1]);
    fs::write(
        env.world.work.join("exp.toml"),
        "schema = \"experiment/1\"\nreplicates = 1\n\n[fixed]\npermission_mode = \"plan\"\n\n[[factors]]\npath = \"plan.model\"\nlevels = [\"alpha\", \"beta\"]\n\n[budget.per_run]\nturns = 30\nwall_s = 600\nmax_cost_usd = 1.0\n\n[harness]\nname = \"h\"\n\n[treatment]\nname = \"t\"\n",
    )
    .expect("the experiment writes");
    let out = succeeded(
        env.command(&["analyze", "--experiment", "exp.toml", "--format", "csv"])
            .output()
            .expect("the binary runs"),
    );
    let text = stdout_of(&out);
    let mut lines = text.lines();
    assert!(
        lines
            .next()
            .is_some_and(|head| head.starts_with("cell,experiment,role,expectation,")),
        "the arms table: {text}"
    );
    assert_eq!(lines.count(), 2, "one row per expectation: {text}");
}

/// Directories a verb must not create beside the source tree or the binary: the names of what a
/// store holds. The source tree has an `assets` directory of its own, so that name is only looked
/// for beside the binary.
const STORE_PARTS: [&str; 4] = [".candler", "runs", "jobs", "images"];

/// Panics when a store, or what one holds, appeared in the source tree or beside the binary.
fn assert_no_stray(env: &Env) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let beside = env.exe.parent().expect("the binary's directory");
    for dir in [source.as_path(), beside] {
        for part in STORE_PARTS {
            assert!(
                !dir.join(part).exists(),
                "{} appeared in {}",
                part,
                dir.display()
            );
        }
    }
    for part in ["assets", "store"] {
        assert!(
            !beside.join(part).exists(),
            "{part} appeared beside the binary"
        );
    }
}

/// Guards: U1 — every verb that `candler --help` lists has a row in the working-directory
/// matrix, and every row names a listed verb.
/// Oracle: the `Commands:` block of the real binary's `--help`, minus `help`.
/// Bite: a verb added to clap with no row names the verb and fails, as does a row left behind
/// for a verb that is gone.
#[test]
fn u1_every_verb_has_a_row() {
    let listed = listed_verbs();
    let rows: BTreeSet<String> = MATRIX.iter().map(|row| row.verb.to_owned()).collect();
    assert!(!listed.is_empty(), "the help lists no verb");
    assert_eq!(rows, listed, "the matrix has one row for each listed verb");
}

/// Guards: U1 — each verb runs from a fresh directory unrelated to the project and to the
/// binary, with `HOME` and the XDG variables inside it and every path argument relative to it,
/// and answers as its row says: `run`, `watch`, `ps` and `evaluate` start real jobs against the
/// fake-docker world and wait for them to end. Nothing appears in the source tree or beside the
/// binary.
/// Oracle: the verb's own answer and the files of the fixture world.
/// Bite: a path joined to the project directory or to the binary's directory in a verb that has
/// a relative argument fails that verb's row.
#[test]
fn u1_each_verb_runs_from_an_unrelated_directory() {
    assert!(!MATRIX.is_empty(), "the matrix has no row");
    for row in MATRIX {
        let env = Env::with(Path::new(env!("CARGO_BIN_EXE_candler")));
        let verb = row.verb;
        let check = row.check;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&env)));
        assert!(caught.is_ok(), "the row of `{verb}` failed");
        assert_no_stray(&env);
    }
}

/// Guards: U1 — with a `candler.toml` in the working directory and no `--store`, the store is
/// `<working directory>/.candler/store`; with no project it is below `$XDG_STATE_HOME`; neither
/// writes below the source tree or beside the binary.
/// Oracle: where the one run of `ingest traces/` is found, from the directories of the fixture
/// world.
/// Bite: a store root fixed to the source tree or the binary's directory, or a project store
/// that falls back to the state home.
#[test]
fn u1_the_store_comes_from_the_project_or_the_state_home() {
    let runs_in = |store: &Path| {
        fs::read_dir(store.join("runs")).map_or(0, |entries| entries.flatten().count())
    };
    let project = Env::with(Path::new(env!("CARGO_BIN_EXE_candler")));
    write_traces(&project);
    fs::write(
        project.world.work.join("candler.toml"),
        "[plugins]\ndirs = []\n",
    )
    .expect("the project file writes");
    succeeded(
        project
            .command(&["ingest", "traces/"])
            .output()
            .expect("the binary runs"),
    );
    assert_eq!(
        runs_in(&project.world.work.join(".candler").join("store")),
        1
    );
    assert!(
        !project.world.store().exists(),
        "nothing below the state home"
    );
    assert_no_stray(&project);

    let bare = Env::with(Path::new(env!("CARGO_BIN_EXE_candler")));
    write_traces(&bare);
    succeeded(
        bare.command(&["ingest", "traces/"])
            .output()
            .expect("the binary runs"),
    );
    assert_eq!(
        runs_in(&bare.world.store()),
        1,
        "the store is below the state home"
    );
    assert!(
        !bare.world.work.join(".candler").exists(),
        "no project store appears"
    );
    assert_no_stray(&bare);
}

/// The record on the last line of a refusal's standard error, parsed.
fn record_on(out: &Output) -> Value {
    let stderr = text_of(&out.stderr);
    let line = stderr
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default();
    serde_json::from_str(line).unwrap_or_else(|e| panic!("`{line}` is not a record: {e}"))
}

/// A refusal a caller causes in a verb's own checks: the arguments that cause it, what the store
/// holds before, and the code and side the record must carry.
struct Site {
    name: &'static str,
    args: &'static [&'static str],
    setup: fn(&Seeded),
    code: &'static str,
    side: &'static str,
    /// What the message must contain.
    says: &'static str,
}

fn nothing(_: &Seeded) {}

fn plant_unreadable_job(store: &Seeded) {
    fs::create_dir_all(
        store
            .dir
            .path()
            .join("store/jobs/01a1ccbc-3339-7146-a3ef-ac307f9d93fd"),
    )
    .expect("a directory");
}

fn plant_rules_with_a_bad_pattern(store: &Seeded) {
    let rules = "format = \"htn-results@1\"\ninstrument = \"htn-creator-eval\"\nsuite_kind = \"legacy-check\"\nentry_params = [\"scale\"]\n\n[harness]\nname = \"claude-code\"\nversion_pointer = \"/claude_code_version\"\n\n[treatment]\npointer = \"/plan/skill_version\"\nresolved = \"resolved_skill_version\"\n\n[[marker]]\npattern = \"(\"\ncode = \"x\"\n";
    fs::write(store.dir.path().join("rules.toml"), rules).expect("the rules file writes");
}

fn plant_second_replicate(store: &Seeded) {
    seed_second_replicate(&store.dir.path().join("store"));
}

fn plant_directory_with_a_manifest(store: &Seeded) {
    let dir = store.dir.path().join("elsewhere");
    fs::create_dir_all(&dir).expect("a directory");
    fs::write(dir.join("manifest.json"), "{}").expect("a file");
}

/// The sites, one row each; `{key8}` in an argument stands for the first eight characters of the
/// seeded run's key. The store holds one run of the cell of `plan.model = alpha`, graded by
/// `ev@1.0.0` and `ev@2.0.0`.
const SITES: &[Site] = &[
    Site {
        name: "resolve of a missing experiment file",
        args: &["resolve", "nope.toml"],
        setup: nothing,
        code: "config.experiment_unreadable",
        side: "config",
        says: "nope.toml",
    },
    Site {
        name: "run of a missing experiment file",
        args: &["run", "nope.toml"],
        setup: nothing,
        code: "config.experiment_unreadable",
        side: "config",
        says: "nope.toml",
    },
    Site {
        name: "kill without -y",
        args: &["kill", "01a1ccbc-3339-7146-a3ef-ac307f9d93fd"],
        setup: nothing,
        code: "config.kill_not_confirmed",
        side: "config",
        says: "-y",
    },
    Site {
        name: "kill -y of a missing job",
        args: &["kill", "-y", "01a1ccbc-3339-7146-a3ef-ac307f9d93fd"],
        setup: nothing,
        code: "jobs.job_not_found",
        side: "config",
        says: "01a1ccbc-3339-7146-a3ef-ac307f9d93fd",
    },
    Site {
        name: "ps with an unreadable job",
        args: &["ps"],
        setup: plant_unreadable_job,
        code: "jobs.io_failed",
        side: "environment",
        says: "see its row",
    },
    Site {
        name: "inspect of a selector that is none of the three forms",
        args: &["inspect", "zz"],
        setup: nothing,
        code: "config.inspect_target_invalid",
        side: "config",
        says: "`zz`",
    },
    Site {
        name: "inspect of a prefix no run has",
        args: &["inspect", "0123456789abcdef"],
        setup: nothing,
        code: "config.no_such_run",
        side: "config",
        says: "0123456789abcdef",
    },
    Site {
        name: "inspect of a prefix several runs have",
        args: &["inspect", "{key8}"],
        setup: plant_second_replicate,
        code: "config.ambiguous_run",
        side: "config",
        says: "-r2-a1",
    },
    Site {
        name: "inspect of a directory that is no run directory",
        args: &["inspect", "elsewhere"],
        setup: plant_directory_with_a_manifest,
        code: "config.run_directory_invalid",
        side: "config",
        says: "elsewhere",
    },
    Site {
        name: "show of an evaluator that is not name@version",
        args: &["show", "--evaluator", "bad@x", "--format", "json"],
        setup: nothing,
        code: "config.evaluator_invalid",
        side: "config",
        says: "--evaluator `bad@x`",
    },
    Site {
        name: "diff from a name@version whose version is not SemVer",
        args: &[
            "diff", "--from", "a@1", "--to", "b@1.0.0", "--format", "json",
        ],
        setup: nothing,
        code: "config.evaluator_invalid",
        side: "config",
        says: "--from `a@1`",
    },
    Site {
        name: "show of a name@version whose version is not SemVer",
        args: &["show", "--evaluator", "a@1", "--format", "json"],
        setup: nothing,
        code: "config.evaluator_invalid",
        side: "config",
        says: "--evaluator `a@1`",
    },
    Site {
        name: "show of a bare version no evaluator holds",
        args: &["show", "--evaluator", "9.9.9", "--format", "json"],
        setup: nothing,
        code: "analysis.unknown_evaluator",
        side: "config",
        says: "9.9.9",
    },
    Site {
        name: "diff from an evaluator that is not name@version",
        args: &[
            "diff", "--from", "bad@x", "--to", "ev@1.0.0", "--format", "json",
        ],
        setup: nothing,
        code: "config.evaluator_invalid",
        side: "config",
        says: "--from `bad@x`",
    },
    Site {
        name: "diff from an evaluator the store lacks",
        args: &[
            "diff", "--from", "ev@9.0.0", "--to", "ev@1.0.0", "--format", "json",
        ],
        setup: nothing,
        code: "config.nothing_compared",
        side: "config",
        says: "ev@9.0.0",
    },
    Site {
        name: "ingest with a rules file whose marker pattern is invalid",
        args: &["ingest", "results", "--rules", "rules.toml"],
        setup: plant_rules_with_a_bad_pattern,
        code: "config.import_rules_invalid",
        side: "config",
        says: "rules `marker.pattern`",
    },
    Site {
        name: "an invalid --set path",
        args: &["run", "exp.toml", "--set", "bad"],
        setup: nothing,
        code: "cli.usage_invalid",
        side: "config",
        says: "expected PATH=VALUE",
    },
    Site {
        name: "an unknown option",
        args: &["--bogus"],
        setup: nothing,
        code: "cli.usage_invalid",
        side: "config",
        says: "unexpected argument '--bogus'",
    },
    Site {
        name: "watch of an unknown job",
        args: &["watch", "01a1ccbc-3339-7146-a3ef-ac307f9d9399"],
        setup: nothing,
        code: "jobs.job_not_found",
        side: "config",
        says: "01a1ccbc-3339-7146-a3ef-ac307f9d9399",
    },
];

/// Guards: U6 — each refusal that a caller causes in a verb's own checks prints, under
/// `--error-format json`, one `refusal/1` record whose code and side are the row's and not
/// `candler.internal`: a missing experiment file, `kill` without `-y` and of a missing job,
/// `ps` over an unreadable job, `inspect` of a selector that is invalid, unknown or ambiguous,
/// an evaluator that is malformed, unknown or compares nothing, and clap's usage errors, whose
/// message is clap's text.
/// Oracle: the codes written in `docs/contract/codes.toml` by hand for these sites, and the
/// store the row builds.
/// Bite: a site back to `CliError::refused(String)`, which prints `candler.internal` with side
/// `candler`.
#[test]
fn u6_each_caller_mistake_carries_a_code() {
    let mut misses: Vec<String> = Vec::new();
    for site in SITES {
        let store = Seeded::with(TWO_EVALUATORS);
        (site.setup)(&store);
        let key8 = manifest(1)
            .run()
            .to_string()
            .chars()
            .take(8)
            .collect::<String>();
        let mut args = vec!["--error-format".to_owned(), "json".to_owned()];
        args.extend(site.args.iter().map(|arg| arg.replace("{key8}", &key8)));
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = store.candler(&args);
        let stderr = text_of(&out.stderr);
        let line = stderr
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default();
        let record: Value = serde_json::from_str(line).unwrap_or(Value::Null);
        let message = record["message"].as_str().unwrap_or_default();
        let held = out.status.code() == Some(2)
            && record["schema"] == "refusal/1"
            && record["code"] == site.code
            && record["side"] == site.side
            && message.contains(site.says);
        if !held {
            misses.push(format!(
                "{}: exit {:?}, {line}",
                site.name,
                out.status.code()
            ));
        }
    }
    assert!(
        misses.is_empty(),
        "sites without their code:\n{}",
        misses.join("\n")
    );
}

/// Guards: U6 — a missing experiment file is named in the record's `file`, and clap's usage
/// errors keep their text and exit status when no JSON is asked for, while `--help` still
/// prints its text on stdout and exits 0 under `--error-format json`.
/// Oracle: the text clap writes for the same arguments with no `--error-format`, and the status
/// of `--help`.
/// Bite: the usage error printed as a record in the text form, or `--help` taken for an error.
#[test]
fn u6_usage_text_and_help_are_unchanged_and_the_file_is_named() {
    let store = Seeded::with(TWO_EVALUATORS);
    let out = store.candler(&["--error-format", "json", "resolve", "nope.toml"]);
    assert_eq!(record_on(&out)["file"], "nope.toml");

    let out = store.candler(&["--bogus"]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = text_of(&out.stderr);
    assert!(
        stderr.starts_with("error: unexpected argument '--bogus'"),
        "{stderr}"
    );
    assert!(out.stdout.is_empty(), "nothing on stdout");

    let out = store.candler(&["--error-format", "json", "--help"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        text_of(&out.stdout).contains("Commands:"),
        "the help is on stdout"
    );
    assert!(out.stderr.is_empty(), "nothing on stderr");
}
