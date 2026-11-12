#![cfg(test)]
//! The shared fixture suite on the Claude Code driver: the recorded stream-json streams and the two
//! recorded sessions.
//!
//! Every expectation is read from the fixtures' hand-written `expected.toml` facts or from the raw
//! lines with `serde_json`, never from the driver. The inventory a case expects is the `tools` and
//! `mcp_servers` of the first init line, taken from the raw line.

use std::error::Error;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use candler_harness::conformance::{
    Case, ExpectedSession, ReaderSource, RecordBytes, Setup, TrackStep, run,
};
use candler_harness::driver::HarnessDriver;
use candler_harness::skills::SkillTree;
use candler_harness::spec::{
    Cell, CensusSource, FileRoot, GatewayEnv, InitReport, PendingItem, PendingKind, PendingState,
    ProcessEnd, ProcessExit, Workspace,
};
use candler_harness::spec::{SessionClose, SessionRecord};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_harness_claude_code::skills::ClaudeSkills;
use candler_schemas::execution::{GitCommit, TreatmentRef};
use candler_schemas::primitives::{FieldPath, ReasonCode};
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, SessionId};
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::{Value, json};

// TRAIT_OBJECT: a test returns any error its steps raise; the error is only printed
type Res<T> = Result<T, Box<dyn Error>>;

/// A value that stands for the gateway's token; it must appear in no command.
const CANARY: &str = "sk-conformance-canary-7f3a";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn sid(text: &str) -> Res<SessionId> {
    Ok(text.parse()?)
}

fn stdout() -> Res<StreamName> {
    Ok(StreamName::from_str("agent_stream.jsonl")?)
}

fn exited() -> ProcessExit {
    ProcessExit::new(ExitStatus::Code { code: 0 }, ProcessEnd::Exited, Vec::new())
}

/// The cell of a run: the values the driver reads.
fn values() -> Res<Vec<ResolvedValue>> {
    [
        ("model.id", json!("claude-haiku-4-5")),
        ("permission_mode", json!("acceptEdits")),
        ("prompt.template", json!("Build {thing} now")),
        ("prompt.values", json!({"thing": "a domain"})),
    ]
    .into_iter()
    .map(|(path, value)| {
        Ok(ResolvedValue::recorded(
            FieldPath::from_str(path)?,
            JsonValue::new(value)?,
        ))
    })
    .collect()
}

/// The inventory of the first init line of `bytes`, read from the raw line.
fn first_init(bytes: &[u8]) -> Res<InitReport> {
    for line in bytes.split(|b| *b == b'\n') {
        let Ok(Value::Object(object)) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        let init = object.get("type") == Some(&json!("system"))
            && object.get("subtype") == Some(&json!("init"));
        if init {
            let tools = object.get("tools").ok_or("the init line has no tools")?;
            return Ok(InitReport::new(
                tools.clone(), // BORROW: .clone() copies the list out of the line
                object.get("mcp_servers").cloned(),
                CensusSource::Harness,
            )?);
        }
    }
    Err("no init line".into())
}

/// The bytes of a stream fixture, decompressed.
fn gz_stream(name: &str) -> Res<Vec<u8>> {
    let path = fixtures()
        .join("streams")
        .join(name)
        .join("agent_stream.jsonl.gz");
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(path)?).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// A stream case: sessions as the fixture's `expected.toml` lists them.
fn stream_case(name: &str, sessions: Vec<ExpectedSession>, end: EndedBy) -> Res<Case> {
    let bytes = gz_stream(name)?;
    let init = first_init(&bytes)?;
    let observed = sessions
        .iter()
        .filter(|s| s.parent.is_none())
        .map(|s| s.id.clone()) // BORROW: .clone() copies the id out of the expectation
        .collect();
    Ok(Case::new(
        name.to_owned(), // BORROW: .to_owned() makes the owned case name
        ReaderSource::Live,
        stdout()?,
        bytes,
    )
    .with_sessions(sessions, observed)
    .with_init(init)
    .with_end(exited(), end))
}

/// The tasks of a pending list, as the harness lists them.
fn tasks(ids: &[&str]) -> Vec<PendingItem> {
    ids.iter()
        .map(|id| PendingItem::new(PendingKind::Task, (*id).to_owned())) // BORROW: .to_owned() makes owned text
        .collect()
}

/// A session case: the recorded stdout, the hook records interleaved as the host reads them (a Stop
/// line was written before its turn's `result`), and the states expected at `checkpoints`, which
/// are 1-based stdout lines (`"o<n>"`) and 1-based hook lines (`"h<n>"`).
fn session_case(name: &str, session: &str, checkpoints: &[(&str, PendingState)]) -> Res<Case> {
    let dir = fixtures().join("sessions").join(name);
    let bytes = fs::read(dir.join("stdout.jsonl"))?;
    let hooks = fs::read_to_string(dir.join("hooks.jsonl"))?;
    let mut stops = hooks
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(r#""hook_event_name":"Stop""#))
        .map(|(index, line)| (index.saturating_add(1), line.to_owned())) // BORROW: .to_owned() makes owned text
        .collect::<Vec<_>>()
        .into_iter();
    let expect = |label: &str| {
        checkpoints
            .iter()
            .find(|(at, _)| *at == label)
            .map(|(_, state)| state.clone()) // BORROW: .clone() copies the expected state
    };
    let mut track = Vec::new();
    for (index, line) in bytes
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .enumerate()
    {
        let number = index.saturating_add(1);
        let is_result = line.starts_with(br#"{"type":"result""#)
            || serde_json::from_slice::<Value>(line)
                .is_ok_and(|v| v.get("type") == Some(&json!("result")));
        if is_result && let Some((hook_number, stop)) = stops.next() {
            track.push(TrackStep::new(
                RecordBytes::Hook(stop.into_bytes()),
                expect(&format!("h{hook_number}")),
            ));
        }
        track.push(TrackStep::new(
            RecordBytes::Stream(line.to_vec()), // BORROW: .to_vec() copies the line
            expect(&format!("o{number}")),
        ));
    }
    let init = first_init(&bytes)?;
    Ok(Case::new(
        name.to_owned(), // BORROW: .to_owned() makes the owned case name
        ReaderSource::Live,
        stdout()?,
        bytes,
    )
    .with_sessions(
        vec![ExpectedSession::new(
            sid(session)?,
            None,
            EndedBy::Completed,
        )],
        vec![sid(session)?],
    )
    .with_init(init)
    .with_end(exited(), EndedBy::Completed)
    .with_track(track))
}

/// All the cases of this crate's fixtures.
fn cases() -> Res<Vec<Case>> {
    let root = |id: &str| sid(id);
    let child = |parent: &str, call: &str| sid(&format!("{parent}/{call}"));
    let multi = "ea98d538-0c5d-4bb7-a243-1b890dda15d3";
    let denials = "c4795c86-ed65-45c8-9b21-e095d2025a1b";
    let api_error: ReasonCode = "api_error".parse()?;
    Ok(vec![
        stream_case(
            "api_error",
            vec![ExpectedSession::new(
                root("50b18959-0cb5-4deb-b336-1c8c57970383")?,
                None,
                EndedBy::agent_error(Some(api_error.clone())), // BORROW: .clone() copies the reason
            )],
            EndedBy::agent_error(Some(api_error)),
        )?,
        stream_case(
            "multi_init",
            vec![
                ExpectedSession::new(root(multi)?, None, EndedBy::Completed),
                ExpectedSession::new(
                    child(multi, "toolu_01GvxyCVQDtQf2ByXX2qt9gx")?,
                    Some(root(multi)?),
                    EndedBy::Completed,
                ),
            ],
            EndedBy::Completed,
        )?,
        stream_case(
            "subagent_denials",
            vec![
                ExpectedSession::new(root(denials)?, None, EndedBy::Completed),
                ExpectedSession::new(
                    child(denials, "toolu_013DzQNP5eghtZzg2B3yVjBD")?,
                    Some(root(denials)?),
                    EndedBy::Completed,
                ),
            ],
            EndedBy::Completed,
        )?,
        // c1: turn 1 starts a job and ends with it pending (Stop line 1 lists it); the job's
        // notification starts turn 2, which ends with nothing pending. c1's stdout lines are 1..14.
        session_case(
            "c1",
            "e10c5a1a-d40d-4ea6-910d-4a7cd75ace6c",
            &[
                ("o1", PendingState::new(0, false, false, vec![])),
                (
                    "o4",
                    PendingState::new(0, false, false, tasks(&["ba3lu4zpb"])),
                ),
                (
                    "h1",
                    PendingState::new(0, false, true, tasks(&["ba3lu4zpb"])),
                ),
                (
                    "o8",
                    PendingState::new(1, true, true, tasks(&["ba3lu4zpb"])),
                ),
                (
                    "o10",
                    PendingState::new(1, false, true, tasks(&["ba3lu4zpb"])),
                ),
                ("o11", PendingState::new(1, false, true, vec![])),
                ("o12", PendingState::new(1, false, true, vec![])),
                ("h2", PendingState::new(1, false, true, vec![])),
                ("o14", PendingState::new(2, true, true, vec![])),
            ],
        )?,
        // c2: one answer and nothing pending.
        session_case(
            "c2",
            "f8f5f782-f8a0-45c6-a5a5-1fe0e2c3ebdc",
            &[
                ("o1", PendingState::new(0, false, false, vec![])),
                ("h1", PendingState::new(0, false, true, vec![])),
                ("o3", PendingState::new(1, true, true, vec![])),
            ],
        )?,
    ])
}

/// Guards: R01 §Harness drivers, §Session runner — the Claude Code driver passes the shared suite
/// on its recorded streams and sessions: sessions and ends as `expected.toml` lists them, the
/// inventory of the first init line labelled as the harness's, a session spec whose command and
/// hook files carry no secret, a tracker that follows the recorded Stop payloads and the turn ends,
/// and the headless names the core owns. Oracle: the fixtures' hand-written `expected.toml` and the
/// raw lines read with `serde_json`.
#[test]
fn conformance_claude_code_fixtures() -> Res<()> {
    let driver = ClaudeCodeDriver::new()?;
    let values = values()?;
    let cell = Cell::new(&values);
    let workspace = Workspace::new(Path::new("."));
    let canaries = [CANARY.to_owned()]; // BORROW: .to_owned() makes the owned canary
    run(
        &driver,
        &Setup::new(&cell, &workspace, &canaries),
        &cases()?,
    )?;
    Ok(())
}

/// Guards: R01 §Session runner — the session command is the recorded one (stdin open, stream-json
/// both ways, the settings file outside the workspace), the first message is one stream-json user
/// line carrying the rendered prompt, the session closes by end of input, and the hooks are
/// `Stop` and `SessionEnd` commands that end in `|| true; exit 0`. Oracle: the `command` of
/// `fixtures/sessions/PROVENANCE.toml`, recorded by `session_facts`, and the hand-written prompt.
#[test]
fn conformance_claude_session_spec() -> Res<()> {
    let driver = ClaudeCodeDriver::new()?;
    let values = values()?;
    let workspace = Workspace::new(Path::new("."));
    let Some(spec) = driver.session(&Cell::new(&values), &workspace)? else {
        return Err("the driver returns no session spec".into());
    };
    assert_eq!(
        spec.command.argv(),
        [
            "claude",
            "--print",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--model",
            "claude-haiku-4-5",
            "--setting-sources",
            "project,local",
            "--permission-mode",
            "acceptEdits",
            "--settings",
            "/home/agent/.candler/settings.json",
        ]
    );
    let input: Value = serde_json::from_slice(&spec.input)?;
    assert_eq!(
        input,
        json!({"type": "user", "message": {"role": "user",
            "content": [{"type": "text", "text": "Build a domain now"}]}})
    );
    assert_eq!(spec.input.last(), Some(&b'\n'));
    assert_eq!(spec.close, SessionClose::Eof);
    let Some(hooks) = spec.hooks else {
        return Err("the session has no hooks".into());
    };
    assert_eq!(hooks.log.as_str(), ".candler/hooks.jsonl");
    let [file] = hooks.files.as_slice() else {
        return Err("expected one settings file".into());
    };
    assert_eq!(file.path.as_str(), ".candler/settings.json");
    assert_eq!(file.root, FileRoot::Home);
    let settings: Value = serde_json::from_slice(&file.contents)?;
    let events: Vec<&str> = settings["hooks"]
        .as_object()
        .ok_or("no hooks object")?
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(events, ["SessionEnd", "Stop"]);
    for event in events {
        let command = settings["hooks"][event][0]["hooks"][0]["command"]
            .as_str()
            .ok_or("no hook command")?;
        assert!(command.ends_with("|| true; exit 0"), "{command}");
    }
    Ok(())
}

/// Guards: R01 §Session runner, D5 — the hook command appends one JSON line `{written_ns,
/// payload}` to the log, prints nothing and exits 0 even when the log cannot be written, so it can
/// never block or steer the agent; the tracker reads the line it wrote. Oracle: a hand-written
/// payload, and `sh` as the second engine.
#[test]
fn conformance_claude_hook_command_observes_without_deciding() -> Res<()> {
    use std::process::{Command, Stdio};
    let driver = ClaudeCodeDriver::new()?;
    let values = values()?;
    let workspace = Workspace::new(Path::new("."));
    let Some(spec) = driver.session(&Cell::new(&values), &workspace)? else {
        return Err("the driver returns no session spec".into());
    };
    let hooks = spec.hooks.ok_or("the session has no hooks")?;
    let settings: Value =
        serde_json::from_slice(&hooks.files.first().ok_or("no settings file")?.contents)?;
    let command = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
        .as_str()
        .ok_or("no hook command")?;
    let payload = r#"{"hook_event_name":"Stop","background_tasks":[{"id":"b1"}],"session_crons":[{"id":"w1","recurring":false},{"id":"c1","recurring":true}]}"#;
    let run = |home: &Path| -> Res<std::process::Output> {
        let mut child = Command::new("sh")
            .args(["-c", command])
            .env("HOME", home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        std::io::Write::write_all(
            &mut child.stdin.take().ok_or("no stdin")?,
            payload.as_bytes(),
        )?;
        Ok(child.wait_with_output()?)
    };
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join("claude_hook_home");
    if home.exists() {
        fs::remove_dir_all(&home)?;
    }
    fs::create_dir_all(home.join(".candler"))?;
    let output = run(&home)?;
    assert!(output.status.success());
    assert_eq!((output.stdout, output.stderr), (Vec::new(), Vec::new()));
    let log = fs::read_to_string(home.join(hooks.log.as_str()))?;
    let line: Value = serde_json::from_str(log.trim_end())?;
    assert_eq!(line["payload"], serde_json::from_str::<Value>(payload)?);
    assert!(line["written_ns"].as_u64().is_some_and(|ns| ns > 0));
    assert_eq!(log.lines().count(), 1);

    // The log's directory does not exist: the command still exits 0 and prints nothing.
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("claude_hook_no_home");
    let output = run(&missing)?;
    assert!(output.status.success());
    assert_eq!(output.stdout, b"");

    let mut tracker = driver.session_tracker().ok_or("no tracker")?;
    tracker.feed(SessionRecord::Hook(log.trim_end().as_bytes()))?;
    assert_eq!(
        tracker.state().pending,
        vec![
            PendingItem::new(PendingKind::Task, "b1".to_owned()),
            PendingItem::new(PendingKind::WakeUp, "w1".to_owned()),
            PendingItem::new(PendingKind::Cron, "c1".to_owned()),
        ]
    );
    Ok(())
}

/// Guards: R01 §Harness drivers — the Claude Code skill layout, through the trait: the archive
/// command is `git archive` of `.claude/skills/<name>` at the pinned commit, and the files land at
/// `.claude/skills/<name>/`. Oracle: `git` itself and the hand-written layout path.
#[test]
fn conformance_claude_skills_layout() -> Res<()> {
    use std::process::Command;
    let repo = Path::new(env!("CARGO_TARGET_TMPDIR")).join("claude_skills_repo");
    if repo.exists() {
        fs::remove_dir_all(&repo)?;
    }
    fs::create_dir_all(repo.join(".claude/skills/demo"))?;
    fs::write(
        repo.join(".claude/skills/demo/SKILL.md"),
        "---\nname: demo\n---\n",
    )?;
    let git = |args: &[&str]| -> Res<Vec<u8>> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .output()?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into_owned().into());
        }
        Ok(out.stdout)
    };
    git(&["init", "-q"])?;
    git(&["add", "."])?;
    git(&["commit", "-q", "-m", "skill"])?;
    let commit = String::from_utf8(git(&["rev-parse", "HEAD"])?)?;
    let mut treatment = TreatmentRef::new("demo".parse()?);
    treatment.commit = Some(GitCommit::from_str(commit.trim())?);
    let skills: &dyn SkillTree = &ClaudeSkills; // TRAIT_OBJECT: the core holds skill trees as objects
    let command = skills.archive_command(&treatment, &repo)?;
    let archive = Command::new(command.argv().first().ok_or("no program")?)
        .args(command.argv().iter().skip(1))
        .output()?;
    assert!(archive.status.success());
    let files = skills.files(&treatment, &archive.stdout)?;
    let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(paths, [".claude/skills/demo/SKILL.md"]);
    Ok(())
}

/// Guards: R01 §Session runner, D2 fallback, §Agent environment and secrets — the tracker and
/// pending-work table: (1) a turn that ends with no `Stop` line (an `error_max_turns` result) is a
/// turn end whose pending list is the latest `background_tasks_changed`, and the next turn starts
/// at the notification; (2) no Stop line ever seen keeps a readable empty task list as "not
/// nothing pending", with or without a task list at all; (3) a Stop whose cron list cannot be read
/// is unknown until a readable task list follows; (4) a readable Stop and task list make the state
/// known, and a line about the task list that does not parse (cut JSON, non-JSON, a renamed key, a
/// non-list) makes it unknown while keeping the last known list, restored by the next readable
/// list; (5) entries and keys that cannot be read (`run_id` instead of `task_id`, an empty id, `id`
/// instead of `task_id`, a bare string) leave the state unknown with the list unchanged; (6) a Stop
/// payload with an id-less item or a missing list is refused and leaves the tracker unknown.
/// Oracle: the hand-written lines, shaped as `session_facts` F1 and the recorded c1 lines describe
/// them.
#[test]
fn conformance_claude_tracker_and_pending_work() -> Res<()> {
    type Tracker = Box<dyn candler_harness::spec::SessionTracker>; // TRAIT_OBJECT: the core holds trackers as objects

    // (1) A turn that ends with no Stop line.
    let driver = ClaudeCodeDriver::new()?;
    let mut tracker = driver.session_tracker().ok_or("no tracker")?;
    let lines = [
        r#"{"type":"system","subtype":"init","session_id":"s"}"#,
        r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"b9"}],"session_id":"s"}"#,
        r#"{"type":"result","subtype":"error_max_turns","is_error":true,"session_id":"s"}"#,
    ];
    for line in lines {
        tracker.feed(SessionRecord::Stream(line.as_bytes()))?;
    }
    assert_eq!(
        tracker.state(),
        PendingState::new(1, true, false, tasks(&["b9"]))
    );
    tracker.feed(SessionRecord::Stream(
        br#"{"type":"system","subtype":"task_notification","task_id":"b9","session_id":"s"}"#,
    ))?;
    assert_eq!(
        tracker.state(),
        PendingState::new(1, false, false, tasks(&["b9"]))
    );

    let fresh = || driver.session_tracker().ok_or("no tracker");
    let stream =
        |tracker: &mut Tracker, line: &str| tracker.feed(SessionRecord::Stream(line.as_bytes()));
    let hook = |tracker: &mut Tracker, payload: &str| {
        tracker.feed(SessionRecord::Hook(
            format!(r#"{{"written_ns":1,"payload":{payload}}}"#).as_bytes(),
        ))
    };
    let tasks_line = |tasks: &str| {
        format!(r#"{{"type":"system","subtype":"background_tasks_changed","tasks":{tasks}}}"#)
    };
    let stop = |tasks: &str, crons: &str| {
        format!(
            r#"{{"hook_event_name":"Stop","background_tasks":{tasks},"session_crons":{crons}}}"#
        )
    };

    // (2) No Stop line was ever seen: a readable empty task list is not "nothing pending".
    let mut tracker = fresh()?;
    stream(&mut tracker, r#"{"type":"assistant"}"#)?;
    stream(&mut tracker, &tasks_line("[]"))?;
    stream(
        &mut tracker,
        r#"{"type":"result","subtype":"error_max_turns"}"#,
    )?;
    assert_eq!(tracker.state(), PendingState::new(1, true, false, vec![]));
    // The same with no task list at all.
    let mut tracker = fresh()?;
    stream(
        &mut tracker,
        r#"{"type":"result","subtype":"error_max_turns"}"#,
    )?;
    assert_eq!(tracker.state(), PendingState::new(1, true, false, vec![]));

    // (3) A Stop whose cron list cannot be read, then a readable task list.
    let mut tracker = fresh()?;
    assert!(hook(&mut tracker, &stop("[]", r#"[{"x":1}]"#)).is_err());
    assert!(!tracker.state().known);
    stream(&mut tracker, &tasks_line("[]"))?;
    assert_eq!(tracker.state(), PendingState::new(0, false, false, vec![]));

    // (4) A readable Stop and task list make the state known; a line about the task list that
    // does not parse then makes it unknown and keeps the list.
    let mut tracker = fresh()?;
    hook(&mut tracker, &stop(r#"[{"id":"b9"}]"#, "[]"))?;
    assert_eq!(
        tracker.state(),
        PendingState::new(0, false, true, tasks(&["b9"]))
    );
    for cut in [
        r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"b9"#,
        "not json at all, background_tasks_changed",
    ] {
        stream(&mut tracker, cut)?;
        assert_eq!(
            tracker.state(),
            PendingState::new(0, false, false, tasks(&["b9"])),
            "{cut}"
        );
        stream(&mut tracker, &tasks_line(r#"[{"task_id":"b9"}]"#))?;
        assert!(tracker.state().known, "a readable list restores it");
    }
    // A line that does not parse and is not about the task list changes nothing.
    stream(&mut tracker, "not json")?;
    assert!(tracker.state().known);

    // (5) Entries and keys that cannot be read.
    for bad in [
        r#"[{"task_id":"b8"},{"run_id":"r"}]"#,
        r#"[{"task_id":""}]"#,
        r#"[{"id":"b8"}]"#,
        r#""b8""#,
    ] {
        stream(&mut tracker, &tasks_line(bad))?;
        assert_eq!(
            tracker.state(),
            PendingState::new(0, false, false, tasks(&["b9"])),
            "{bad}"
        );
        stream(&mut tracker, &tasks_line(r#"[{"task_id":"b9"}]"#))?;
        assert!(tracker.state().known);
    }
    stream(
        &mut tracker,
        r#"{"type":"system","subtype":"background_tasks_changed","jobs":[{"task_id":"b8"}]}"#,
    )?;
    assert!(!tracker.state().known);

    // (6) A Stop payload with an id-less item or a missing list is refused and unknown.
    for bad in [
        stop(r#"[{"type":"shell"}]"#, "[]"),
        stop("[]", r#"[{"id":""}]"#),
        r#"{"hook_event_name":"Stop","background_tasks":[]}"#.to_owned(),
    ] {
        let mut tracker = fresh()?;
        hook(&mut tracker, &stop(r#"[{"id":"b9"}]"#, "[]"))?;
        assert!(hook(&mut tracker, &bad).is_err(), "{bad}");
        assert!(!tracker.state().known, "{bad}");
    }
    Ok(())
}

/// Guards: R01 §Agent environment and secrets — Claude Code's gateway names are the three the core
/// owns today, and the driver keeps no state path, so the container argv stays as it is. Oracle:
/// `RESERVED_NAMES`, `BASE_URL_NAME` and `AUTH_TOKEN_NAME` of the core's `run.rs` at a26fe53,
/// copied by hand.
#[test]
fn conformance_claude_gateway_names_are_the_cores_names() -> Res<()> {
    let driver = ClaudeCodeDriver::new()?;
    assert_eq!(
        driver.gateway(),
        Some(GatewayEnv::new(
            "ANTHROPIC_BASE_URL".to_owned(),
            "ANTHROPIC_AUTH_TOKEN".to_owned(),
            vec![
                "ANTHROPIC_BASE_URL".to_owned(),
                "ANTHROPIC_AUTH_TOKEN".to_owned(),
                "ANTHROPIC_API_KEY".to_owned(),
            ],
        ))
    );
    assert_eq!(driver.state_paths(), Vec::new());
    Ok(())
}

/// Guards: R01 §Session runner — the session command keeps every element of the headless command
/// except the prompt, and adds only the input format and the settings file. Oracle: the headless
/// `command()` of the same cell, whose line `guard_claude_code_driver` pins to a26fe53's.
#[test]
fn conformance_claude_session_command_keeps_the_headless_arguments() -> Res<()> {
    let driver = ClaudeCodeDriver::new()?;
    let mut values = values()?;
    for (path, value) in [
        ("model.effort", json!("high")),
        ("budget.per_run.turns", json!(7)),
        ("budget.per_run.max_cost_usd", json!(0.5)),
    ] {
        values.push(ResolvedValue::recorded(
            FieldPath::from_str(path)?,
            JsonValue::new(value)?,
        ));
    }
    let cell = Cell::new(&values);
    let workspace = Workspace::new(Path::new("."));
    let headless = driver.command(&cell, &workspace)?;
    let session = driver.session(&cell, &workspace)?.ok_or("no session")?;
    let mut headless_args: Vec<&str> = headless.argv().iter().map(String::as_str).collect();
    let print = headless_args
        .iter()
        .position(|arg| *arg == "--print")
        .ok_or("no --print")?;
    // The prompt is the argument after `--print`.
    headless_args.remove(print.saturating_add(1));
    let mut extras: Vec<&str> = session.command.argv().iter().map(String::as_str).collect();
    for arg in &headless_args {
        let at = extras
            .iter()
            .position(|extra| extra == arg)
            .ok_or_else(|| format!("the session command lost `{arg}`"))?;
        extras.remove(at);
    }
    assert_eq!(
        extras,
        [
            "--input-format",
            "stream-json",
            "--settings",
            "/home/agent/.candler/settings.json"
        ]
    );
    Ok(())
}
