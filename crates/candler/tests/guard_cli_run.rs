//! `candler run` starts a detached supervisor and returns; nothing marked outlives the job.
#![cfg(test)]

#[path = "support/cli_run.rs"]
mod cli_run;

use std::fs;
use std::time::Duration;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::lock::{JobLock, LockState};
use candler_core::store::layout::StoreLayout;
use candler_core::supervise::platform::process_start;
use candler_schemas::primitives::JobId;

use cli_run::{World, wait_for, world};

/// The word of a docker call that removes a container, built from parts.
fn remove_verb() -> String {
    ["r", "m"].concat()
}

/// Guards: I15, I16 — run returns at once, the supervisor outlives it holding the job lock, and
/// after job.ended nothing marked or unmatched remains.
///
/// Oracle: the OS; the PATH-fake docker log.
///
/// Bite: wait on the supervisor in the launcher (no detach).
#[test]
fn b35_run_returns_supervisor_outlives() {
    let world = world();
    let out = world.launch();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "the launcher exits 0: {:?}\nstdout: {stdout}\nstderr: {stderr}\ncalls:\n{}",
        out.status,
        world.calls()
    );
    let id: JobId = stdout.trim().parse().unwrap();
    let layout = StoreLayout::new(&world.store());
    let dir = JobDir::open(&layout, &id).unwrap();

    // The launcher has exited, and the supervisor named in job.json is alive, with its start.
    let record = dir.record().unwrap();
    let supervisor = record.supervisor();
    assert_ne!(supervisor.pid(), std::process::id());
    assert_eq!(
        process_start(supervisor.pid()),
        *supervisor.start(),
        "the supervisor named in job.json is alive with the recorded start time"
    );
    // A test-side try of the job lock reads held: the supervisor holds it.
    assert_eq!(
        JobLock::probe(&dir).unwrap(),
        LockState::Held,
        "the supervisor holds the job lock after the launcher has gone"
    );

    // The job ends, and the lock is released last.
    let events = dir.path().join("events.jsonl");
    let ended = wait_for(Duration::from_secs(90), || {
        fs::read_to_string(&events).is_ok_and(|text| text.contains("job.ended"))
            && JobLock::probe(&dir).unwrap() == LockState::Free
    });
    assert!(
        ended,
        "job.ended appears and the lock is released\ncalls:\n{}\nsupervisor.log:\n{}",
        world.calls(),
        fs::read_to_string(dir.path().join("supervisor.log")).unwrap_or_default()
    );

    // No process carries a marker of this job's: the gateway, the skill archive, a control
    // container of the supervisor, or a run key the fake saw.
    let calls = world.calls();
    let mut markers = vec![
        "CANDLER_RUN=gateway ".to_owned(),
        "CANDLER_RUN=skill-archive ".to_owned(),
        format!("CANDLER_RUN=control-{}-", supervisor.pid()),
    ];
    for key in cli_run::run_keys(&calls) {
        markers.push(format!("CANDLER_RUN={key} "));
    }
    let listing = cli_run::ps_with_environment();
    for line in listing.lines() {
        for marker in &markers {
            assert!(
                !line.contains(marker.as_str()),
                "a process marked by the job remains: {line}"
            );
        }
    }

    // Every container the fake handed out has a removal in its log.
    let handed = world.handed_out();
    assert!(!handed.is_empty(), "the job created containers\n{calls}");
    let verb = remove_verb();
    for id in &handed {
        assert!(
            calls
                .lines()
                .any(|line| line.contains(&format!(" {verb} ")) && line.ends_with(id.as_str())),
            "container {id} has no removal in the log\n{calls}"
        );
    }
    world.remove();
}

/// Waits for the job `launch` printed to end and release its lock, and returns its id.
fn finish(world: &World, out: &std::process::Output) -> JobId {
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "the launcher exits 0: {:?}\nstdout: {stdout}\nstderr: {stderr}",
        out.status
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

/// What the launcher said on its standard error.
fn stderr_of(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Guards: R01 §Experiment file — a relative `[treatment] source` is read from the experiment
/// file's directory, whatever directory `candler run` is started in.
///
/// Oracle: hand-written: the skills repository sits beside the experiment file as `skills`, the
/// working directory holds no `skills`, and the launcher must exit 0.
///
/// Bite: join the source to the working directory instead of the experiment file's directory.
#[test]
fn treatment_source_is_relative_to_the_experiment_file() {
    let world = world();
    world.set_source_line("source = \"skills\"\n");
    assert!(
        !world.work.join("skills").exists(),
        "the working directory holds no skills"
    );
    let out = world.launch();
    finish(&world, &out);
    world.remove();
}

/// Guards: R01 §Experiment file — a treatment pinned to a tag that names no source is refused,
/// and the refusal names `treatment.source`.
///
/// Oracle: hand-written: the message must hold the path `treatment.source`, and no job starts.
///
/// Bite: drop the refusal of a tag with no source.
#[test]
fn treatment_source_is_required_by_a_tag() {
    let world = world();
    world.set_source_line("");
    let out = world.launch();
    let stderr = stderr_of(&out);
    assert!(!out.status.success(), "the launcher refuses\n{stderr}");
    assert!(
        stderr.contains("treatment.source"),
        "the refusal names the path\n{stderr}"
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        "no job id is printed"
    );
    world.remove();
}

/// Guards: R01 §Experiment file — a treatment that names no tag, no commit and no source is
/// refused before any job exists, and the refusal names `treatment.source`.
///
/// Oracle: hand-written: the message holds the path `treatment.source`, no job id is printed,
/// and the store holds no job.
///
/// Bite: accept a treatment that names no source.
#[test]
fn treatment_source_is_required_by_a_name_alone() {
    let world = world();
    world.set_name_only();
    let out = world.launch();
    let stderr = stderr_of(&out);
    assert!(!out.status.success(), "the launcher refuses\n{stderr}");
    assert!(
        stderr.contains("treatment.source"),
        "the refusal names the path\n{stderr}"
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        "no job id is printed"
    );
    let jobs = world.store().join("jobs");
    assert!(
        fs::read_dir(&jobs)
            .ok()
            .is_none_or(|mut found| found.next().is_none()),
        "no job directory exists"
    );
    world.remove();
}

/// Guards: R01 §Experiment file — the retired setting `request.treatment_source` is refused by
/// name, a relative value and an absolute one alike, before any job exists, and the refusal
/// points to `[treatment] source`.
///
/// Oracle: hand-written: the message holds the setting's path and `[treatment] source`, no job
/// id is printed and the store holds no job.
///
/// Bite: drop the refusal of the setting in `request_settings`.
#[test]
fn treatment_source_setting_is_refused_by_name() {
    let world = world();
    let absolute = world.skills.display().to_string();
    for value in ["skills", absolute.as_str()] {
        let setting = format!("request.treatment_source={value}");
        let out = world.launch_with(&["--set", setting.as_str()]);
        let stderr = stderr_of(&out);
        assert!(
            !out.status.success(),
            "{value}: the launcher refuses\n{stderr}"
        );
        assert!(
            stderr.contains("request.treatment_source") && stderr.contains("[treatment] source"),
            "{value}: the refusal names the setting and its replacement\n{stderr}"
        );
        assert!(
            String::from_utf8_lossy(&out.stdout).trim().is_empty(),
            "{value}: no job id is printed"
        );
        let jobs = world.store().join("jobs");
        assert!(
            fs::read_dir(&jobs)
                .ok()
                .is_none_or(|mut found| found.next().is_none()),
            "{value}: no job directory exists"
        );
    }
    world.remove();
}

/// Guards: R01 §Experiment file — a source given by `--set request.treatment` is read against
/// the current directory, as a command-line path is, and its `from` is an absolute path with no
/// `.` or `..` segment.
///
/// Oracle: hand-written: the skills repository is `../skills` from the working directory, and
/// the manifest's `from` must name that repository, canonically, without a relative segment.
///
/// Bite: leave the `--set` source unresolved.
#[test]
fn treatment_source_given_by_set_is_relative_to_the_cwd() {
    let world = world();
    let treatment = r#"request.treatment={"name":"demo","tag":"v1","source":"../skills"}"#;
    let out = world.launch_with(&["--set", treatment]);
    finish(&world, &out);
    let layout = StoreLayout::new(&world.store());
    let mut froms = Vec::new();
    for text in manifests_under(&layout.runs_dir()) {
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        collect_from(&json, &mut froms);
    }
    assert!(!froms.is_empty(), "the manifest records a `from`");
    for from in froms {
        let path = std::path::Path::new(&from);
        assert!(path.is_absolute(), "`{from}` is absolute");
        assert!(
            path.components().all(|part| matches!(
                part,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )),
            "`{from}` holds no `.` or `..` segment"
        );
        assert_eq!(
            fs::canonicalize(path).unwrap(),
            fs::canonicalize(&world.skills).unwrap(),
            "`{from}` is the skills repository"
        );
    }
    world.remove();
}

/// Every string held under a key `from`, at any depth of `value`.
fn collect_from(value: &serde_json::Value, found: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, inner) in map {
                match (key.as_str(), inner) {
                    ("from", serde_json::Value::String(text)) => found.push(text.clone()),
                    _ => collect_from(inner, found),
                }
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|item| collect_from(item, found)),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
}

/// Guards: I7, I12 — a tag moved in the checkout after a run is refused when the experiment is
/// run again, the refusal shows the commit the run pinned and the one the tag names now, and
/// `--force` runs anyway and records the deviation.
///
/// Oracle: `git rev-parse v1^{commit}` in the checkout before and after the move.
///
/// Bite: resolve the tag to the stored commit instead of reading it from the checkout again.
#[test]
fn treatment_source_moved_tag_is_refused_on_resume() {
    let world = world();
    let out = world.launch();
    finish(&world, &out);

    let (before, after) = world.move_tag();
    assert_ne!(before, after, "the tag moved");
    let refused = world.launch();
    let stderr = stderr_of(&refused);
    assert!(!refused.status.success(), "the launcher refuses\n{stderr}");
    assert!(
        stderr.contains("treatment"),
        "the refusal names the treatment\n{stderr}"
    );
    assert!(
        stderr.contains(&before) && stderr.contains(&after),
        "the refusal shows both commits ({before}, {after})\n{stderr}"
    );

    let forced = world.launch_with(&["--force"]);
    finish(&world, &forced);
    let layout = StoreLayout::new(&world.store());
    let manifests = manifests_under(&layout.runs_dir());
    assert!(
        manifests.iter().any(|text| text.contains("\"deviations\"")
            && text.contains(&before)
            && text.contains(&after)),
        "a forced run records the deviation"
    );
    world.remove();
}

/// The text of every `manifest.json` below `dir`.
fn manifests_under(dir: &std::path::Path) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(manifests_under(&path));
        } else if path.file_name().is_some_and(|name| name == "manifest.json") {
            found.extend(fs::read_to_string(&path));
        }
    }
    found
}
