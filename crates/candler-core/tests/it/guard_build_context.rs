//! Guards for the digest of a build context and the tag of the base image built from one.
//!
//! The context holds what a naive merge of the tree walkers would move: `a-b` beside `a/b` (the
//! joined string and the path components sort them in opposite orders), a file link, a directory
//! link, a non-ASCII name and a hidden file. The literals below were taken from the code as it was
//! before the walkers were merged, so they are the vector a refactor must keep.
#![cfg(test)]

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use candler_core::isolation::image::base_tag;
use candler_core::tree::BuildContext;

/// The digest of the build inputs of [`write_context`].
const BUILD_INPUT_DIGEST: &str =
    "sha256:d8e8cbe75780c830a5e2b43c47f48cb251a8e3b172bb7c8799b1ea84b1f099db";
/// The base tag of [`write_context`] built with the argument `A=b`.
const BASE_TAG: &str = "candler-base:src-610940524574";

/// Writes the Dockerfile `root/Dockerfile` and the context `root/ctx`.
fn write_context(root: &Path) {
    let ctx = root.join("ctx");
    fs::create_dir_all(ctx.join("a/n")).unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();
    fs::write(ctx.join("a-b"), "dash\n").unwrap();
    fs::write(ctx.join("a/b"), "slash\n").unwrap();
    fs::write(ctx.join("a/n/deep.txt"), "deep\n").unwrap();
    fs::write(ctx.join(".hidden"), "hidden\n").unwrap();
    fs::write(ctx.join("caf\u{e9}-\u{65e5}\u{672c}.txt"), "unicode\n").unwrap();
    symlink("a-b", ctx.join("link-file")).unwrap();
    symlink("a", ctx.join("link-dir")).unwrap();
}

/// Guards: the build inputs of a context with nested directories, `a-b` and `a/b`, a file link, a
/// directory link, a non-ASCII name and a hidden file digest to the pinned literal, and its base
/// tag is the pinned literal. Oracle: the literals, taken from the code before the walkers merged.
#[test]
fn guard_build_context() {
    let dir = tempfile::tempdir().unwrap();
    write_context(dir.path());
    let ctx = dir.path().join("ctx");
    let inputs = BuildContext::read(&dir.path().join("Dockerfile"), &ctx).unwrap();
    let digest = inputs.digest().unwrap();
    assert_eq!(digest.to_string(), BUILD_INPUT_DIGEST);
    // A base context holds its own Dockerfile, which the tag digest covers.
    fs::write(ctx.join("Dockerfile"), "FROM scratch\n").unwrap();
    let tag = base_tag(&ctx, &[("A".to_owned(), "b".to_owned())]).unwrap();
    assert_eq!(tag, BASE_TAG);
}
