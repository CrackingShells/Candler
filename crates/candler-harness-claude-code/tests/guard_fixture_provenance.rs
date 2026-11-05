#![cfg(test)]
//! Fixture provenance: every fixture file is listed with a hash that recomputes.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Files that sit in the fixtures directory without a `PROVENANCE.toml` entry.
const EXEMPT: [&str; 2] = ["PROVENANCE.toml", "digests.toml"];

type Res<T> = Result<T, Box<dyn Error>>;

struct Entry {
    path: String,
    sha256: String,
    synthetic: bool,
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn entries() -> Res<Vec<Entry>> {
    let text = fs::read_to_string(fixtures().join("PROVENANCE.toml"))?;
    let doc: toml::Table = text.parse()?;
    let files = doc
        .get("files")
        .and_then(toml::Value::as_array)
        .ok_or("PROVENANCE.toml has no `files` array")?;
    let mut out = Vec::new();
    for file in files {
        let field = |key: &str| file.get(key).ok_or(format!("entry lacks `{key}`"));
        out.push(Entry {
            path: field("path")?
                .as_str()
                .ok_or("`path` is not a string")?
                .to_owned(), // BORROW: owned entry
            sha256: field("sha256")?
                .as_str()
                .ok_or("`sha256` is not a string")?
                .to_owned(), // BORROW: owned entry
            synthetic: field("synthetic")?
                .as_bool()
                .ok_or("`synthetic` is not a bool")?,
        });
    }
    Ok(out)
}

fn walk(dir: &Path, base: &Path, out: &mut BTreeSet<String>) -> Res<()> {
    for item in fs::read_dir(dir)? {
        let path = item?.path();
        if path.is_dir() {
            walk(&path, base, out)?;
        } else {
            let rel = path.strip_prefix(base)?;
            out.insert(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// Guards: Fixture honesty — a fixture cannot change, vanish or appear without `PROVENANCE.toml` saying so, and a synthetic stream is never passed off as a recorded one. Oracle: the sha256 of each file's bytes as read now against the hex string written in `PROVENANCE.toml`; the `synthetic_` path prefix against its flag.
#[test]
fn provenance_matches_the_fixture_files() -> Res<()> {
    let base = fixtures();
    let listed = entries()?;
    let mut mismatches = Vec::new();
    for entry in &listed {
        let bytes = fs::read(base.join(&entry.path))?;
        let mut hex = String::new();
        for byte in &Sha256::digest(&bytes) {
            // BORROW: byte view of the digest
            write!(hex, "{byte:02x}")?;
        }
        if hex != entry.sha256 {
            mismatches.push(format!("{}: recomputed {hex}", entry.path));
        }
        let synthetic_path = entry
            .path
            .split('/')
            .any(|part| part.starts_with("synthetic_"));
        if synthetic_path != entry.synthetic {
            mismatches.push(format!(
                "{}: synthetic flag is {}",
                entry.path, entry.synthetic
            ));
        }
    }
    let mut present = BTreeSet::new();
    walk(&base, &base, &mut present)?;
    let known: BTreeSet<&str> = listed.iter().map(|entry| entry.path.as_str()).collect(); // BORROW: view of the owned path
    for path in &present {
        if !known.contains(path.as_str()) && !EXEMPT.contains(&path.as_str()) {
            // BORROW: str view for lookup
            mismatches.push(format!("{path}: present but unlisted"));
        }
    }
    for entry in &listed {
        if !entry.path.starts_with("..") && !present.contains(&entry.path) {
            mismatches.push(format!("{}: listed but absent", entry.path));
        }
    }
    assert!(
        mismatches.is_empty(),
        "provenance violations: {mismatches:#?}"
    );
    Ok(())
}
