//! One cell, one real MCP server, two harnesses: the server is installed on each and the stored
//! trace records the agent's call to its tool in the same form.
//!
//! The runs are the product's own: real Claude Code and real OpenCode, each in its agent base
//! image, behind the production gateway sidecar, against the scripted fake model
//! (`assets/docker/fakemodel`, model `p1`, which calls the tool `stamp` of the server `parity`
//! under the name the request offers for it). The fake model is reached through the machine
//! setting `[gateway] upstream_ca`. One experiment file is written once; the line `[harness]`
//! names is the only thing that differs between the two runs.
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use candler_schemas::trace::ToolOrigin;
use serde_json::Value;

use super::i8_e2e::{Made, fake_host, must};
use super::{docker, real_docker};
use crate::support::cli_run::{wait_for, world};
use crate::support::seam::entries_under;

/// The credential the fake model demands and the experiment's secret supplies.
const TOKEN: &str = "parity-fixture-token-0123456789";

/// The server the cell declares, and the tool of it the fake model calls.
const SERVER: &str = "parity";
const TOOL: &str = "stamp";

/// The experiment of one cell, with the harness the one line `[harness]` names.
fn experiment(base: &str, harness: &str) -> String {
    base.replace(
        "[harness]\nname = \"claude-code\"",
        &format!("[harness]\nname = \"{harness}\""),
    )
}

/// What a run of one harness stored: the resolved `mcp.*` values `candler inspect` shows, the
/// MCP file its driver rendered (one parsed file per file of that name in the run), and the origin
/// of each tool call of the stored trace.
struct Observed {
    mcp: Value,
    rendered: Vec<Value>,
    origins: Vec<Value>,
}

/// The origin of every tool call among the events of the trace `text`, one JSON object per line.
fn call_origins(text: &str) -> Vec<Value> {
    fn walk(value: &Value, found: &mut Vec<Value>) {
        match value {
            Value::Object(map) => {
                if map.get("kind") == Some(&Value::from("tool_call"))
                    && let Some(origin) = map.get("origin")
                {
                    found.push(origin.clone()); // BORROW: .clone() copies the origin
                }
                map.values().for_each(|inner| walk(inner, found));
            }
            Value::Array(items) => items.iter().for_each(|inner| walk(inner, found)),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    let mut found = Vec::new();
    for line in text.lines() {
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            walk(&value, &mut found);
        }
    }
    found
}

/// Runs the experiment `text` with the real `candler run` against the fake model at `endpoint`
/// and reads back what the sealed run stored, the MCP file being the one named `file`.
fn run_once(text: &str, endpoint: &str, ca: &Path, file: &str) -> Observed {
    let world = world();
    let inputs = world.root.join("inputs").join("servers");
    fs::create_dir_all(&inputs).unwrap();
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity/parity_mcp.py"),
        inputs.join("parity_mcp.py"),
    )
    .unwrap();
    let config = world.root.join("config");
    fs::create_dir_all(config.join("candler")).unwrap();
    fs::write(
        config.join("candler").join("config.toml"),
        format!("[gateway]\nupstream_ca = \"{}\"\n", ca.display()),
    )
    .unwrap();
    fs::write(world.work.join("env.lock"), "").unwrap();
    let written = text
        .replace("http://127.0.0.1:9", endpoint)
        .replace("@SKILLS@", &world.skills.display().to_string());
    fs::write(&world.experiment, written).unwrap();

    let launched = Command::new(env!("CARGO_BIN_EXE_candler"))
        .arg("run")
        .arg(&world.experiment)
        .args([
            "--set",
            "isolation.build.lock=env.lock",
            "--set",
            "model.token_secret=FIXTURE_TOKEN",
        ])
        .current_dir(&world.work)
        .env_remove("CANDLER_STORE")
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CONFIG_HOME", &config)
        .env("FIXTURE_TOKEN", TOKEN)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&launched.stdout).into_owned();
    assert!(
        launched.status.success(),
        "the launcher exits 0: {stdout}\n{}",
        String::from_utf8_lossy(&launched.stderr)
    );
    let job = stdout.trim().to_owned();
    let store = world.store();
    let events = store.join("jobs").join(&job).join("events.jsonl");
    let ended = wait_for(Duration::from_secs(600), || {
        fs::read_to_string(&events).is_ok_and(|log| log.contains("job.ended"))
    });
    let log = fs::read_to_string(&events).unwrap_or_default();
    assert!(ended, "the job ends\n{log}");

    let run_dir = fs::read_dir(store.join("runs"))
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.join("run.done").is_file())
        .unwrap_or_else(|| panic!("a sealed run\n{log}"));
    let inspected = Command::new(env!("CARGO_BIN_EXE_candler"))
        .arg("inspect")
        .arg(&run_dir)
        .current_dir(&world.work)
        .env_remove("CANDLER_STORE")
        .env("XDG_STATE_HOME", &world.state)
        .output()
        .unwrap();
    assert!(inspected.status.success(), "inspect exits 0: {inspected:?}");
    let record: Value = serde_json::from_slice(&inspected.stdout).unwrap();
    let trace = fs::read_to_string(run_dir.join("trace.jsonl")).unwrap_or_default();
    let rendered = entries_under(&run_dir)
        .into_iter()
        .filter(|path| path.file_name().is_some_and(|name| name == file))
        .filter_map(|path| serde_json::from_slice(&fs::read(path).ok()?).ok())
        .collect();
    Observed {
        rendered,
        mcp: record["mcp"].clone(), // BORROW: .clone() copies the member
        origins: call_origins(&trace),
    }
}

/// Guards: README (an MCP server of a cell is installed and its calls are traced alike on every
/// harness) -- one experiment, run with the real `candler run` on Claude Code and on `OpenCode`,
/// code mode at its default, shows on each: the declared server among the run's resolved `mcp.*`
/// values, and the scripted call stored in the trace as an MCP call of that server and tool.
///
/// Oracle: the server name and tool the fixture declares and the fake model scripts, and the
/// origin the trace schema writes for them (`ToolOrigin::mcp_tool`); the two experiment files are
/// compared line by line.
///
/// Bite: `OpenCode`'s reader dropping the server from the origin (`other`), or a driver that skips
/// the server (no tool is offered, the fake model answers with text, and no call is stored).
#[test]
#[ignore = "needs Docker; run with --run-ignored only"]
fn parity_one_installed_server_one_traced_call() {
    let started = Instant::now();
    let real = real_docker();
    let host = fake_host(&real);
    let publish = format!(
        "{}::8443",
        if cfg!(target_os = "linux") {
            host.as_str()
        } else {
            "127.0.0.1"
        }
    );
    let before = must(&real, &["ps", "-aq"]);
    let mut made = Made::default();
    made.real.clone_from(&real); // BORROW: the cleanup owns its client path

    let context = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/fakemodel");
    let tag = format!("candler-fakemodel:parity-{}", std::process::id());
    must(
        &real,
        &[
            "build",
            "-q",
            "--label",
            "candler.test=parity",
            "-t",
            &tag,
            context,
        ],
    );
    made.images.push(must(
        &real,
        &["image", "inspect", "--format", "{{.Id}}", &tag],
    ));
    let fake = must(
        &real,
        &[
            "run",
            "-d",
            "--label",
            "candler.test=parity",
            "-p",
            &publish,
            "-e",
            &format!("FAKE_TOKEN={TOKEN}"),
            "-e",
            &format!("FAKE_HOST={host}"),
            &tag,
        ],
    );
    made.containers.push(fake.clone()); // BORROW: the cleanup owns its ids
    let port = must(&real, &["port", &fake, "8443/tcp"])
        .lines()
        .next()
        .and_then(|line| line.rsplit(':').next())
        .expect("a host port")
        .to_owned();
    assert!(
        wait_for(Duration::from_secs(30), || {
            docker(&real, &["exec", &fake, "test", "-f", "/tmp/fake/ca.pem"])
                .status
                .success()
                && docker(&real, &["logs", &fake])
                    .stdout
                    .windows(5)
                    .any(|w| w == b"ready")
        }),
        "the fake model is ready"
    );
    let scratch = tempfile::tempdir().unwrap();
    let ca = scratch.path().join("ca.pem");
    must(
        &real,
        &[
            "cp",
            &format!("{fake}:/tmp/fake/ca.pem"),
            &ca.display().to_string(),
        ],
    );
    let endpoint = format!("https://{host}:{port}");

    // one experiment file; only the harness line differs
    let base = {
        let shape = world();
        let text = fs::read_to_string(&shape.experiment)
            .unwrap()
            .replace(&shape.skills.display().to_string(), "@SKILLS@");
        text.replace("\"plan\"", "\"bypassPermissions\"")
            .replace("\"fixture-model\"", "\"p1\"")
            .replace(
                "say hello",
                "Call the MCP tool stamp of the server parity once, then reply done.",
            )
            .replace("wall_s = 30", "wall_s = 300")
            + &format!(
                "\n[fixed.mcp.servers.{SERVER}]\ncommand = \"python3\"\nargs = [\"servers/parity/parity_mcp.py\"]\n\n\
                 [[inputs]]\nkind = \"servers\"\npath = \"inputs/servers\"\nat = \"servers/parity\"\n"
            )
    };
    let claude = experiment(&base, "claude-code");
    let opencode = experiment(&base, "opencode");
    let differing: Vec<(&str, &str)> = claude
        .lines()
        .zip(opencode.lines())
        .filter(|(left, right)| left != right)
        .collect();
    assert_eq!(
        differing,
        [("name = \"claude-code\"", "name = \"opencode\"")],
        "the harness is the only line that differs"
    );

    let expected =
        serde_json::to_value(ToolOrigin::mcp_tool(SERVER.to_owned(), TOOL.to_owned())).unwrap(); // BORROW: .to_owned() makes the owned names
    let mut failures = Vec::new();
    // each driver's MCP file: its name in the run, and the object that holds the servers
    for (harness, text, file, holder) in [
        ("claude-code", &claude, ".mcp.json", "/mcpServers"),
        ("opencode", &opencode, "opencode.json", "/mcp/servers"),
    ] {
        let seen = run_once(text, &endpoint, &ca, file);
        let installed = seen
            .mcp
            .get(format!("mcp.servers.{SERVER}.command"))
            .is_some();
        if !installed {
            failures.push(format!(
                "{harness}: no mcp.servers.{SERVER} in {}",
                seen.mcp
            ));
        }
        let rendered = seen
            .rendered
            .iter()
            .filter(|config| {
                config
                    .pointer(holder)
                    .and_then(|held| held.get(SERVER))
                    .is_some()
            })
            .count();
        if rendered != 1 {
            failures.push(format!(
                "{harness}: {rendered} {file} files hold the server under {holder}: {:?}",
                seen.rendered
            ));
        }
        let mcp_calls: Vec<&Value> = seen
            .origins
            .iter()
            .filter(|origin| origin.get("kind") == Some(&Value::from("mcp")))
            .collect();
        if mcp_calls != [&expected] {
            failures.push(format!(
                "{harness}: MCP tool calls {mcp_calls:?} among {:?}, expected [{expected}]",
                seen.origins
            ));
        }
    }
    drop(made);
    assert_eq!(
        must(&real, &["ps", "-aq"]),
        before,
        "no container is left behind"
    );
    eprintln!("parity: {:?} wall", started.elapsed());
    assert!(failures.is_empty(), "{failures:#?}");
}
