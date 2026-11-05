//! Guards for the scan helper that every scan guard uses.
#![cfg(test)]

mod support;

use std::fs;
use std::io::Write;
use std::panic;

use flate2::Compression;
use flate2::write::GzEncoder;
use support::{assert_clean, scan};

/// Guards: oracle rule 6 — a scan finds a needle planted in a plain file and inside a gzip file
/// in a nested directory, passes a directory without it, and refuses a directory with no file.
/// Oracle: the files this test plants.
#[test]
fn the_scan_finds_planted_needles_and_refuses_an_empty_directory() -> std::io::Result<()> {
    let root = tempfile::tempdir()?;
    let nested = root.path().join("a").join("b");
    fs::create_dir_all(&nested)?;
    fs::write(root.path().join("plain.txt"), b"nothing here\n")?;
    assert_clean(root.path(), "canary-7f3e");

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(b"line one\nthe canary-7f3e is here\n")?;
    fs::write(nested.join("stream.jsonl.gz"), encoder.finish()?)?;
    fs::write(root.path().join("leak.json"), b"{\"v\":\"canary-7f3e\"}")?;
    let found = scan(root.path(), "canary-7f3e")?;
    assert_eq!(found.files, 3);
    // The hits are sorted: `a/b/stream.jsonl.gz` before `leak.json`.
    assert_eq!(
        found.hits,
        vec![
            nested.join("stream.jsonl.gz"),
            root.path().join("leak.json")
        ]
    );
    let planted = root.path().to_path_buf();
    assert!(panic::catch_unwind(|| assert_clean(&planted, "canary-7f3e")).is_err());

    let empty = tempfile::tempdir()?;
    fs::create_dir(empty.path().join("only-a-directory"))?;
    let empty_path = empty.path().to_path_buf();
    assert!(panic::catch_unwind(|| assert_clean(&empty_path, "canary-7f3e")).is_err());
    Ok(())
}
