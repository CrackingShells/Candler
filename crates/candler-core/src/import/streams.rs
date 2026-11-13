//! Framing legacy streams and normalising their traces: the agent's stream and standard error become raw frame files, and the stream's trace is compared with the token counts the legacy timing file states.

use std::io::Read as _;
use std::path::Path;

use candler_harness::reader::TraceReader;
use candler_harness::spec::Framing;
use candler_schemas::primitives::RunKey;
use candler_schemas::raw::{Clock, StreamName};
use candler_schemas::trace::{EventKind, TraceFile, UsageScope};
use flate2::read::MultiGzDecoder;

use super::ImportError;
use super::legacy::TOKEN_COUNTERS;
use super::read::{JsonFile, LegacyRun, SourceFile};
use crate::frames::{FrameWriter, RawFileInfo};

/// Makes a fresh trace reader for one stream; the caller of the import supplies it.
// TRAIT_OBJECT: the caller of the import supplies readers of unknown concrete type
pub type ReaderFactory = dyn Fn() -> Box<dyn TraceReader> + Send + Sync;

/// A run whose stream could not be normalised, with the reader's explanation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[non_exhaustive]
pub struct TraceFailure {
    /// The run.
    pub run: RunKey,
    /// Why the stream could not be normalised.
    pub explanation: String,
}

impl TraceFailure {
    /// A trace failure of `run`.
    #[must_use]
    pub const fn new(run: RunKey, explanation: String) -> Self {
        Self { run, explanation }
    }
}

/// A token counter whose sum over the trace differs from the legacy timing file, with both numbers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[non_exhaustive]
pub struct UsageDiff {
    /// The run.
    pub run: RunKey,
    /// The counter: `input_tokens`, `output_tokens`, `cache_creation_tokens` or `cache_read_tokens`.
    pub counter: String,
    /// The sum of the counter over the trace's `usage` events of scope `message`.
    pub trace: u64,
    /// The value in the legacy timing file; absent when the file states no such counter.
    pub legacy: Option<u64>,
    /// What the two numbers count and why they differ.
    pub explanation: String,
}

impl UsageDiff {
    /// A usage difference.
    #[must_use]
    pub const fn new(
        run: RunKey,
        counter: String,
        trace: u64,
        legacy: Option<u64>,
        explanation: String,
    ) -> Self {
        Self {
            run,
            counter,
            trace,
            legacy,
            explanation,
        }
    }
}

/// The error of reading `file` of `run`.
fn io_error(file: &SourceFile, detail: &impl std::fmt::Display) -> ImportError {
    ImportError::Io {
        path: file.path.to_string(),
        detail: detail.to_string(),
    }
}

/// Frames `bytes` as `stream` under an unrecorded clock, the frames cut from `file`.
fn frame_one(
    dir: &Path,
    stream: &str,
    bytes: &[u8],
    file: &SourceFile,
) -> Result<RawFileInfo, ImportError> {
    let name = stream
        .parse::<StreamName>()
        .map_err(|e| io_error(file, &e))?;
    // BORROW: .clone() gives the header its own copy of the legacy file's digest
    let source = Some(file.sha256.clone());
    let mut writer = FrameWriter::create(dir, name, Framing::Lines, Clock::Unrecorded, source)
        .map_err(|e| io_error(file, &e))?;
    writer.write(bytes, None).map_err(|e| io_error(file, &e))?;
    writer.finish().map_err(|e| io_error(file, &e))
}

/// Frames the agent's stream as `stdout` and its standard error, when the run has one, as `stderr`, into `dir`.
///
/// Both files have an unrecorded clock, no frame times, and the digest of the legacy file as their
/// `source`. The stream is decompressed first. The files are returned in that order.
///
/// # Memory
/// The whole decompressed stream is held in memory while it is framed; there is no cap and no
/// streaming in this version.
///
/// # Errors
/// Returns [`ImportError::Io`] if the stream cannot be decompressed or a frame file cannot be
/// written.
pub fn frame_streams(run: &LegacyRun, dir: &Path) -> Result<Vec<RawFileInfo>, ImportError> {
    let mut plain = Vec::new();
    // BORROW: explicit .as_slice() instead of Deref coercion
    MultiGzDecoder::new(run.stream.bytes.as_slice())
        .read_to_end(&mut plain)
        .map_err(|e| io_error(&run.stream, &e))?;
    // BORROW: explicit .as_slice()
    let mut files = vec![frame_one(dir, "stdout", plain.as_slice(), &run.stream)?];
    if let Some(stderr) = &run.stderr {
        // BORROW: explicit .as_slice() instead of Deref coercion
        files.push(frame_one(dir, "stderr", stderr.bytes.as_slice(), stderr)?);
    }
    Ok(files)
}

/// The token sums of the trace's `usage` events of scope `message`, in the order of [`TOKEN_COUNTERS`].
fn trace_sums(trace: &TraceFile) -> [u64; 4] {
    let mut sums = [0_u64; 4];
    for item in trace.events() {
        if let EventKind::Usage {
            scope: UsageScope::Message,
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
            ..
        } = &item.event.kind
        {
            let counts = [
                *input_tokens,
                *output_tokens,
                *cache_creation_tokens,
                *cache_read_tokens,
            ];
            for (sum, count) in sums.iter_mut().zip(counts) {
                *sum = sum.saturating_add(count);
            }
        }
    }
    sums
}

/// The differences between the trace's token sums and the legacy timing file's counters.
///
/// # Errors
/// Returns [`ImportError::Layout`] if the timing file holds a counter that is not a non-negative
/// integer.
pub fn usage_diffs(
    run: &RunKey,
    trace: &TraceFile,
    timing: &JsonFile,
) -> Result<Vec<UsageDiff>, ImportError> {
    let sums = trace_sums(trace);
    let mut diffs = Vec::new();
    for (counter, sum) in TOKEN_COUNTERS.iter().zip(sums) {
        let legacy = match timing.value.get(*counter) {
            // EXPLICIT: a counter the legacy timing does not record has no legacy value
            None => None,
            Some(value) => Some(value.as_u64().ok_or_else(|| ImportError::Layout {
                file: timing.path.to_string(),
                run: run.to_string(),
                detail: format!("`{counter}` is not a non-negative integer"),
            })?),
        };
        if legacy != Some(sum) {
            let rule = format!(
                "the trace sums `{counter}` over the usage events of scope `message`, taken from the first line that carries each message id, and holds {sum}"
            );
            let explanation = match legacy {
                Some(legacy) => format!(
                    "{rule}; the legacy timing file states {legacy}, the run total its harness reported when the run ended"
                ),
                None => format!("{rule}; the legacy timing file states no `{counter}`"),
            };
            diffs.push(UsageDiff::new(
                // BORROW: .clone() gives the diff its own key
                run.clone(),
                // BORROW: .to_owned() makes the owned counter name
                (*counter).to_owned(),
                sum,
                legacy,
                explanation,
            ));
        }
    }
    Ok(diffs)
}
