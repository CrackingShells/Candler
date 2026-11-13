#![cfg(test)]
//! Guards for the evidence of a run: the input-leak check, the census and the run tree.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;

use candler_core::isolation::image::parse_packages;
use candler_core::jobs::model::{JobCause, JobControl};
use candler_core::jobs::orchestrate::orchestrate_with;
use candler_core::runner::evidence::census::census;
use candler_core::runner::evidence::leak::input_leak;
use candler_core::runner::evidence::tree::tree_json;
use candler_core::runner::evidence::{Evidence, InputDigest, Inputs, OutputDigest};
use candler_harness::reader::{FrameIn, TraceReader};
use candler_harness_claude_code::ClaudeCodeReader;
use candler_schemas::execution::{InvalidReason, RunTree};
use candler_schemas::primitives::Digest;
use candler_schemas::raw::{FrameData, RawFrame, RawRef};
use candler_schemas::values::MeasuredValue;
use flate2::read::MultiGzDecoder;
use serde_json::{Value, json};
use support::job_orchestrate::{FakeReads, Rig, spec};
use support::run_plan::Facts;
use tokio::sync::watch;

mod support;

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

/// The census of `lines` read through the Claude Code reader: the inventory the reader observed.
fn census_of(lines: &[String]) -> Vec<MeasuredValue> {
    let mut reader = ClaudeCodeReader::new().expect("the reader");
    for (index, line) in lines.iter().enumerate() {
        let frame = RawFrame::new(None, FrameData::from_bytes(line.as_bytes()));
        let at = u64::try_from(index).expect("a frame index");
        let raw_ref = RawRef::new("agent_stream.jsonl".parse().expect("a stream name"), at);
        reader
            .feed(FrameIn::new(raw_ref, None, &frame))
            .expect("the frame reads");
    }
    census(reader.observed().init.as_ref()).expect("a census")
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

/// The name of the file that marks a directory as an agent environment.
const ENV_MARKER: &str = "pyvenv.cfg";

/// The evidence of a run that produced `paths`, each with the same content.
fn outputs_of(paths: &[&str]) -> Evidence {
    Evidence {
        outputs: paths
            .iter()
            .map(|path| OutputDigest {
                path: (*path).to_owned(), // BORROW: the evidence owns its path
                digest: digest(INPUT_SHA),
            })
            .collect(),
        ..evidence("unused", INPUT_SHA)
    }
}

/// The paths of the tree built from `evidence` and `inputs`, in tree order.
fn tree_paths(evidence: &Evidence, inputs: &Inputs) -> Vec<String> {
    tree_json(evidence, inputs)
        .expect("a tree")
        .entries()
        .iter()
        .map(|entry| entry.path().as_str().to_owned()) // BORROW: the list owns its paths
        .collect()
}

/// Guards: the run tree lists what the agent produced. No path under a directory that holds the
/// environment marker enters it (the marker included), at any depth, and no path under a
/// read-only input mount point does; a sibling whose name only starts like one stays.
/// Oracle: the fixture tree of the driver's discovery guard (`out/a.txt`, a `venv/` with its
/// marker and `lib/`, a read-only input path), extended with a nested environment and
/// near-miss names; the kept list is written by hand.
/// Bite: make the marker match no output name, or drop the input mount points from the filter.
#[test]
fn b39_environment_and_input_mounts_never_enter_the_tree() {
    let marker = format!("venv/{ENV_MARKER}");
    let nested = format!("tools/env/{ENV_MARKER}");
    let near_marker = format!("venvish/{ENV_MARKER}.bak");
    let produced = outputs_of(&[
        "out/a.txt",
        marker.as_str(), // BORROW: explicit .as_str()
        "venv/lib/python3.12/site-packages/x.py",
        nested.as_str(), // BORROW: explicit .as_str()
        "tools/env/bin/python",
        "tools/run.sh",
        near_marker.as_str(), // BORROW: explicit .as_str()
        "inputs/corpus/a.txt",
        "inputs/corpus/deep/b.txt",
        "inputs/corpusx/b.txt",
    ]);
    let inputs = Inputs {
        digests: vec![InputDigest {
            path: "inputs/corpus".to_owned(), // BORROW: the inputs own their path
            digest: digest(INPUT_SHA),
        }],
    };
    assert_eq!(
        tree_paths(&produced, &inputs),
        [
            "inputs/corpusx/b.txt",
            "out/a.txt",
            "tools/run.sh",
            near_marker.as_str(), // BORROW: explicit .as_str()
        ]
    );
    // Positive control: with no marker and no input, every output is listed.
    let plain = outputs_of(&["out/a.txt", "venv/lib/python3.12/site-packages/x.py"]);
    assert_eq!(
        tree_paths(&plain, &Inputs::default()),
        ["out/a.txt", "venv/lib/python3.12/site-packages/x.py"]
    );
}

/// Guards: an environment made at the workspace root makes the whole workspace an environment,
/// so the run tree is empty.
/// Oracle: the empty list.
/// Bite: make the marker match no output name.
#[test]
fn b39_an_environment_at_the_root_hides_the_workspace() {
    let produced = outputs_of(&[ENV_MARKER, "out/a.txt", "lib/x.py"]);
    assert_eq!(
        tree_paths(&produced, &Inputs::default()),
        Vec::<String>::new()
    );
}

/// The manifest a one-run job seals, with the names the fake container read was asked for.
///
/// The job runs over a fake docker client whose env image holds `x` and `y`; `record` is the
/// value of `isolation.record_packages`, when the experiment sets it.
fn sealed_with(record: Option<&[&str]>) -> (Value, Vec<Vec<String>>) {
    // The image's answer, as the probe script prints it.
    let held = parse_packages(r#"{"x": [["x-dist", "1.2.3"]], "y": [["y-dist", "4.5.6"]]}"#)
        .expect("a probe answer");
    let reads = Arc::new(FakeReads {
        held,
        ..FakeReads::default()
    });
    let rig = Rig::new(0);
    let facts = Facts {
        secrets: Vec::new(),
        ..Facts::base()
    };
    let mut job = spec(&["m1"], &facts, 3600, 1);
    if let Some(names) = record {
        let mut resolved = serde_json::to_value(&job.resolution).expect("resolution JSON");
        resolved["settings"]
            .as_array_mut()
            .expect("settings")
            .push(json!({"path": "isolation.record_packages", "value": names}));
        resolved["sources"]
            .as_array_mut()
            .expect("sources")
            .push(json!({
                "path": "isolation.record_packages",
                "source": {"kind": "experiment_file"}
            }));
        job.resolution = serde_json::from_value(resolved).expect("a resolution");
    }
    let (cancel, _keep) = watch::channel::<Option<JobCause>>(None);
    let ctl = JobControl {
        cancel: Arc::new(cancel),
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let outcome = runtime.block_on(orchestrate_with(
        rig.runtime(),
        job,
        ctl,
        rig.host_reading(&reads),
    ));
    let key = outcome.runs.first().expect("one run").key.clone(); // BORROW: .clone() owns the key
    let path = rig
        .store
        .layout()
        .root()
        .join("runs")
        .join(key.to_string())
        .join("manifest.json");
    let manifest = serde_json::from_slice(&std::fs::read(path).expect("a manifest file"))
        .expect("manifest JSON");
    let asked = reads.asked.lock().expect("the log of reads").clone(); // BORROW: .clone() owns the log
    (manifest, asked)
}

/// The paths of the values a manifest lists under `field`.
fn paths_under(manifest: &Value, field: &[&str]) -> Vec<String> {
    let mut at = manifest;
    for key in field {
        at = &at[*key];
    }
    at.as_array()
        .expect("a list of values")
        .iter()
        .map(|value| value["path"].as_str().expect("a path").to_owned()) // BORROW: the list owns its paths
        .collect()
}

/// The value the manifest measured at `path`.
fn measured_at(manifest: &Value, path: &str) -> Option<Value> {
    manifest["measured"]
        .as_array()?
        .iter()
        .find(|value| value["path"] == path)
        .map(|value| value["value"].clone()) // BORROW: .clone() owns the value
}

/// Guards: R01 §Run manifest — the packages an experiment names in `isolation.record_packages`
/// are measured, each as `provenance.packages.<import>.distribution` and `.version`, and never
/// enter the cell's `comparable`; a package it does not name is not read, and with no setting no
/// `provenance.packages.*` entry is written.
/// Oracle: the manifest rows are written by hand below from the fake image's `x` (x-dist 1.2.3)
/// and `y` (y-dist 4.5.6), never from the provenance code.
/// Bite: ignore the setting in `jobs/orchestrate/mod.rs` (no entries), or write the readings
/// under another path in `provenance.rs`. A setting classed as a cell value, which would put it
/// into `comparable`, is `b19_every_path_class`'s bite.
#[test]
fn a2_record_packages_are_measured_never_comparable() {
    let (named, asked) = sealed_with(Some(&["x"]));
    assert_eq!(
        asked,
        [["x"]],
        "the container read asked for the named package only"
    );
    assert_eq!(
        measured_at(&named, "provenance.packages.x.distribution"),
        Some(json!("x-dist"))
    );
    assert_eq!(
        measured_at(&named, "provenance.packages.x.version"),
        Some(json!("1.2.3"))
    );
    let packages: Vec<String> = paths_under(&named, &["measured"])
        .into_iter()
        .filter(|path| path.starts_with("provenance.packages."))
        .collect();
    assert_eq!(
        packages,
        [
            "provenance.packages.x.distribution",
            "provenance.packages.x.version"
        ],
        "the unnamed package `y` is not recorded"
    );
    let cell = paths_under(&named, &["comparable", "values"]);
    assert!(
        cell.iter().all(|path| !path.starts_with("provenance.")),
        "the cell holds no provenance value: {cell:?}"
    );

    // Positive control: no setting, no read and no entry.
    let (none, asked) = sealed_with(None);
    assert!(asked.is_empty(), "no setting, no container read");
    assert!(
        paths_under(&none, &["measured"])
            .iter()
            .all(|path| !path.starts_with("provenance.packages.")),
        "no setting, no provenance.packages entry"
    );
    assert!(
        measured_at(&none, "provenance.sandbox_contract").is_some(),
        "the manifest still measures provenance"
    );
}
