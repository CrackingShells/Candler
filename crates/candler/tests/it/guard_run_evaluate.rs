//! A run graded through the binary: the agent writes a file into its workspace, and an evaluation
//! whose plug-in needs that file grades the bytes the run stored.
#![cfg(test)]
#![cfg(unix)]

use crate::support::cli_run::{self, AGENT_BYTES, AGENT_FILE, World};
use crate::support::seam::{entries_under, finish, shasum, sync_environment};

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use candler_core::store::layout::StoreLayout;
use serde_json::Value;

/// The plug-in's module: one kind that needs the agent's file and reports the sha256 of each
/// file of its context's `outputs/`, by its path there.
const KINDS: &str = r#""""A plug-in of one kind that needs the agent's file and reports what it read."""

import hashlib
import json

NAME = "graded"
VERSION = "0.1.0"
KIND_SPECS = {"read_output": {"needs": {"artifacts": ["outputs/answer.txt"]}}}


def read_output(params, context, prior):
    outputs = context.root / "outputs"
    seen = {
        path.relative_to(outputs).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(outputs.rglob("*"))
        if path.is_file()
    }
    return {"status": "pass", "evidence": [{"note": json.dumps(seen, sort_keys=True)}]}


KINDS = {"read_output": read_output}
"#;

/// The suite of one gate graded by [`KINDS`].
const SUITE: &str = r#"schema = "suite/1"

[[expectations]]
id = "G1"
role = "gate"
kind = "read_output"
depends_on = []

[expectations.evaluator]
name = "graded"
range = "^0.1"

[expectations.params]
"#;

/// Sets the plug-in up in the world: its module, its manifest, a link to the helper's project
/// and the suite. Returns the manifest and the suite.
fn plugin(world: &World) -> (PathBuf, PathBuf) {
    let dir = world.root.join("graded");
    let kinds = dir.join("kinds");
    fs::create_dir_all(&kinds).unwrap();
    fs::write(kinds.join("graded_output.py"), KINDS).unwrap();
    let helper = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../python/candler-evaluator");
    symlink(helper, dir.join("env")).unwrap();
    let manifest = dir.join("plugin.toml");
    fs::write(
        &manifest,
        format!(
            r#"schema = "plugin-manifest/1"
name = "graded"
version = "0.1.0"
command = ["env", "PYTHONPATH={}", "uv", "run", "--frozen", "python", "-m", "candler_evaluator", "graded_output"]

[protocol]
major = 1
minor = 0

[environment]
kind = "uv"
project = "env"
lock = "env/uv.lock"
"#,
            kinds.display()
        ),
    )
    .unwrap();
    let suite = dir.join("suite.toml");
    fs::write(&suite, SUITE).unwrap();
    (manifest, suite)
}

/// Runs `candler evaluate` over the store of the world with `suite`, from a project directory
/// whose `candler.toml` lists the manifest's directory under `plugins.dirs`.
fn evaluate(world: &World, manifest: &Path, suite: &Path) -> Output {
    let project = world.root.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("candler.toml"),
        format!(
            "[plugins]\ndirs = [\"{}\"]\n",
            manifest.parent().unwrap().display()
        ),
    )
    .unwrap();
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .arg("--store")
        .arg(world.store())
        .arg("evaluate")
        .arg(suite)
        .current_dir(&project)
        .env_remove("CANDLER_STORE")
        .env("HOME", world.root.join("home"))
        .env("XDG_CONFIG_HOME", world.root.join("config"))
        .env("XDG_STATE_HOME", &world.state)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// The one sealed run directory of the store.
fn sealed_run(layout: &StoreLayout) -> PathBuf {
    let runs: Vec<PathBuf> = fs::read_dir(layout.runs_dir())
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("run.done").is_file())
        .collect();
    assert_eq!(runs.len(), 1, "one sealed run: {runs:?}");
    runs.into_iter().next().unwrap()
}

/// Guards: S1, the seal as the one owner of output bytes — a run whose agent writes a file is
/// graded from that file's bytes. After `candler run` the store holds the blob of every file
/// entry of the run's `tree.json` and the run keeps no `workspace/`; `candler evaluate` with a
/// plug-in that needs the file reports the sha256 of the bytes the agent wrote.
///
/// Oracle: the agent's bytes are a literal of the fake client, and their sha256 is taken by
/// `shasum`; the blob's place `blobs/<hex of sha256>` is R01's store layout.
///
/// Bite: leave the blob step out of the seal (`evaluate` fails with "open a blob failed").
#[test]
fn a_run_that_writes_a_file_is_graded_from_its_stored_bytes() {
    let world = cli_run::world();
    let (manifest, suite) = plugin(&world);
    sync_environment(&world);

    let launched = world.launch();
    finish(&world, &launched);
    world.assert_sealed(1);
    let layout = StoreLayout::new(&world.store());
    let run = sealed_run(&layout);
    let want = shasum(AGENT_BYTES.as_bytes());

    let evaluated = evaluate(&world, &manifest, &suite);
    let stderr = String::from_utf8_lossy(&evaluated.stderr).into_owned();
    let stdout = String::from_utf8_lossy(&evaluated.stdout).into_owned();
    assert!(
        evaluated.status.success(),
        "evaluate exits 0: {:?}\nstdout: {stdout}\nstderr: {stderr}",
        evaluated.status
    );
    let report: Value = serde_json::from_slice(&evaluated.stdout).unwrap();
    assert_eq!(report["failed_requests"], serde_json::json!([]), "{report}");

    let records: Vec<Value> = entries_under(&layout.outcomes_dir())
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .flat_map(|path| {
            fs::read_to_string(path)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(records.len(), 1, "one outcome record: {records:?}");
    assert_eq!(
        records[0]["status"], "pass",
        "the plug-in passed: {}",
        records[0]
    );
    let note = records[0]["evidence"][0]["note"].as_str().unwrap();
    let seen: Value = serde_json::from_str(note).unwrap();
    let read: Vec<&str> = seen
        .as_object()
        .unwrap()
        .values()
        .map(|hash| hash.as_str().unwrap())
        .collect();
    assert_eq!(
        read,
        [want.as_str()],
        "the plug-in read the agent's bytes: {seen}\ntree: {}",
        fs::read_to_string(run.join("tree.json")).unwrap()
    ); // BORROW: explicit .as_str()

    // The store side: a blob for every file of the tree, and no workspace kept beside them.
    let tree: Value =
        serde_json::from_str(&fs::read_to_string(run.join("tree.json")).unwrap()).unwrap();
    let files: Vec<&Value> = tree["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["kind"] == "file")
        .collect();
    assert!(
        files.iter().any(|entry| entry["path"] == AGENT_FILE),
        "the agent's file is in the tree: {tree}"
    );
    for entry in files {
        let digest = entry["sha256"].as_str().unwrap();
        let hex = digest.rsplit(':').next().unwrap();
        assert!(
            layout.blobs_dir().join(hex).is_file(),
            "the blob of {entry} is stored"
        );
    }
    assert!(
        !run.join("workspace").exists(),
        "the sealed run keeps no workspace"
    );
    world.remove();
}
