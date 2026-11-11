//! The input seam through the binary: a run declares an input for the agent and one for the
//! evaluators, and an evaluation whose plug-in needs both reads them back.
#![cfg(test)]
#![cfg(unix)]

#[expect(
    dead_code,
    reason = "this guard uses the world and its launch, not every helper of the shared module"
)]
#[path = "support/cli_run.rs"]
mod cli_run;
#[path = "support/seam.rs"]
mod seam;

use std::collections::BTreeMap;
use std::fs;

use candler_core::store::layout::StoreLayout;
use serde_json::Value;

use cli_run::world;
use seam::{
    CANARY_BYTES, CANARY_NAME, declare_inputs, entries_under, evaluate, file_hashes, finish,
    list_digest, occurrences, servers_tree, sync_environment, truth_tree,
};

/// What the plug-in reported for one input kind.
struct Seen {
    digest: String,
    files: BTreeMap<String, String>,
    writable: Vec<String>,
}

/// The `evidence` note of the one outcome record `evaluate` wrote, by input kind.
fn reported(world: &cli_run::World) -> BTreeMap<String, Seen> {
    let layout = StoreLayout::new(&world.store());
    let records: Vec<Value> = entries_under(&layout.outcomes_dir())
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .flat_map(|path| {
            fs::read_to_string(path)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(records.len(), 1, "one outcome record: {records:?}");
    let record = &records[0];
    assert_eq!(record["status"], "pass", "the plug-in passed: {record}");
    let note = record["evidence"][0]["note"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(note).unwrap();
    parsed
        .as_object()
        .unwrap()
        .iter()
        .map(|(kind, body)| {
            let map_of = |key: &str| -> BTreeMap<String, String> {
                body[key]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                    .collect()
            };
            (
                kind.clone(), // BORROW: the map owns its keys
                Seen {
                    digest: body["digest"].as_str().unwrap().to_owned(),
                    files: map_of("files"),
                    writable: body["writable"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap().to_owned())
                        .collect(),
                },
            )
        })
        .collect()
}

/// Guards: R01 §Evaluator protocol, I8 — a run that declares an agent input and an
/// evaluator-only input, then an evaluation whose plug-in needs both: the plug-in reads both
/// trees (an empty file and nested directories among them) with the sha256 of every file, each
/// context `digest` is the digest of the input's list, neither tree is writable to it, and the
/// evaluator-only bytes and file name occur nowhere under the run directory, the job directory,
/// the recorded docker argv or the workspace, while the agent's input is visibly mounted.
///
/// Oracle: the files' sha256 by `shasum`; the list digest is the sha256 (by `shasum`) of the
/// RFC 8785 text of the sorted `[{path, sha256}]` list, written by hand; the canary is a literal
/// string in the truth tree's content and in one of its file names.
///
/// Bite: route the evaluator-only input to the agent's mounts (the canary assertion turns red),
/// or store the list blob under another key than the input's digest (the plug-in's read turns
/// red).
#[test]
fn input_seam_plugin_reads_both_trees_and_the_agent_never_held_the_canary() {
    let world = world();
    declare_inputs(&world);
    let manifest = seam::plugin(&world);
    sync_environment(&world);

    let launched = world.launch();
    let job = finish(&world, &launched);
    let evaluated = evaluate(&world, &manifest);
    let stderr = String::from_utf8_lossy(&evaluated.stderr).into_owned();
    assert!(
        evaluated.status.success(),
        "evaluate exits 0: {:?}\nstdout: {}\nstderr: {stderr}",
        evaluated.status,
        String::from_utf8_lossy(&evaluated.stdout)
    );
    let report: Value = serde_json::from_slice(&evaluated.stdout).unwrap();
    assert_eq!(report["failed_requests"], serde_json::json!([]), "{report}");

    // The plug-in read both trees, with the digests of the files and of the lists.
    let seen = reported(&world);
    let kinds: Vec<&str> = seen.keys().map(String::as_str).collect();
    assert_eq!(kinds, ["servers", "truth"], "the context served both kinds");
    for (kind, files) in [("servers", servers_tree()), ("truth", truth_tree())] {
        let got = &seen[kind];
        assert_eq!(
            got.files,
            file_hashes(&files),
            "{kind}: sha256 of each file"
        );
        assert_eq!(
            got.digest,
            list_digest(&files),
            "{kind}: the digest of the list"
        );
        assert_eq!(
            got.writable,
            Vec::<String>::new(),
            "{kind}: no file is writable"
        );
    }

    // The agent's side: the control is that its own input is visible in the recorded argv.
    let calls = world.calls();
    assert!(
        calls.contains("srv/nested/deep/b.txt"),
        "the agent's input is mounted, so the scan can see an argv\n{calls}"
    );
    let layout = StoreLayout::new(&world.store());
    let argv_file = world.bin.join("docker.log");
    // The argv carries mount paths and names, never file bytes, so only the name and the file
    // digests (below) can show a leak there; the bytes are searched where files are written.
    assert_eq!(
        occurrences(&world.bin, CANARY_NAME),
        Vec::<String>::new(),
        "`{CANARY_NAME}` in the recorded docker argv"
    );
    for needle in [CANARY_BYTES, CANARY_NAME] {
        for (place, root) in [
            ("run directory", layout.runs_dir()),
            ("job directory", layout.job_dir(&job)),
        ] {
            assert_eq!(
                occurrences(&root, needle),
                Vec::<String>::new(),
                "`{needle}` under the {place}"
            );
        }
    }
    // The file digests only the evaluator-only files have are not in the argv either (the empty
    // file's digest is the agent's too, so it proves nothing).
    let agent_hashes = file_hashes(&servers_tree());
    let argv = fs::read_to_string(&argv_file).unwrap();
    for (path, hash) in file_hashes(&truth_tree()) {
        if !agent_hashes.values().any(|known| *known == hash) {
            assert!(
                !argv.contains(hash.as_str()),
                "the digest of `{path}` is in the argv"
            );
        }
    }
    world.remove();
}
