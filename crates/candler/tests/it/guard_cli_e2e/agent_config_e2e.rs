//! Instructions and hooks on both real harnesses, read from what a consumer reads.
//!
//! The runs are the product's own: real Claude Code and real OpenCode, each in its agent base
//! image, behind the production gateway sidecar, against the scripted fake model
//! (`assets/docker/fakemodel`, model `h1`, which calls the shell tool once). One experiment file
//! is written once, with a global text per harness, two arms (workspace text A or B), one
//! session-start hook and one blocking hook per harness; the line `[harness]` names is the only
//! thing that differs between the two jobs. What is read is `candler inspect`'s record, the stored
//! trace's `context_start`, and the files the run sealed, never the fake model's own notes, except
//! the answer it was given to its one tool call.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde_json::Value;

use super::i8_e2e::{Made, must};
use super::parity_e2e::{Fake, Ran, TOKEN, answers_to, records, run_job, start_fake};
use super::real_docker;
use crate::support::cli_run::world;

/// The call the fake model's script `h1` makes, and the text the shell prints when it runs.
const CALL_ID: &str = "toolu_fake_hook";
const NOT_BLOCKED: &str = "hook-not-blocked";

/// The text each harness's global instruction file holds, and the two workspace texts of the arms.
const GLOBAL_CC: &str = "GLOBAL_TEXT_CC_71a3";
const GLOBAL_OC: &str = "GLOBAL_TEXT_OC_71a3";
const ARMS: [&str; 2] = ["WORKSPACE_TEXT_A_5be2", "WORKSPACE_TEXT_B_5be2"];

/// What the session-start hook of each harness adds, and what the blocking hook answers with.
const START_CC: &str = "SESSION_START_MARKER_CC_c40d";
const START_OC: &str = "SESSION_START_MARKER_OC_c40d";
const BLOCKED: &str = "BLOCKED_BY_HOOK_9d4f";

/// The hooks and global texts of the experiment, for both harnesses. The Claude Code blocking
/// script exits 2 with the marker on stderr; the `OpenCode` plugin throws the marker in
/// `tool.execute.before` and adds its start marker to the system text in `session` `context`.
fn agent_config() -> String {
    format!(
        r#"
[fixed.instructions.global]
claude-code = "{GLOBAL_CC}"
opencode = "{GLOBAL_OC}"

[fixed.hooks.claude-code.settings]
SessionStart = [{{ hooks = [{{ type = "command", command = "echo {START_CC}" }}] }}]
PreToolUse = [{{ matcher = "Bash", hooks = [{{ type = "command", command = "sh .claude/hooks/block.sh" }}] }}]

[fixed.hooks.claude-code.files]
"block.sh" = "echo {BLOCKED} >&2\nexit 2\n"

[fixed.hooks.opencode.plugins]
"guard.ts" = '''
export default {{
  id: "guard",
  setup(api) {{
    api.session.hook("context", (event) => {{
      event.system.push({{ type: "text", text: "{START_OC}" }});
    }});
    api.tool.hook("execute.before", (event) => {{
      if (/shell|bash/i.test(String(event.tool))) throw new Error("{BLOCKED}");
    }});
  }},
}};
'''

[[factors]]
path = "instructions.workspace"
levels = ["{}", "{}"]
"#,
        ARMS[0], ARMS[1]
    )
}

/// The experiment of one cell shape: the world's own file made to run the shell script `h1` with
/// nothing else asked of it, the harness a placeholder, and `extra` appended.
fn base(extra: &str) -> String {
    let shape = world();
    let text = fs::read_to_string(&shape.experiment)
        .unwrap()
        .replace(&shape.skills.display().to_string(), "@SKILLS@");
    text.replace("\"plan\"", "\"bypassPermissions\"")
        .replace("\"fixture-model\"", "\"h1\"")
        .replace("say hello", "Run one shell command, then reply done.")
        .replace("wall_s = 30", "wall_s = 300")
        .replacen(
            "[harness]\nname = \"claude-code\"",
            "[harness]\nname = \"@HARNESS@\"",
            1,
        )
        + extra
}

/// What one sealed run stored.
struct Seen {
    /// `candler inspect`'s record of the run.
    record: Value,
    /// The `system` and `first_message` of the trace's first `context_start`.
    system: String,
    first_message: String,
    /// The files the run collected as outputs, as paths relative to the run's `outputs`.
    outputs: Vec<String>,
    /// The run's seal, as written.
    seal: String,
}

/// The first event of `kind` among the lines of the trace `text`.
fn first_event(text: &str, kind: &str) -> Option<Value> {
    fn walk(value: &Value, kind: &str) -> Option<Value> {
        match value {
            Value::Object(map) => {
                if map.get("kind") == Some(&Value::from(kind)) {
                    return Some(value.clone()); // BORROW: .clone() copies the event
                }
                map.values().find_map(|inner| walk(inner, kind))
            }
            Value::Array(items) => items.iter().find_map(|inner| walk(inner, kind)),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
        }
    }
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|value| walk(&value, kind))
}

/// A JSON member as the text a consumer reads: a string as it is, anything else as its JSON.
fn shown(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned) // BORROW: .to_owned() makes the owned text
}

/// The files below `dir`, relative to it.
fn files_below(dir: &Path) -> Vec<String> {
    crate::support::seam::entries_under(dir)
        .into_iter()
        .filter(|path| path.is_file())
        .filter_map(|path| Some(path.strip_prefix(dir).ok()?.display().to_string()))
        .collect()
}

/// Every sealed run of the job in `ran`, read as a consumer reads it.
fn sealed_runs(ran: &Ran) -> Vec<Seen> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(ran.store.join("runs"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("run.done").is_file())
        .collect();
    dirs.sort();
    dirs.iter()
        .map(|run_dir| {
            let inspected = Command::new(env!("CARGO_BIN_EXE_candler"))
                .arg("inspect")
                .arg(run_dir)
                .current_dir(&ran.world.work)
                .env_remove("CANDLER_STORE")
                .env("XDG_STATE_HOME", &ran.world.state)
                .output()
                .unwrap();
            assert!(inspected.status.success(), "inspect exits 0: {inspected:?}");
            let trace = fs::read_to_string(run_dir.join("trace.jsonl")).unwrap_or_default();
            let start = first_event(&trace, "context_start")
                .unwrap_or_else(|| panic!("a context_start in the trace of {run_dir:?}:\n{trace}"));
            Seen {
                record: serde_json::from_slice(&inspected.stdout).unwrap(),
                system: shown(&start["system"]),
                first_message: shown(&start["first_message"]),
                outputs: files_below(&run_dir.join("outputs")),
                seal: fs::read_to_string(run_dir.join("run.done")).unwrap_or_default(),
            }
        })
        .collect()
}

/// The workspace text a run's record shows it was given.
fn arm_of(seen: &Seen) -> String {
    shown(&seen.record["instructions"]["instructions.workspace"])
}

/// `candler resolve` of the experiment at `world`, which must succeed. Its preflight reads the
/// model list of the endpoint from the host, which the fake model's TLS name and certificate do
/// not allow, so the endpoint is a plain local listener that answers the one read.
fn resolve(world: &crate::support::cli_run::World, config: &Path) -> String {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let endpoint = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let answering = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0_u8; 1024];
            drop(std::io::Read::read(&mut stream, &mut request));
            drop(std::io::Write::write_all(
                &mut stream,
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
            ));
        }
    });
    let resolved = Command::new(env!("CARGO_BIN_EXE_candler"))
        .arg("resolve")
        .arg(&world.experiment)
        .args([
            "--set",
            "isolation.build.lock=env.lock",
            "--set",
            "model.token_secret=FIXTURE_TOKEN",
            "--set",
        ])
        .arg(format!("model.endpoint={endpoint}"))
        .env("FIXTURE_TOKEN", TOKEN)
        .current_dir(&world.work)
        .env_remove("CANDLER_STORE")
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CONFIG_HOME", config)
        .output()
        .unwrap();
    drop(answering.join());
    assert!(resolved.status.success(), "resolve exits 0: {resolved:?}");
    String::from_utf8_lossy(&resolved.stdout).into_owned()
}

/// The instruction files of the census a run recorded, as `(path, scope)`.
fn census(seen: &Seen) -> Vec<(String, String)> {
    seen.record["measured"]["census.instructions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| (shown(&entry["path"]), shown(&entry["scope"])))
        .collect()
}

/// Asserts that nothing the jobs `jobs` and the fake model `fake` started is left in Docker: no
/// container of a job, no network of a job and not the fake model. Containers of a guard that runs
/// beside this one are not looked at.
fn assert_nothing_left(real: &Path, jobs: &[String], fake: &str) {
    for job in jobs {
        let filter = format!("label=candler.job={job}");
        assert_eq!(
            must(real, &["ps", "-aq", "--filter", &filter]),
            "",
            "no container of job {job} is left behind"
        );
        assert_eq!(
            must(real, &["network", "ls", "-q", "--filter", &filter]),
            "",
            "no network of job {job} is left behind"
        );
    }
    assert_eq!(
        must(real, &["ps", "-aq", "--filter", &format!("id={fake}")]),
        "",
        "the fake model is removed"
    );
}

/// Guards: README (instructions and hooks a cell sets reach the agent on every harness, and what
/// reached it is in the trace) -- one experiment, run with the real `candler run` on Claude Code
/// and on `OpenCode`, two arms each, shows on every run: `resolve` succeeds and the record shows
/// the arm's instructions and the hooks (G1); the `context_start` holds the global text and the
/// arm's workspace text and not the other arm's (G4); the blocking hook's marker answers the
/// shell call as an error (G5); the census lists the global and the workspace file with their
/// scopes (G7); no installed file is among the collected outputs and no run is sealed
/// `input_leak` (G9); the session-start marker is in the `system` or the `first_message` of the
/// `context_start` (G11; measured in `system` on both harnesses under Candler: Claude Code 2.1.311
/// writes a `SessionStart:startup hook success:` line there, while the probe's bare `--print` run
/// had it in the first message).
///
/// Oracle: the texts the experiment fixes, and where the probe measured each to land
/// (`__reports__/agent_config/03-findings_probe_harness_checks_v0.md`).
///
/// Bite: a driver that installs nothing (no text in `context_start`), the workspace file of one
/// arm shown to the other, a hook that is not installed (the shell call succeeds), an installed
/// file collected as an output, or the first request that offers no tools recorded as the start.
#[test]
#[ignore = "needs Docker; run with --run-ignored only"]
fn guard_agent_config_e2e() {
    let started = Instant::now();
    let real = real_docker();
    let mut made = Made::default();
    made.real.clone_from(&real); // BORROW: the cleanup owns its client path
    let model: Fake = start_fake(&real, &mut made, "agentconfig");

    let text = base(&agent_config());
    let mut failures = Vec::new();
    let mut jobs = Vec::new();
    // each harness: its global text, where its start marker lands, and its two instruction files
    for (harness, global, marker, user_file, project_file) in [
        (
            "claude-code",
            GLOBAL_CC,
            START_CC,
            ".claude/CLAUDE.md",
            "CLAUDE.md",
        ),
        (
            "opencode",
            GLOBAL_OC,
            START_OC,
            "opencode/AGENTS.md",
            "AGENTS.md",
        ),
    ] {
        let experiment = text.replace("@HARNESS@", harness);
        let skip = records(&real, &model.container).lines().count();
        let ran = run_job(&experiment, &model.endpoint, &model.ca, &mut made, |_| {});
        jobs.extend(made.job.clone()); // BORROW: the leftover check names every job
        let config = ran.world.root.join("config");
        let resolved = resolve(&ran.world, &config);
        if ![global, START_CC, START_OC, BLOCKED]
            .iter()
            .all(|shown| resolved.contains(shown))
        {
            failures.push(format!(
                "{harness}: resolve shows no instructions or hooks: {resolved}"
            ));
        }
        let seen = sealed_runs(&ran);
        if seen.len() != 2 {
            failures.push(format!(
                "{harness}: {} sealed runs, expected 2\n{}",
                seen.len(),
                ran.log
            ));
        }
        for run in &seen {
            let arm = arm_of(run);
            let other = ARMS.iter().find(|candidate| **candidate != arm);
            if !ARMS.contains(&arm.as_str()) {
                failures.push(format!("{harness}: no arm in the record: {}", run.record));
                continue;
            }
            // G1: the record shows the instructions and the hooks of the arm
            let instructions = run.record["instructions"].to_string();
            let hooks = run.record["hooks"].to_string();
            if !instructions.contains(global) {
                failures.push(format!(
                    "{harness}/{arm}: record instructions {instructions}"
                ));
            }
            if !hooks.contains(BLOCKED) {
                failures.push(format!("{harness}/{arm}: record hooks {hooks}"));
            }
            // G4: the trace holds the global text and the arm's text, and not the other arm's
            let reached = format!("{}\n{}", run.system, run.first_message);
            for want in [global, arm.as_str()] {
                if !reached.contains(want) {
                    failures.push(format!("{harness}/{arm}: context_start lacks {want}"));
                }
            }
            if other.is_some_and(|other| reached.contains(other)) {
                failures.push(format!(
                    "{harness}/{arm}: context_start holds the other arm"
                ));
            }
            // G11: the start marker is in the `system` or the `first_message` (measured in `system`
            // on both harnesses under Candler; Claude Code 2.1.311 writes a
            // `SessionStart:startup hook success:` line there, the probe's bare `--print` run had
            // it in the first message).
            if !reached.contains(marker) {
                failures.push(format!(
                    "{harness}/{arm}: start marker not in context_start"
                ));
            }
            // G7: the census lists both files with their scope
            // (Claude Code's census is read from its hook log, in the order the files arrived.)
            let mut listed = census(run);
            listed.sort_by(|a, b| a.1.cmp(&b.1));
            let fits = matches!(
                listed.as_slice(),
                [(project, scope_project), (user, scope_user)]
                    if user.starts_with('/') && user.ends_with(user_file) && scope_user == "user"
                        && project == project_file && scope_project == "project"
            );
            if !fits {
                failures.push(format!("{harness}/{arm}: census {listed:?}"));
            }
            // G9: no installed file among the outputs, no run sealed `input_leak`
            let installed = [
                "CLAUDE.md",
                "AGENTS.md",
                "settings.json",
                "block.sh",
                "guard.ts",
            ];
            if let Some(leak) = run.outputs.iter().find(|path| {
                installed.iter().any(|name| {
                    Path::new(path)
                        .file_name()
                        .is_some_and(|found| found.to_string_lossy() == *name)
                })
            }) {
                failures.push(format!("{harness}/{arm}: collected output {leak}"));
            }
            if run.seal.contains("input_leak") || run.record.to_string().contains("input_leak") {
                failures.push(format!("{harness}/{arm}: sealed input_leak: {}", run.seal));
            }
        }
        // G5: the shell call is answered with an error carrying the marker, never its output
        let answers = answers_to(&records(&real, &model.container), skip, CALL_ID);
        let blocked = |(error, text): &(bool, String)| *error && text.contains(BLOCKED);
        if answers.len() != 2
            || !answers.iter().all(blocked)
            || answers.iter().any(|(_, text)| text.contains(NOT_BLOCKED))
        {
            failures.push(format!("{harness}: the shell call's answers: {answers:?}"));
        }
    }
    let fake = model.container.clone(); // BORROW: the check names the fake after the cleanup
    drop(made);
    assert_nothing_left(&real, &jobs, &fake);
    eprintln!("agent_config: {:?} wall", started.elapsed());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// One harness's payload for `[fixed.hooks]`: an invalid one, the text the harness's own output
/// quotes for it, and a valid one.
struct Payload {
    harness: &'static str,
    invalid: &'static str,
    quoted: &'static str,
    valid: &'static str,
}

const PAYLOADS: [Payload; 2] = [
    Payload {
        harness: "claude-code",
        invalid: "\n[fixed.hooks.claude-code.settings]\nNoSuchEvent = [{ hooks = [{ type = \"command\", command = \"true\" }] }]\n",
        quoted: "Invalid settings",
        valid: "\n[fixed.hooks.claude-code.settings]\nSessionStart = [{ hooks = [{ type = \"command\", command = \"true\" }] }]\n",
    },
    Payload {
        harness: "opencode",
        invalid: "\n[fixed.hooks.opencode.plugins]\n\"bad.ts\" = \"export const nothing = 1\\n\"\n",
        quoted: "Plugin must export a default definition",
        valid: "\n[fixed.hooks.opencode.plugins]\n\"good.ts\" = \"export default { id: \\\"good\\\", setup() {} }\\n\"\n",
    },
];

/// Guards: README (the harness's own check refuses what it cannot read before a run starts) --
/// a job of the real `candler run` on real images, with an invalid payload (a Claude Code hooks
/// entry on an unknown event; an `OpenCode` plugin with no default export) is refused before its
/// first run, the refusal quotes the harness's own output and no run is sealed; the same job with
/// a valid payload is not refused and seals its run (G6).
///
/// Oracle: the harness outputs recorded for both cases (`fixtures/doctor` and `fixtures/check`
/// of the two harness crates); the refusal line is the one the supervisor logs for
/// `experiment.harness_rejected_input` (`Candler installed .. at ..; <harness> rejects it: ..`).
///
/// Bite: a job that starts its first run whatever the check says (a sealed run, no refusal line),
/// or a check that refuses a payload the harness accepts.
#[test]
#[ignore = "needs Docker; run with --run-ignored only"]
fn guard_harness_check_refuses() {
    let real = real_docker();
    let mut made = Made::default();
    made.real.clone_from(&real); // BORROW: the cleanup owns its client path
    let model: Fake = start_fake(&real, &mut made, "harnesscheck");

    let mut failures = Vec::new();
    let mut jobs = Vec::new();
    for payload in PAYLOADS {
        for (label, hooks) in [("invalid", payload.invalid), ("valid", payload.valid)] {
            let experiment = base(hooks).replace("@HARNESS@", payload.harness);
            let ran = run_job(&experiment, &model.endpoint, &model.ca, &mut made, |_| {});
            let job = made.job.clone().unwrap_or_default(); // BORROW: the job id names its log
            jobs.push(job.clone()); // BORROW: the leftover check names every job
            let supervisor =
                fs::read_to_string(ran.store.join("jobs").join(&job).join("supervisor.log"))
                    .unwrap_or_default();
            let sealed = fs::read_dir(ran.store.join("runs"))
                .into_iter()
                .flatten()
                .flatten()
                .filter(|entry| entry.path().join("run.done").is_file())
                .count();
            let rejects = format!("{} rejects it: ", payload.harness);
            let refused = supervisor.contains(&rejects);
            if label == "invalid" {
                if !refused || !supervisor.contains(payload.quoted) || sealed != 0 {
                    failures.push(format!(
                        "{}: invalid payload: refused {refused}, sealed {sealed}\n{supervisor}",
                        payload.harness
                    ));
                }
            } else if refused || sealed == 0 {
                failures.push(format!(
                    "{}: valid payload: refused {refused}, sealed {sealed}\n{supervisor}",
                    payload.harness
                ));
            }
        }
    }
    let fake = model.container.clone(); // BORROW: the check names the fake after the cleanup
    drop(made);
    assert_nothing_left(&real, &jobs, &fake);
    assert!(failures.is_empty(), "{failures:#?}");
}
