//! Guards the MCP configuration `candler inspect` shows for a run the real runner sealed: the
//! resolved `mcp.*` values the cell digest hashes.
#![cfg(test)]
#![cfg(unix)]

use crate::support::cli_run::world;
use crate::support::seam::finish;

use std::fs;
use std::process::Command;

use serde_json::Value;

/// Guards: a run launched with one declared MCP server is read back through `inspect`, as JSON and
/// as text, and the declared server is among the shown `mcp.*` values.
///
/// Oracle: the server name and command are the literals the test passes to `candler run --set`.
///
/// Bite: leave the `mcp.*` values out of what `inspect` shows (the `mcp` member of the record and
/// the text lines both lose the server).
#[test]
fn inspect_shows_the_mcp_server_a_run_was_launched_with() {
    let world = world();
    let launched = world.launch_with(&[
        "--set",
        "mcp.servers.fs.command=mcp-fs",
        "--set",
        "mcp.servers.fs.args=[\"data\"]",
    ]);
    finish(&world, &launched);
    world.assert_sealed(1);

    let runs = world.store().join("runs");
    let run_dir = fs::read_dir(&runs)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.join("run.done").is_file())
        .expect("a sealed run");
    let inspect = |extra: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_candler"))
            .arg("inspect")
            .arg(&run_dir)
            .args(extra)
            .current_dir(&world.work)
            .env_clear()
            .output()
            .unwrap();
        assert!(out.status.success(), "inspect exits 0: {out:?}");
        String::from_utf8(out.stdout).unwrap()
    };

    let record: Value = serde_json::from_str(&inspect(&[])).unwrap();
    let shown = record["mcp"]
        .as_object()
        .unwrap_or_else(|| panic!("the record shows the mcp values: {record}"));
    assert_eq!(
        shown.get("mcp.servers.fs.command"),
        Some(&Value::from("mcp-fs")),
        "the declared command is shown: {record}"
    );
    assert_eq!(
        shown.get("mcp.servers.fs.args"),
        Some(&serde_json::json!(["data"])),
        "the declared arguments are shown: {record}"
    );

    let text = inspect(&["--format", "text"]);
    assert!(
        text.lines()
            .any(|line| line.starts_with("mcp.servers.fs") && line.contains("mcp-fs")),
        "the text shows the declared server:\n{text}"
    );
}

/// Guards: a cell Candler resolves and runs carries no `snapshots` key, and its digest, which
/// names the run, is the digest of the cell written without it.
///
/// Oracle: the run's key names the cell digest the runner computed; the test recomputes it from
/// the stored `comparable` text with `Digest::of_canonical`, so a key written or hashed but not
/// stored (or the other way round) moves one side only.
///
/// Bite: have resolve fill `comparable.snapshots` with an empty list (the key appears in the
/// stored cell and the digest changes).
#[test]
fn a_cell_from_resolve_is_stored_and_hashed_without_snapshots() {
    let world = world();
    let launched = world.launch();
    finish(&world, &launched);
    world.assert_sealed(1);

    let runs = world.store().join("runs");
    let run_dir = fs::read_dir(&runs)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.join("run.done").is_file())
        .expect("a sealed run");
    let manifest: Value =
        serde_json::from_slice(&fs::read(run_dir.join("manifest.json")).unwrap()).unwrap();
    let comparable = manifest["comparable"].as_object().expect("a stored cell");
    assert!(
        !comparable.contains_key("snapshots"),
        "a new cell carries no snapshots key: {comparable:?}"
    );
    let recomputed = candler_schemas::primitives::Digest::of_canonical(&manifest["comparable"])
        .expect("the cell has a canonical form");
    assert_eq!(
        manifest["run"]["cell"].as_str(),
        Some(recomputed.as_str()),
        "the run is keyed by the digest of the cell as stored"
    );
}
