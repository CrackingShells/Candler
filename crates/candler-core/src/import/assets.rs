//! Importing a run's artifacts and sources: every file becomes a blob, and the files under `outputs/` become the run's workspace tree.

use std::collections::BTreeSet;

use candler_schemas::execution::{RunTree, RunTreeBuilder, TreeEntry};
use candler_schemas::primitives::{RelPath, RunKey};

use super::ImportError;
use super::legacy::COLLECTED_OUTPUTS;
use super::read::{LegacyRun, SourceFile, rel_path};
use crate::store::Store;

/// What the artifacts of a run become: its workspace tree and the differences with the outputs the legacy grader collected.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ImportedAssets {
    /// The run's workspace tree: one entry per file under `outputs/` and one per empty directory.
    pub tree: RunTree,
    /// The names that `outputs/` and the timing file's list of collected outputs do not share, sorted by path.
    pub tree_diffs: Vec<TreeDiff>,
}

/// A name that only one of `outputs/` and the timing file's list of collected outputs holds, with the side that holds it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[non_exhaustive]
pub struct TreeDiff {
    /// The run.
    pub run: RunKey,
    /// The path relative to `outputs/`.
    pub path: RelPath,
    /// Which side holds the name.
    pub explanation: String,
}

impl TreeDiff {
    /// A tree difference.
    #[must_use]
    pub const fn new(run: RunKey, path: RelPath, explanation: String) -> Self {
        Self {
            run,
            path,
            explanation,
        }
    }
}

/// The error of a store that refuses the blob of `file`, a file of `run`.
fn store_error(run: &str, file: &SourceFile, error: &crate::store::StoreError) -> ImportError {
    ImportError::Store {
        // BORROW: .to_owned() makes the owned label
        run: run.to_owned(),
        detail: format!("the blob of `{}`: {error}", file.path),
    }
}

/// Stores each file of `run` as a blob under the digest it was read with.
///
/// # Errors
/// Returns [`ImportError::Store`] naming `run` and the file if the store refuses its blob, which
/// includes a file whose bytes no longer match the digest it was read with.
pub fn put_sources(store: &Store, run: &str, files: &[SourceFile]) -> Result<(), ImportError> {
    for file in files {
        store
            // BORROW: explicit .as_slice()
            .put_blob(file.bytes.as_slice(), Some(&file.sha256))
            .map_err(|e| store_error(run, file, &e))?;
    }
    Ok(())
}

/// The directory of `outputs/` of a run: the directory of its stream, then `outputs/`.
fn outputs_prefix(run: &LegacyRun) -> Result<String, ImportError> {
    // BORROW: explicit .as_str()
    let stream = run.stream.path.as_str();
    let (dir, _) = stream.rsplit_once('/').ok_or_else(|| ImportError::Layout {
        // BORROW: .to_owned() makes the owned path
        file: stream.to_owned(),
        // BORROW: .clone() copies the label into the error
        run: run.label.clone(),
        detail: "the stream is not inside a run directory".into(),
    })?;
    Ok(format!("{dir}/outputs"))
}

/// The names the timing file lists as collected outputs.
fn collected(run: &LegacyRun) -> Result<BTreeSet<String>, ImportError> {
    let not_object = || ImportError::Layout {
        file: run.timing.path.to_string(),
        // BORROW: .clone() copies the label into the error
        run: run.label.clone(),
        detail: format!("`{COLLECTED_OUTPUTS}` is not an object"),
    };
    match run.timing.value.get(COLLECTED_OUTPUTS) {
        None => Ok(BTreeSet::new()),
        Some(value) => Ok(value
            // BORROW: Value::as_object reads the list
            .as_object()
            .ok_or_else(not_object)?
            .keys()
            // BORROW: .cloned() makes the owned names
            .cloned()
            .collect()),
    }
}

/// Imports the artifacts of a run: stores every file under `outputs/` and every source of the run as a blob, and builds the workspace tree with [`plan_assets`].
///
/// A file's tree path is its path below `outputs/`; its `executable` flag is the execute bit it
/// had. An empty directory is a `dir` entry, and an empty `outputs/` gives an empty tree. A
/// `tree_diffs` entry names each collected output that `outputs/` lacks and each file of `outputs/`
/// that the timing file does not list; a timing file with no list of collected outputs lists none.
///
/// # Errors
/// Returns [`ImportError::Store`] if the store refuses a blob, and the errors of
/// [`plan_assets`].
pub fn import_assets(
    store: &Store,
    run: &LegacyRun,
    key: &RunKey,
) -> Result<ImportedAssets, ImportError> {
    // BORROW: explicit .as_str() and .as_slice()
    put_sources(store, run.label.as_str(), run.sources.as_slice())?;
    // BORROW: explicit .as_str() and .as_slice()
    put_sources(store, run.label.as_str(), run.outputs.as_slice())?;
    plan_assets(run, key)
}

/// Builds the workspace tree of a run and the differences with the outputs the legacy grader collected, and writes nothing.
///
/// # Errors
/// Returns [`ImportError::Layout`] if the timing file's list is not an object or a path is not
/// below `outputs/`, [`ImportError::Path`] for a name that is no relative path, and
/// [`ImportError::Record`] if the entries do not form a valid tree.
pub fn plan_assets(run: &LegacyRun, key: &RunKey) -> Result<ImportedAssets, ImportError> {
    let prefix = outputs_prefix(run)?;
    let below = |path: &RelPath| -> Result<String, ImportError> {
        // BORROW: explicit .as_str()
        path.as_str()
            // BORROW: explicit .as_str()
            .strip_prefix(prefix.as_str())
            .and_then(|rest| rest.strip_prefix('/'))
            // BORROW: .to_owned() makes the owned relative path
            .map(str::to_owned)
            .ok_or_else(|| ImportError::Layout {
                file: path.to_string(),
                // BORROW: .clone() copies the label into the error
                run: run.label.clone(),
                detail: format!("is not below `{prefix}/`"),
            })
    };
    let mut tree = RunTreeBuilder::default();
    let mut present = BTreeSet::new();
    for file in &run.outputs {
        let name = below(&file.path)?;
        tree = tree.entry(TreeEntry::file(
            // BORROW: explicit .as_str()
            rel_path(name.as_str())?,
            // BORROW: .clone() copies the digest into the entry
            file.sha256.clone(),
            file.executable,
        ));
        present.insert(name);
    }
    for dir in &run.empty_dirs {
        // EXPLICIT: `outputs/` itself, when empty, is the empty tree and not an entry
        // BORROW: explicit .as_str() instead of Deref coercion
        if dir.as_str() == prefix.as_str() {
            continue;
        }
        let name = below(dir)?;
        // BORROW: explicit .as_str()
        tree = tree.entry(TreeEntry::dir(rel_path(name.as_str())?));
    }
    let tree = tree.build().map_err(|e| ImportError::Record {
        // BORROW: .clone() copies the path into the error
        file: prefix.clone(),
        // BORROW: .clone() copies the label into the error
        run: run.label.clone(),
        detail: e.to_string(),
    })?;
    let listed = collected(run)?;
    let mut tree_diffs = Vec::new();
    for name in listed.difference(&present) {
        tree_diffs.push(TreeDiff::new(
            // BORROW: .clone() gives the diff its own key
            key.clone(),
            // BORROW: explicit .as_str()
            rel_path(name.as_str())?,
            format!("`{name}` is listed in `{COLLECTED_OUTPUTS}` of the timing file, which holds it; `outputs/` has no such file"),
        ));
    }
    for name in present.difference(&listed) {
        tree_diffs.push(TreeDiff::new(
            // BORROW: .clone() gives the diff its own key
            key.clone(),
            // BORROW: explicit .as_str()
            rel_path(name.as_str())?,
            format!("`{name}` is a file of `outputs/`, which holds it; `{COLLECTED_OUTPUTS}` of the timing file does not list it"),
        ));
    }
    tree_diffs.sort();
    Ok(ImportedAssets { tree, tree_diffs })
}
