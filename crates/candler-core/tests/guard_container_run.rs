//! Guards for the argv of the agent container: what may enter its environment.
#![cfg(test)]

use std::path::PathBuf;
use std::str::FromStr as _;
use std::time::Duration;

use candler_core::isolation::container::run::{RunArgvError, run_argv};
use candler_core::isolation::model::{
    ContainerPlan, ImageId, ImageRef, Labels, Limits, NetworkName, UserSpec,
};
use candler_harness::driver::HarnessDriver as _;
use candler_harness::spec::{CommandSpec, EnvEntry};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::primitives::{JobId, SecretName};

/// A value no refusal may echo into an argv or an error.
const CANARY: &str = "canary-3c9e51a7d20b46f8";

fn plan() -> ContainerPlan {
    let job = JobId::from_str("01900000-0000-7000-8000-000000000000").expect("a job id");
    ContainerPlan {
        image: ImageRef {
            id: ImageId(String::from("sha256:abc")),
            cc_version: None,
        },
        labels: Labels::new()
            .with("candler.job", "job")
            .with("candler.run", "run")
            .with("candler.role", "agent"),
        mounts: Vec::new(),
        env: vec![(String::from("HOME"), String::from("/home/agent"))],
        gateway: ClaudeCodeDriver::new().expect("the driver").gateway(),
        state: Vec::new(),
        network: NetworkName::for_job(&job),
        egress: None,
        limits: Limits {
            pids_limit: 64,
            memory: String::from("1g"),
            cpus: 1.0,
        },
        cidfile: PathBuf::from("/c/a.cid"),
        user: UserSpec::Engine,
        workdir: String::from("/work"),
        home: String::from("/home/agent"),
        expiry: Duration::from_secs(10),
        project_root: PathBuf::from("/p"),
        store_root: PathBuf::from("/s"),
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
    let refused = run_argv(&plan(), &command(vec![secret]));
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
        let refused = run_argv(&plan(), &command(vec![entry]));
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
    let argv = run_argv(&plan(), &command(vec![plain])).expect("a plain entry is allowed");
    assert!(
        argv.windows(2)
            .any(|pair| pair == ["-e", "PLAIN_NAME=plain-value"]),
        "{argv:?}"
    );
    assert!(!argv.iter().any(|part| part.contains(CANARY)));
}
