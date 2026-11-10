//! Guards for the run context a plug-in receives: the digest of what was materialised, the file
//! rule that chooses the artifacts, and the isolation of the workspace from the store. The digest
//! vectors come from the independent Python generator `gen_live_context.py`; the path sets are
//! written out by hand below.
#![cfg(test)]

#[path = "support/legacy_import.rs"]
mod legacy_import;
#[path = "support/legacy_rules.rs"]
mod legacy_rules;
#[path = "oracle/mod.rs"]
mod oracle;

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use candler_core::evaluation::context::{ContextError, assemble, materialise};
use candler_core::store::Store;
use candler_core::store::run::{Existing, RunWriter};
use candler_schemas::evaluation::{ContextArtifact, Needs, RunContext};
use candler_schemas::execution::{RunManifest, RunTree, TreeEntry};
use candler_schemas::primitives::{Digest, FieldPath, RelPath, RunKey};
use legacy_import::{cell_of, copy_tree, import_with_reader, mini};
use sha2::{Digest as _, Sha256};

/// The metadata path every suite entry of the parity suite needs.
const DOMAIN: &str = "plan.prompt_domain_name";

/// The needs of the parity suite: every output, and the domain name.
fn parity_needs() -> Needs {
    let mut needs = Needs::default();
    needs.artifacts = vec!["outputs/**".into()];
    needs.metadata = vec![FieldPath::from_str(DOMAIN).expect("a field path")];
    needs
}

/// Needs with the given artifact globs and no metadata.
fn needs_of(globs: &[&str]) -> Needs {
    let mut needs = Needs::default();
    needs.artifacts = globs.iter().map(|g| (*g).to_owned()).collect();
    needs
}

/// A store that holds the import of the mini-corpus, and the directory that keeps it alive.
fn mini_store(corpus: &Path) -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(dir.path()).expect("the store initialises");
    import_with_reader(corpus, &store).expect("the corpus imports");
    (dir, store)
}

/// The sealed run of `replicate` in the store.
fn key_of(store: &Store, replicate: u32) -> RunKey {
    store
        .runs()
        .expect("the runs list")
        .into_iter()
        .find(|entry| entry.key.replicate() == replicate)
        .unwrap_or_else(|| panic!("no run {replicate} in the store"))
        .key
}

/// The lowercase hex of the SHA-256 of `bytes`, taken here and not from the store.
fn sha256_of(bytes: &[u8]) -> Digest {
    let mut hex = String::new();
    for byte in Sha256::digest(bytes) {
        hex.extend(char::from_digit(u32::from(byte >> 4), 16));
        hex.extend(char::from_digit(u32::from(byte & 0xf), 16));
    }
    Digest::from_hex(&hex).expect("a digest")
}

/// One file of a synthetic `outputs/` tree.
struct File {
    path: &'static str,
    bytes: &'static [u8],
    executable: bool,
}

/// Seals a new run beside `template`, which is the manifest it copies, with exactly the files
/// `files` under `outputs/`, and returns its key. The store holds each file as a blob.
fn seal_synthetic(store: &Store, template: &RunKey, attempt: u32, files: &[File]) -> RunKey {
    let mut manifest = serde_json::to_value(store.read_manifest(template).expect("the manifest"))
        .expect("a manifest serialises");
    *manifest
        .pointer_mut("/run/attempt")
        .expect("the manifest names an attempt") = attempt.into();
    let manifest: RunManifest = serde_json::from_value(manifest).expect("a manifest reads");
    let mut tree = RunTree::builder();
    for file in files {
        let digest = store.put_blob(file.bytes, None).expect("a blob stores");
        tree = tree.entry(TreeEntry::file(
            RelPath::from_str(file.path).expect("a path"),
            digest,
            file.executable,
        ));
    }
    let key = manifest.run().clone();
    let mut writer = RunWriter::create(store, &manifest, Existing::Refuse).expect("a run starts");
    writer
        .tree(&tree.build().expect("a tree"))
        .expect("the tree writes");
    writer.done().expect("the run seals");
    key
}

/// Every file below `dir`, by path relative to it with `/` separators.
fn files_below(dir: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in fs::read_dir(&next).expect("a directory lists") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let rel = path.strip_prefix(dir).expect("below the directory");
                found.insert(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found
}

/// The string `path` of every artifact of a context.
fn artifact_paths(context: &RunContext) -> BTreeSet<String> {
    context
        .artifacts()
        .iter()
        .map(|a| a.path.to_string())
        .collect()
}

/// The vector table, as `toml::Table`.
fn live_vectors() -> toml::Table {
    let text = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle/vectors/live_context_runs.toml"),
    )
    .expect("the vectors read");
    text.parse().expect("the vectors parse")
}

/// A text field of a vector.
fn text_at<'t>(vector: &'t toml::Value, key: &str) -> &'t str {
    vector
        .get(key)
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| panic!("the vector lacks `{key}`"))
}

/// The replicate `N` of a vector's `<config>/run-<N>`.
fn replicate_of(run: &str) -> u32 {
    run.rsplit_once("run-")
        .and_then(|(_, number)| number.parse().ok())
        .unwrap_or_else(|| panic!("`{run}` is not `<config>/run-<N>`"))
}

/// The digest of the context that `run` of `store` gets in a fresh workspace.
fn context_of(store: &Store, run: &RunKey, needs: &Needs, workspace: &Path) -> RunContext {
    let assembled = assemble(store, run, needs).expect("the run assembles");
    materialise(&assembled, workspace).expect("the run materialises")
}

/// Guards: R9 — the digest of a materialised live context equals the independent vector of the run's raw `outputs/` files and `experiment.json`, and a twin whose `domain.py` differs by one byte has the vector of the twin, which is not the original's. Oracle: `live_context_runs.toml`, written by `gen_live_context.py` with python `rfc8785` from the raw corpus.
#[test]
fn the_live_context_digest_is_the_independent_vector() {
    let vectors = live_vectors();
    let runs = vectors
        .get("run")
        .and_then(toml::Value::as_array)
        .expect("the vectors list runs");
    assert_eq!(runs.len(), 3, "vector runs");
    let twin = vectors.get("twin").expect("the vectors hold a twin");

    let (_keep, store) = mini_store(&mini());
    let mut base_digest = None;
    for vector in runs {
        let (cell, run, digest) = (
            text_at(vector, "cell"),
            text_at(vector, "run"),
            text_at(vector, "digest"),
        );
        let replicate = replicate_of(run);
        assert_eq!(cell_of(replicate), cell, "the cell of {run}");
        let workspace = tempfile::tempdir().expect("a workspace");
        let context = context_of(
            &store,
            &key_of(&store, replicate),
            &parity_needs(),
            workspace.path(),
        );
        assert!(!context.artifacts().is_empty(), "{run} has artifacts");
        assert_eq!(context.context_digest().to_string(), digest, "{run}");
        if run == text_at(twin, "run") {
            base_digest = Some(digest.to_owned());
        }
    }

    // The twin: the same corpus with one byte of one output of run 6 changed.
    let copy = tempfile::tempdir().expect("a corpus copy");
    copy_tree(&mini(), copy.path());
    let target = copy
        .path()
        .join(text_at(twin, "cell"))
        .join("_run_cache/with_skill/run-6/outputs")
        .join(text_at(twin, "file"));
    let mut bytes = fs::read(&target).expect("the output reads");
    assert_eq!(
        twin.get("xor_first_byte").and_then(toml::Value::as_integer),
        Some(1)
    );
    let first = bytes.first_mut().expect("a non-empty output");
    *first ^= 1;
    fs::write(&target, &bytes).expect("the twin output writes");
    let (_keep_twin, twin_store) = mini_store(copy.path());
    let workspace = tempfile::tempdir().expect("a workspace");
    let twin_context = context_of(
        &twin_store,
        &key_of(&twin_store, replicate_of(text_at(twin, "run"))),
        &parity_needs(),
        workspace.path(),
    );
    let twin_digest = twin_context.context_digest().to_string();
    assert_eq!(twin_digest, text_at(twin, "digest"), "the twin");
    assert_ne!(
        Some(twin_digest),
        base_digest,
        "one byte changes the digest"
    );
}

/// One row of the file-rule table: artifact globs, the files the run stored, and the hand-listed
/// set of paths that the context carries.
struct Case {
    name: &'static str,
    globs: &'static [&'static str],
    stored: &'static [File],
    kept: &'static [&'static str],
}

/// Shorthand for a stored file whose bytes are its own path.
const fn file(path: &'static str) -> File {
    File {
        path,
        bytes: path.as_bytes(),
        executable: false,
    }
}

/// The file-rule table; the sets in `kept` are written by hand from R03's file rule.
const CASES: &[Case] = &[
    Case {
        name: "a nested match counts",
        globs: &["outputs/**"],
        stored: &[file("a.py"), file("sub/deep/b.py"), file("sub/c.json")],
        kept: &["a.py", "sub/c.json", "sub/deep/b.py"],
    },
    Case {
        name: ".claude is dropped, at any depth",
        globs: &["outputs/**"],
        stored: &[
            file("domain.py"),
            file(".claude/x.py"),
            file(".claude/deep/y.py"),
            file("sub/.claude/z.py"),
        ],
        kept: &["domain.py"],
    },
    Case {
        name: "AppleDouble files are dropped",
        globs: &["outputs/**"],
        stored: &[
            file("domain.py"),
            file("._domain.py"),
            file("sub/._b.py"),
            file("sub/b.py"),
        ],
        kept: &["domain.py", "sub/b.py"],
    },
    Case {
        name: ".DS_Store is dropped",
        globs: &["outputs/**"],
        stored: &[file("x.py"), file(".DS_Store"), file("sub/.DS_Store")],
        kept: &["x.py"],
    },
    Case {
        name: "a hidden directory below a nested path is dropped",
        globs: &["outputs/**"],
        stored: &[file("sub/.hidden/y.py"), file("sub/z.py")],
        kept: &["sub/z.py"],
    },
    Case {
        name: "an empty outputs gives no artifact and no error",
        globs: &["outputs/**"],
        stored: &[],
        kept: &[],
    },
    Case {
        name: "a glob that names the component keeps it, and only what it names",
        globs: &["outputs/.claude/**"],
        stored: &[
            file("domain.py"),
            file(".claude/x.py"),
            file(".claude/.hid"),
        ],
        kept: &[".claude/x.py"],
    },
    Case {
        name: "a literal dot file is kept by its own glob and not by another",
        globs: &["outputs/**", "outputs/.env"],
        stored: &[file(".env"), file("a/.env"), file("x")],
        kept: &[".env", "x"],
    },
    Case {
        name: "a glob keeps only what it matches",
        globs: &["outputs/*.py"],
        stored: &[file("a.py"), file("sub/b.py"), file("c.json")],
        kept: &["a.py"],
    },
];

/// Guards: R9 — the paths of a materialised context and the paths its digest covers are exactly the files the file rule keeps: a nested match counts, `.claude/`, `._*` and `.DS_Store` are dropped unless a glob names them, and an empty `outputs/` gives no artifact and no error. Oracle: hand-listed path sets, and the digest of a context built from the listed paths and the literal bytes.
#[test]
fn the_file_rule_decides_the_paths_and_the_digest() {
    let (_keep, store) = mini_store(&mini());
    let template = key_of(&store, 12);
    for (n, case) in (2_u32..).zip(CASES) {
        let run = seal_synthetic(&store, &template, n, case.stored);
        let workspace = tempfile::tempdir().expect("a workspace");
        let context = context_of(&store, &run, &needs_of(case.globs), workspace.path());
        let expected: BTreeSet<String> = case.kept.iter().map(|p| (*p).to_owned()).collect();
        assert_eq!(
            files_below(&workspace.path().join("outputs")),
            expected,
            "materialised files: {}",
            case.name
        );
        assert_eq!(
            artifact_paths(&context),
            expected,
            "digest path list: {}",
            case.name
        );
        // The digest is that of exactly the listed paths with the hashes of their literal bytes.
        let mut builder = RunContext::builder()
            .run(run.clone())
            .root("/unused".into());
        for path in case.kept {
            builder = builder.artifact(ContextArtifact::new(
                RelPath::from_str(path).expect("a path"),
                String::from("/unused"),
                sha256_of(path.as_bytes()),
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

/// Guards: R9 and I11 — a plug-in that appends to, or overwrites, a materialised file, after making it writable as a careless one would, leaves every blob of the store with its recorded sha256. Oracle: the sha256 the store's tree records for each blob, and a read of the blob's bytes taken here.
#[test]
fn a_plug_in_cannot_write_through_to_the_store() {
    let (_keep, store) = mini_store(&mini());
    let run = key_of(&store, 6);
    let workspace = tempfile::tempdir().expect("a workspace");
    let context = context_of(&store, &run, &parity_needs(), workspace.path());
    assert!(context.artifacts().len() >= 3, "the run has artifacts");

    let tree = store.read_tree(&run).expect("the tree reads");
    let mut recorded: Vec<(PathBuf, RelPath, Digest)> = Vec::new();
    for entry in tree.entries() {
        if let TreeEntry::File { path, sha256, .. } = entry {
            recorded.push((store.blob_path(sha256), path.clone(), sha256.clone()));
        }
    }
    let before: Vec<(Vec<u8>, u32)> = recorded
        .iter()
        .map(|(blob, _, _)| {
            let meta = fs::metadata(blob).expect("a blob");
            (fs::read(blob).expect("a blob reads"), meta.mode())
        })
        .collect();

    for (blob, path, sha) in &recorded {
        let copy = workspace.path().join("outputs").join(path.as_str());
        assert_eq!(
            fs::read(&copy).expect("a copy reads"),
            fs::read(blob).expect("a blob reads"),
            "the copy of {path} holds the blob's bytes"
        );
        assert_eq!(&sha256_of(&fs::read(blob).expect("a blob reads")), sha);
        // A careless plug-in makes the file writable, appends, then overwrites the head.
        fs::set_permissions(&copy, fs::Permissions::from_mode(0o644)).expect("chmod u+w");
        let mut append = OpenOptions::new().append(true).open(&copy).expect("opens");
        append.write_all(b"\nTAMPERED\n").expect("appends");
        drop(append);
        let mut head = OpenOptions::new().write(true).open(&copy).expect("opens");
        head.write_all(b"XXXX").expect("overwrites");
    }

    for ((blob, path, sha), (bytes, mode)) in recorded.iter().zip(&before) {
        let after = fs::read(blob).expect("a blob reads");
        assert_eq!(&sha256_of(&after), sha, "the store blob of {path}");
        assert_eq!(&after, bytes, "the bytes of the store blob of {path}");
        assert_eq!(
            fs::metadata(blob).expect("a blob").mode(),
            *mode,
            "the mode of the store blob of {path}"
        );
    }
}

/// Guards: R9 — the digest of a materialised context is taken from the bytes written to the workspace, and a store blob that no longer holds the bytes of its recorded sha256 is refused with `CorruptBlob` and no context. Oracle: a blob of the store overwritten here with other bytes, so the recorded sha256 and the written bytes differ.
#[test]
fn a_corrupt_blob_is_refused_and_not_digested() {
    let (_keep, store) = mini_store(&mini());
    let run = key_of(&store, 6);
    let tree = store.read_tree(&run).expect("the tree reads");
    let victim = tree
        .entries()
        .iter()
        .find_map(|entry| {
            if let TreeEntry::File { path, sha256, .. } = entry {
                Some((path.clone(), sha256.clone()))
            } else {
                None
            }
        })
        .expect("the run stores a file");
    let blob = store.blob_path(&victim.1);
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o644)).expect("chmod u+w");
    fs::write(&blob, b"not the recorded bytes").expect("the blob is overwritten");

    let workspace = tempfile::tempdir().expect("a workspace");
    let assembled = assemble(&store, &run, &parity_needs()).expect("the run assembles");
    let outcome = materialise(&assembled, workspace.path());
    assert!(
        matches!(outcome, Err(ContextError::CorruptBlob { .. })),
        "a corrupt blob must be refused, got {outcome:?}"
    );
}

/// Guards: R9 and I11 — every materialised file is a private copy, not a link: a fresh inode with one link, mode `0444`, or `0555` when the tree marks it executable, while the store blob keeps its own inode and mode. Oracle: `stat` of the copy and of the blob, and the executable bit written into the synthetic tree here.
#[test]
fn a_materialised_file_is_a_private_read_only_copy() {
    let (_keep, store) = mini_store(&mini());
    let template = key_of(&store, 12);
    let stored = [
        file("plain.py"),
        File {
            path: "run.sh",
            bytes: b"#!/bin/sh\n",
            executable: true,
        },
    ];
    let run = seal_synthetic(&store, &template, 2, &stored);
    let workspace = tempfile::tempdir().expect("a workspace");
    context_of(&store, &run, &needs_of(&["outputs/**"]), workspace.path());

    for (name, mode) in [("plain.py", 0o444_u32), ("run.sh", 0o555_u32)] {
        let copy = fs::metadata(workspace.path().join("outputs").join(name)).expect("a copy");
        let digest =
            sha256_of(&fs::read(workspace.path().join("outputs").join(name)).expect("reads"));
        let blob = fs::metadata(store.blob_path(&digest)).expect("a blob");
        assert_eq!(copy.mode() & 0o777, mode, "the mode of {name}");
        assert_eq!(copy.nlink(), 1, "the copy of {name} has one link");
        assert_ne!(copy.ino(), blob.ino(), "the copy of {name} is not the blob");
        assert_eq!(blob.nlink(), 1, "the blob of {name} gained no link");
        assert_eq!(blob.mode() & 0o777, 0o444, "the mode of the blob of {name}");
    }
}

/// Guards: I11 — a workspace that is, or lies above, the store root is refused before any directory is made, so a plug-in cannot reach the store through its own workspace. Oracle: the parent of the store root, and the store root itself, handed in as workspaces.
#[test]
fn a_workspace_that_contains_the_store_is_refused() {
    let holder = tempfile::tempdir().expect("a holder directory");
    let root = holder.path().join("store");
    fs::create_dir(&root).expect("a store directory");
    let store = Store::init(&root).expect("the store initialises");
    import_with_reader(&mini(), &store).expect("the corpus imports");
    let run = key_of(&store, 6);
    let assembled = assemble(&store, &run, &parity_needs()).expect("the run assembles");
    for workspace in [holder.path(), root.as_path()] {
        let outcome = materialise(&assembled, workspace);
        assert!(
            matches!(outcome, Err(ContextError::Workspace { .. })),
            "{} must be refused, got {outcome:?}",
            workspace.display()
        );
    }
    assert!(
        !holder.path().join("outputs").exists(),
        "no outputs/ was made above the store"
    );
}

/// The vectors of the context that serves inputs, as `toml::Table`.
fn input_vectors() -> toml::Table {
    let text = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle/vectors/context_inputs.toml"),
    )
    .expect("the vectors read");
    text.parse().expect("the vectors parse")
}

/// The array of tables `key` of `value`.
fn tables_at<'t>(value: &'t toml::Value, key: &str) -> &'t [toml::Value] {
    value
        .get(key)
        .and_then(toml::Value::as_array)
        .unwrap_or_else(|| panic!("the vector lacks `{key}`"))
}

/// One input of a vector, as the fabricated store holds it: its kind, the manifest list that
/// names it, its digest, and its files by path and content.
struct HeldInput {
    kind: String,
    in_evaluation_inputs: bool,
    digest: Digest,
    files: Vec<(String, String)>,
}

/// Stores the files and the list blob of every input of `vector` as the campaign's input-blob
/// layout says, and returns what was stored. The list blob's bytes are the vector's `list_json`.
fn store_inputs(store: &Store, vector: &toml::Value) -> Vec<HeldInput> {
    let mut held = Vec::new();
    for input in tables_at(vector, "input") {
        let files: Vec<(String, String)> = tables_at(input, "file")
            .iter()
            .map(|f| {
                (
                    text_at(f, "path").to_owned(),
                    text_at(f, "content").to_owned(),
                )
            })
            .collect();
        for (_, content) in &files {
            store
                .put_blob(content.as_bytes(), None)
                .expect("a file blob stores");
        }
        let digest = store
            .put_blob(text_at(input, "list_json").as_bytes(), None)
            .expect("the list blob stores");
        assert_eq!(digest.to_string(), text_at(input, "digest"), "the list key");
        held.push(HeldInput {
            kind: text_at(input, "kind").to_owned(),
            in_evaluation_inputs: text_at(input, "list") == "evaluation",
            digest,
            files,
        });
    }
    held
}

/// Seals a run beside `template` whose manifest names `comparable` in `comparable.inputs` and
/// `evaluation` in `evaluation_inputs`, as `(kind, digest)` pairs, and has no outputs.
fn seal_with_inputs(
    store: &Store,
    template: &RunKey,
    attempt: u32,
    comparable: &[(String, String)],
    evaluation: &[(String, String)],
) -> RunKey {
    let refs = |list: &[(String, String)]| -> serde_json::Value {
        let mut sorted = list.to_vec();
        sorted.sort();
        sorted
            .iter()
            .map(|(kind, digest)| serde_json::json!({"kind": kind, "digest": digest}))
            .collect()
    };
    let mut manifest = serde_json::to_value(store.read_manifest(template).expect("the manifest"))
        .expect("a manifest serialises");
    *manifest
        .pointer_mut("/run/attempt")
        .expect("the manifest names an attempt") = attempt.into();
    *manifest
        .pointer_mut("/comparable/inputs")
        .expect("the manifest has inputs") = refs(comparable);
    if !evaluation.is_empty() {
        manifest
            .as_object_mut()
            .expect("a manifest is an object")
            .insert("evaluation_inputs".into(), refs(evaluation));
    }
    // The key names the digest of the cell, which the new inputs change: read it from a probe.
    let probe: RunManifest = serde_json::from_value(manifest.clone()).expect("a manifest reads");
    *manifest
        .pointer_mut("/run/cell")
        .expect("the manifest names a cell") = probe.cell_digest().to_string().into();
    let manifest: RunManifest = serde_json::from_value(manifest).expect("a manifest reads");
    let key = manifest.run().clone();
    let mut writer = RunWriter::create(store, &manifest, Existing::Refuse).expect("a run starts");
    writer
        .tree(&RunTree::builder().build().expect("a tree"))
        .expect("the tree writes");
    writer.done().expect("the run seals");
    key
}

/// Needs that name only the input `kinds`.
fn input_needs(kinds: &[&str]) -> Needs {
    let mut needs = Needs::default();
    needs.inputs = kinds
        .iter()
        .map(|k| k.parse().expect("a plain name"))
        .collect();
    needs
}

/// The `(kind, digest)` pairs of the held inputs that live in the chosen manifest list.
fn pairs(held: &[HeldInput], evaluation: bool) -> Vec<(String, String)> {
    held.iter()
        .filter(|h| h.in_evaluation_inputs == evaluation)
        .map(|h| (h.kind.clone(), h.digest.to_string()))
        .collect()
}

/// Guards: R01 §Evaluator protocol and I8 — a need for the kinds `servers` (in `comparable.inputs`) and `truth` (in `evaluation_inputs`) gives one `{kind, path, digest}` per kind; each `path` is a tree whose files carry the bytes of their blobs and cannot be written; the input digests are the keys of the list blobs; and the request's `context_digest` is the independent value that covers `{kind, digest}` of exactly the inputs served, with `build_inputs` and the unserved kinds left out. Oracle: `context_inputs.toml`, written by `gen_context_inputs.py` with python `rfc8785`; file contents are the vector's literals (`uv run --with rfc8785 python crates/candler-core/tests/oracle/vectors/gen_context_inputs.py --check`).
#[test]
fn needs_inputs_are_served_from_both_manifest_lists() {
    let (_keep, store) = mini_store(&mini());
    let template = key_of(&store, 6);
    let vectors = input_vectors();
    let mut served_cases = 0_u32;
    for (n, vector) in (2_u32..).zip(tables_at(&toml::Value::Table(vectors), "vector")) {
        let held = store_inputs(&store, vector);
        let run = seal_with_inputs(
            &store,
            &template,
            n,
            &pairs(&held, false),
            &pairs(&held, true),
        );
        let wanted: Vec<&str> = tables_at(vector, "needs")
            .iter()
            .map(|k| k.as_str().expect("a kind"))
            .collect();
        let workspace = tempfile::tempdir().expect("a workspace");
        let context = context_of(&store, &run, &input_needs(&wanted), workspace.path());
        let title = text_at(vector, "title");

        let mut served: Vec<&str> = context.inputs().iter().map(|i| i.kind.as_str()).collect();
        served.sort_unstable();
        let mut expected_kinds = wanted.clone();
        expected_kinds.sort_unstable();
        assert_eq!(served, expected_kinds, "the kinds served: {title}");

        for entry in context.inputs() {
            let input = held
                .iter()
                .find(|h| h.kind == entry.kind.as_str())
                .expect("a held input");
            assert_eq!(
                entry.digest, input.digest,
                "digest of {}: {title}",
                input.kind
            );
            let tree = Path::new(&entry.path);
            assert!(
                tree.is_absolute() && tree.is_dir(),
                "a tree: {}",
                entry.path
            );
            let expected: BTreeSet<String> = input.files.iter().map(|(p, _)| p.clone()).collect();
            assert_eq!(
                files_below(tree),
                expected,
                "files of {}: {title}",
                input.kind
            );
            for (path, content) in &input.files {
                let file = tree.join(path);
                assert_eq!(
                    fs::read(&file).expect("a file reads"),
                    content.as_bytes(),
                    "bytes of {path}: {title}"
                );
                let mode = fs::metadata(&file).expect("a file").mode();
                assert_eq!(mode & 0o222, 0, "{path} is read-only: {title}");
            }
        }
        assert_eq!(
            context.context_digest().to_string(),
            text_at(vector, "context_digest"),
            "the context digest: {title}"
        );
        served_cases += 1;
    }
    assert_eq!(served_cases, 3, "the vectors the guard ran");
}

/// Guards: R01 §Evaluator protocol — a need that the run cannot meet is refused, naming the kind and the reason: a kind the manifest holds in neither list, the kind `build_inputs` even where the manifest holds it, and a kind whose digest has no list blob in the store, as on an imported run; no need is served as an empty list. Oracle: hand-written; the kinds and the reasons are the ones written below.
#[test]
fn needs_inputs_refused_when_the_run_cannot_serve_them() {
    let (_keep, store) = mini_store(&mini());
    let template = key_of(&store, 6);
    let list_blob = store
        .put_blob(b"[]", None)
        .expect("a blob stores")
        .to_string();
    let imported = sha256_of(b"an imported input").to_string();
    let run = seal_with_inputs(
        &store,
        &template,
        2,
        &[
            ("build_inputs".into(), list_blob),
            ("imported".into(), imported),
        ],
        &[],
    );
    let cases = [
        ("ghost", "is not an input of the run"),
        ("build_inputs", "is reserved for the image"),
        ("imported", "has no list blob in the store"),
    ];
    for (kind, reason) in cases {
        let error = assemble(&store, &run, &input_needs(&[kind]))
            .expect_err("a need the run cannot meet is refused");
        let text = error.to_string();
        assert!(
            text.contains(&format!("`{kind}`")) && text.contains(reason),
            "{kind}: `{text}` names the kind and `{reason}`"
        );
        assert!(
            !text.contains("unsupported"),
            "{kind}: `{text}` is a refusal, not an unsupported need"
        );
    }
}

/// Guards: R01 §Evaluator protocol — an input whose list blob does not hold the list its digest
/// names is refused as a non-canonical list, naming the kind, and is never served: the blob under
/// the digest key holds a valid list of an existing file with another sha256 than the one the
/// digest was made of. Oracle: hand-written; the two canonical list texts are written out below,
/// and the reason is the one text the refusal gives.
#[test]
fn needs_inputs_refused_when_the_list_blob_is_not_its_digests_list() {
    let (_keep, store) = mini_store(&mini());
    let template = key_of(&store, 6);
    let list_of = |content: &str| -> String {
        let file = store
            .put_blob(content.as_bytes(), None)
            .expect("a file blob stores");
        format!(r#"[{{"path":"a.txt","sha256":"{file}"}}]"#)
    };
    let digest = store
        .put_blob(list_of("one\n").as_bytes(), None)
        .expect("the list blob stores");
    let run = seal_with_inputs(
        &store,
        &template,
        2,
        &[("doc".into(), digest.to_string())],
        &[],
    );
    // The served list is intact before the overwrite, so the refusal below is the overwrite's.
    let workspace = tempfile::tempdir().expect("a workspace");
    context_of(&store, &run, &input_needs(&["doc"]), workspace.path());

    let blob = store.layout().blob(&digest);
    fs::remove_file(&blob).expect("the list blob is removed");
    fs::write(&blob, list_of("two\n")).expect("the list blob is overwritten");
    let error = assemble(&store, &run, &input_needs(&["doc"]))
        .expect_err("a list that is not its digest's is refused");
    let text = error.to_string();
    assert!(
        text.contains("`doc`") && text.contains("is not the canonical list of its digest"),
        "`{text}` names the kind and the non-canonical list"
    );
}
