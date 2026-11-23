//! Guards for the digest of a build context and the tag of the base image built from one, for the
//! tree a listing stages, and for the files a run leaves out when it lists its output.
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

/// Guards: a staged copy of a listing is the tree its digest covers: the build context read from
/// the staged copy has the digest of the one it was staged from, links included. Oracle: the
/// digest of the source, taken before the copy exists.
#[test]
fn a_staged_copy_has_the_digest_of_its_listing() {
    let dir = tempfile::tempdir().unwrap();
    write_context(dir.path());
    let ctx = dir.path().join("ctx");
    let dockerfile = dir.path().join("Dockerfile");
    let listing = BuildContext::walk().list(&ctx).unwrap();
    let source = BuildContext::of(&dockerfile, &listing).unwrap();
    let staged = dir.path().join("staged");
    listing.stage(&staged).unwrap();
    let copy = BuildContext::read(&dockerfile, &staged).unwrap();
    assert_eq!(copy.digest().unwrap(), source.digest().unwrap());
    assert!(
        fs::symlink_metadata(staged.join("link-dir"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

/// Guards: the files a run produced leave out a link (the file link and the directory link of the
/// tree) and are listed by joined string, so a planted link never enters the digest. Oracle: the
/// hand-listed paths.
#[test]
fn the_files_of_a_run_leave_out_links() {
    use candler_core::runner::exec::gather::collect_files;
    use candler_harness::spec::CollectSpec;
    let dir = tempfile::tempdir().unwrap();
    write_context(dir.path());
    let found = collect_files(
        &dir.path().join("ctx"),
        &[CollectSpec::file("**/*".to_owned())],
    )
    .unwrap();
    let paths: Vec<&str> = found.iter().map(|out| out.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            ".hidden",
            "a-b",
            "a/b",
            "a/n/deep.txt",
            "caf\u{e9}-\u{65e5}\u{672c}.txt"
        ]
    );
}

/// Guards: README "evaluation, fresh" for `OpenCode` — what the `OpenCode` driver collects, taken
/// through the core's own file collection, stores the file the agent wrote and leaves out the
/// skills the harness mounts. Oracle: the hand-listed paths of the audit run
/// (`workspace/outputs/answer.txt` written, `tree.json` empty).
/// Bite: `OpenCodeDriver::collect` returns only the export command (the run stores no file).
#[test]
fn opencode_collect_stores_the_files_the_agent_wrote() {
    use candler_core::runner::exec::gather::collect_files;
    use candler_harness::driver::HarnessDriver as _;
    use candler_harness::spec::{Observed, Workspace};
    use candler_harness_opencode::driver::OpenCodeDriver;
    use candler_schemas::trace::SessionId;
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    for (path, text) in [
        ("outputs/answer.txt", "stamp-7f3a"),
        ("notes.md", "kept"),
        (".opencode/skills/s/SKILL.md", "mounted"),
    ] {
        let file = ws.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }
    let id: SessionId = "ses_e32c17b4fffe2bjBVM2E3xAaLZ".parse().unwrap();
    let specs = OpenCodeDriver::new().collect(&Workspace::new(ws), &Observed::new(vec![id]));
    let found = collect_files(ws, &specs).unwrap();
    let paths: Vec<&str> = found.iter().map(|out| out.path.as_str()).collect();
    assert_eq!(paths, ["notes.md", "outputs/answer.txt"]);
}
