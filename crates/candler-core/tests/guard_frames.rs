//! Guards for raw frame files: the framing property and the literal cases.
#![cfg(test)]

use std::fs;
use std::path::Path;

use candler_core::frames::{FrameMode, FrameReader, FrameWriter, frame_bytes};
use candler_schemas::primitives::Digest;
use candler_schemas::raw::{Clock, RawFrame, RawRecord, StreamName};
use proptest::prelude::*;
use serde_json::{Value, json};

fn stdout_name() -> StreamName {
    "stdout".parse().expect("a stream name")
}

fn zero_digest() -> Digest {
    format!("sha256:{}", "0".repeat(64))
        .parse()
        .expect("a digest")
}

fn read_frames(path: &Path) -> Vec<RawFrame> {
    FrameReader::open(path)
        .expect("a readable frame file")
        .collect::<Result<Vec<_>, _>>()
        .expect("readable frames")
}

/// Writes `chunks` through a writer and returns the frames read back from the finished file.
fn write_and_read(mode: FrameMode, chunks: &[&[u8]]) -> (Vec<RawFrame>, Vec<(u64, RawFrame)>) {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut writer = FrameWriter::create(dir.path(), stdout_name(), mode, Clock::Unrecorded, None)
        .expect("a writer");
    let mut live = Vec::new();
    for (n, chunk) in chunks.iter().enumerate() {
        live.extend(
            writer
                .write(chunk, Some(u64::try_from(n).expect("small")))
                .expect("a write"),
        );
    }
    live.extend(writer.flush_tail().expect("a tail"));
    let info = writer.finish().expect("a finished file");
    assert_eq!(info.path, dir.path().join("stdout.jsonl.gz"));
    assert!(!dir.path().join("stdout.jsonl.tmp").exists());
    // The gzip header's mtime field (bytes 4 to 8) is 0.
    let bytes = fs::read(&info.path).expect("the gz bytes");
    // INDEX: a gzip header is 10 bytes long, and the file holds at least that
    assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
    assert_eq!(&bytes[4..8], &[0, 0, 0, 0]);
    (read_frames(&info.path), live)
}

fn data_of(frame: &RawFrame) -> Vec<u8> {
    frame.data.to_bytes()
}

fn byte_strategy() -> impl Strategy<Value = u8> {
    prop_oneof![3 => Just(b'\n'), 1 => any::<u8>(), 3 => 0x20_u8..0x7f]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Guards: D1 — for any bytes cut into any chunks, the frames' data concatenate to the input
    /// and frame N is the N-th line of the input with its newline kept.
    /// Oracle: `slice::split_inclusive` of the standard library on the input bytes.
    #[test]
    fn frames_concatenate_to_the_input_and_cut_at_newlines(
        input in prop::collection::vec(byte_strategy(), 0..300),
        cuts in prop::collection::vec(0_usize..400, 0..6),
    ) {
        let mut bounds: Vec<usize> = cuts.iter().map(|c| c % (input.len() + 1)).collect();
        bounds.push(0);
        bounds.push(input.len());
        bounds.sort_unstable();
        let chunks: Vec<&[u8]> = bounds
            .windows(2)
            // INDEX: windows(2) yields slices of exactly two bounds, each at most input.len()
            .map(|w| &input[w[0]..w[1]])
            .collect();
        let (frames, live) = write_and_read(FrameMode::Lines, &chunks);
        let expected: Vec<&[u8]> = input.split_inclusive(|b| *b == b'\n').collect();
        let got: Vec<Vec<u8>> = frames.iter().map(data_of).collect();
        prop_assert_eq!(got.concat(), input.clone()); // BORROW: .clone() keeps the input for the next assertion
        prop_assert_eq!(got.len(), expected.len());
        for (frame, piece) in got.iter().zip(expected.iter()) {
            prop_assert_eq!(frame.as_slice(), *piece); // BORROW: explicit .as_slice() instead of Deref coercion
        }
        // The frames the writer handed back while writing are the frames of the file, indexed from 0.
        let indices: Vec<u64> = live.iter().map(|(i, _)| *i).collect();
        let want: Vec<u64> = (0..u64::try_from(frames.len()).expect("small")).collect();
        prop_assert_eq!(indices, want);
        let live_frames: Vec<RawFrame> = live.into_iter().map(|(_, f)| f).collect();
        prop_assert_eq!(live_frames, frames);
    }
}

fn text(piece: &str) -> Value {
    json!({"data": {"kind": "text", "text": piece}})
}

/// Guards: D1 — the exact records of the nine framing cases: empty stream, a bare newline, a
/// partial line, a complete line, a line and a partial line, CRLF, two newlines and invalid
/// UTF-8 under `Lines`, and a two-line stream as one frame under `Whole`.
/// Oracle: the records written by hand from the `raw-frames/1` text of the design.
#[test]
fn the_nine_cases_frame_as_the_design_states() {
    // `printf '\xff\n' | base64` gives `/wo=`, and `printf '\xfe' | base64` gives `/g==`.
    let cases: Vec<(FrameMode, &[u8], Vec<Value>)> = vec![
        (FrameMode::Lines, b"", vec![]),
        (FrameMode::Lines, b"\n", vec![text("\n")]),
        (FrameMode::Lines, b"a", vec![text("a")]),
        (FrameMode::Lines, b"a\n", vec![text("a\n")]),
        (FrameMode::Lines, b"a\nb", vec![text("a\n"), text("b")]),
        (FrameMode::Lines, b"\r\n", vec![text("\r\n")]),
        (FrameMode::Lines, b"\n\n", vec![text("\n"), text("\n")]),
        (
            FrameMode::Lines,
            b"\xff\n\xfe",
            vec![
                json!({"data": {"kind": "bytes", "base64": "/wo="}}),
                json!({"data": {"kind": "bytes", "base64": "/g=="}}),
            ],
        ),
        (FrameMode::Whole, b"a\nb", vec![text("a\nb")]),
    ];
    let header = json!({
        "kind": "header",
        "schema": "raw-frames/1",
        "stream": "stdout",
        "clock": {"kind": "unrecorded"},
        "source": format!("sha256:{}", "0".repeat(64)),
    });
    for (mode, input, frames) in cases {
        let wanted: Vec<Value> = frames
            .into_iter()
            .map(|mut f| {
                // INDEX: serde_json's IndexMut inserts the key into an object value
                f["kind"] = json!("frame");
                f
            })
            .collect();
        // After the fact.
        let records = frame_bytes(stdout_name(), mode, input, zero_digest());
        let values: Vec<Value> = records
            .iter()
            .map(|r: &RawRecord| serde_json::to_value(r).expect("a record value"))
            .collect();
        // INDEX: frame_bytes returns the header first, so the slice from 1 exists
        assert_eq!(values[0], header, "{mode:?} {input:?}");
        assert_eq!(&values[1..], wanted.as_slice(), "{mode:?} {input:?}");
        // While writing, in a single chunk and byte by byte: the same frames.
        let one: Vec<&[u8]> = vec![input];
        let singles: Vec<&[u8]> = input.chunks(1).collect();
        for chunks in [one, singles] {
            let (frames, _) = write_and_read(mode, &chunks);
            let got: Vec<Value> = frames
                .iter()
                .map(|f| {
                    let mut v = serde_json::to_value(f).expect("a frame value");
                    // INDEX: serde_json's IndexMut inserts the key into an object value
                    v["kind"] = json!("frame");
                    // The writer stamps each frame with the chunk's time; the case states the shape only.
                    // INDEX: serde_json's Index on a missing key reads as Null, so it cannot panic
                    assert!(v["t_mono_ns"].is_u64());
                    v.as_object_mut().expect("an object").remove("t_mono_ns");
                    v
                })
                .collect();
            assert_eq!(got, wanted, "{mode:?} {input:?} written");
        }
    }
}
