//! I8 against a real daemon: the evaluator-only canary never reaches the agent of a real
//! `candler run`.
//!
//! The run is the product's own: real Claude Code in the agent base image, behind the production
//! gateway sidecar, against the scripted fake model (`assets/docker/fakemodel`, model `c3`, which
//! holds its answer so that the run stays live). The fake model is reached through the machine
//! setting `[gateway] upstream_ca`, as a user would reach a private upstream. The experiment
//! declares an agent input, an evaluator-only input carrying the canary, and a plug-in agent
//! environment (values and agent inputs only).
//!
//! Checkpoints: one inspect of the agent container's config and its image history before anything
//! runs in it; the testagent's seeker, piped to `docker exec -i --privileged -u 0 <id> sh -s`,
//! at the start, mid-turn and near the end of the run, before the container is destroyed (it reports the files,
//! `/proc/*/environ` including `/proc/1/environ`, and the mounts); `candler ps`, `show` and
//! `inspect` while the run is live; and, once it ended, the fake model's request log (what
//! the gateway forwarded), the run and job directories, and every container the job made.
use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use candler_core::jobs::dir::JobDir;
use candler_core::store::layout::StoreLayout;
use serde_json::Value;

use super::{docker, real_docker};
use crate::support::cli_run::{wait_for, world};
use crate::support::seam::{CANARY_BYTES, CANARY_NAME, declare_inputs, occurrences};

/// The credential the fake model demands and the experiment's secret supplies.
const TOKEN: &str = "i8-fixture-token-0123456789";
/// How long the fake model holds its answer, in seconds.
const DELAY_SECS: u64 = 100;

/// The name or address the fake model's certificate is for and the job's containers reach it by:
/// `host.docker.internal` on Docker Desktop, the default bridge's gateway address on a Linux
/// engine, which resolves no such name in a container the product starts.
fn fake_host(real: &Path) -> String {
    if cfg!(target_os = "linux") {
        must(
            real,
            &[
                "network",
                "inspect",
                "bridge",
                "--format",
                "{{(index .IPAM.Config 0).Gateway}}",
            ],
        )
    } else {
        String::from("host.docker.internal")
    }
}

/// The pattern the seeker is given for the canary: its bytes or its file name.
fn canary_pattern() -> String {
    format!("{CANARY_BYTES}|{CANARY_NAME}")
}

/// `text` as an extended regular expression that matches it literally.
fn ere_escape(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            let escaped = r"\.[]{}()*+?^$|".contains(c);
            escaped.then_some('\\').into_iter().chain([c])
        })
        .collect()
}

/// What the test made and must give back, by exact id.
#[derive(Default)]
struct Made {
    real: PathBuf,
    job: Option<String>,
    containers: Vec<String>,
    images: Vec<String>,
}

impl Drop for Made {
    fn drop(&mut self) {
        if let Some(job) = &self.job {
            let filter = format!("label=candler.job={job}");
            for kind in [["ps", "-aq"], ["network", "ls"]] {
                let mut args: Vec<&str> = kind.to_vec();
                if kind[0] == "network" {
                    args.push("-q");
                }
                args.extend(["--filter", &filter]);
                let found = String::from_utf8_lossy(&docker(&self.real, &args).stdout).into_owned();
                for id in found.split_whitespace() {
                    let verb: &[&str] = if kind[0] == "ps" {
                        &["rm", "-f", "-v"]
                    } else {
                        &["network", "rm"]
                    };
                    let mut rm: Vec<&str> = verb.to_vec();
                    rm.push(id);
                    docker(&self.real, &rm);
                }
            }
        }
        for id in &self.containers {
            docker(&self.real, &["rm", "-f", "-v", id]);
        }
        for id in &self.images {
            docker(&self.real, &["image", "rm", id]);
        }
    }
}

/// What `docker <args>` printed on stdout, for a command that must succeed.
fn must(real: &Path, args: &[&str]) -> String {
    let out = docker(real, args);
    assert!(
        out.status.success(),
        "docker {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// Panics when `text`, the observation of `what`, holds the canary's bytes or its file name.
fn assert_absent(what: &str, text: &str) {
    for canary in [CANARY_BYTES, CANARY_NAME] {
        assert!(!text.contains(canary), "the canary `{canary}` is in {what}");
    }
}

/// The seeker of the testagent run in the agent container as root with every capability, so it
/// reads `/proc/1/environ` whoever owns it, reading its script from stdin; the canary is the pattern it is given, never baked into an image.
fn seek(real: &Path, agent: &str, phase: &str, pattern: &str) -> String {
    let script = fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/docker/testagent/testagent.sh"
    ))
    .expect("the testagent script");
    let mut child = Command::new(real)
        .args([
            "exec",
            "-i",
            "--privileged",
            "-u",
            "0",
            agent,
            "sh",
            "-s",
            "seeker",
            pattern,
            phase,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("docker exec");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(&script)
        .expect("the script reaches the container");
    let out = child.wait_with_output().expect("the seeker ends");
    assert!(
        out.status.success(),
        "the seeker ({phase}) failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Checks one seeker report: it ran to its end, saw the mount table and `/proc/1/environ`, and
/// found the canary in no file, no environment and no mount.
fn assert_seen_nothing(phase: &str, report: &str) {
    assert!(
        report.contains(&format!("seeker:{phase}:done")),
        "the {phase} report is complete:\n{report}"
    );
    assert!(
        report.contains(&format!("seeker:{phase}:mount:")),
        "the {phase} report has the mount table"
    );
    assert!(
        report.contains(&format!("seeker:{phase}:environ:1:")),
        "the {phase} report read /proc/1/environ:\n{report}"
    );
    let files: Vec<&str> = report
        .lines()
        .filter(|line| line.starts_with(&format!("seeker:{phase}:file:")))
        .collect();
    assert!(
        files.is_empty(),
        "the {phase} seeker found files: {files:?}"
    );
    assert_absent(&format!("the {phase} seeker's report"), report);
}

/// Whether the fake model has recorded a `/v1/messages` request for model `c3`.
fn c3_arrived(real: &Path, fake: &str) -> bool {
    let out = docker(real, &["exec", fake, "cat", "/tmp/fake/records.jsonl"]);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .any(|record| {
            record["model"] == "c3"
                && record["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.starts_with("/v1/messages"))
        })
}

/// Runs `candler <args>` against the world's store and returns its stdout and stderr together.
fn candler(env: &[(&str, PathBuf)], work: &Path, args: &[&str]) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_candler"));
    command
        .args(args)
        .current_dir(work)
        .env_remove("CANDLER_STORE");
    for (key, value) in env {
        command.env(key, value);
    }
    let out = command.stdin(Stdio::null()).output().expect("candler runs");
    assert!(
        out.status.success(),
        "candler {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Guards: R01 I8, B7 — in a real `candler run`, an evaluator-only input's canary (its bytes and
/// its file name) is absent from the agent's container at every checkpoint (the container's
/// configuration and image history before it runs anything, and its files, every
/// `/proc/*/environ` including PID 1's, and its mounts at the start, mid-turn and near the end,
/// all before destroy), from `candler ps`, `show` and `inspect` while the run is live, and, once
/// it ended, from the fake model's request log, the run and job directories and every container
/// the job made.
///
/// Oracle: the canary as the fixture writes it (`CANARY_BYTES`, `CANARY_NAME` of the seam
/// support), looked for by the testagent's seeker as the agent's user and by plain substring on
/// everything else; never read back from Candler's code.
///
/// Bite: flip the audience filter in `mount_inputs` (the canary file is in the seeker's report), or
/// add the input to the agent's environment (`/proc/1/environ`), or disable the D9 step with the
/// canary in the plug-in's agent input; the same canary in the build context is refused at
/// resolve, before any container exists.
#[test]
#[ignore = "needs Docker; run with --run-ignored only"]
fn i8_canary_never_reaches_the_agent() {
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

    // the fake model, on a port of the host's loopback, under its own authority
    let context = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/fakemodel");
    let tag = format!("candler-fakemodel:i8-{}", std::process::id());
    must(
        &real,
        &[
            "build",
            "-q",
            "--label",
            "candler.test=i8",
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
            "candler.test=i8",
            "-p",
            &publish,
            "-e",
            &format!("FAKE_TOKEN={TOKEN}"),
            "-e",
            &format!("FAKE_HOST={host}"),
            "-e",
            &format!("FAKE_DELAY_SECS={DELAY_SECS}"),
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

    // the world: the machine setting that trusts the fake's authority, the plug-in, the inputs
    let world = world();
    declare_inputs(&world);
    let ca = world.root.join("ca.pem");
    must(
        &real,
        &[
            "cp",
            &format!("{fake}:/tmp/fake/ca.pem"),
            &ca.display().to_string(),
        ],
    );
    let config = world.root.join("config");
    fs::create_dir_all(config.join("candler")).unwrap();
    fs::write(
        config.join("candler").join("config.toml"),
        format!("[gateway]\nupstream_ca = \"{}\"\n", ca.display()),
    )
    .unwrap();
    let plugin = world.root.join("plugins").join("i8-env");
    fs::create_dir_all(plugin.join("agent")).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugins/i8-env");
    fs::copy(fixture.join("plugin.toml"), plugin.join("plugin.toml")).unwrap();
    fs::copy(
        fixture.join("agent/notes.txt"),
        plugin.join("agent/notes.txt"),
    )
    .unwrap();
    fs::write(
        world.work.join("candler.toml"),
        format!("[plugins]\ndirs = [\"{}\"]\n", plugin.display()),
    )
    .unwrap();
    fs::write(world.work.join("env.lock"), "").unwrap();
    let text = fs::read_to_string(&world.experiment).unwrap();
    let text = text
        .replace("http://127.0.0.1:9", &format!("https://{host}:{port}"))
        .replace("\"fixture-model\"", "\"c3\"")
        .replace("wall_s = 30", "wall_s = 300");
    fs::write(&world.experiment, text).unwrap();

    let env = [
        ("XDG_STATE_HOME", world.state.clone()),
        ("XDG_CONFIG_HOME", config),
        ("FIXTURE_TOKEN", PathBuf::from(TOKEN)),
    ];
    let launched = Command::new(env!("CARGO_BIN_EXE_candler"))
        .arg("run")
        .arg(&world.experiment)
        .args([
            "--set",
            "isolation.build.lock=env.lock",
            "--set",
            "isolation.build.plugin=i8-env/plain",
            "--set",
            "model.token_secret=FIXTURE_TOKEN",
        ])
        .current_dir(&world.work)
        .env_remove("CANDLER_STORE")
        .envs(env.iter().map(|(key, value)| (*key, value)))
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
    made.job = Some(job.clone()); // BORROW: the cleanup owns the job id
    let dir = JobDir::open(
        &StoreLayout::new(&(world.work.join(".candler").join("store"))),
        &job.parse().unwrap(),
    )
    .unwrap();

    // the agent container, as soon as the job has made it: one inspect and its image's history
    let filter = format!("label=candler.job={job}");
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let mut agent: Option<String> = None;
    let found = wait_for(Duration::from_secs(600), || {
        gather(&real, &dir, &filter, &mut seen);
        agent = seen.keys().find(|id| events_name_agent(&dir, id)).cloned(); // BORROW: the test owns the agent's id
        agent.is_some()
    });
    assert!(
        found,
        "the job made an agent container; it made: {:?}",
        seen.keys()
    );
    let agent = agent.expect("an agent");
    let inspected = &seen[&agent];
    let value: Value = serde_json::from_str(inspected).unwrap();
    assert!(
        !value[0]["Mounts"].as_array().expect("mounts").is_empty(),
        "the agent has mounts, so the check reads something: {}",
        inspected.chars().take(3000).collect::<String>()
    );
    assert_absent("the agent's inspect (env, mounts, labels)", inspected);
    let image = value[0]["Image"].as_str().expect("an image").to_owned(); // BORROW: the test owns the id
    let history = must(
        &real,
        &[
            "history",
            "--no-trunc",
            "--format",
            "{{.CreatedBy}}",
            &image,
        ],
    );
    assert_absent("the agent image's history", &history);

    // the seeker's positive control: a pattern that is there, so a silent seeker is a blind one
    let notes = fs::read_to_string(fixture.join("agent/notes.txt")).unwrap();
    let known = ere_escape(notes.lines().next().expect("a line in notes.txt").trim());
    let control = seek(&real, &agent, "control", &known);
    assert!(
        control
            .lines()
            .any(|line| line.starts_with("seeker:control:file:") && line.ends_with("/notes.txt")),
        "the seeker finds the agent input's own text, so it can match:\n{control}"
    );

    // live: the start, then mid-turn once the fake model holds the agent's request
    assert_seen_nothing("start", &seek(&real, &agent, "start", &canary_pattern()));
    assert!(
        wait_for(Duration::from_secs(300), || c3_arrived(&real, &fake)),
        "the agent's request reached the fake model"
    );
    let arrived = Instant::now();
    let reported = seek(&real, &agent, "turn", &canary_pattern());
    assert_seen_nothing("turn", &reported);
    for args in [vec!["ps"], vec!["show"], vec!["inspect", &job]] {
        let mut full = vec!["--store"];
        let store = (world.work.join(".candler").join("store"))
            .display()
            .to_string();
        full.push(&store);
        full.extend(args.iter().copied());
        assert_absent(
            &format!("`candler {}`", args.join(" ")),
            &candler(&env, &world.work, &full),
        );
    }
    assert!(
        arrived.elapsed() < Duration::from_secs(DELAY_SECS - 5),
        "the slow reply is still held after the turn's seeker ({:?} of {DELAY_SECS}s)",
        arrived.elapsed()
    );
    assert_seen_nothing("end", &seek(&real, &agent, "end", &canary_pattern()));

    // the run ends by the normal path
    let events = dir.path().join("events.jsonl");
    assert!(
        wait_for(Duration::from_secs(300), || {
            gather(&real, &dir, &filter, &mut seen); // the collect step's container comes after the agent's
            fs::read_to_string(&events).is_ok_and(|log| log.contains("job.ended"))
        }),
        "the job ends"
    );
    gather(&real, &dir, &filter, &mut seen);
    eprintln!("i8: inspected {} job containers", seen.len());
    for (id, inspected) in &seen {
        assert_absent(&format!("the inspect of job container {id}"), inspected);
    }

    // after: what the gateway forwarded, and what the run left on disk
    let records = must(&real, &["exec", &fake, "cat", "/tmp/fake/records.jsonl"]);
    assert!(
        records.contains("\"c3\""),
        "the fake model saw the agent's turn"
    );
    assert_absent("the fake model's request log", &records);
    assert_absent("the fake model's output", &must(&real, &["logs", &fake]));
    for root in [
        (world.work.join(".candler").join("store")).join("runs"),
        dir.path().to_path_buf(),
    ] {
        let hits = [CANARY_BYTES, CANARY_NAME]
            .iter()
            .flat_map(|canary| occurrences(&root, canary))
            .collect::<Vec<_>>();
        assert!(
            hits.is_empty(),
            "the canary is under {}: {hits:?}",
            root.display()
        );
    }

    // give back what the test made, by exact id, and compare
    drop(made);
    assert_eq!(
        must(&real, &["ps", "-aq"]),
        before,
        "no container is left behind"
    );
    eprintln!("i8: {:?} wall", started.elapsed());
}

/// Inspects every container of the job that `seen` does not hold yet: those with the job's label
/// and those its event log names.
fn gather(real: &Path, dir: &JobDir, filter: &str, seen: &mut BTreeMap<String, String>) {
    let labelled = must(real, &["ps", "-aq", "--filter", filter]);
    let log = fs::read_to_string(dir.path().join("events.jsonl")).unwrap_or_default();
    for id in must(real, &["ps", "-aq"]).split_whitespace() {
        let ours = labelled.split_whitespace().any(|l| l == id)
            || log.contains(&format!("\"cid\":\"{id}"));
        if ours && !seen.contains_key(id) {
            let inspected = docker(real, &["inspect", id]); // a container may be destroyed meanwhile
            if inspected.status.success() {
                seen.insert(
                    id.to_owned(), // BORROW: the map owns its keys
                    String::from_utf8_lossy(&inspected.stdout).into_owned(),
                );
            }
        }
    }
}

/// Whether the job's event log records `short`, a container id as `docker ps` prints it, as the
/// job's agent container.
fn events_name_agent(dir: &JobDir, short: &str) -> bool {
    fs::read_to_string(dir.path().join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .any(|line| {
            line.contains("\"role\":\"agent\"") && line.contains(&format!("\"cid\":\"{short}"))
        })
}
