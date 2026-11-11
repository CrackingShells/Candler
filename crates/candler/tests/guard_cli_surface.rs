#![cfg(test)]
#![cfg(unix)]
//! Guards for the surface every verb of the `candler` binary shares: each verb has a row in the
//! working-directory matrix or a place on its pending list, and each option's help ends with a
//! stamp that says why the option exists. The binary is the real one; the verbs and options are
//! read from its `--help`, so a verb or an option added later is found without editing the guards.

#[expect(
    dead_code,
    reason = "this guard uses the world and its fake client, not every helper of the shared module"
)]
#[path = "support/cli_run.rs"]
mod cli_run;

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

/// What `candler --help` lists as verbs and has no row for yet: each verb here works from a fixture
/// world only. A leaf that makes a verb work from any directory moves it to [`MATRIX`].
pub const PENDING_CWD: &[&str] = &[
    "run", "watch", "ingest", "evaluate", "show", "diff", "resolve", "ls", "analyze", "inspect",
];

/// A verb and the arguments that run it, by a relative `--store`, on an empty directory.
struct Row {
    verb: &'static str,
    args: &'static [&'static str],
}

/// The verbs that run from an unrelated directory with no world around them.
const MATRIX: &[Row] = &[
    Row {
        verb: "ps",
        args: &["--store", "store", "ps"],
    },
    Row {
        verb: "kill",
        args: &["--store", "store", "kill", "--all", "--yes"],
    },
];

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

/// Guards: U1 — every verb that `candler --help` lists has a row in the working-directory matrix
/// or a place on `PENDING_CWD`, and no verb has both. Oracle: the `Commands:` block of the real
/// binary's `--help`. Bite: a verb added to clap with neither row nor pending entry, or a pending
/// entry removed, names the verb and fails; so does a row kept on the list after it has one.
#[test]
fn u1_every_verb_has_a_row() {
    let listed = listed_verbs();
    let rows: BTreeSet<String> = MATRIX.iter().map(|row| row.verb.to_owned()).collect();
    let pending: BTreeSet<String> = PENDING_CWD.iter().map(|verb| (*verb).to_owned()).collect();
    assert!(!listed.is_empty(), "the help lists no verb");
    let both: Vec<_> = rows.intersection(&pending).collect();
    assert!(
        both.is_empty(),
        "verbs with a row and a pending entry: {both:?}"
    );
    let covered: BTreeSet<String> = rows.union(&pending).cloned().collect();
    assert_eq!(
        covered, listed,
        "the matrix and PENDING_CWD cover the listed verbs"
    );
}

/// Guards: U1 — each verb with a row runs from a fresh directory, with `HOME` and the XDG
/// variables inside it and a relative `--store`, exits 0 and leaves the directory empty. Oracle:
/// the exit status and the directory listing of that run. Bite: a verb that reads the checkout it
/// was built in, or writes beside the working directory, fails its row.
#[test]
fn u1_each_verb_runs_from_an_unrelated_directory() {
    assert!(!MATRIX.is_empty(), "the matrix has no row");
    for row in MATRIX {
        let dir = TempDir::new().expect("a directory");
        let home: &Path = dir.path();
        let output = Command::new(env!("CARGO_BIN_EXE_candler"))
            .args(row.args)
            .current_dir(home)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", home)
            .env("XDG_DATA_HOME", home)
            .env("XDG_STATE_HOME", home)
            .env("XDG_CACHE_HOME", home)
            .output()
            .expect("the binary runs");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{}: {stderr}", row.verb);
        let left: Vec<_> = std::fs::read_dir(home)
            .expect("the directory lists")
            .map(|entry| entry.expect("an entry").file_name())
            .collect();
        assert!(
            left.is_empty(),
            "{} left {left:?} in the directory",
            row.verb
        );
    }
}

/// Guards: U2 — the help of every option, global or of a verb, ends with a sentence that starts
/// `cannot be derived: ` or `not an experiment setting: `; `--help`, `--version` and positional
/// arguments are outside the walk. Oracle: the `--help` of the real binary, each option's
/// continuation lines joined. Bite: an option whose stamp is deleted, or whose stamp is followed by
/// another sentence, names the option and fails.
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
    /// variables inside the world and the wrapped fake client first on the path.
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
                    "{}:{}:/usr/bin:/bin",
                    self.wrap.display(),
                    self.world.bin.display()
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
        let dir = JobDir::open(&StoreLayout::new(&self.world.store()), &id).expect("the job");
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
/// world from an unrelated directory: the job id is printed, and every `docker build` it asks for
/// is given a context below the store root and outside the source tree; the contexts of the base
/// and gateway images are the written assets, below `<store root>/assets/<digest>/docker/`, hold
/// the bytes of the tree's directory of the same name, and are the only contexts that outlive
/// their build. Oracle: the fake client's call log and the tree's files. Bite: asset paths found
/// through `CARGO_MANIFEST_DIR`, so the recorded context sits under the source tree.
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
/// directory `<store root>/assets/<digest>/docker` holds every file of `assets/docker` with equal
/// bytes and no other, and a second `resolve` writes nothing. Oracle: the tree, read from
/// `CARGO_MANIFEST_DIR` (test code), and the modification times of the written files. Bite: a file
/// of the tree left out of the embedded table, or an embedded file that differs from the tree.
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
fn manifest() -> RunManifest {
    let body = |cell: &str| {
        json!({
            "schema": "run-manifest/1",
            "run": {"cell": cell, "replicate": 1, "attempt": 1},
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
    let manifest = manifest();
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
/// the bare version. Oracle: the output of the same call with the names written out, and the
/// evaluators written into the store. Bite: the `name@` prefix required again (the bare call exits
/// 2 with no report), or the first holder of an ambiguous version taken.
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

/// Guards: U5 — `show --rates --format toml` with no `--evaluator` equals the call that names the
/// evaluator when the store holds one evaluator, and is refused, listing the evaluators, when it
/// holds two. Oracle: the output of the same call with the evaluator named, and the evaluators
/// written into the store. Bite: `--rates` requiring `--evaluator` again (the bare call exits 2
/// with a usage error), or the first of two evaluators taken.
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
/// evaluator holds that version. Oracle: the output of the same call with the name written out.
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
