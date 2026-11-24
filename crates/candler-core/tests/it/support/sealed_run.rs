//! The one producer of a sealed run that holds output files, for guards that read what a run
//! stored. It seals through the runner's own `seal_run`, the step that stores each output's blob,
//! so a gap in that step shows in every guard that reads outputs.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::str::FromStr as _;

use candler_core::runner::exec::seal::seal_run;
use candler_core::store::Store;
use candler_core::store::run::{Existing, RunWriter};
use candler_schemas::execution::{RunManifest, RunTree, TreeEntry};
use candler_schemas::primitives::{Digest, RelPath, RunKey};
use candler_schemas::trace::TraceFile;

/// One file the agent left in its workspace.
pub struct File {
    pub path: &'static str,
    pub bytes: &'static [u8],
    pub executable: bool,
}

/// A trace of no session: the header alone.
fn empty_trace() -> TraceFile {
    let header = r#"{"kind":"header","schema":"normalised-trace/1","reader":"claude-code-reader@0.1.0","raw":[]}"#;
    TraceFile::read(format!("{header}\n").as_bytes()).expect("a trace file")
}

/// Seals a new run beside `template`, whose manifest it copies with `attempt`, over a workspace
/// that holds exactly `files`, and returns its key. The runner's `seal_run` stores each file as a
/// blob, then writes the manifest, the tree and `run.done`.
pub fn seal_workspace(store: &Store, template: &RunKey, attempt: u32, files: &[File]) -> RunKey {
    let mut manifest = serde_json::to_value(store.read_manifest(template).expect("the manifest"))
        .expect("a manifest serialises");
    *manifest
        .pointer_mut("/run/attempt")
        .expect("the manifest names an attempt") = attempt.into();
    let manifest: RunManifest = serde_json::from_value(manifest).expect("a manifest reads");

    let workspace = tempfile::tempdir().expect("a workspace");
    let mut tree = RunTree::builder();
    for file in files {
        let at = workspace.path().join(file.path);
        fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
        fs::write(&at, file.bytes).expect("a file writes");
        let mode = if file.executable { 0o755 } else { 0o644 };
        fs::set_permissions(&at, fs::Permissions::from_mode(mode)).expect("a mode");
        tree = tree.entry(TreeEntry::file(
            RelPath::from_str(file.path).expect("a path"),
            Digest::of_bytes(file.bytes),
            file.executable,
        ));
    }
    let key = manifest.run().clone();
    let writer = RunWriter::create(store, &manifest, Existing::Refuse).expect("a run starts");
    seal_run(
        writer,
        workspace.path(),
        &manifest,
        &tree.build().expect("a tree"),
        &[],
        &empty_trace(),
    )
    .expect("the run seals");
    key
}
