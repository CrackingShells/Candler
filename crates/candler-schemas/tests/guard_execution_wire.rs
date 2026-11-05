#![cfg(test)]
//! The wire form of the execution records: objects only, never a list, and the structural rules of
//! each record refuse what R01 forbids.

use candler_schemas::execution::{RunTree, TreeEntry};
use candler_schemas::primitives::{Digest, RelPath};
use serde_json::{Value, json};
use std::str::FromStr;

fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

fn message<T>(result: Result<T, serde_json::Error>) -> String {
    match result {
        Ok(_) => String::from("accepted"),
        Err(error) => error.to_string(),
    }
}

fn file(path: &str, executable: bool) -> Value {
    json!({"kind": "file", "path": path, "sha256": digest_text('a'), "executable": executable})
}

fn dir(path: &str) -> Value {
    json!({"kind": "dir", "path": path})
}

fn tree(entries: &[Value]) -> Value {
    json!({"schema": "run-tree/1", "entries": entries})
}

/// A sorted tree: an empty directory, a nested file and an executable file.
fn base_tree() -> Vec<Value> {
    vec![dir("a-empty"), file("src/main.rs", false), file("z", true)]
}

/// Guards: I17 — a run tree holds run-relative paths in sorted order, a path is listed once, no entry lies below a file or below a directory listed as empty, and the builder and the reader refuse the same. Oracle: R01 §Workspace tree, worked as a table of hand-written rows around the sorted base.
#[test]
fn run_tree_refuses_overlaps_and_paths_that_are_not_relative() {
    assert_eq!(
        message(serde_json::from_value::<RunTree>(tree(&base_tree()))),
        "accepted"
    );

    // Each row: what to insert at which index, and what the reader says.
    let rows: [(&str, usize, Value, &str); 6] = [
        ("repeat", 1, dir("a-empty"), "repeats an entry"),
        (
            "under a file",
            3,
            file("z/inner", false),
            "lies below the file",
        ),
        (
            "below an empty directory",
            1,
            file("a-empty/x", false),
            "listed as empty",
        ),
        ("absolute", 3, file("/etc/passwd", false), "/etc/passwd"),
        ("parent segment", 0, file("../x", false), "../x"),
        ("backslash", 3, dir("z\\y"), "backslash"),
    ];
    for (name, at, entry, needle) in rows {
        let mut entries = base_tree();
        entries.insert(at, entry);
        let text = message(serde_json::from_value::<RunTree>(tree(&entries)));
        assert!(text.contains(needle), "{name}: {text}");
    }

    // The file `a` and `a/c` are not neighbours in sorted order ("a-b" lies between them).
    let far = [file("a", false), file("a-b", false), file("a/c", false)];
    let text = message(serde_json::from_value::<RunTree>(tree(&far)));
    assert!(text.contains("lies below the file"), "{text}");

    let mut unsorted = base_tree();
    unsorted.swap(0, 2);
    let text = message(serde_json::from_value::<RunTree>(tree(&unsorted)));
    assert!(text.contains("not sorted"), "{text}");

    // The builder sorts what it is given and refuses the same overlaps.
    let digest = Digest::from_str(digest_text('a').as_str()).expect("digest"); // BORROW: explicit .as_str()
    let path = |text: &str| RelPath::from_str(text).expect("path");
    let built = RunTree::builder()
        .entry(TreeEntry::file(path("z"), digest.clone(), true)) // BORROW: .clone() reuses the digest
        .entry(TreeEntry::dir(path("a-empty")))
        .entry(TreeEntry::file(path("src/main.rs"), digest.clone(), false)) // BORROW: .clone() reuses the digest
        .build()
        .expect("builds");
    assert_eq!(
        built,
        serde_json::from_value::<RunTree>(tree(&base_tree())).expect("reads")
    );
    assert_eq!(
        serde_json::to_value(&built).expect("writes"),
        tree(&base_tree())
    );

    let under_file = RunTree::builder()
        .entry(TreeEntry::file(path("a"), digest.clone(), false)) // BORROW: .clone() reuses the digest
        .entry(TreeEntry::file(path("a-b"), digest.clone(), false)) // BORROW: .clone() reuses the digest
        .entry(TreeEntry::file(path("a/c"), digest.clone(), false)) // BORROW: .clone() reuses the digest
        .build();
    assert!(under_file.is_err());
    let under_dir = RunTree::builder()
        .entry(TreeEntry::dir(path("d")))
        .entry(TreeEntry::file(path("d/f"), digest.clone(), false)) // BORROW: .clone() reuses the digest
        .build();
    assert!(under_dir.is_err());
    let repeat = RunTree::builder()
        .entry(TreeEntry::dir(path("d")))
        .entry(TreeEntry::dir(path("d")))
        .build();
    assert!(repeat.is_err());
}

/// Guards: I18 — every struct and tagged enum of the execution records is read from an object only; a list with the right values in field order is refused, and the object form is the positive control. Oracle: the serde derive accepts a list for a struct by default (L0 wire rules), so each row is a hand-written list.
#[test]
fn execution_records_refuse_the_list_form() {
    let sha = digest_text('a');
    let rows: Vec<(&str, Value, Value)> = vec![
        (
            "RunTree",
            tree(&base_tree()),
            json!(["run-tree/1", base_tree()]),
        ),
        (
            "TreeEntry file",
            file("z", true),
            json!(["file", "z", sha, true]),
        ),
        ("TreeEntry dir", dir("d"), json!(["dir", "d"])),
    ];
    for (name, object, list) in rows {
        let (accepted, refused) = match name {
            "RunTree" => (
                message(serde_json::from_value::<RunTree>(object)),
                message(serde_json::from_value::<RunTree>(list)),
            ),
            _ => (
                message(serde_json::from_value::<TreeEntry>(object)),
                message(serde_json::from_value::<TreeEntry>(list)),
            ),
        };
        assert_eq!(accepted, "accepted", "{name} object form");
        assert_ne!(refused, "accepted", "{name} list form");
    }
}
