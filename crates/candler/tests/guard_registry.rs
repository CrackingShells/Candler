//! The binary selects the harness driver, its skill layout, its base image context and its reader
//! by the name an experiment gives, and refuses a name it does not ship.
//!
//! The binary is run from a working directory unrelated to the repository, over the PATH-fake
//! docker of `support/cli_run.rs`: what the registry chose is read from what the fake was asked
//! and from what the run left in the store.
#![cfg(test)]

#[expect(
    dead_code,
    reason = "this guard uses the world and its launch, not every helper of the shared module"
)]
#[path = "support/cli_run.rs"]
mod cli_run;

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
        assert!(
            stderr.contains("claude-code"),
            "{case}: the registered names are listed: {stderr}"
        );
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

/// Guards: R01 §Harness drivers — an import reads the corpus with the reader of the harness its
/// rules name, and refuses a harness with no registered reader, naming it.
/// Oracle: the mini corpus fixture and its rules, whose harness is `claude-code` (stdout counts
/// its runs); the same rules with another name.
/// Bite: a reader chosen without the name: the unknown name would import the corpus.
#[test]
fn registry_ingest_selects_the_reader_by_name() {
    let dir = tempfile::TempDir::new().expect("a directory");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let corpus = root.join("../candler-core/tests/fixtures/mini-corpus");
    let rules = root.join("tests/fixtures/cli/mini-corpus-rules.toml");
    let text = fs::read_to_string(&rules).expect("the rules");
    // BORROW: the argument owns its text
    let given = |path: &PathBuf| path.to_str().expect("a UTF-8 path").to_owned();

    let ok = import(
        dir.path(),
        &dir.path().join("store"),
        &[&given(&corpus), "--rules", &given(&rules)],
    );
    assert!(ok.status.success(), "{ok:?}");
    assert!(
        String::from_utf8_lossy(&ok.stdout).contains("\"runs\""),
        "{ok:?}"
    );

    let renamed = text.replace("name = \"claude-code\"", "name = \"nonesuch\"");
    assert_ne!(renamed, text, "the rules name the harness");
    let other = dir.path().join("other-rules.toml");
    fs::write(&other, renamed).expect("the rules write");
    let refused = import(
        dir.path(),
        &dir.path().join("store-two"),
        &[&given(&corpus), "--rules", &given(&other)],
    );
    assert_eq!(refused.status.code(), Some(2), "{refused:?}");
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("no reader for `nonesuch`"), "{stderr}");
}
