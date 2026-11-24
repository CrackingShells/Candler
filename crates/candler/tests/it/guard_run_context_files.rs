//! Guards for the file rule of a run context over runs the real runner sealed: the agent leaves a
//! file set in its workspace under `candler run`, and the context a plug-in receives is built from
//! the store that run left. The runner never collects `.claude` (`EXCLUDED_DIRS` in the Claude Code
//! driver's `discovery.rs`), so the file rule's `.claude` cases are sealed in the core's guards.
#![cfg(test)]
#![cfg(unix)]

use crate::support::cli_run::{World, world};
use crate::support::seam::finish;

use std::collections::BTreeSet;
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;
use std::str::FromStr as _;

use candler_core::evaluation::context::{assemble, materialise};
use candler_core::store::Store;
use candler_schemas::evaluation::{ContextArtifact, Needs, RunContext};
use candler_schemas::primitives::{Digest, RelPath, RunKey};
use candler_testkit::file_set;

/// One row of the file-rule table: artifact globs, the files the agent leaves (each holds its own
/// path as bytes), and the hand-listed set of paths that the context carries.
struct Case {
    name: &'static str,
    globs: &'static [&'static str],
    stored: &'static [&'static str],
    kept: &'static [&'static str],
}

/// The file-rule table; the sets in `kept` are written by hand from R03's file rule.
const CASES: &[Case] = &[
    Case {
        name: "a nested match counts",
        globs: &["outputs/**"],
        stored: &["a.py", "sub/deep/b.py", "sub/c.json"],
        kept: &["a.py", "sub/c.json", "sub/deep/b.py"],
    },
    Case {
        name: "AppleDouble files are dropped",
        globs: &["outputs/**"],
        stored: &["domain.py", "._domain.py", "sub/._b.py", "sub/b.py"],
        kept: &["domain.py", "sub/b.py"],
    },
    Case {
        name: ".DS_Store is dropped",
        globs: &["outputs/**"],
        stored: &["x.py", ".DS_Store", "sub/.DS_Store"],
        kept: &["x.py"],
    },
    Case {
        name: "a hidden directory below a nested path is dropped",
        globs: &["outputs/**"],
        stored: &["sub/.hidden/y.py", "sub/z.py"],
        kept: &["sub/z.py"],
    },
    Case {
        name: "an empty outputs gives no artifact and no error",
        globs: &["outputs/**"],
        stored: &[],
        kept: &[],
    },
    Case {
        name: "a literal dot file is kept by its own glob and not by another",
        globs: &["outputs/**", "outputs/.env"],
        stored: &[".env", "a/.env", "x"],
        kept: &[".env", "x"],
    },
    Case {
        name: "a glob keeps only what it matches",
        globs: &["outputs/*.py"],
        stored: &["a.py", "sub/b.py", "c.json"],
        kept: &["a.py"],
    },
];

/// Needs with the given artifact globs and no metadata.
fn needs_of(globs: &[&str]) -> Needs {
    let mut needs = Needs::default();
    needs.artifacts = globs.iter().map(|g| (*g).to_owned()).collect();
    needs
}

/// Runs the experiment of a world whose agent leaves exactly `files` (each holding its own path as
/// bytes) through `candler run`, and returns the world, its store and the one sealed run.
fn sealed_run_with(files: &[&str]) -> (World, Store, RunKey) {
    let world = world();
    let leaves: Vec<(&str, &[u8])> = files.iter().map(|f| (*f, f.as_bytes())).collect();
    world.agent_leaves(&leaves);
    let launched = world.launch();
    finish(&world, &launched);
    world.assert_sealed(1);
    let store = Store::open(&world.store()).expect("the store opens");
    let run = store
        .runs()
        .expect("the runs list")
        .into_iter()
        .next()
        .expect("a run")
        .key;
    (world, store, run)
}

/// The context `run` gets in `workspace` under `needs`.
fn context_of(store: &Store, run: &RunKey, needs: &Needs, workspace: &Path) -> RunContext {
    let assembled = assemble(store, run, needs).expect("the run assembles");
    materialise(&assembled, workspace).expect("the run materialises")
}

/// Guards: R9 — the paths of a materialised context and the paths its digest covers are exactly the files the file rule keeps: a nested match counts, `._*` and `.DS_Store` are dropped unless a glob names them, and an empty `outputs/` gives no artifact and no error. Each run is made by `candler run` over a fake agent that leaves the case's files. Oracle: hand-listed path sets, and the digest of a context built from the listed paths and the literal bytes.
#[test]
fn the_file_rule_decides_the_paths_and_the_digest() {
    for case in CASES {
        let (_world, store, run) = sealed_run_with(case.stored);
        let workspace = tempfile::tempdir().expect("a workspace");
        let context = context_of(&store, &run, &needs_of(case.globs), workspace.path());
        let expected: BTreeSet<String> = case.kept.iter().map(|p| (*p).to_owned()).collect();
        assert_eq!(
            file_set(&workspace.path().join("outputs")),
            expected,
            "materialised files: {}",
            case.name
        );
        let artifacts: BTreeSet<String> = context
            .artifacts()
            .iter()
            .map(|a| a.path.to_string())
            .collect();
        assert_eq!(artifacts, expected, "digest path list: {}", case.name);
        let mut builder = RunContext::builder()
            .run(run.clone())
            .root("/unused".into());
        for path in case.kept {
            builder = builder.artifact(ContextArtifact::new(
                RelPath::from_str(path).expect("a path"),
                String::from("/unused"),
                Digest::of_bytes(path.as_bytes()),
            ));
        }
        assert_eq!(
            context.context_digest(),
            builder.build().expect("a context").context_digest(),
            "digest: {}",
            case.name
        );
    }
}

/// Guards: R9 and I11 — a file the agent left is materialised as a private read-only copy, not a link: a fresh inode with one link and mode `0444`, while the store blob keeps its own inode and mode `0444`. The run is made by `candler run`. Oracle: `stat` of the copy and of the blob.
#[test]
fn a_materialised_file_is_a_private_read_only_copy() {
    let (_world, store, run) = sealed_run_with(&["plain.py"]);
    let workspace = tempfile::tempdir().expect("a workspace");
    context_of(&store, &run, &needs_of(&["outputs/**"]), workspace.path());

    let copy = std::fs::metadata(workspace.path().join("outputs/plain.py")).expect("a copy");
    let blob = std::fs::metadata(store.blob_path(&Digest::of_bytes(b"plain.py"))).expect("a blob");
    assert_eq!(copy.mode() & 0o777, 0o444, "the mode of the copy");
    assert_eq!(copy.nlink(), 1, "the copy has one link");
    assert_ne!(copy.ino(), blob.ino(), "the copy is not the blob");
    assert_eq!(blob.nlink(), 1, "the blob gained no link");
    assert_eq!(blob.mode() & 0o777, 0o444, "the mode of the blob");
}
