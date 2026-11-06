//! Helpers for the guards of run planning: builders and fixtures, never expected values.
#![allow(dead_code, reason = "each guard binary uses a subset of the helpers")]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use candler_core::resolve::layers::Resolution;
use candler_core::store::Store;
use candler_core::store::run::{Existing, RunWriter};
use candler_schemas::execution::{Comparable, ExperimentFile, RunManifest};
use serde_json::{Value, json};
use tempfile::TempDir;

/// The text of a digest made of the character `c`.
pub fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

/// The facts a cell stores and a resolution observes: the four re-checked ones.
#[derive(Debug, Clone)]
pub struct Facts {
    /// The image ID.
    pub image: String,
    /// The Claude Code version.
    pub version: String,
    /// The commit the treatment's tag names.
    pub commit: String,
    /// The secrets present, by name.
    pub secrets: Vec<String>,
}

impl Facts {
    /// The facts every stored cell of a fixture holds.
    pub fn base() -> Self {
        Self {
            image: digest_text('a'),
            version: "2.1.0".to_owned(),
            commit: "1".repeat(40),
            secrets: vec!["API_KEY".to_owned()],
        }
    }
}

/// An experiment of one factor `model.id` over `models`, with `replicates` runs of each cell.
pub fn experiment(models: &[&str], replicates: u32) -> ExperimentFile {
    let levels = models
        .iter()
        .map(|model| format!("\"{model}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let text = format!(
        "schema = \"experiment/1\"\nreplicates = {replicates}\n\n[[factors]]\npath = \"model.id\"\nlevels = [{levels}]\n\n[budget.per_run]\nwall_s = 3600\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"htn\"\n"
    );
    ExperimentFile::from_toml_str(&text).expect("an experiment file")
}

/// The treatment record `facts` names.
fn treatment(facts: &Facts) -> Value {
    json!({"name": "htn", "tag": "v1", "commit": facts.commit})
}

/// A fresh resolution that observes `facts`: the values a preflight derived and the secrets it
/// found present.
pub fn resolution(facts: &Facts) -> Resolution {
    let checks: Vec<Value> = facts
        .secrets
        .iter()
        .map(|name| {
            json!({
                "name": format!("secret@{}", name.replace('_', "-")),
                "passed": true,
                "explanation": "present",
            })
        })
        .collect();
    serde_json::from_value(json!({
        "values": [
            {"path": "harness", "value": {"name": "claude-code", "version": facts.version}},
            {"path": "isolation.image", "value": facts.image},
            {"path": "treatment", "value": treatment(facts)},
        ],
        "settings": [],
        "sources": [],
        "preflight": [{"step": "secrets@1", "derived": [], "checks": checks}],
    }))
    .expect("a resolution")
}

/// What one stored run of a fixture is.
#[derive(Debug, Clone)]
pub struct Run<'a> {
    /// The model of the run's cell.
    pub model: &'a str,
    /// The replicate, from 1.
    pub replicate: u32,
    /// The attempt, from 1.
    pub attempt: u32,
    /// The facts the cell was stored with.
    pub facts: &'a Facts,
    /// The integrity JSON.
    pub integrity: Value,
    /// Whether `run.done` seals the run.
    pub sealed: bool,
}

fn manifest_json(run: &Run<'_>, cell: &str) -> Value {
    json!({
        "schema": "run-manifest/1",
        "run": {"cell": cell, "replicate": run.replicate, "attempt": run.attempt},
        "experiment": digest_text('e'),
        "factors": ["model.id"],
        "comparable": {
            "values": [
                {"path": "budget.per_run.wall_s", "value": 3600},
                {"path": "isolation.image", "value": run.facts.image},
                {"path": "model.id", "value": run.model},
            ],
            "harness": {"name": "claude-code", "version": run.facts.version},
            "treatment": treatment(run.facts),
            "snapshots": [],
            "inputs": [],
            "environments": [],
        },
        "sources": [
            {"path": "budget.per_run.wall_s", "source": {"kind": "experiment_file"}},
            {"path": "harness", "source": {"kind": "experiment_file"}},
            {"path": "isolation.image", "source": {"kind": "experiment_file"}},
            {"path": "model.id", "source": {"kind": "experiment_file"}},
            {"path": "treatment", "source": {"kind": "experiment_file"}},
        ],
        "secrets": run.facts.secrets.iter()
            .map(|name| json!({"name": name, "present": true}))
            .collect::<Vec<_>>(),
        "measured": [],
        "deviations": [],
        "integrity": run.integrity,
    })
}

/// The manifest of `run`, hand-built; its key carries the cell's real digest.
pub fn manifest(run: &Run<'_>) -> RunManifest {
    let probe: RunManifest =
        serde_json::from_value(manifest_json(run, &digest_text('f'))).expect("a manifest");
    let cell = probe.cell_digest().to_string();
    serde_json::from_value(manifest_json(run, &cell)).expect("a manifest")
}

/// The stored cell of the cell of `model` under `facts`, read back from a hand-built manifest.
pub fn stored_cell(model: &str, facts: &Facts) -> Comparable {
    let run = Run {
        model,
        replicate: 1,
        attempt: 1,
        facts,
        integrity: json!({"kind": "valid"}),
        sealed: true,
    };
    manifest(&run).comparable().clone()
}

/// Writes `run` into `store`: an unsealed run is left as a killed supervisor leaves it, with a
/// partial `events` file beside its manifest.
pub fn write_run(store: &Store, run: &Run<'_>) -> RunManifest {
    let manifest = manifest(run);
    let writer = RunWriter::create(store, &manifest, Existing::Refuse).expect("create a run");
    if run.sealed {
        writer.done().expect("seal a run");
    } else {
        let dir = store.layout().run_dir(manifest.run());
        fs::write(dir.join("events"), b"{\"seq\":0,").expect("a partial events file");
    }
    manifest
}

/// A fresh store and the path of an experiment lock in the same temporary directory.
pub fn fresh_store() -> (TempDir, Store, PathBuf) {
    let dir = TempDir::new().expect("a temporary directory");
    let store = Store::init(&dir.path().join("store")).expect("a store");
    let lock = dir.path().join("experiments").join("x").join("lock");
    (dir, store, lock)
}

/// The bytes of every file under `<store>/runs`, by path relative to the store.
pub fn snapshot(store_root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![store_root.join("runs")];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let entry = entry.expect("a directory entry");
            let path = entry.path();
            if entry.file_type().expect("a file type").is_dir() {
                pending.push(path);
            } else {
                let relative = path.strip_prefix(store_root).expect("below the store");
                files.insert(
                    relative.to_path_buf(),
                    fs::read(&path).expect("read a file"),
                );
            }
        }
    }
    files
}

/// The attempt numbers on disk for `cell_hex` and `replicate`, read from the directory names
/// `<cell hex>-r<replicate>-a<attempt>`.
pub fn attempts_on_disk(store_root: &Path, cell_hex: &str, replicate: u32) -> Vec<u32> {
    let prefix = format!("{cell_hex}-r{replicate}-a");
    let mut found: Vec<u32> = fs::read_dir(store_root.join("runs"))
        .expect("the runs directory")
        .filter_map(|entry| {
            let name = entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned();
            name.strip_prefix(&prefix)?.parse().ok()
        })
        .collect();
    found.sort_unstable();
    found
}
