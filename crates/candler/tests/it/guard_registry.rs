//! The binary selects the harness driver, its skill layout, its base image context and its reader
//! by the name an experiment gives, and refuses a name it does not ship.
//!
//! The binary is run from a working directory unrelated to the repository, over the PATH-fake
//! docker of `support/cli_run.rs`: what the registry chose is read from what the fake was asked
//! and from what the run left in the store.
#![cfg(test)]

use crate::support::cli_run;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::lock::{JobLock, LockState};
use candler_core::store::layout::StoreLayout;
use candler_schemas::primitives::JobId;
use cli_run::{World, wait_for, world};

/// The world with the experiment's harness name replaced by `name`.
fn world_naming(name: &str) -> World {
    let world = world();
    let text = fs::read_to_string(&world.experiment).expect("the experiment");
    let renamed = text.replace("name = \"claude-code\"", &format!("name = \"{name}\""));
    assert_ne!(renamed, text, "the experiment names the harness");
    fs::write(&world.experiment, renamed).expect("the experiment writes");
    world
}

/// Runs `candler resolve` on the world's experiment with `extra` arguments, from the unrelated
/// working directory of the world, over the PATH-fake docker.
fn resolve_with(world: &World, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .arg("resolve")
        .arg(&world.experiment)
        .args(extra)
        .current_dir(&world.work)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", world.bin.display()))
        .env("HOME", world.root.join("home"))
        .env("XDG_STATE_HOME", &world.state)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the binary runs")
}

/// Guards: R01 §Harness drivers — a harness name the binary does not ship is refused before any
/// docker call, with the code `experiment.unknown_harness`, naming the harness and the registered
/// ones, whichever way the name arrives: in the experiment file, as an object given to `--set
/// request.harness`, or as the bare name given to it; and `candler resolve` refuses it the same
/// way `candler run` does.
/// Oracle: the registered name, `claude-code`, which the experiment fixtures and the recorded
/// streams carry; the fake docker's log, empty before the refusal; the code in
/// `docs/contract/codes.toml`.
/// Bite: an unknown name that falls back to Claude Code, which makes the launcher start a job; a
/// `resolve` that builds its services before it asks the registry; a bare name that is not read.
#[test]
fn registry_unknown_harness_is_refused_naming_it() {
    let json = ["--error-format", "json"];
    // (case, the experiment names this harness, command, extra arguments, the name refused)
    let rows: [(&str, &str, &str, &[&str], &str); 6] = [
        ("run, in the file", "nonesuch", "run", &[], "nonesuch"),
        (
            "run, an object given to --set",
            "claude-code",
            "run",
            &["--set", "request.harness={\"name\":\"other-one\"}"],
            "other-one",
        ),
        (
            "run, a bare name given to --set",
            "claude-code",
            "run",
            &["--set", "request.harness=bare-one"],
            "bare-one",
        ),
        (
            "resolve, in the file",
            "nonesuch",
            "resolve",
            &[],
            "nonesuch",
        ),
        (
            "resolve, a bare name given to --set",
            "claude-code",
            "resolve",
            &["--set", "request.harness=bare-two"],
            "bare-two",
        ),
        (
            "resolve, an object given to --set",
            "claude-code",
            "resolve",
            &["--set", "request.harness={\"name\":\"other-two\"}"],
            "other-two",
        ),
    ];
    for (case, named, command, extra, refused) in rows {
        let world = if named == "claude-code" {
            self::world()
        } else {
            world_naming(named)
        };
        let args: Vec<&str> = json.iter().chain(extra.iter()).copied().collect();
        let out = if command == "run" {
            world.launch_with(&args)
        } else {
            resolve_with(&world, &args)
        };
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(out.status.code(), Some(2), "{case}: stderr: {stderr}");
        assert!(
            stderr.contains("experiment.unknown_harness"),
            "{case}: the refusal carries its code: {stderr}"
        );
        assert!(
            stderr.contains(&format!("`{refused}`")),
            "{case}: the name is in the refusal: {stderr}"
        );
        for registered in ["claude-code", "opencode"] {
            assert!(
                stderr.contains(registered),
                "{case}: the registered name {registered} is listed: {stderr}"
            );
        }
        assert_eq!(world.calls(), "", "{case}: no docker call precedes it");
    }
}

/// Guards: R01 §Harness drivers — the harness an experiment names is the one whose driver reads
/// the image's version (its probe runs in the control container), whose base context names the
/// base image, and whose skill layout the detached supervisor uses: the job, started from an
/// unrelated working directory, seals a run.
/// Oracle: the fake docker's log (the probe is `claude --version` under `timeout`, 10 seconds;
/// the base tag is `candler-base:src-<hash of assets/docker/base>`) and the sealed run directory of the store.
/// Bite: a registry that returns another driver, base context or skill layout for the name.
#[test]
fn registry_selects_the_driver_skills_and_base_context_by_name() {
    let world = world();
    let out = world.launch();
    assert!(
        out.status.success(),
        "stderr: {}\ncalls:\n{}",
        String::from_utf8_lossy(&out.stderr),
        world.calls()
    );
    let id: JobId = String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .expect("a job id");
    let dir = JobDir::open(&StoreLayout::new(&world.store()), &id).expect("the job directory");
    let events = dir.path().join("events.jsonl");
    let ended = wait_for(Duration::from_secs(90), || {
        fs::read_to_string(&events).is_ok_and(|text| text.contains("job.ended"))
            && JobLock::probe(&dir).expect("a probe") == LockState::Free
    });
    assert!(ended, "the job ends\ncalls:\n{}", world.calls());
    let calls = world.calls();
    assert!(
        calls
            .lines()
            .any(|line| line.contains("--entrypoint timeout")
                && line.ends_with(" 10 claude --version")),
        "the driver's probe ran in a control container:\n{calls}"
    );
    assert!(
        calls
            .lines()
            .any(|line| line.starts_with("image inspect") && line.contains("candler-base:src-")),
        "the base image is named by its harness's base context:\n{calls}"
    );
    world.assert_sealed(1);
}

/// The binary run as `import` from `cwd`, with `args`.
fn import(cwd: &Path, store: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .arg("--store")
        .arg(store)
        .arg("import")
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", cwd)
        .output()
        .expect("the binary runs")
}

/// Guards: R01 §Harness drivers — ingest reads each path with the reader of the harness that
/// wrote it, named as its driver names itself: one `candler ingest` over a Claude Code stream and
/// an `OpenCode` stream stores two runs, each under its own harness name and none read with the
/// other's reader; `candler import` of rules that name a harness with no reader is refused,
/// naming it.
/// Oracle: the recorded `completed_o1` stdout of `opencode_facts` and a hand-written Claude Code
/// stream; the harness names `claude-code` and `opencode` the drivers' `id()` give; the stored
/// manifests' `comparable.harness.name`; the mini corpus rules with another name.
/// Bite: one reader chosen once for every path, or the recognised name taken from a literal: the
/// second run is read as Claude Code, or both are named `claude-code`; a reader chosen without
/// the name: the unknown name would import the corpus.
#[test]
fn registry_ingest_reads_each_path_with_its_own_harness() {
    let dir = tempfile::TempDir::new().expect("a directory");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let opencode = fs::read(
        root.join("../candler-harness-opencode/tests/fixtures/streams/completed_o1/stdout.jsonl"),
    )
    .expect("the recorded stream");
    let claude = concat!(
        r#"{"type":"system","subtype":"init","session_id":"S","model":"m","claude_code_version":"2.1.220"}"#,
        "\n",
        r#"{"type":"result","subtype":"success","is_error":false,"session_id":"S"}"#,
        "\n"
    );
    let first = dir.path().join("claude");
    let second = dir.path().join("opencode");
    for folder in [&first, &second] {
        fs::create_dir_all(folder).expect("a folder");
    }
    fs::write(first.join("stdout.jsonl"), claude).expect("a stream");
    fs::write(second.join("stdout.jsonl"), opencode).expect("a stream");
    // The run's collected export, beside its stream: ingest reads it as the run did.
    fs::copy(
        root.join("../candler-harness-opencode/tests/fixtures/streams/completed_o1/export.json"),
        second.join("export.json"),
    )
    .expect("the recorded export");

    let store = dir.path().join("store");
    let out = Command::new(env!("CARGO_BIN_EXE_candler"))
        .current_dir(dir.path())
        .env_remove("CANDLER_STORE")
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join("config"))
        .env("XDG_STATE_HOME", dir.path().join("state"))
        .arg("--store")
        .arg(&store)
        .args(["ingest", "claude", "opencode"])
        .output()
        .expect("the binary runs");
    assert!(out.status.success(), "{out:?}");

    let mut names = Vec::new();
    let mut readers = Vec::new();
    for entry in fs::read_dir(store.join("runs")).expect("runs lists") {
        let path = entry.expect("an entry").path();
        if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'))
        {
            continue;
        }
        let text = fs::read_to_string(path.join("manifest.json")).expect("a manifest");
        let manifest: serde_json::Value = serde_json::from_str(&text).expect("JSON");
        assert_eq!(
            manifest["integrity"]["kind"], "valid",
            "the run is supported by its harness's reader: {manifest}"
        );
        let trace = fs::read_to_string(path.join("trace.jsonl")).expect("a trace");
        let header: serde_json::Value =
            serde_json::from_str(trace.lines().next().expect("a header")).expect("header JSON");
        readers.push(
            header["reader"]
                .as_str()
                .unwrap_or_else(|| panic!("a reader: {header}"))
                .to_owned(), // BORROW: the list owns the names
        );
        names.push(
            manifest["comparable"]["harness"]["name"]
                .as_str()
                .unwrap_or_else(|| panic!("a harness name: {manifest}"))
                .to_owned(), // BORROW: the list owns the names
        );
    }
    names.sort();
    assert_eq!(names, ["claude-code", "opencode"]);
    // Each path is read by its own harness's reader, and the export wins as it does at run time.
    readers.sort();
    assert_eq!(readers.len(), 2, "two runs: {readers:?}");
    assert!(readers[0].starts_with("claude-code"), "{readers:?}"); // INDEX: two readers, checked above
    assert_eq!(readers[1], "opencode-export@0.1.0", "{readers:?}"); // INDEX: two readers, checked above

    let corpus = root.join("../candler-core/tests/fixtures/mini-corpus");
    let rules = fs::read_to_string(root.join("tests/fixtures/cli/mini-corpus-rules.toml"))
        .expect("the rules");
    let renamed = rules.replace("name = \"claude-code\"", "name = \"nonesuch\"");
    assert_ne!(renamed, rules, "the rules name the harness");
    let other = dir.path().join("other-rules.toml");
    fs::write(&other, renamed).expect("the rules write");
    let refused = import(
        dir.path(),
        &dir.path().join("store-two"),
        &[
            corpus.to_str().expect("a UTF-8 path"),
            "--rules",
            other.to_str().expect("a UTF-8 path"),
        ],
    );
    assert_eq!(refused.status.code(), Some(2), "{refused:?}");
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("no reader for `nonesuch`"), "{stderr}");
}
