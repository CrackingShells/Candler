//! A real run on each harness, graded by a plug-in from what its trace says.
//!
//! The rig is the parity guard's: real Claude Code and real OpenCode, each in its agent base
//! image, behind the production gateway sidecar, against the scripted fake model (model `g1`).
//! The script is two shell commands, `printf ... > answer.txt` (exit 0) and `exit 3`, then the
//! call of the tool `stamp` of the server `parity`. `candler evaluate` grades each sealed run
//! with a plug-in kind that declares `needs.trace` and `needs.artifacts = ["answer.txt"]`.
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use candler_core::store::layout::StoreLayout;
use serde_json::{Value, json};

use super::i8_e2e::{Made, must};
use super::parity_e2e::{SERVER, Sealed, base_experiment, records, seal, start_fake};
use super::real_docker;
use crate::support::cli_run::World;
use crate::support::seam::{entries_under, sync_environment};

/// The command that writes the answer file and exits 0.
const WRITE: &str = "printf 'graded-answer-3f9a\\n' > answer.txt";

/// What [`WRITE`] leaves in the file.
const ANSWER: &str = "graded-answer-3f9a\n";

/// The command that exits 3.
const EXIT: &str = "exit 3";

/// The plug-in's directory below the fixtures.
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugins/graded-trace")
}

/// The suite of one gate whose params are the script's: each command with the exit status it
/// must show in the trace, the origin of the stamp call and the answer file.
fn suite(shell: &[(&str, i32)]) -> String {
    let mut parts = vec![format!(
        "schema = \"suite/1\"\n\n[[expectations]]\nid = \"G1\"\nrole = \"gate\"\nkind = \"graded_run\"\n\
         depends_on = []\n\n[expectations.evaluator]\nname = \"graded-trace\"\nrange = \"^0.1\"\n\n\
         [expectations.params]\norigin = {{ kind = \"mcp\", server = \"{SERVER}\", tool = \"stamp\" }}\n"
    )];
    parts.extend(shell.iter().map(|(command, code)| {
        format!(
            "[[expectations.params.shell]]\nargv = {}\nexit = {code}\n",
            json!([command])
        )
    }));
    parts.push(format!(
        "[expectations.params.artifact]\npath = \"answer.txt\"\ntext = {}\n",
        json!(ANSWER)
    ));
    parts.join("\n")
}

/// Sets the plug-in up in the world, with the helper's project linked beside it, and returns its
/// directory and the suite.
fn plugin(world: &World) -> (PathBuf, PathBuf) {
    let dir = world.root.join("graded-trace");
    fs::create_dir_all(&dir).unwrap();
    let helper = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../python/candler-evaluator");
    symlink(helper, dir.join("env")).unwrap();
    let manifest = fs::read_to_string(fixture().join("plugin.toml"))
        .unwrap()
        .replace("@KINDS@", &fixture().join("kinds").display().to_string());
    fs::write(dir.join("plugin.toml"), manifest).unwrap();
    let suite_file = dir.join("suite.toml");
    fs::write(&suite_file, suite(&[(WRITE, 0), (EXIT, 3)])).unwrap();
    (dir, suite_file)
}

/// The commands of the shell calls the fake model made in the requests after the first `skip`
/// records, in order, as the longest conversation among them holds them.
fn scripted_commands(records: &str, skip: usize) -> Vec<String> {
    let mut longest: Vec<String> = Vec::new();
    let mut size = 0;
    for line in records.lines().skip(skip) {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let messages = record["body"]["messages"].as_array();
        let held = messages.map_or(0, Vec::len);
        let commands: Vec<String> = messages
            .into_iter()
            .flatten()
            .filter(|message| message["role"] == "assistant")
            .flat_map(|message| message["content"].as_array().into_iter().flatten())
            .filter(|block| block["type"] == "tool_use" && block["input"]["command"].is_string())
            .filter_map(|block| block["input"]["command"].as_str().map(str::to_owned))
            .collect();
        if held >= size {
            size = held;
            longest = commands;
        }
    }
    longest
}

/// Grades the sealed run of `world` with the plug-in and returns what went wrong, one line each:
/// an evaluation that fails, or an outcome that is not one pass.
fn graded(world: &World) -> Vec<String> {
    let (dir, suite_file) = plugin(world);
    sync_environment(world);
    let project = world.root.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("candler.toml"),
        format!("[plugins]\ndirs = [\"{}\"]\n", dir.display()),
    )
    .unwrap();
    let evaluated = Command::new(env!("CARGO_BIN_EXE_candler"))
        .arg("--store")
        .arg(world.store())
        .arg("evaluate")
        .arg(&suite_file)
        .current_dir(&project)
        .env_remove("CANDLER_STORE")
        .env("HOME", world.root.join("home"))
        .env("XDG_CONFIG_HOME", world.root.join("config"))
        .env("XDG_STATE_HOME", &world.state)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&evaluated.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&evaluated.stderr).into_owned();
    if !evaluated.status.success() {
        return vec![format!(
            "evaluate exits {:?}\nstdout: {stdout}\nstderr: {stderr}",
            evaluated.status
        )];
    }
    let mut problems = Vec::new();
    match serde_json::from_str::<Value>(&stdout) {
        Ok(report) if report["failed_requests"] == json!([]) => {}
        Ok(report) => problems.push(format!("the evaluation refused a request: {report}")),
        Err(error) => problems.push(format!("the report is not JSON ({error}): {stdout}")),
    }
    let layout = StoreLayout::new(&world.store());
    let outcomes: Vec<Value> = entries_under(&layout.outcomes_dir())
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .flat_map(|path| {
            fs::read_to_string(path)
                .unwrap_or_default()
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .collect::<Vec<_>>()
        })
        .collect();
    if outcomes.len() != 1 || outcomes[0]["status"] != "pass" {
        problems.push(format!("the kind did not pass once: {outcomes:?}"));
    }
    problems
}

/// Guards: README (a run's trace is "graded" and each tool call is censused: a plug-in that
/// needs the trace sees each shell call's command and exit status and each MCP call's server and
/// tool, on every harness) -- one experiment, run with the real `candler run` on Claude Code and
/// on `OpenCode` and graded with `candler evaluate`, passes a kind that declares `needs.trace`
/// and `needs.artifacts = ["answer.txt"]` only when its trace holds, for each of the two shell
/// commands, the pair of the command's argv and its exit status, the origin of the `stamp` call,
/// and the served `answer.txt` holds the text the first command wrote.
///
/// Oracle: the fake model's own script: the commands it sent (read back from the requests it
/// recorded), the exit statuses a shell gives them (0 and 3), and the server and tool it calls.
///
/// Bite: `assemble` serving no trace (the kind's outcome is a fail or the evaluation a refusal),
/// or the `OpenCode` reader writing no `exit` (the `OpenCode` row fails, the Claude Code row does
/// not).
#[test]
#[ignore = "needs Docker; run with --run-ignored only"]
fn graded_run_sees_trace() {
    let real = real_docker();
    let mut made = Made::default();
    let script = json!([WRITE, EXIT]).to_string();
    let fake = start_fake(
        &real,
        &mut made,
        "graded-trace",
        &[format!("FAKE_G1_COMMANDS={script}")],
    );
    let base = base_experiment(
        "g1",
        "Run the commands you are given, call the MCP tool stamp of the server parity, then reply done.",
    );
    let mut failures = Vec::new();
    let mut jobs = Vec::new();
    for harness in ["claude-code", "opencode"] {
        let skip = records(&real, &fake.id).lines().count();
        let Sealed {
            job,
            world,
            run_dir,
        } = seal(
            &base.replace("@HARNESS@", harness),
            &fake.endpoint,
            &fake.ca,
            &mut made,
        );
        jobs.push(job);
        let sent = scripted_commands(&records(&real, &fake.id), skip);
        if sent != [WRITE, EXIT] {
            let seen: Vec<String> = records(&real, &fake.id)
                .lines()
                .skip(skip)
                .map(|line| {
                    let record: Value = serde_json::from_str(line).unwrap_or_default();
                    format!("{}/{}", record["decision"], record["body"]["tools"])
                })
                .collect();
            failures.push(format!(
                "{harness}: the model's commands were {sent:?}; requests: {seen:?}"
            ));
        }
        if !run_dir.join("trace.jsonl").is_file() {
            failures.push(format!("{harness}: the run stored no trace"));
        }
        failures.extend(
            graded(&world)
                .into_iter()
                .map(|problem| format!("{harness}: {problem}")),
        );
    }
    drop(made);
    // the containers this guard started, by their labels: the machine's other containers are not
    // this guard's to count
    let mut labels = vec!["candler.test=graded-trace".to_owned()];
    labels.extend(jobs.iter().map(|job| format!("candler.job={job}")));
    for label in labels {
        let left = must(&real, &["ps", "-aq", "--filter", &format!("label={label}")]);
        if !left.is_empty() {
            failures.push(format!("containers of {label} are left behind: {left}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
