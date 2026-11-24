//! Guards for the argv of the agent container: what may enter its environment.
#![cfg(test)]

use std::path::PathBuf;
use std::str::FromStr as _;
use std::sync::Arc;
use std::time::Duration;

use candler_core::isolation::container::exec::collect_spec;
use candler_core::isolation::container::mounts::{MountMode, MountSpec};
use candler_core::isolation::container::run::{RunArgvError, build_start_request, create_argv};
use candler_core::isolation::control::ControlSpec;
use candler_core::isolation::docker::{Call, ClientEnv, DockerCli};
use candler_core::isolation::driver::DockerDriver as _;
use candler_core::isolation::image::{compile_spec, probe_spec, timeout_spec};
use candler_core::isolation::model::{
    ContainerPlan, ImageId, ImageRef, Labels, Limits, NetworkName, UserSpec,
};
use candler_core::jobs::orchestrate::controls::fake_spec;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::driver::HarnessDriver as _;
use candler_harness::spec::{CommandSpec, EnvEntry};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{Digest, RunKey};
use candler_schemas::primitives::{JobId, SecretName};

/// A value no refusal may echo into an argv or an error.
const CANARY: &str = "canary-3c9e51a7d20b46f8";

fn plan() -> ContainerPlan {
    let job = JobId::from_str("01900000-0000-7000-8000-000000000000").expect("a job id");
    ContainerPlan {
        image: ImageRef {
            id: ImageId(String::from("sha256:abc")),
            harness_version: None,
        },
        labels: Labels::new()
            .with("candler.job", "job")
            .with("candler.run", "run")
            .with("candler.role", "agent"),
        mounts: Vec::new(),
        env: vec![(String::from("HOME"), String::from("/home/agent"))],
        gateway: ClaudeCodeDriver::new().gateway(),
        state: Vec::new(),
        network: NetworkName::for_job(&job),
        egress: None,
        add_hosts: Vec::new(),
        limits: Limits {
            pids_limit: 64,
            memory: String::from("1g"),
            cpus: 1.0,
        },
        record_dir: PathBuf::from("/c"),
        user: UserSpec::Engine,
        workdir: String::from("/work"),
        home: String::from("/home/agent"),
        expiry: Duration::from_secs(10),
        project_root: PathBuf::from("/p"),
        store_root: PathBuf::from("/s"),
        interactive: false,
    }
}

fn command(overlay: Vec<EnvEntry>) -> CommandSpec {
    let mut builder = CommandSpec::builder().argv(vec![String::from("true")]);
    for entry in overlay {
        builder = builder.env(entry);
    }
    builder.build().expect("a command")
}

/// Guards: B07 — no secret or auth name enters the agent container (R15).
/// Oracle: the error variant each of four overlays must give, written here from R15, and no value
/// in any argv or error; a plain literal reaches the argv as `-e NAME=value`.
/// Bite: `EnvValue::Secret(_) => continue`, or override the three names instead of refusing.
#[test]
fn b07_secret_and_auth_overlay_refused() {
    let secret = EnvEntry::secret(
        String::from("CANARY_SECRET"),
        SecretName::from_str("CANARY_SECRET").expect("a secret name"),
    );
    let refused = create_argv(&plan(), &command(vec![secret]));
    assert!(
        matches!(refused, Err(RunArgvError::SecretRefused { .. })),
        "a secret: {refused:?}"
    );

    for name in [
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_API_KEY",
    ] {
        let entry = EnvEntry::literal(String::from(name), String::from(CANARY));
        let refused = create_argv(&plan(), &command(vec![entry]));
        match refused {
            Err(error @ RunArgvError::ReservedName { .. }) => {
                assert!(
                    !error.to_string().contains(CANARY),
                    "{name}: the error echoes the value"
                );
            }
            other => panic!("{name}: expected a refusal, got {other:?}"),
        }
    }

    // The positive control: a refusal-everywhere stub fails here.
    let plain = EnvEntry::literal(String::from("PLAIN_NAME"), String::from("plain-value"));
    let argv = create_argv(&plan(), &command(vec![plain])).expect("a plain entry is allowed");
    assert!(
        argv.windows(2)
            .any(|pair| pair == ["-e", "PLAIN_NAME=plain-value"]),
        "{argv:?}"
    );
    assert!(!argv.iter().any(|part| part.contains(CANARY)));
}

/// The values that follow each occurrence of `flag` in `args`.
fn values_of<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
    args.windows(2)
        .filter(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_str())
        .collect()
}

/// One control kind's expected flags, typed by hand from the argv the core built before the
/// builders were one.
struct Kind {
    name: &'static str,
    args: Vec<String>,
    /// The `--network` value, none when the kind names no network.
    network: Option<&'static str>,
    /// Every `--tmpfs` value.
    tmpfs: &'static [&'static str],
    pids: &'static str,
    memory: &'static str,
    /// The `--cpus` value, none when the kind sets no cpu limit.
    cpus: Option<&'static str>,
    detached: bool,
}

/// The plan with a project and a store that exist under `root`.
fn plan_under(root: &std::path::Path) -> ContainerPlan {
    let mut planned = plan();
    planned.project_root = root.join("project");
    planned.store_root = root.join("store");
    let home = planned.store_root.join("home");
    for dir in [&planned.project_root, &home] {
        std::fs::create_dir_all(dir).expect("a directory");
    }
    planned.mounts = vec![MountSpec {
        source: home,
        target: String::from("/home/agent"),
        mode: MountMode::ReadWrite,
    }];
    planned
}

/// The five control kinds, each built the way its caller builds it.
fn kinds(root: &std::path::Path) -> Vec<Kind> {
    let image = ImageRef {
        id: ImageId(String::from("sha256:abc")),
        harness_version: None,
    };
    let env = [(String::from("HOME"), String::from("/tmp/home"))];
    let probe = vec![String::from("claude"), String::from("--version")];
    let args = |spec: ControlSpec| spec.args();
    vec![
        Kind {
            name: "version probe",
            args: args(probe_spec(&image, &env, &probe)),
            network: Some("none"),
            tmpfs: &["/tmp:rw,noexec,nosuid,size=64m"],
            pids: "64",
            memory: "256m",
            cpus: None,
            detached: false,
        },
        Kind {
            name: "package probe",
            args: args(timeout_spec(
                &image,
                &[],
                &["10", "python", "-c", "print()"],
            )),
            network: Some("none"),
            tmpfs: &["/tmp:rw,noexec,nosuid,size=64m"],
            pids: "64",
            memory: "256m",
            cpus: None,
            detached: false,
        },
        Kind {
            name: "lock compile",
            args: args(compile_spec(
                &image.id,
                &PathBuf::from("/src"),
                &[String::from("a.in")],
            )),
            network: Some("bridge"),
            tmpfs: &["/tmp"],
            pids: "256",
            memory: "2g",
            cpus: None,
            detached: false,
        },
        Kind {
            name: "collect",
            args: args(
                collect_spec(
                    &plan_under(root),
                    &command(Vec::new()),
                    Duration::from_secs(5),
                )
                .expect("a collect spec"),
            ),
            network: Some("none"),
            tmpfs: &["/tmp:rw,noexec,nosuid,size=256m"],
            pids: "64",
            memory: "1g",
            cpus: Some("1"),
            detached: false,
        },
        Kind {
            name: "fake model",
            args: args(fake_spec("sha256:abc", "token")),
            network: None,
            tmpfs: &["/fake:rw,mode=1777,size=1m,noexec,nosuid"],
            pids: "64",
            memory: "256m",
            cpus: None,
            detached: true,
        },
    ]
}

/// Guards: R01 §Process supervision (control containers) — every control container removes
/// itself, wears `candler.role=control` and no job label, has a read-only root, no capabilities
/// and no new privileges, and keeps its own network and limits: the probes 64 pids and 256m, the
/// compile 256 pids and 2g on the bridge, the collect its caller's limits with a
/// cpu cap, and the fake model no network flag.
/// Oracle: the flags of each kind's argv at 18142ec, typed here by hand.
/// Bite: a flag dropped from the shared builder (`--cap-drop`, `--read-only`, the label, `--rm`),
/// or the limits of every kind unified to one kind's.
#[test]
fn control_kind_flags_and_limits() {
    let root = tempfile::tempdir().expect("a temp dir");
    for kind in kinds(root.path()) {
        let name = kind.name;
        let args = &kind.args;
        assert_eq!(
            &args[..2],
            ["run", "--rm"],
            "{name}: it removes itself: {args:?}"
        );
        for flag in ["--init", "--read-only"] {
            assert!(
                args.iter().any(|arg| arg == flag),
                "{name}: {flag}: {args:?}"
            );
        }
        assert_eq!(
            values_of(args, "--cap-drop"),
            ["ALL"],
            "{name}: capabilities"
        );
        assert_eq!(
            values_of(args, "--security-opt"),
            ["no-new-privileges"],
            "{name}: privileges"
        );
        assert_eq!(
            values_of(args, "--label"),
            ["candler.role=control"],
            "{name}: the one label, with no job"
        );
        assert_eq!(
            values_of(args, "--network"),
            kind.network.into_iter().collect::<Vec<_>>(),
            "{name}: the network"
        );
        assert_eq!(values_of(args, "--tmpfs"), kind.tmpfs, "{name}: tmpfs");
        assert_eq!(values_of(args, "--pids-limit"), [kind.pids], "{name}: pids");
        assert_eq!(values_of(args, "--memory"), [kind.memory], "{name}: memory");
        assert_eq!(
            values_of(args, "--cpus"),
            kind.cpus.into_iter().collect::<Vec<_>>(),
            "{name}: cpus"
        );
        assert_eq!(
            args.iter().any(|arg| arg == "-d"),
            kind.detached,
            "{name}: detached"
        );
    }
}

/// A sink that keeps no event.
struct Quiet;

impl EventSink for Quiet {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// The names `/usr/bin/env` printed, one per line of `dump`.
fn names_in(dump: &str) -> Vec<String> {
    dump.lines()
        .filter_map(|line| line.split_once('=').map(|(name, _)| name.to_owned()))
        .collect()
}

/// Guards: R01 §Process supervision (the client allowlist) — whatever the caller's environment
/// holds, a docker client child sees only `DOCKER_HOST`, `DOCKER_CONTEXT` and `HOME` (and the
/// supervisor's own marker), on every way the one client spawns: a read, a control container and
/// a gateway-style call; and a run's attached client sees only those of the process's own.
/// Oracle: the allowlist written here, and the environment a fake docker binary printed with
/// `/usr/bin/env` for a client given `PATH`, a secret and an API key beside `HOME`.
/// Bite: an allowlist that lets `PATH` or any other name through, or a spawn site that builds its
/// own environment.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn client_env_is_the_allowlist_on_every_spawn() {
    use std::os::unix::fs::PermissionsExt as _;

    let work = tempfile::tempdir().expect("a temp dir");
    let dump = work.path().join("env.txt");
    let script = work.path().join("docker");
    std::fs::write(
        &script,
        format!("#!/bin/sh\nexec /usr/bin/env > '{}'\n", dump.display()),
    )
    .expect("a fake docker");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("executable");
    let records = work.path().join("records");
    std::fs::create_dir_all(&records).expect("a record dir");
    let given: Vec<(String, String)> = [
        ("PATH", "/usr/bin:/bin"),
        ("HOME", "/home/someone"),
        ("DOCKER_HOST", "unix:///run/docker.sock"),
        ("DOCKER_CONTEXT", "default"),
        ("ANTHROPIC_API_KEY", "canary-key"),
        ("SOME_SECRET", "canary-secret"),
    ]
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .to_vec();
    let sink: Arc<dyn EventSink> = Arc::new(Quiet);
    let supervisor = Arc::new(Supervisor::new(sink));
    let cli = DockerCli::with_binary(
        Arc::clone(&supervisor),
        script,
        ClientEnv::from_pairs(&given),
        records,
    );
    let spec = probe_spec(
        &ImageRef {
            id: ImageId(String::from("sha256:abc")),
            harness_version: None,
        },
        &[],
        &[String::from("true")],
    );
    let wait = Duration::from_secs(20);
    for way in ["capture", "control", "call"] {
        let _ = std::fs::remove_file(&dump);
        match way {
            "capture" => drop(cli.capture(vec![String::from("info")], wait).await),
            "control" => drop(cli.control(&spec, wait, 4096).await),
            "call" => drop(
                cli.call(
                    &supervisor,
                    vec![String::from("logs")],
                    Call::new(wait, 4096),
                )
                .await,
            ),
            other => unreachable!("{other}"),
        }
        let printed = std::fs::read_to_string(&dump).expect("the child printed its environment");
        let mut names = names_in(&printed);
        // The shell that starts the fake adds its own bookkeeping names.
        names.retain(|name| !["PWD", "OLDPWD", "SHLVL", "_"].contains(&name.as_str()));
        names.sort();
        assert_eq!(
            names,
            ["CANDLER_RUN", "DOCKER_CONTEXT", "DOCKER_HOST", "HOME"],
            "{way}: the child's environment names"
        );
        assert!(!printed.contains("canary"), "{way}: no value of a secret");
    }

    // The attached client's environment comes from the process, filtered by the same list.
    let outputs = {
        let framed = |stream: &str| {
            candler_core::supervise::spawn::OutputSpec::framed(
                work.path().to_path_buf(),
                stream.parse().expect("a stream"),
                candler_harness::spec::Framing::Lines,
            )
        };
        candler_core::isolation::RunOutputs::new(framed("stdout"), framed("stderr"))
    };
    let request = build_start_request(
        &candler_core::isolation::testing::RecordingDocker::new(),
        &plan_under(work.path()),
        &"abababababababababababababababababababababababababababababababab"
            .parse()
            .expect("a container id"),
        "client-env".parse().expect("a marker"),
        &RunKey::new(Digest::from_hex(&"c".repeat(64)).expect("a digest"), 1, 1).expect("a key"),
        None,
        outputs,
    )
    .expect("a request");
    let shown = format!("{request:?}");
    let listed = shown
        .split("env_names: [")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .expect("the request lists its environment names");
    for name in ["PATH", "ANTHROPIC_API_KEY"] {
        assert!(!listed.contains(name), "{name} is not passed: {listed}");
    }
    for name in ["DOCKER_HOST", "DOCKER_CONTEXT", "HOME"] {
        assert_eq!(
            listed.contains(name),
            std::env::var_os(name).is_some(),
            "{name} is passed exactly when the process has it: {listed}"
        );
    }
}
