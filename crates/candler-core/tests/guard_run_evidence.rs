#![cfg(test)]
//! Guards for the evidence of a run: the input-leak check and the census.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use candler_core::runner::evidence::census::census;
use candler_core::runner::evidence::leak::input_leak;
use candler_core::runner::evidence::{Evidence, InputDigest, Inputs, OutputDigest};
use candler_harness::reader::{FrameIn, TraceReader};
use candler_harness_claude_code::ClaudeCodeReader;
use candler_schemas::execution::{InvalidReason, RunTree};
use candler_schemas::primitives::Digest;
use candler_schemas::raw::{FrameData, RawFrame, RawRef};
use candler_schemas::values::MeasuredValue;
use flate2::read::MultiGzDecoder;
use serde_json::{Value, json};

// Provenance, B24: the OS `shasum -a 256` of two hand-built 24-byte files.
//   printf 'ground truth 0123456789\n' > in.txt       -> 42b22b52...9738
//   printf 'ground truth 0123456780\n' > changed.txt  -> 6492dc56...25c8
// `changed.txt` differs from `in.txt` in its last digit and has the same size.
const INPUT_SHA: &str = "sha256:42b22b52728d3215127e823468ab6e90dad57cdf69578d9026b4b9e82ce79738";
const CHANGED_SHA: &str = "sha256:6492dc566b9d985350373770c60703943802488188e17a70cf85e8ac23c825c8";

fn digest(text: &str) -> Digest {
    text.parse().expect("a digest")
}

fn evidence(path: &str, sha: &str) -> Evidence {
    Evidence {
        sessions: Vec::new(),
        events: Vec::new(),
        outputs: vec![OutputDigest {
            path: path.to_owned(), // BORROW: the evidence owns its path
            digest: digest(sha),
        }],
        tree: RunTree::builder().build().expect("an empty tree"),
    }
}

/// Guards: I10 — an output that copies an input is refused, whatever its name or size.
/// Oracle: OS sha256 of hand-built files.
/// Bite: compare sizes instead of digests (here, paths, since the evidence holds no sizes).
#[test]
fn b24_digest_equal_output_is_a_leak() {
    let inputs = Inputs {
        digests: vec![InputDigest {
            path: "data/in.txt".to_owned(), // BORROW: the inputs own their path
            digest: digest(INPUT_SHA),
        }],
    };
    assert_eq!(
        input_leak(&evidence("renamed/copy.txt", INPUT_SHA), &inputs),
        Some(InvalidReason::InputLeak)
    );
    // Positive control: one byte changed, same size and same name, stays valid.
    assert_eq!(
        input_leak(&evidence("data/in.txt", CHANGED_SHA), &inputs),
        None
    );
}

/// The lines of the recorded Claude Code stream `synthetic_mcp`, read by relative path.
fn stream_lines() -> Vec<String> {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "../candler-harness-claude-code/tests/fixtures/streams/synthetic_mcp/agent_stream.jsonl.gz",
    ]
    .iter()
    .collect();
    let file = File::open(path).expect("the fixture stream");
    BufReader::new(MultiGzDecoder::new(file))
        .lines()
        .map(|line| line.expect("a line"))
        .collect()
}

/// The census of `lines` read through the Claude Code reader.
fn census_of(lines: &[String]) -> Vec<MeasuredValue> {
    let mut reader = ClaudeCodeReader::new().expect("the reader");
    let mut events = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let frame = RawFrame::new(None, FrameData::from_bytes(line.as_bytes()));
        let at = u64::try_from(index).expect("a frame index");
        let raw_ref = RawRef::new("agent_stream.jsonl".parse().expect("a stream name"), at);
        events.extend(
            reader
                .feed(FrameIn::new(raw_ref, None, &frame))
                .expect("the frame reads"),
        );
    }
    let sessions = Box::new(reader).finish().expect("sessions");
    census(sessions.as_slice(), events.as_slice()).expect("a census") // BORROW: explicit .as_slice()
}

fn value_at(values: &[MeasuredValue], path: &str) -> Value {
    values
        .iter()
        .find(|value| value.path().to_string() == path) // BORROW: .to_string() gives text to compare
        .unwrap_or_else(|| panic!("no value at {path}"))
        .value()
        .as_value()
        .clone() // BORROW: .clone() releases the borrow of the values
}

/// Guards: I10 — the census records the tools and servers the agent saw, and `census.init_tools`
/// holds the sorted tool names only.
/// Oracle: jq over the fixture stream.
/// Bite: write `mcp_servers` into `census.init_tools`.
#[test]
fn b25_census_from_a_real_init() {
    // Provenance: from the stream above, whose first init opens the first session and whose
    // second init opens another session,
    //   gzcat <stream> | jq -c 'select(.type=="system" and .subtype=="init")' | head -1 | jq -c .tools
    //     -> ["Bash","Agent"]
    //   ... | jq -c '.tools|sort'  -> ["Agent","Bash"]
    //   ... | jq -c .mcp_servers   -> []
    let lines = stream_lines();
    let values = census_of(lines.as_slice()); // BORROW: explicit .as_slice()
    assert_eq!(values.len(), 3);
    assert_eq!(value_at(&values, "census.tools"), json!(["Bash", "Agent"]));
    assert_eq!(value_at(&values, "census.mcp_servers"), json!([]));
    assert_eq!(
        value_at(&values, "census.init_tools"),
        json!(["Agent", "Bash"])
    );

    // The recorded init lists no MCP server, so the same stream with two servers listed in its
    // first init is the control that tells the server list from the tool list.
    let servers = json!([
        {"name": "rikyu-hpc", "status": "connected"},
        {"name": "aaa-first", "status": "failed"}
    ]);
    let mut edited = lines;
    let first = edited
        .iter_mut()
        .find(|line| line.contains(r#""subtype":"init""#))
        .expect("an init line");
    let mut frame: Value = serde_json::from_str(first.as_str()).expect("an object"); // BORROW: explicit .as_str()
    if let Some(object) = frame.as_object_mut() {
        object.insert("mcp_servers".to_owned(), servers.clone()); // BORROW: .to_owned() and .clone() give the frame its own copies
    }
    *first = frame.to_string();
    let with_servers = census_of(edited.as_slice()); // BORROW: explicit .as_slice()
    assert_eq!(value_at(&with_servers, "census.mcp_servers"), servers);
    assert_eq!(
        value_at(&with_servers, "census.init_tools"),
        json!(["Agent", "Bash"])
    );
    assert_eq!(
        value_at(&with_servers, "census.tools"),
        json!(["Bash", "Agent"])
    );
}
