//! Helpers shared by the core's guard binaries.
//!
//! Every scan guard calls [`assert_clean`], so a scan that silently reads nothing cannot pass.
#![allow(dead_code, reason = "each guard binary uses a subset of the helpers")]

pub mod comparability;
pub mod container_run;
pub mod container_stop;
pub mod experiment_resolution;
pub mod gateway;
pub mod image;
pub mod job_orchestrate;
pub mod job_runtime;
pub mod job_store;
pub mod run_evidence;
pub mod run_exec;
pub mod run_plan;
pub mod supervision_gates;

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use flate2::read::MultiGzDecoder;

/// The two bytes that open a gzip member.
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// What one scan read and found.
#[derive(Debug, Default)]
pub struct Scan {
    /// The number of regular files read.
    pub files: usize,
    /// The files whose bytes, or whose gunzipped bytes, hold the needle.
    pub hits: Vec<PathBuf>,
}

/// Reads every regular file under `dir`, gunzipping a file that opens with the gzip magic, and
/// lists the files whose bytes hold `needle` exactly.
///
/// # Errors
///
/// Returns the first I/O error met while walking `dir` or reading a file.
pub fn scan(dir: &Path, needle: &str) -> io::Result<Scan> {
    let mut result = Scan::default();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in fs::read_dir(&next)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                result.files += 1;
                if holds(&read_plain(&entry.path())?, needle.as_bytes()) {
                    result.hits.push(entry.path());
                }
            }
        }
    }
    result.hits.sort();
    Ok(result)
}

/// Asserts that `dir` holds at least one regular file and that no file under it holds `needle`.
pub fn assert_clean(dir: &Path, needle: &str) {
    let result = scan(dir, needle);
    assert!(
        result.is_ok(),
        "scan of {} failed: {result:?}",
        dir.display()
    );
    if let Ok(found) = result {
        assert!(found.files > 0, "scan of {} read no file", dir.display());
        assert!(
            found.hits.is_empty(),
            "{needle:?} found under {}: {:?}",
            dir.display(),
            found.hits
        );
    }
}

/// The bytes of a file, gunzipped when it opens with the gzip magic.
fn read_plain(path: &Path) -> io::Result<Vec<u8>> {
    let bytes = fs::read(path)?;
    if bytes.starts_with(&GZIP_MAGIC) {
        let mut plain = Vec::new();
        // BORROW: the decoder reads the slice of `bytes` in place
        MultiGzDecoder::new(bytes.as_slice()).read_to_end(&mut plain)?;
        Ok(plain)
    } else {
        Ok(bytes)
    }
}

/// Whether `haystack` holds `needle` as a contiguous run of bytes.
fn holds(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// The fake `docker` script of the fixtures: it logs its arguments to `$FAKE_DOCKER_LOG` and
/// answers from `$FAKE_DOCKER_DIR/<verb>.out` and `<verb>.code`.
pub fn fake_docker() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/docker/fake-docker")
}
