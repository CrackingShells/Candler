//! Guards for the tree walker: the order of a listing, what each link policy does with a link,
//! what a name rule does, and that a staged copy is the tree its digest covers.
#![cfg(test)]

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use candler_core::tree::{BuildContext, Kind, Links, Names, Refusal, Screen, Walk, WalkError};

/// Writes `a-b` beside `a/b`, a nested file, a hidden file, a file link and a directory link under
/// `ctx`, and a Dockerfile beside it.
fn write_tree(root: &Path) {
    let ctx = root.join("ctx");
    fs::create_dir_all(ctx.join("a/n")).unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();
    fs::write(ctx.join("a-b"), "dash\n").unwrap();
    fs::write(ctx.join("a/b"), "slash\n").unwrap();
    fs::write(ctx.join("a/n/deep.txt"), "deep\n").unwrap();
    fs::write(ctx.join(".hidden"), "hidden\n").unwrap();
    symlink("a-b", ctx.join("link-file")).unwrap();
    symlink("a", ctx.join("link-dir")).unwrap();
}

fn names(walk: Walk, ctx: &Path) -> Vec<String> {
    let listing = walk.list(ctx).unwrap();
    listing.entries().iter().map(|e| e.rel.clone()).collect()
}

/// Guards: a listing is sorted by the joined relative path as a string, so `a-b` comes before
/// `a/b`; the order of path components would put `a/b` first. Oracle: the hand-listed order.
#[test]
fn a_listing_is_sorted_by_the_joined_string() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());
    let listed = names(
        Walk::new(Links::Skip, Names::Strict),
        &dir.path().join("ctx"),
    );
    assert_eq!(listed, [".hidden", "a-b", "a/b", "a/n/deep.txt"]);
}

/// Guards: `Record` lists a link as a link, never followed (a directory link is one entry),
/// `Skip` leaves it out and `Refuse` fails the walk on it. Oracle: the hand-listed entries.
#[test]
fn each_link_policy_treats_a_link_as_named() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());
    let ctx = dir.path().join("ctx");
    let recorded = Walk::new(Links::Record, Names::Strict).list(&ctx).unwrap();
    let links: Vec<&str> = recorded
        .entries()
        .iter()
        .filter(|e| e.kind == Kind::Link)
        .map(|e| e.rel.as_str())
        .collect();
    assert_eq!(links, ["link-dir", "link-file"]);
    let skipped = names(Walk::new(Links::Skip, Names::Strict), &ctx);
    assert!(!skipped.iter().any(|name| name.starts_with("link-")));
    let refused = Walk::new(Links::Refuse, Names::Strict).list(&ctx);
    assert!(matches!(
        refused,
        Err(WalkError::Refused {
            why: Refusal::Link,
            ..
        })
    ));
}

fn refuse_comma(name: &str) -> Screen {
    if name.contains(',') {
        Screen::Refuse
    } else {
        Screen::Keep
    }
}

fn skip_a(name: &str) -> Screen {
    if name == "a" {
        Screen::Skip
    } else {
        Screen::Keep
    }
}

/// Guards: a name rule refuses or leaves out an entry by its own name, at any depth. Oracle: the
/// hand-listed entries.
#[test]
fn a_name_rule_refuses_or_skips_by_name() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());
    let ctx = dir.path().join("ctx");
    fs::write(ctx.join("a/n/x,y"), "comma\n").unwrap();
    let refused = Walk::new(Links::Skip, Names::Strict)
        .screen(refuse_comma)
        .list(&ctx);
    assert!(matches!(
        refused,
        Err(WalkError::Refused { ref rel, why: Refusal::Name }) if rel == "a/n/x,y"
    ));
    let skipped = names(Walk::new(Links::Skip, Names::Strict).screen(skip_a), &ctx);
    assert_eq!(skipped, [".hidden", "a-b"]);
}

/// Guards: the tree a listing stages is the tree its digest covers: the build context read from
/// the staged copy has the digest of the one it was staged from, links included. Oracle: the
/// digest of the source, taken before the copy exists.
#[test]
fn a_staged_copy_has_the_digest_of_its_listing() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());
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
    write_tree(dir.path());
    let found = collect_files(
        &dir.path().join("ctx"),
        &[CollectSpec::file("**/*".to_owned())],
    )
    .unwrap();
    let paths: Vec<&str> = found.iter().map(|out| out.path.as_str()).collect();
    assert_eq!(paths, [".hidden", "a-b", "a/b", "a/n/deep.txt"]);
}
