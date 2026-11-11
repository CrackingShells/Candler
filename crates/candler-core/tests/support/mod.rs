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

/// One denied name that a segment scan found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentHit {
    /// The file, or the file whose path holds the name.
    pub file: PathBuf,
    /// The one-based line number, or `0` when the name is in the path itself.
    pub line: usize,
    /// The denied segment, the denied sequence (joined by `_`) or the denied literal.
    pub segment: String,
}

/// What one segment scan read and found.
#[derive(Debug, Default)]
pub struct SegmentScan {
    /// The number of regular files read.
    pub files: usize,
    /// The denied names found, in file and line order.
    pub hits: Vec<SegmentHit>,
}

/// What a segment scan denies and excuses.
#[derive(Debug, Clone, Copy)]
pub struct SegmentRules<'a> {
    /// The lowercase segments that no identifier may hold.
    pub denied: &'a [&'a str],
    /// The runs of consecutive lowercase segments that no identifier may hold.
    pub sequences: &'a [&'a [&'a str]],
    /// The exact literals that no line may hold, with no alphanumeric right before or after.
    pub literals: &'a [&'a str],
    /// The excused lines: `(path relative to the scan's base, exact trimmed line)`.
    pub allow: &'a [(&'a str, &'a str)],
    /// The file extensions of the known binary files, whose bytes are not read as text.
    pub binary: &'a [&'a str],
    /// The exact names of the files and folders of tools and of the OS (`.DS_Store`,
    /// `__pycache__`): they are skipped, and a skipped folder is not entered.
    pub skip: &'a [&'a str],
}

/// The lowercase segments of one run of alphanumerics: a split at every lower-to-upper change,
/// at every acronym boundary (`HTTPServer` gives `http` and `server`) and at every change
/// between letters and digits.
pub fn split_run(run: &str) -> Vec<String> {
    let chars: Vec<char> = run.chars().collect();
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    for (position, &c) in chars.iter().enumerate() {
        if let Some(previous) = current.chars().last() {
            let next_is_lower = chars
                .get(position.saturating_add(1))
                .is_some_and(|n| n.is_lowercase());
            let boundary = (previous.is_lowercase() && c.is_uppercase())
                || (previous.is_numeric() != c.is_numeric())
                || (previous.is_uppercase() && c.is_uppercase() && next_is_lower);
            if boundary {
                parts.push(std::mem::take(&mut current).to_lowercase());
            }
        }
        current.push(c);
    }
    if !current.is_empty() {
        parts.push(current.to_lowercase());
    }
    parts
}

/// The identifier-like chunks of `text`, each as its lowercase segments. A chunk is a maximal run
/// of alphanumerics, `_`, `-` and `.`; it is split into segments on every other separator and by
/// [`split_run`], so `plan_trace`, `PLAN-TRACE`, `plan.trace` and `PlanTrace` give the same
/// segments.
pub fn chunk_segments(text: &str) -> Vec<Vec<String>> {
    text.split(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.')))
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| {
            chunk
                .split(|c: char| !c.is_alphanumeric())
                .filter(|run| !run.is_empty())
                .flat_map(split_run)
                .collect()
        })
        .collect()
}

/// Whether `line` holds `literal` with no alphanumeric character right before or after it.
fn holds_literal(line: &str, literal: &str) -> bool {
    line.match_indices(literal).any(|(start, _)| {
        let before = line
            .get(..start)
            .and_then(|head| head.chars().last())
            .is_some_and(char::is_alphanumeric);
        let after = line
            .get(start.saturating_add(literal.len())..)
            .and_then(|tail| tail.chars().next())
            .is_some_and(char::is_alphanumeric);
        !before && !after
    })
}

/// The denied names that `text` (a line, or a file name) holds.
fn denied_in(text: &str, rules: &SegmentRules<'_>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for segments in chunk_segments(text) {
        for segment in &segments {
            if rules.denied.contains(&segment.as_str()) && !seen.contains(segment) {
                // BORROW: `.clone()` copies the segment into the hit
                seen.push(segment.clone());
            }
        }
        for sequence in rules.sequences {
            let held = segments.windows(sequence.len()).any(|window| {
                window
                    .iter()
                    .map(String::as_str)
                    .eq(sequence.iter().copied())
            });
            let joined = sequence.join("_");
            if held && !seen.contains(&joined) {
                seen.push(joined);
            }
        }
    }
    for literal in rules.literals {
        if holds_literal(text, literal) {
            // BORROW: `.to_owned()` copies the literal into the hit
            seen.push((*literal).to_owned());
        }
    }
    seen
}

/// The path of `path` below `base`, with `/` separators.
fn relative(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Reads one regular file into `result`.
fn scan_file(
    base: &Path,
    path: &Path,
    rules: &SegmentRules<'_>,
    result: &mut SegmentScan,
) -> io::Result<()> {
    result.files += 1;
    let mut found: Vec<(usize, String)> = Vec::new();
    if let Some(name) = path.file_name() {
        for segment in denied_in(&name.to_string_lossy(), rules) {
            found.push((0, segment));
        }
    }
    let known_binary = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| rules.binary.contains(&extension));
    if !known_binary {
        let text = String::from_utf8(read_plain(path)?).map_err(|_| {
            io::Error::other(format!(
                "{} is neither UTF-8 text, gzip of UTF-8 text nor a listed binary type",
                path.display()
            ))
        })?;
        let shown = relative(base, path);
        for (index, line) in text.lines().enumerate() {
            let excused = rules
                .allow
                .iter()
                .any(|(file, body)| shown == *file && line.trim() == *body);
            if !excused {
                for segment in denied_in(line, rules) {
                    found.push((index.saturating_add(1), segment));
                }
            }
        }
    }
    for (line, segment) in found {
        result.hits.push(SegmentHit {
            file: path.to_path_buf(),
            line,
            segment,
        });
    }
    Ok(())
}

/// Reads `target`, a file or every regular file under a folder (gunzipping a file that opens
/// with the gzip magic), and lists each denied name: in a line, or in the name of a file or of
/// a folder met below `target`. A line is excused when `rules.allow` holds its exact path below
/// `base` and its exact trimmed text.
///
/// # Errors
///
/// Returns the first I/O error met, a symlink met below `target` (named), and a file that is
/// neither UTF-8 text, gzip of UTF-8 text nor of a listed binary type (named).
pub fn scan_segments(
    base: &Path,
    target: &Path,
    rules: &SegmentRules<'_>,
) -> io::Result<SegmentScan> {
    let mut result = SegmentScan::default();
    if fs::symlink_metadata(target)?.is_file() {
        scan_file(base, target, rules, &mut result)?;
    } else {
        let mut pending = vec![target.to_path_buf()];
        while let Some(next) = pending.pop() {
            for entry in fs::read_dir(&next)? {
                let entry = entry?;
                if rules
                    .skip
                    .contains(&entry.file_name().to_string_lossy().as_ref())
                {
                    continue;
                }
                let kind = entry.file_type()?;
                if kind.is_symlink() {
                    return Err(io::Error::other(format!(
                        "{} is a symlink",
                        entry.path().display()
                    )));
                }
                if kind.is_dir() {
                    for segment in denied_in(&entry.file_name().to_string_lossy(), rules) {
                        result.hits.push(SegmentHit {
                            file: entry.path(),
                            line: 0,
                            segment,
                        });
                    }
                    pending.push(entry.path());
                } else if kind.is_file() {
                    scan_file(base, &entry.path(), rules, &mut result)?;
                }
            }
        }
    }
    result
        .hits
        .sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    Ok(result)
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
