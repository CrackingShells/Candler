#![cfg(test)]
//! Guards for the build identity of the `candler` binary: what each origin of a build records
//! (a git checkout, a packaged crate's vcs file, the packager's variables, nothing) and, above
//! all, that a field an origin cannot know stays unrecorded instead of being guessed. The
//! resolver is `build_infra.rs`, the file `build.rs` includes; the test includes it the same way.

#[path = "../build_infra.rs"]
mod build_infra;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use build_infra::{Identity, Origin, resolve};
use tempfile::TempDir;

const VERSION: &str = "7.8.9";
const SHA: &str = "7d9e9d8fe24f28910ede7ec8d6c74062dd86e015";

/// Runs `git` in `dir` and returns its trimmed output.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// A checkout laid out like the workspace: the crate in `crates/candler`, a sibling it watches,
/// and a file nothing watches. Returns the repository and the crate directory.
fn checkout() -> (TempDir, std::path::PathBuf) {
    let repo = TempDir::new().unwrap();
    let root = repo.path();
    for (path, text) in [
        ("crates/candler/src/main.rs", "fn main() {}\n"),
        ("crates/candler/Cargo.toml", "[package]\n"),
        ("crates/candler-core/src/lib.rs", "\n"),
        ("NOTES.md", "notes\n"),
    ] {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }
    git(root, &["init", "-q"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "first"]);
    let dir = root.join("crates/candler");
    (repo, dir)
}

/// The lookup of a build whose environment holds `pairs`.
fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    move |name| map.get(name).cloned()
}

/// A directory that holds only a vcs file with `body`, as cargo unpacks a crate.
fn packaged(body: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".cargo_vcs_info.json"), body).unwrap();
    dir
}

/// The vcs file as `cargo package` 1.99 writes it for a clean tree.
fn vcs_clean() -> String {
    format!(
        "{{\n  \"git\": {{\n    \"sha1\": \"{SHA}\"\n  }},\n  \"path_in_vcs\": \"crates/candler\"\n}}"
    )
}

/// The vcs file as `cargo package --allow-dirty` 1.99 writes it for a dirty tree.
fn vcs_dirty() -> String {
    format!(
        "{{\n  \"git\": {{\n    \"sha1\": \"{SHA}\",\n    \"dirty\": true\n  }},\n  \"path_in_vcs\": \"crates/candler\"\n}}"
    )
}

/// Guards: R01 §Run manifest, `eval_infra` — a clean git checkout records its full 40-hex commit
/// and `dirty = false`, and a change outside what the binary is built from does not make it dirty.
/// Oracle: `git rev-parse HEAD`.
#[test]
fn git_checkout_clean_records_the_full_commit() {
    let (repo, dir) = checkout();
    fs::write(repo.path().join("NOTES.md"), "changed\n").unwrap();
    let got = resolve(&dir, VERSION, env(&[]));
    let head = git(repo.path(), &["rev-parse", "HEAD"]);
    assert_eq!(head.len(), 40);
    assert_eq!(got.origin, Origin::Git);
    assert_eq!(got.commit.as_deref(), Some(head.as_str()));
    assert_eq!(got.dirty, Some(false));
}

/// Guards: R01 §Run manifest, `eval_infra` — a git checkout with a change in a watched crate is
/// recorded `dirty = true`, whether the sibling or the binary's own source changed.
/// Oracle: the test edits the file itself, and `git rev-parse HEAD`.
#[test]
fn git_checkout_dirty_records_dirty() {
    for watched in [
        "crates/candler/src/main.rs",
        "crates/candler-core/src/lib.rs",
    ] {
        let (repo, dir) = checkout();
        fs::write(repo.path().join(watched), "changed\n").unwrap();
        let got = resolve(&dir, VERSION, env(&[]));
        assert_eq!(got.origin, Origin::Git, "{watched}");
        assert_eq!(got.dirty, Some(true), "{watched}");
        assert_eq!(
            got.commit,
            Some(git(repo.path(), &["rev-parse", "HEAD"])),
            "{watched}"
        );
    }
}

/// Guards: R01 §Run manifest, `eval_infra` — when `git` cannot say whether the sources changed
/// (the crate sits at the repository root, so the watched sibling paths lie outside it), the
/// commit is recorded and `dirty` stays unrecorded instead of becoming `false`.
/// Oracle: `git status` itself fails there, which the test asserts before using it.
#[test]
fn git_checkout_that_cannot_tell_leaves_dirty_unrecorded() {
    let repo = TempDir::new().unwrap();
    fs::write(repo.path().join("a.txt"), "a\n").unwrap();
    git(repo.path(), &["init", "-q"]);
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-q", "-m", "first"]);
    let status = Command::new("git")
        .current_dir(repo.path())
        .args(["status", "--porcelain", "--", "../outside"])
        .output()
        .unwrap();
    assert!(!status.status.success());
    let got = resolve(repo.path(), VERSION, env(&[]));
    assert_eq!(got.commit, Some(git(repo.path(), &["rev-parse", "HEAD"])));
    assert_eq!(got.dirty, None);
}

/// Guards: R01 §Run manifest, `eval_infra` — a git checkout records the nearest `v` tag and the
/// number of commits since it.
/// Oracle: hand-made history, a tag then one commit.
#[test]
fn git_checkout_records_the_nearest_tag_and_distance() {
    let (repo, dir) = checkout();
    git(repo.path(), &["tag", "v0.1.0"]);
    fs::write(repo.path().join("NOTES.md"), "second\n").unwrap();
    git(repo.path(), &["commit", "-q", "-am", "second"]);
    let got = resolve(&dir, VERSION, env(&[]));
    assert_eq!(got.tag.as_deref(), Some("v0.1.0"));
    assert_eq!(got.commits_ahead, Some(1));
}

/// Guards: R01 §Run manifest, `eval_infra` — a packaged crate with a clean vcs file records its
/// `git.sha1` and `dirty = false`, the tag is `v<version>` and the distance is unrecorded.
/// Oracle: the vcs file bytes `cargo package` 1.99 wrote for a clean tree (probe P1).
#[test]
fn packaged_clean_records_sha_and_clean() {
    let dir = packaged(&vcs_clean());
    let got = resolve(dir.path(), VERSION, env(&[]));
    assert_eq!(
        got,
        Identity {
            origin: Origin::Packaged,
            tag: Some(format!("v{VERSION}")),
            commits_ahead: None,
            commit: Some(SHA.to_owned()),
            dirty: Some(false),
        }
    );
}

/// Guards: R01 §Run manifest, `eval_infra` — a vcs file with `"dirty": true` records the build
/// as dirty.
/// Oracle: the vcs file bytes `cargo package --allow-dirty` 1.99 wrote (probe P1).
#[test]
fn packaged_dirty_records_dirty() {
    let dir = packaged(&vcs_dirty());
    let got = resolve(dir.path(), VERSION, env(&[]));
    assert_eq!(got.origin, Origin::Packaged);
    assert_eq!(got.commit.as_deref(), Some(SHA));
    assert_eq!(got.dirty, Some(true));
    assert_eq!(got.tag, Some(format!("v{VERSION}")));
    assert_eq!(got.commits_ahead, None);
}

/// Guards: R01 §Run manifest, `eval_infra` — a packaged crate unpacked inside an unrelated git
/// checkout records its own vcs file's commit, never the surrounding repository's.
/// Oracle: `git rev-parse HEAD` of the surrounding repository differs from the vcs file's sha.
#[test]
fn packaged_inside_a_checkout_records_the_vcs_file() {
    let (repo, _) = checkout();
    let inner = repo.path().join("vendor/candler");
    fs::create_dir_all(&inner).unwrap();
    fs::write(inner.join(".cargo_vcs_info.json"), vcs_clean()).unwrap();
    let got = resolve(&inner, VERSION, env(&[]));
    assert_ne!(git(repo.path(), &["rev-parse", "HEAD"]), SHA);
    assert_eq!(got.origin, Origin::Packaged);
    assert_eq!(got.commit.as_deref(), Some(SHA));
}

/// Guards: R01 §Run manifest, `eval_infra` — the packager's variables are recorded as given, and
/// a flag they do not set stays unrecorded rather than becoming `false`.
/// Oracle: hand-written variables.
#[test]
fn packager_variables_are_recorded_and_unset_dirty_stays_unrecorded() {
    let dir = TempDir::new().unwrap();
    let all = env(&[
        ("CANDLER_EVAL_INFRA_TAG", "v1.2.3"),
        ("CANDLER_EVAL_INFRA_COMMIT", SHA),
        ("CANDLER_EVAL_INFRA_DIRTY", "false"),
    ]);
    let got = resolve(dir.path(), VERSION, all);
    assert_eq!(got.origin, Origin::Packager);
    assert_eq!(got.tag.as_deref(), Some("v1.2.3"));
    assert_eq!(got.commit.as_deref(), Some(SHA));
    assert_eq!(got.dirty, Some(false));
    assert_eq!(got.commits_ahead, None);

    let commit_only = resolve(
        dir.path(),
        VERSION,
        env(&[("CANDLER_EVAL_INFRA_COMMIT", SHA)]),
    );
    assert_eq!(commit_only.origin, Origin::Packager);
    assert_eq!(commit_only.commit.as_deref(), Some(SHA));
    assert_eq!(commit_only.dirty, None);

    let garbled = resolve(
        dir.path(),
        VERSION,
        env(&[("CANDLER_EVAL_INFRA_DIRTY", "maybe")]),
    );
    assert_eq!(garbled.dirty, None);
}

/// Guards: R01 §Run manifest, `eval_infra` — a build with no origin records `tag = v<version>`
/// and nothing else, so the verdict can name why it refuses. A vcs file that does not name a
/// commit is no origin.
/// Oracle: hand-written.
#[test]
fn no_origin_records_the_tag_only() {
    let bare = TempDir::new().unwrap();
    let broken = packaged("{\"git\": {\"dirty\": true}}");
    let garbage = packaged("not json");
    for dir in [bare.path(), broken.path(), garbage.path()] {
        let got = resolve(dir, VERSION, env(&[]));
        assert_eq!(
            got,
            Identity {
                origin: Origin::None,
                tag: Some(format!("v{VERSION}")),
                commits_ahead: None,
                commit: None,
                dirty: None,
            },
            "{dir:?}"
        );
    }
}

/// Guards: R01 §Run manifest, `eval_infra` — a build in this very checkout records its commit in
/// full, the one `git` names for it.
/// Oracle: `git rev-parse HEAD` in the crate's directory.
#[test]
fn this_checkout_records_the_full_commit() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let head = git(dir, &["rev-parse", "HEAD"]);
    assert_eq!(head.len(), 40);
    assert_eq!(
        option_env!("CANDLER_EVAL_INFRA_COMMIT"),
        Some(head.as_str())
    );
    assert!(option_env!("CANDLER_EVAL_INFRA_DIRTY").is_some());
    assert_eq!(resolve(dir, VERSION, env(&[])).commit, Some(head));
}
