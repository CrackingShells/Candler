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

use candler_core::evaluation::context::{assemble, materialise};
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
#[allow(
    clippy::field_reassign_with_default,
    reason = "`Needs` is non-exhaustive, so a struct literal is not available here"
)]
fn parity_needs() -> Needs {
    let mut needs = Needs::default();
    needs.artifacts = vec!["outputs/**".into()];
    needs.metadata = vec![FieldPath::from_str(DOMAIN).expect("a field path")];
    needs
}

/// Needs with the given artifact globs and no metadata.
#[allow(
    clippy::field_reassign_with_default,
    reason = "`Needs` is non-exhaustive, so a struct literal is not available here"
)]
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
            false,
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
