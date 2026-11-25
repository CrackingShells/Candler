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
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::i8_e2e::{Made, fake_host, must};
use super::{docker, real_docker};
use crate::support::cli_run::{World, wait_for, world};
use crate::support::seam::entries_under;

/// The credential the fake model demands and the experiment's secret supplies.
pub(super) const TOKEN: &str = "parity-fixture-token-0123456789";

/// The server the cell declares.
pub(super) const SERVER: &str = "parity";

/// The text the fixture's tool returns.
const STAMP: &str = "parity-stamp-5c1e";

/// The id the fake model gives its call to the tool, which the answer to the call carries back.
const CALL_ID: &str = "toolu_fake_stamp";

/// The experiment of one cell, with the harness the one line `[harness]` names.
fn experiment(base: &str, harness: &str) -> String {
    base.replace("@HARNESS@", harness)
}

/// The answers to the stamp call among the requests the fake model recorded after the first
/// `skip`: whether each is an error, and its text.
fn stamp_answers(records: &str, skip: usize) -> Vec<(bool, String)> {
    let mut found = Vec::new();
    for line in records.lines().skip(skip) {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let blocks = record["body"]["messages"]
            .as_array()
            .and_then(|messages| messages.last())
            .and_then(|message| message["content"].as_array());
        for block in blocks.into_iter().flatten() {
            if block["type"] == "tool_result" && block["tool_use_id"] == CALL_ID {
                let text = block["content"].as_str().map_or_else(
                    || block["content"].to_string(),
                    str::to_owned, // BORROW: .to_owned() makes the owned text
                );
                found.push((block["is_error"] == true, text));
            }
        }
    }
    found
}

/// What the fake model has recorded so far: nothing before its first request.
pub(super) fn records(real: &Path, fake: &str) -> String {
    let out = docker(real, &["exec", fake, "cat", "/tmp/fake/records.jsonl"]);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The program and arguments of a rendered server entry, whether the file keeps them as a command
/// and its arguments or as one list.
fn argv_of(entry: &Value) -> Vec<Value> {
    if let Some(list) = entry["command"].as_array() {
        return list.clone(); // BORROW: .clone() copies the list
    }
    std::iter::once(entry["command"].clone()) // BORROW: .clone() copies the program
        .chain(entry["args"].as_array().into_iter().flatten().cloned()) // BORROW: .cloned() copies the arguments
        .collect()
}

/// What a run of one harness stored: the resolved `mcp.*` values `candler inspect` shows, the
/// MCP files its driver rendered at the place the harness reads (the parsed files below the
/// run's workspace or home that end in the path), and the origin of each tool call of the stored
/// trace.
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

/// A run `candler run` sealed in its own world.
pub(super) struct Sealed {
    /// The world the run was made in; dropping it removes its directories.
    pub(super) world: World,
    /// The run's directory in the world's store.
    pub(super) run_dir: PathBuf,
}

/// Runs the experiment `text` with the real `candler run` against the fake model at `endpoint`
/// and waits for its one run to seal.
pub(super) fn seal(text: &str, endpoint: &str, ca: &Path, made: &mut Made) -> Sealed {
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
    made.jobs.push(job.clone()); // BORROW: the cleanup owns the job id
    let store = world.store();
    let events = store.join("jobs").join(&job).join("events.jsonl");
    let ended = wait_for(Duration::from_secs(600), || {
        fs::read_to_string(&events).is_ok_and(|log| log.contains("job.ended"))
    });
    let log = fs::read_to_string(&events).unwrap_or_default();
    assert!(ended, "the job ends\n{log}");

    let run_dir = fs::read_dir(store.join("runs"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.join("run.done").is_file())
        .unwrap_or_else(|| panic!("a sealed run\n{log}"));
    Sealed { world, run_dir }
}

/// Runs the experiment `text` with [`seal`] and reads back what the sealed run stored, the MCP
/// file being the one `place` names: the directory of the run it is written under and the path
/// it ends in.
fn run_once(
    text: &str,
    endpoint: &str,
    ca: &Path,
    place: (&str, &str),
    made: &mut Made,
) -> Observed {
    let Sealed { world, run_dir, .. } = seal(text, endpoint, ca, made);
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
    let rendered = entries_under(&run_dir.join(place.0))
        .into_iter()
        .filter(|path| path.ends_with(place.1))
        .filter_map(|path| serde_json::from_slice(&fs::read(path).ok()?).ok())
        .collect();
    Observed {
        rendered,
        mcp: record["mcp"].clone(), // BORROW: .clone() copies the member
        origins: call_origins(&trace),
    }
}

/// The scripted fake model, started for one guard.
pub(super) struct Fake {
    /// The id of the fake model's container.
    pub(super) id: String,
    /// The `https` address the gateway reaches the fake model at.
    pub(super) endpoint: String,
    /// The path of the fake model's certificate authority, copied out of its container.
    pub(super) ca: PathBuf,
    /// The directory that holds `ca.pem`; it keeps the file alive for as long as the fake lives.
    _scratch: tempfile::TempDir,
}

/// Builds the fake model's image and starts it, labelled `label` and given `env`, recording
/// both in `made`; waits for it to be ready and copies out its certificate authority.
pub(super) fn start_fake(real: &Path, made: &mut Made, label: &str, env: &[String]) -> Fake {
    let host = fake_host(real);
    let publish = format!(
        "{}::8443",
        if cfg!(target_os = "linux") {
            host.as_str()
        } else {
            "127.0.0.1"
        }
    );
    made.real = real.to_owned(); // BORROW: the cleanup owns its client path
    let context = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/fakemodel");
    let tag = format!("candler-fakemodel:{label}-{}", std::process::id());
    let marked = format!("candler.test={label}");
    must(
        real,
        &["build", "-q", "--label", &marked, "-t", &tag, context],
    );
    made.images.push(must(
        real,
        &["image", "inspect", "--format", "{{.Id}}", &tag],
    ));
    let token = format!("FAKE_TOKEN={TOKEN}");
    let named = format!("FAKE_HOST={host}");
    let mut words = vec![
        "run", "-d", "--label", &marked, "-p", &publish, "-e", &token, "-e", &named,
    ];
    for setting in env {
        words.extend(["-e", setting.as_str()]);
    }
    words.push(&tag);
    let fake = must(real, &words);
    made.containers.push(fake.clone()); // BORROW: the cleanup owns its ids
    let port = must(real, &["port", &fake, "8443/tcp"])
        .lines()
        .next()
        .and_then(|line| line.rsplit(':').next())
        .expect("a host port")
        .to_owned();
    assert!(
        wait_for(Duration::from_secs(30), || {
            docker(real, &["exec", &fake, "test", "-f", "/tmp/fake/ca.pem"])
                .status
                .success()
                && docker(real, &["logs", &fake])
                    .stdout
                    .windows(5)
                    .any(|w| w == b"ready")
        }),
        "the fake model is ready"
    );
    let scratch = tempfile::tempdir().unwrap();
    let ca = scratch.path().join("ca.pem");
    must(
        real,
        &[
            "cp",
            &format!("{fake}:/tmp/fake/ca.pem"),
            &ca.display().to_string(),
        ],
    );
    Fake {
        endpoint: format!("https://{host}:{port}"),
        id: fake,
        ca,
        _scratch: scratch,
    }
}

/// The experiment both cells share, with the line `[harness]` names left as `@HARNESS@`: the
/// shape's model replaced by `model`, its prompt by `prompt`, and the server `parity` declared.
pub(super) fn base_experiment(model: &str, prompt: &str) -> String {
    let shape = world();
    let text = fs::read_to_string(&shape.experiment)
        .unwrap()
        .replace(&shape.skills.display().to_string(), "@SKILLS@");
    text.replace("\"plan\"", "\"bypassPermissions\"")
        .replace("\"fixture-model\"", &format!("\"{model}\""))
        .replace("say hello", prompt)
        .replace("wall_s = 30", "wall_s = 300")
        .replacen(
            "[harness]\nname = \"claude-code\"",
            "[harness]\nname = \"@HARNESS@\"",
            1,
        )
        + &format!(
            "\n[fixed.mcp.servers.{SERVER}]\ncommand = \"python3\"\nargs = [\"servers/parity/parity_mcp.py\"]\n\n\
             [[inputs]]\nkind = \"servers\"\npath = \"inputs/servers\"\nat = \"servers/parity\"\n"
        )
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
    let before = must(&real, &["ps", "-aq"]);
    let mut made = Made::default();
    let fake = start_fake(&real, &mut made, "parity", &[]);
    let (endpoint, ca) = (fake.endpoint.as_str(), fake.ca.as_path());

    // one experiment file; only the harness line differs
    let base = base_experiment(
        "p1",
        "Call the MCP tool stamp of the server parity once, then reply done.",
    );
    let claude = experiment(&base, "claude-code");
    let opencode = experiment(&base, "opencode");
    assert_eq!(
        claude.replace("claude-code", "@HARNESS@"),
        opencode.replace("opencode", "@HARNESS@"),
        "the harness is the only thing that differs between the two files"
    );

    let expected = json!({"kind": "mcp", "server": "parity", "tool": "stamp"});
    let mut failures = Vec::new();
    // each driver's MCP file: the directory of the run it is written under, and its path there
    for (harness, text, place) in [
        ("claude-code", &claude, ("workspace", ".mcp.json")),
        ("opencode", &opencode, ("home", "opencode/opencode.json")),
    ] {
        let skip = records(&real, &fake.id).lines().count();
        let seen = run_once(text, endpoint, ca, place, &mut made);
        let answers = stamp_answers(&records(&real, &fake.id), skip);
        if !answers
            .iter()
            .any(|(error, text)| !error && text.contains(STAMP))
        {
            failures.push(format!(
                "{harness}: the model never received the stamp from the tool: {answers:?}"
            ));
        }
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
        let resolved: Vec<Value> =
            std::iter::once(seen.mcp[format!("mcp.servers.{SERVER}.command")].clone())
                .chain(
                    seen.mcp[format!("mcp.servers.{SERVER}.args")]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .cloned(),
                )
                .collect(); // BORROW: .clone() and .cloned() copy the resolved values
        let rendered: Vec<Vec<Value>> = seen
            .rendered
            .iter()
            .map(|config| {
                argv_of(
                    config
                        .pointer("/mcpServers")
                        .or_else(|| config.pointer("/mcp/servers"))
                        .map_or(&Value::Null, |held| &held[SERVER]),
                )
            })
            .collect();
        if rendered != [resolved.clone()] {
            failures.push(format!(
                "{harness}: the rendered file(s) at {place:?} hold {rendered:?}, the resolved command is {resolved:?}"
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
