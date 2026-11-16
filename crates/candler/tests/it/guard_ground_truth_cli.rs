#![cfg(test)]
//! Guards for the refusal of ground truth through the composed resolver of `candler run`: the
//! binary resolves an experiment whose evaluator-only input has no copy in what the agent
//! receives, and refuses one whose agent input holds a copy of an evaluator-only file.

use crate::support::cli_run::world;
use crate::support::seam::{CANARY_BYTES, declare_inputs};

use std::fs;

/// Guards: R01 §Contracts I8 / D9 — `candler run` resolves an experiment whose evaluator-only
/// input has no copy anywhere (the empty files of both trees share a digest and do not count),
/// and refuses, naming the input and the place (the CLI prints the message, not the code), one whose
/// agent input holds the bytes of an evaluator-only file under another name.
/// Oracle: hand-written; the canary is a literal string, and the trees are those of the input seam.
/// Bite: drop the `register_ground_truth` call of `compose_resolver`, and the second row launches.
#[test]
fn run_refuses_a_copy_and_accepts_a_tree_without_one() {
    let clean = world();
    declare_inputs(&clean);
    let out = clean.launch();
    assert!(
        out.status.success(),
        "no copy, no refusal\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let copied = world();
    declare_inputs(&copied);
    let servers = copied.experiment.parent().unwrap().join("inputs/servers");
    fs::write(servers.join("copy.bin"), format!("{CANARY_BYTES}\n")).unwrap();
    let out = copied.launch();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(!out.status.success(), "the launcher refuses\n{stderr}");
    assert!(
        stderr.contains("evaluator-only input `truth` (answer.txt)")
            && stderr.contains("`copy.bin` in agent input `servers`"),
        "{stderr}"
    );
}
