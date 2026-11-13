//! The pieces of the input-seam guard: the two input trees, the fixture plug-in set up beside a
//! [`World`], an environment for the plug-in, an independent sha256, and the scans for a canary.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use candler_core::evaluation::registry::EnvironmentKey;
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::lock::{JobLock, LockState};
use candler_core::store::layout::StoreLayout;
use candler_schemas::primitives::JobId;

use super::cli_run::{World, wait_for};

/// The bytes only the evaluators may see: the content of `answer.txt` of the `truth` input.
pub const CANARY_BYTES: &str = "canary-bytes-7f3a9c41e2d8";

/// The name only the evaluators may see: the stem of a file name of the `truth` input.
pub const CANARY_NAME: &str = "canary-name-5b1e0d97c3a6";

/// The files of the `servers` input (audience `agent`), by path relative to its root: an empty
/// file and a file in nested directories among them.
pub fn servers_tree() -> BTreeMap<String, String> {
    tree(&[
        ("a.txt", "alpha\n"),
        ("empty.txt", ""),
        ("nested/deep/b.txt", "bravo\n"),
    ])
}

/// The files of the `truth` input (audience `evaluators`), by path relative to its root: one
/// holds the canary bytes, one has the canary in its name, one is empty, and two are nested.
pub fn truth_tree() -> BTreeMap<String, String> {
    let named = format!("nested/{CANARY_NAME}.txt");
    let mut files = tree(&[("empty.dat", ""), ("nested/dir/key.txt", "kilo\n")]);
    files.insert("answer.txt".to_owned(), format!("{CANARY_BYTES}\n"));
    files.insert(named, "other\n".to_owned());
    files
}

fn tree(files: &[(&str, &str)]) -> BTreeMap<String, String> {
    files
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect()
}

/// Writes `files` below `root`.
fn write_tree(root: &Path, files: &BTreeMap<String, String>) {
    for (path, text) in files {
        let target = root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, text).unwrap();
    }
}

/// Writes both trees beside the world's experiment file and declares them in it: `servers` for
/// the agent, which is the default audience, and `truth` for the evaluators.
pub fn declare_inputs(world: &World) {
    let base = world.experiment.parent().unwrap().join("inputs");
    write_tree(&base.join("servers"), &servers_tree());
    write_tree(&base.join("truth"), &truth_tree());
    let mut text = fs::read_to_string(&world.experiment).unwrap();
    text.push_str(
        "\n[[inputs]]\nkind = \"servers\"\npath = \"inputs/servers\"\nat = \"srv\"\n\n\
         [[inputs]]\nkind = \"truth\"\npath = \"inputs/truth\"\naudience = \"evaluators\"\n",
    );
    fs::write(&world.experiment, text).unwrap();
}

/// The sha256 of `bytes` as `shasum` prints it, as 64 hex digits.
pub fn shasum(bytes: &[u8]) -> String {
    let mut child = Command::new("shasum")
        .args(["-a", "256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "shasum ran");
    String::from_utf8(out.stdout).unwrap()[..64].to_owned()
}

/// The expected sha256 of each file of `files`, by path.
pub fn file_hashes(files: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    files
        .iter()
        .map(|(path, text)| (path.clone(), shasum(text.as_bytes())))
        .collect()
}

/// The digest of the input whose files are `files`: the sha256 of the RFC 8785 text of the list
/// `[{path, sha256}]` sorted by path, written by hand. The keys come in code-point order, the
/// text has no space, and the names here are ASCII with no character to escape.
pub fn list_digest(files: &BTreeMap<String, String>) -> String {
    let entries: Vec<String> = file_hashes(files)
        .iter()
        .map(|(path, hash)| format!("{{\"path\":\"{path}\",\"sha256\":\"sha256:{hash}\"}}"))
        .collect();
    format!(
        "sha256:{}",
        shasum(format!("[{}]", entries.join(",")).as_bytes())
    )
}

/// The helper's `uv` project, from which the fixture plug-in runs.
fn helper() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../python/candler-evaluator")
}

/// The fixture plug-in's directory.
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugins/needs-inputs")
}

/// The fixture plug-in's suite.
pub fn suite() -> PathBuf {
    fixture().join("suite.toml")
}

/// Sets the fixture plug-in up in the world: its manifest with the kinds directory filled in,
/// and a link to the helper's project. Returns the manifest.
pub fn plugin(world: &World) -> PathBuf {
    let dir = world.root.join("plugin");
    fs::create_dir_all(&dir).unwrap();
    symlink(helper(), dir.join("env")).unwrap();
    let manifest = fs::read_to_string(fixture().join("plugin.toml"))
        .unwrap()
        .replace("@KINDS@", &fixture().join("kinds").display().to_string());
    let path = dir.join("plugin.toml");
    fs::write(&path, manifest).unwrap();
    path
}

/// Builds the helper's environment under the world's state home and marks it `ready`, where
/// `candler evaluate` looks for it.
pub fn sync_environment(world: &World) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let key = runtime
        .block_on(EnvironmentKey::of_files(
            &helper().join("uv.lock"),
            &helper().join("pyproject.toml"),
        ))
        .unwrap();
    let dir = key.dir(&world.state.join("candler").join("environments"));
    fs::create_dir_all(&dir).unwrap();
    let synced = Command::new("uv")
        .args(["sync", "--frozen"])
        .env("UV_PROJECT_ENVIRONMENT", dir.join(".venv"))
        .current_dir(helper())
        .status()
        .unwrap();
    assert!(synced.success(), "uv sync built the helper's environment");
    fs::write(dir.join("ready"), key.digest()).unwrap();
}

/// Runs `candler evaluate` over the store of the world with the fixture plug-in and suite, from a
/// project directory whose `candler.toml` lists the manifest's directory under `plugins.dirs`.
pub fn evaluate(world: &World, manifest: &Path) -> Output {
    let project = world.root.join("project");
    fs::create_dir_all(&project).unwrap();
    let dir = manifest.parent().unwrap();
    fs::write(
        project.join("candler.toml"),
        format!("[plugins]\ndirs = [\"{}\"]\n", dir.display()),
    )
    .unwrap();
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .arg("--store")
        .arg(world.store())
        .arg("evaluate")
        .arg(suite())
        .current_dir(&project)
        .env_remove("CANDLER_STORE")
        .env("HOME", world.root.join("home"))
        .env("XDG_CONFIG_HOME", world.root.join("config"))
        .env("XDG_STATE_HOME", &world.state)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// Waits for the job `launch` printed to end and release its lock, and returns its id.
pub fn finish(world: &World, out: &Output) -> JobId {
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "the launcher exits 0: {:?}\nstdout: {stdout}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let id: JobId = stdout.trim().parse().unwrap();
    let dir = JobDir::open(&StoreLayout::new(&world.store()), &id).unwrap();
    let events = dir.path().join("events.jsonl");
    let ended = wait_for(Duration::from_secs(90), || {
        fs::read_to_string(&events).is_ok_and(|text| text.contains("job.ended"))
            && JobLock::probe(&dir).unwrap() == LockState::Free
    });
    assert!(ended, "the job ends\ncalls:\n{}", world.calls());
    id
}

/// Every file below `root`, at any depth, and every directory too.
pub fn entries_under(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_owned()];
    while let Some(dir) = pending.pop() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            found.push(path.clone());
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                pending.push(path);
            }
        }
    }
    found
}

/// The places below `root` where `needle` occurs, as a name of a path or as bytes of a file.
pub fn occurrences(root: &Path, needle: &str) -> Vec<String> {
    let mut found = Vec::new();
    for path in entries_under(root) {
        if path.to_string_lossy().contains(needle) {
            found.push(format!("name {}", path.display()));
        }
        let is_file = fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file());
        if is_file
            && fs::read(&path).is_ok_and(|bytes| {
                bytes
                    .windows(needle.len())
                    .any(|window| window == needle.as_bytes())
            })
        {
            found.push(format!("bytes {}", path.display()));
        }
    }
    found
}
