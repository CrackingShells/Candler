//! What the guards of the legacy import share: the mini-corpus, the reader, the import into a
//! store, a copy of a folder, a walk of a store and the projection of a stored record to the
//! oracle's plain strings.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use candler_core::import::ImportError;
use candler_core::import::assemble::import;
use candler_core::import::report::ImportReport;
use candler_core::store::Store;
use candler_harness::reader::TraceReader;
use candler_harness_claude_code::ClaudeCodeReader;
use serde_json::Value;

use crate::oracle::ExpectedRow;
use crate::support::legacy_rules;

/// The cell directory of the synthetic runs of the mini-corpus.
pub const SYNTHETIC: &str = "synthetic_cell";
/// The cell directory of runs 6 and 17 of the mini-corpus.
pub const V3: &str = "20260806-035940_v0.2.3_claude-haiku-4-5_model-claude-haiku-4-5_skill_version-htn-creator-v0.2.3";
/// The cell directory of run 12 of the mini-corpus.
pub const V6: &str = "20260806-035940_v0.2.6_claude-haiku-4-5_model-claude-haiku-4-5_skill_version-htn-creator-0.2.6";

/// The vendored mini-corpus.
#[must_use]
pub fn mini() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini-corpus")
}

/// The cell directory that holds run `replicate` in the mini-corpus, written down after listing
/// the corpus: runs 1, 2 and 3 are the synthetic cell's, 6 and 17 the v0.2.3 cell's, 12 the
/// v0.2.6 cell's.
///
/// # Panics
///
/// Panics when the mini-corpus has no such run.
#[must_use]
pub fn cell_of(replicate: u32) -> &'static str {
    match replicate {
        1..=3 => SYNTHETIC,
        6 | 17 => V3,
        12 => V6,
        other => panic!("no run {other} in the mini-corpus"),
    }
}

/// A new reader of Claude Code streams.
///
/// # Panics
///
/// Panics when the reader's identity is not valid, which the shipped one always is.
#[must_use]
// TRAIT_OBJECT: the import's factory type returns boxed readers
pub fn reader() -> Box<dyn TraceReader> {
    Box::new(ClaudeCodeReader::new().unwrap_or_else(|e| panic!("the reader builds: {e}")))
}

/// Imports `folder` into `store` with the rules of the mini-corpus and the Claude Code reader.
///
/// # Errors
///
/// Returns the import's error.
pub fn import_with_reader(folder: &Path, store: &Store) -> Result<ImportReport, ImportError> {
    import(folder, &legacy_rules::rules(), &reader, store)
}

/// Copies the directory `from` to the new directory `to`, with the modes of its files.
///
/// # Panics
///
/// Panics when a file cannot be read or written.
pub fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap_or_else(|e| panic!("create {}: {e}", to.display()));
    for entry in fs::read_dir(from).unwrap_or_else(|e| panic!("list {}: {e}", from.display())) {
        let entry = entry.unwrap_or_else(|e| panic!("entry of {}: {e}", from.display()));
        let target = to.join(entry.file_name());
        let kind = entry
            .file_type()
            .unwrap_or_else(|e| panic!("type of {}: {e}", entry.path().display()));
        if kind.is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target)
                .unwrap_or_else(|e| panic!("copy {}: {e}", entry.path().display()));
        }
    }
}

/// Every file and directory under `root`, by path relative to it: a file with its bytes, a
/// directory with none.
///
/// # Panics
///
/// Panics when a directory cannot be listed or a file read.
#[must_use]
pub fn snapshot(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("list {}: {e}", dir.display())) {
            let entry = entry.unwrap_or_else(|e| panic!("entry of {}: {e}", dir.display()));
            let path = entry.path();
            let name = path
                .strip_prefix(root)
                .unwrap_or_else(|e| panic!("{} is under the root: {e}", path.display()))
                .to_string_lossy()
                .into_owned();
            let kind = entry
                .file_type()
                .unwrap_or_else(|e| panic!("type of {}: {e}", path.display()));
            if kind.is_dir() {
                found.insert(name, None);
                pending.push(path);
            } else {
                let bytes =
                    fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
                found.insert(name, Some(bytes));
            }
        }
    }
    found
}

/// Asserts that two snapshots are equal, and says which entries are missing, extra or different
/// instead of printing every byte.
///
/// # Panics
///
/// Panics when the snapshots differ.
pub fn assert_same_store(
    left: &BTreeMap<String, Option<Vec<u8>>>,
    right: &BTreeMap<String, Option<Vec<u8>>>,
    what: &str,
) {
    let only_left: Vec<&String> = left.keys().filter(|k| !right.contains_key(*k)).collect();
    let only_right: Vec<&String> = right.keys().filter(|k| !left.contains_key(*k)).collect();
    let different: Vec<&String> = left
        .iter()
        .filter(|(name, bytes)| right.get(*name).is_some_and(|other| other != *bytes))
        .map(|(name, _)| name)
        .collect();
    assert!(
        only_left.is_empty() && only_right.is_empty() && different.is_empty(),
        "{what}: only in the first {only_left:?}; only in the second {only_right:?}; different {different:?}"
    );
}

/// The value of `key`, or a panic that names the key and the document.
#[must_use]
pub fn field<'v>(value: &'v Value, key: &str) -> &'v Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("no `{key}` in {value}"))
}

/// The string at `key`, owned.
///
/// # Panics
///
/// Panics when `key` is missing or is not a string.
#[must_use]
pub fn text_of(value: &Value, key: &str) -> String {
    // BORROW: `.to_owned()` of a `&str` for the owned result
    field(value, key)
        // BORROW: explicit .as_str() instead of Deref coercion
        .as_str()
        .unwrap_or_else(|| panic!("`{key}` is not a string in {value}"))
        // BORROW: .to_owned() makes the owned copy
        .to_owned()
}

/// A serialized outcome record as the oracle's plain strings.
///
/// # Panics
///
/// Panics when the record lacks a member of its shape.
#[must_use]
pub fn project(cell: &str, run_n: u32, record: &Value) -> ExpectedRow {
    let value = record
        .get("value")
        // BORROW: explicit `.as_str()` of the owned kind to match on it
        .map(|value| match text_of(value, "kind").as_str() {
            "text" => format!("text:{}", text_of(value, "text")),
            "count" => format!("count:{}", field(value, "count")),
            "number" => format!("number:{}", field(value, "number")),
            other => panic!("unknown value kind {other}"),
        });
    let weight = record
        .get("weight")
        .map(|weight| format!("{}/{}", field(weight, "num"), field(weight, "den")));
    let cause = record.get("cause").map(|cause| {
        if let Some(code) = cause.get("legacy") {
            // BORROW: explicit `.as_str()` of the JSON string
            format!("legacy: {}", code.as_str().expect("a legacy code"))
        } else if let Some(path) = cause.get("artifact") {
            // BORROW: explicit `.as_str()` of the JSON string
            format!("artifact: {}", path.as_str().expect("an artifact path"))
        } else {
            format!("reason: {}", text_of(cause, "reason"))
        }
    });
    ExpectedRow {
        // BORROW: `.to_owned()` makes the owned cell name
        cell: cell.to_owned(),
        run_n,
        expectation_id: text_of(record, "expectation_id"),
        role: text_of(record, "role"),
        status: text_of(record, "status"),
        value,
        weight,
        cause,
    }
}
