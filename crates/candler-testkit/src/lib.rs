//! Tree helpers for tests: one copy of each walk that the test crates used to carry.
//!
//! This crate is a dev-dependency only (`publish = false`, no release line). It depends on `std`
//! alone, and never on `candler-core`: a guard that checks the production walker or a digest
//! must keep an expectation that does not come from the code it checks.
//!
//! Every walk is recursive and iterative, lists regular files only, and does not follow or list
//! symbolic links (a link is skipped, so a test that plants one sees it left out). Relative
//! paths are written with `/`. All helpers panic with the path on an I/O error, which in a test
//! is a failure.

// A helper that fails a test panics: that is its error channel, as in the test files it serves.
#![allow(clippy::panic, reason = "a failed test helper panics")]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Calls `visit(absolute, relative)` for every regular file under `root`, in no fixed order.
fn each_file(root: &Path, mut visit: impl FnMut(&Path, String)) {
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("list {}: {e}", dir.display())) {
            let entry = entry.unwrap_or_else(|e| panic!("entry of {}: {e}", dir.display()));
            let kind = entry
                .file_type()
                .unwrap_or_else(|e| panic!("type of {}: {e}", entry.path().display()));
            let path = entry.path();
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or_else(|e| panic!("{} is under the root: {e}", path.display()))
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                visit(&path, rel);
            }
        }
    }
}

/// Copies the tree at `from` to `to`: directories are created, every other entry is copied with
/// `fs::copy` (so a file keeps its mode and a link is read through).
///
/// # Panics
///
/// Panics when a directory cannot be listed or created, or a file copied.
pub fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap_or_else(|e| panic!("create {}: {e}", to.display()));
    for entry in fs::read_dir(from).unwrap_or_else(|e| panic!("list {}: {e}", from.display())) {
        let entry = entry.unwrap_or_else(|e| panic!("entry of {}: {e}", from.display()));
        let target = to.join(entry.file_name());
        let source = entry.path();
        let kind = entry
            .file_type()
            .unwrap_or_else(|e| panic!("type of {}: {e}", source.display()));
        if kind.is_dir() {
            copy_tree(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap_or_else(|e| panic!("copy {}: {e}", source.display()));
        }
    }
}

/// Every regular file under `root` as `/`-separated relative path to its bytes.
///
/// # Panics
///
/// Panics when a directory cannot be listed or a file read.
#[must_use]
pub fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut found = BTreeMap::new();
    each_file(root, |path, rel| {
        let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        found.insert(rel, bytes);
    });
    found
}

/// Every regular file under `root` as a `/`-separated relative path, sorted.
///
/// # Panics
///
/// Panics when a directory cannot be listed.
#[must_use]
pub fn file_list(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    each_file(root, |_, rel| found.push(rel));
    found.sort();
    found
}

/// The relative paths of [`file_list`], as a set.
///
/// # Panics
///
/// Panics when a directory cannot be listed.
#[must_use]
pub fn file_set(root: &Path) -> BTreeSet<String> {
    file_list(root).into_iter().collect()
}

/// Every regular file under `root` as a path that starts with `root`, sorted.
///
/// # Panics
///
/// Panics when a directory cannot be listed.
#[must_use]
pub fn file_paths(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    each_file(root, |path, _| found.push(path.to_path_buf()));
    found.sort();
    found
}
