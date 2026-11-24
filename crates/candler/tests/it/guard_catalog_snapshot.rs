//! Guards the tool-catalog snapshot of a run the real runner sealed: the digest the cell records
//! in `snapshots` resolves to a stored blob, and `candler inspect` shows the catalog the run ran
//! with.
#![cfg(test)]
#![cfg(unix)]

use crate::support::cli_run::world;
use crate::support::seam::finish;

use std::fs;
use std::process::Command;

use candler_core::store::Store;
use candler_schemas::primitives::Digest;
use serde_json::Value;

/// Guards: a sealed run stores the catalog it ran with — the manifest's catalog digest names a
/// blob whose bytes hash to it, and `inspect` of the run directory, as JSON and as text, shows the
/// catalog the fake image printed.
///
/// Oracle: the digest is the sha256 of the blob's bytes, computed by `Digest::of_bytes`; the
/// provider name is the literal the fixture's fake `docker run` prints.
///
/// Bite: skip the snapshot's `put_blob` at the seal (the blob read turns red, and `inspect` shows
/// `catalog not stored`).
#[test]
fn a_sealed_run_stores_the_catalog_it_ran_with_and_inspect_shows_it() {
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
    let digest: Digest = manifest["comparable"]["snapshots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["kind"] == "catalog")
        .expect("the cell records a catalog digest")["digest"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let store = Store::open(&world.store()).expect("the store opens");
    let bytes = fs::read(store.blob_path(&digest)).expect("the catalog digest resolves to a blob");
    assert_eq!(
        Digest::of_bytes(&bytes),
        digest,
        "the blob holds the hashed bytes"
    );

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
    assert_eq!(record["catalog"]["provider"], "candler-image@1", "{record}");
    assert_eq!(
        record["catalog"]["servers"],
        serde_json::json!([]),
        "{record}"
    );
    let text = inspect(&["--format", "text"]);
    assert!(
        text.lines().any(|line| line == "catalog candler-image@1"),
        "the text shows the catalog:\n{text}"
    );
}
