//! Guards for the platform seam: the source files of the workspace that call unix-only APIs are
//! frozen in `vectors/platform_inventory.toml`, each with its Windows counterpart. The list is
//! the checklist of a native Windows port, so it equals the code in both directions.
#![cfg(test)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The text of an API path that only a unix target has.
const UNIX_MARKERS: [&str; 3] = ["std::os::unix", "rustix::", "libc::"];

/// The workspace root, two levels above this crate.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `.rs` file under `dir`, as workspace-relative paths with `/` separators.
fn rust_files_under(root: &Path, dir: &Path, found: &mut BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files_under(root, &path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.insert(relative(root, &path));
        }
    }
}

/// `path` relative to `root`, written with `/`.
fn relative(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The workspace-relative paths of every file that holds a unix-only API on a line that is not
/// a comment, over `crates/*/src/**/*.rs` and `crates/*/build.rs`.
fn files_using_unix_apis() -> BTreeSet<String> {
    let root = workspace_root();
    let mut candidates = BTreeSet::new();
    let crates = fs::read_dir(root.join("crates")).expect("the crates directory is readable");
    for krate in crates.flatten() {
        rust_files_under(&root, &krate.path().join("src"), &mut candidates);
        let build = krate.path().join("build.rs");
        if build.is_file() {
            candidates.insert(relative(&root, &build));
        }
    }
    candidates
        .into_iter()
        .filter(|path| {
            let text = fs::read_to_string(root.join(path)).expect("a source file is readable");
            text.lines().any(|line| {
                !line.trim_start().starts_with("//")
                    && UNIX_MARKERS.iter().any(|marker| line.contains(marker))
            })
        })
        .collect()
}

/// One `[[file]]` table of the inventory.
#[derive(Debug, Default)]
struct Entry {
    path: String,
    uses: Vec<String>,
    windows: String,
}

/// The text between the first and the last `"` of `value`, or `value` itself when it is not
/// quoted.
fn unquote(value: &str) -> String {
    let value = value.trim();
    match (value.find('"'), value.rfind('"')) {
        (Some(open), Some(close)) if open < close => value[open + 1..close].to_string(),
        _ => value.to_string(),
    }
}

/// The quoted strings of a one-line TOML array.
fn quoted_items(value: &str) -> Vec<String> {
    value
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

/// The inventory's entries. The format is fixed: `[[file]]` tables whose `path`, `uses` and
/// `windows` keys each sit on one line.
fn inventory() -> Vec<Entry> {
    let file = workspace_root().join("crates/candler-core/tests/vectors/platform_inventory.toml");
    let text = fs::read_to_string(file).expect("the inventory is readable");
    let mut entries: Vec<Entry> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line == "[[file]]" {
            entries.push(Entry::default());
        } else if let Some(entry) = entries.last_mut() {
            if let Some(value) = line.strip_prefix("path =") {
                entry.path = unquote(value);
            } else if let Some(value) = line.strip_prefix("uses =") {
                entry.uses = quoted_items(value);
            } else if let Some(value) = line.strip_prefix("windows =") {
                entry.windows = unquote(value);
            }
        }
    }
    entries
}

/// Guards: the platform surface of the workspace stays inside the inventory, so a new file that
/// calls a unix-only API (`std::os::unix`, `rustix::`, `libc::`) is added to the inventory with
/// its Windows counterpart on purpose, and a file that stops calling one leaves it.
/// Oracle: the inventory, written by hand from the 21 files that held such a call when it was
/// frozen.
/// Bite: a `use std::os::unix::fs::PermissionsExt as _;` added in `commands/evaluate.rs` fails
/// the test naming that file.
#[test]
fn q1_unix_apis_stay_inside_the_inventory() {
    let listed: BTreeSet<String> = inventory().into_iter().map(|entry| entry.path).collect();
    let used = files_using_unix_apis();
    let unlisted: Vec<&String> = used.difference(&listed).collect();
    let stale: Vec<&String> = listed.difference(&used).collect();
    assert!(
        unlisted.is_empty() && stale.is_empty(),
        "files that use a unix-only API and are not in the inventory: {unlisted:#?}\n\
         inventory entries whose file no longer uses one: {stale:#?}"
    );
}

/// Guards: every inventory entry is a usable line of the port's checklist: it names what the
/// file calls and the Windows counterpart.
/// Oracle: the counterpart table of the design (Job Objects, a held process handle,
/// `LockFileEx`, `DETACHED_PROCESS`, `FlushFileBuffers`, ACLs).
/// Bite: an entry whose `windows` text is cleared fails the test naming its path.
#[test]
fn q1_every_inventory_entry_names_its_use_and_a_windows_counterpart() {
    let entries = inventory();
    assert!(!entries.is_empty(), "the inventory lists no file");
    let bad: Vec<&str> = entries
        .iter()
        .filter(|entry| {
            let windows = entry.windows.trim().to_lowercase();
            entry.path.is_empty()
                || entry.uses.is_empty()
                || entry.uses.iter().any(|item| item.trim().is_empty())
                || windows.is_empty()
                || windows == "todo"
                || windows == "tbd"
        })
        .map(|entry| entry.path.as_str())
        .collect();
    assert!(
        bad.is_empty(),
        "entries without a use or a Windows counterpart: {bad:#?}"
    );
}
