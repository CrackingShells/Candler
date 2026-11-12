#![cfg(test)]
//! Guards for the harness facts the core takes from the driver: the version of the harness, the
//! gateway and state variables of the container, the census, and the `comparable` of the repo's
//! experiment files.
//!
//! The recorded values under `tests/fixtures/harness_baseline/` come from cf3de30 (see the
//! README there). A fake second driver, which shares nothing with Claude Code, shows that the
//! core asks the driver and does not assume Claude Code.

use std::path::PathBuf;
use std::str::FromStr as _;
use std::sync::Arc;

use candler_core::isolation::container::mounts::{MountMode, MountRoots, MountSpec, mounts};
use candler_core::runner::evidence::census::{CensusError, census};
use candler_core::runner::exec::spawn::live_observer;
use candler_core::supervise::spawn::FrameObserver as _;
use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::TraceReader;
use candler_harness::spec::{
    BudgetCaps, Capabilities, Cell, CensusSource, CollectSpec, CommandSpec, EnvEntry, GatewayEnv,
    HarnessId, InitReport, Observed, Prepared, ProcessExit, StateKind, StatePath, StopSignal,
    StopSpec, Workspace,
};
use candler_harness_claude_code::ClaudeCodeReader;
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::primitives::RelPath;
use candler_schemas::raw::{FrameData, RawFrame, StreamName};
use candler_schemas::trace::{EndedBy, Session};
use serde_json::{Value, json};

#[path = "support/harness_facts.rs"]
mod facts;
mod support;

/// What cf3de30 recorded, as one JSON value.
fn recorded(relative: &str) -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests/fixtures/harness_baseline",
        relative,
    ]
    .iter()
    .collect();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The words of a recorded JSON array.
fn words(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("an array of words")
        .iter()
        .map(|word| word.as_str().expect("a word").to_owned()) // BORROW: the list owns its words
        .collect()
}

/// A harness that shares nothing with Claude Code: its own program, version text, gateway names
/// and state locations.
struct FakeHarness;

impl FakeHarness {
    const ID: &'static str = "fake-harness";
}

impl HarnessDriver for FakeHarness {
    fn id(&self) -> HarnessId {
        HarnessId::new(Self::ID.parse().expect("a harness name"))
    }

    fn supported(&self) -> semver::VersionReq {
        semver::VersionReq::parse(">=7, <8").expect("a requirement")
    }

    fn version_probe(&self) -> CommandSpec {
        CommandSpec::builder()
            .argv(vec![String::from("fakeharness"), String::from("--semver")])
            .build()
            .expect("a command")
    }

    fn parse_version(&self, output: &[u8]) -> Result<semver::Version, DriverError> {
        let text = String::from_utf8_lossy(output);
        let number =
            text.trim()
                .strip_prefix("fakeharness ")
                .ok_or_else(|| DriverError::VersionOutput {
                    detail: format!("no version in `{}`", text.trim()),
                })?;
        semver::Version::parse(number).map_err(|e| DriverError::VersionOutput {
            detail: e.to_string(),
        })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::builder()
            .graceful_stop(true)
            .budgets(BudgetCaps::new(false, false))
            .build()
    }

    fn gateway(&self) -> Option<GatewayEnv> {
        Some(GatewayEnv::new(
            String::from("FAKE_GATEWAY_URL"),
            String::from("FAKE_GATEWAY_TOKEN"),
            vec![
                String::from("FAKE_GATEWAY_URL"),
                String::from("FAKE_GATEWAY_TOKEN"),
                String::from("FAKE_GATEWAY_KEY"),
            ],
        ))
    }

    fn state_paths(&self) -> Vec<StatePath> {
        vec![
            StatePath::new(
                String::from("FAKE_STATE"),
                StateKind::Dir,
                RelPath::from_str(".fake/state").expect("a path"),
            ),
            StatePath::new(
                String::from("FAKE_DB"),
                StateKind::File,
                RelPath::from_str(".fake/db/fake.db").expect("a path"),
            ),
        ]
    }

    fn prepare(
        &self,
        _cell: &Cell<'_>,
        _workspace: &Workspace<'_>,
    ) -> Result<Prepared, DriverError> {
        Ok(Prepared::default())
    }

    fn command(
        &self,
        _cell: &Cell<'_>,
        _workspace: &Workspace<'_>,
    ) -> Result<CommandSpec, DriverError> {
        Ok(CommandSpec::builder()
            .argv(vec![String::from("fakeharness"), String::from("go")])
            .build()?)
    }

    fn stop(&self) -> StopSpec {
        StopSpec::new(None, StopSignal::Term, std::time::Duration::from_secs(5))
    }

    // TRAIT_OBJECT: the core selects readers at runtime
    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        None
    }

    // TRAIT_OBJECT: the core selects readers at runtime
    fn offline_reader(&self, _stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        None
    }

    fn classify_exit(&self, _exit: &ProcessExit, _sessions: &[Session]) -> EndedBy {
        EndedBy::Completed
    }

    fn collect(&self, _workspace: &Workspace<'_>, _observed: &Observed) -> Vec<CollectSpec> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------------------------
// H1: the version of the harness.
// ---------------------------------------------------------------------------------------------

/// Guards: R01 §Harness drivers — the control container that reads the harness version runs the
/// Claude Code driver's probe, and the version the service reports is the driver's parse of what
/// the probe printed.
/// Oracle: the argv the PATH-fake docker logged at cf3de30 (`argv.json`, recorded there) and the
/// version text the fake printed.
/// Bite: a probe argv that differs from `timeout 10 claude --version`, or a parse that keeps the
/// `(Claude Code)` suffix.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn harness_version_claude_code_probe_equals_cf3de30() {
    let rig = facts::VersionRig::new(
        Arc::new(ClaudeCodeDriver::new().expect("the driver")),
        "2.1.311 (Claude Code)\n",
    );
    let version = rig.version().await.expect("a version");
    let baseline = recorded("argv.json");
    assert_eq!(
        version.to_string(),
        baseline["version"].as_str().expect("text")
    );
    let runs = rig.runs();
    assert_eq!(runs.len(), 1, "one control container ran: {runs:?}");
    assert_eq!(runs[0], words(&baseline["control_version_probe"]));
}

/// Guards: R01 §Harness drivers — the core runs the probe the driver names and reads the version
/// with the driver's parse, so a harness other than Claude Code needs no change in the core.
/// Oracle: the fake driver's own declared probe and version format, written here by hand; the
/// fake docker's log of what the control container ran.
/// Bite: the core's own `claude --version` argv, or its own parse of the output.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn harness_version_comes_from_the_drivers_probe_and_parse() {
    let rig = facts::VersionRig::new(Arc::new(FakeHarness), "fakeharness 7.8.9\n");
    let version = rig
        .version()
        .await
        .expect("the driver's parse reads the output");
    assert_eq!(version, semver::Version::new(7, 8, 9));
    let runs = rig.runs();
    assert_eq!(runs.len(), 1, "one control container ran: {runs:?}");
    let tail = &runs[0][runs[0].len() - 3..];
    assert_eq!(tail, ["10", "fakeharness", "--semver"]);
}

/// Guards: R01 §Harness drivers — an output the driver cannot parse is refused, and the error
/// names the harness by its id.
/// Oracle: the fake driver's declared format; the harness id the fake declares.
/// Bite: the core's own parse, which accepts or refuses by Claude Code's format, or an error text
/// that names a harness other than the driver's.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn harness_version_refusal_names_the_harness() {
    let rig = facts::VersionRig::new(Arc::new(FakeHarness), "9.9.9\n");
    let error = rig
        .version()
        .await
        .expect_err("the driver cannot read this output");
    let text = error.to_string();
    assert!(
        text.contains(FakeHarness::ID),
        "the error names the harness: {text}"
    );
    assert!(
        !text.contains("Claude"),
        "the error names no other harness: {text}"
    );
}

// ---------------------------------------------------------------------------------------------
// H2: the gateway and state variables.
// ---------------------------------------------------------------------------------------------

/// Guards: R01 §Process supervision, §Agent environment — the headless Claude Code `docker run`
/// argv of the reference cell is the one cf3de30 built, byte for byte.
/// Oracle: `argv.json`, recorded at cf3de30 by the same producer over the same plan.
/// Bite: a gateway or state variable name that differs from Claude Code's, or a state variable
/// set for a driver that has no state path.
#[test]
fn harness_env_claude_code_argv_equals_cf3de30() {
    let dir = tempfile::tempdir().expect("a directory");
    let scene = facts::scene(dir.path());
    let driver = ClaudeCodeDriver::new().expect("the driver");
    let argv = facts::agent_argv(&scene, &driver, &facts::reference_command()).expect("an argv");
    let baseline = recorded("argv.json");
    assert_eq!(argv, words(&baseline["headless_run"]));
}

/// The command of the fake harness with `overlay` as its plain environment.
fn fake_command(overlay: Vec<EnvEntry>) -> CommandSpec {
    let mut builder =
        CommandSpec::builder().argv(vec![String::from("fakeharness"), String::from("go")]);
    for entry in overlay {
        builder = builder.env(entry);
    }
    builder.build().expect("a command")
}

/// Guards: R01 §Harness drivers — the container's gateway and state variables are the driver's:
/// a driver that names its own variables gets them, with the base URL, the token and the
/// per-run state locations, and no `ANTHROPIC_*` variable appears.
/// Oracle: the fake driver's declared names and the scene's home directory, written here by hand.
/// Bite: `ANTHROPIC_*` hardcoded in the core, or the state variables left unset.
#[test]
fn harness_env_second_driver_names_reach_the_argv() {
    let dir = tempfile::tempdir().expect("a directory");
    let scene = facts::scene(dir.path());
    let argv = facts::agent_argv(&scene, &FakeHarness, &fake_command(Vec::new())).expect("an argv");
    let pair = |name: &str, value: &str| {
        let wanted = format!("{name}={value}");
        argv.windows(2).any(|w| w[0] == "-e" && w[1] == wanted)
    };
    assert!(pair("FAKE_GATEWAY_URL", facts::BASE_URL), "{argv:?}");
    assert!(pair("FAKE_GATEWAY_TOKEN", facts::DUMMY_TOKEN), "{argv:?}");
    assert!(pair("FAKE_STATE", "/home/agent/.fake/state"), "{argv:?}");
    assert!(pair("FAKE_DB", "/home/agent/.fake/db/fake.db"), "{argv:?}");
    assert!(
        !argv.iter().any(|word| word.contains("ANTHROPIC")),
        "no Claude Code name: {argv:?}"
    );
    // The core makes each state location per run: a directory, and the parent of a file.
    assert!(scene.home.join(".fake/state").is_dir());
    assert!(scene.home.join(".fake/db").is_dir());
    assert!(!scene.home.join(".fake/db/fake.db").exists());
}

/// Guards: R01 §Agent environment — the names the driver reserves are refused in its plain
/// environment, and the names Claude Code reserves are not reserved for another driver.
/// Oracle: the fake driver's declared reserved list, written here by hand.
/// Bite: a reserved list fixed in the core (the fake's reserved name accepted, or a Claude Code
/// name refused for the fake).
#[test]
fn harness_env_reserved_names_are_the_drivers() {
    let dir = tempfile::tempdir().expect("a directory");
    let scene = facts::scene(dir.path());
    let entry = |name: &str| EnvEntry::literal(name.to_owned(), String::from("v")); // BORROW: the entry owns its name
    let own = facts::agent_argv(
        &scene,
        &FakeHarness,
        &fake_command(vec![entry("FAKE_GATEWAY_KEY")]),
    );
    assert!(
        own.is_err(),
        "the driver's reserved name is refused: {own:?}"
    );
    let other = facts::agent_argv(
        &scene,
        &FakeHarness,
        &fake_command(vec![entry("ANTHROPIC_BASE_URL")]),
    );
    assert!(
        other.is_ok(),
        "a name the driver does not reserve is allowed: {other:?}"
    );
}

/// Guards: R01 §Process supervision — a state location the harness writes must sit in a writable
/// mount: one under a read-only mount is refused, and one in the writable home is accepted.
/// Oracle: the mount modes the test sets, and the rule that a nested read-only mount hides the
/// writable parent; written here by hand.
/// Bite: the check dropped (a state path under a read-only mount accepted), or the check made over
/// the home alone.
#[test]
fn harness_env_state_path_under_a_read_only_mount_is_refused() {
    let dir = tempfile::tempdir().expect("a directory");
    let mut scene = facts::scene(dir.path());
    let ok = facts::agent_argv(&scene, &FakeHarness, &fake_command(Vec::new()));
    assert!(ok.is_ok(), "a state path in the writable home: {ok:?}");
    let shadow = scene.home.join(".fake/state");
    std::fs::create_dir_all(&shadow).expect("a directory");
    scene.plan.mounts.push(MountSpec {
        source: shadow,
        target: String::from("/home/agent/.fake/state"),
        mode: MountMode::ReadOnly,
    });
    let refused = facts::agent_argv(&scene, &FakeHarness, &fake_command(Vec::new()));
    assert!(
        refused.is_err(),
        "a state path under a read-only mount: {refused:?}"
    );
    // Claude Code has no state path, so the same mount list does not concern it.
    let claude = ClaudeCodeDriver::new().expect("the driver");
    let argv = facts::agent_argv(&scene, &claude, &facts::reference_command());
    assert!(argv.is_ok(), "no state path to check: {argv:?}");
}

/// Guards: R01 §Process supervision — a mount list with no writable home is refused, as before.
/// Oracle: the mount modes the test sets.
/// Bite: the home check dropped.
#[test]
fn harness_env_a_read_only_home_is_refused() {
    let dir = tempfile::tempdir().expect("a directory");
    let mut scene = facts::scene(dir.path());
    for mount in &mut scene.plan.mounts {
        if mount.target == "/home/agent" {
            mount.mode = MountMode::ReadOnly;
        }
    }
    let roots = MountRoots::from_plan(&scene.plan);
    assert!(mounts(&scene.plan, &roots).is_err());
}

// ---------------------------------------------------------------------------------------------
// H3: the census.
// ---------------------------------------------------------------------------------------------

/// Guards: R01 §Run manifest (`census.*`) — the three census values of every recorded Claude Code
/// stream are the ones cf3de30 recorded, byte for byte.
/// Oracle: `census.json`, recorded at cf3de30 from the same streams.
/// Bite: the census read by event key in the core with a reordered key, a list of tool names
/// that is not sorted and deduplicated for `census.init_tools`, or a stream recorded as empty.
#[test]
fn harness_census_equals_cf3de30_on_every_fixture() {
    let baseline = recorded("census.json");
    let baseline = baseline.as_object().expect("an object");
    let streams = facts::stream_paths();
    assert!(streams.len() >= 8, "the fixtures are found: {streams:?}");
    let mut recorded_some = 0;
    for (name, path) in &streams {
        let actual = facts::census_value(&facts::lines_of(path));
        let expected = baseline
            .get(name)
            .unwrap_or_else(|| panic!("no baseline for {name}"));
        assert_eq!(&actual, expected, "census of {name}");
        if !actual.is_null() {
            recorded_some += 1;
        }
    }
    assert_eq!(
        baseline.len(),
        streams.len(),
        "every baseline entry has a fixture"
    );
    assert!(recorded_some >= 6, "the check is not over empty values");
}

/// The inventory a source reported, as the driver's reader builds it.
fn inventory(tools: Value, servers: Option<Value>, source: CensusSource) -> InitReport {
    InitReport::new(tools, servers, source).expect("an inventory")
}

/// The paths and values of `census` over `init`.
fn census_pairs(init: Option<&InitReport>) -> Vec<(String, Value)> {
    census(init)
        .expect("a census")
        .iter()
        .map(|value| (value.path().to_string(), value.value().as_value().clone())) // BORROW: the pairs own their values
        .collect()
}

/// Guards: R01 §Run manifest (`census.*`) — an inventory from a source that sees no servers
/// records `census.tools` and `census.init_tools` only, never an empty server list.
/// Oracle: the sorted, deduplicated tool names, written here by hand.
/// Bite: `census.mcp_servers` recorded as an empty list, or `census.init_tools` left unsorted.
#[test]
fn harness_census_without_servers_records_tools_only() {
    let init = inventory(json!(["b", "a", "b"]), None, CensusSource::Gateway);
    assert_eq!(
        census_pairs(Some(&init)),
        [
            (String::from("census.tools"), json!(["b", "a", "b"])),
            (String::from("census.init_tools"), json!(["a", "b"])),
        ]
    );
}

/// Guards: R01 §Run manifest (`census.*`) — an inventory from the harness's own init lists the
/// three values in the order of the baseline, and the server list stays out of
/// `census.init_tools`.
/// Oracle: the order and shape of `census.json`, recorded at cf3de30, and a server list written
/// here by hand.
/// Bite: servers merged into `census.init_tools`, or the three values reordered.
#[test]
fn harness_census_with_servers_records_the_three_values() {
    let servers = json!([{"name": "s", "status": "connected"}]);
    let init = inventory(
        json!(["Bash", "Agent"]),
        Some(servers.clone()),
        CensusSource::Harness,
    );
    assert_eq!(
        census_pairs(Some(&init)),
        [
            (String::from("census.tools"), json!(["Bash", "Agent"])),
            (String::from("census.mcp_servers"), servers),
            (String::from("census.init_tools"), json!(["Agent", "Bash"])),
        ]
    );
}

/// Guards: R01 §Run manifest (`census.*`) — a harness that reported no inventory records nothing:
/// no value, and no empty list.
/// Oracle: the rule that an absent inventory is unrecorded.
/// Bite: an absent inventory recorded as an empty tool list.
#[test]
fn harness_census_absent_report_records_nothing() {
    assert!(matches!(census(None), Err(CensusError::NoInit)));
}

/// Guards: R01 §Run manifest (`census.*`) — the inventory the live reader saw is readable while
/// the run is still going, before the reader finishes, and it is the first init of the stream.
/// Oracle: the baseline census of the same stream, `census.json`.
/// Bite: `LiveHandle::observed` returning an empty view, or reading only after `finish`.
#[test]
fn harness_census_live_handle_reports_the_inventory_before_finish() {
    let baseline = recorded("census.json");
    let (name, path) = facts::stream_paths()
        .into_iter()
        .find(|(name, _)| name == "streams/synthetic_mcp")
        .expect("the stream fixture");
    let reader = ClaudeCodeReader::new().expect("the reader");
    let stream: StreamName = "stdout".parse().expect("a stream name");
    let (mut observer, handle) = live_observer(Box::new(reader), stream);
    for (index, line) in facts::lines_of(&path).iter().enumerate() {
        let frame = RawFrame::new(None, FrameData::from_bytes(line.as_bytes()));
        observer.frame(u64::try_from(index).expect("an index"), &frame);
    }
    let observed = handle.observed();
    let init = observed
        .init
        .as_ref()
        .expect("the inventory is observed before finish");
    assert_eq!(init.tools, baseline[name.as_str()]["census.tools"]);
    assert!(
        !observed.session_ids.is_empty(),
        "the session ids are observed"
    );
    handle.finish().expect("the reader finishes");
}

// ---------------------------------------------------------------------------------------------
// H4: the comparable.
// ---------------------------------------------------------------------------------------------

/// Guards: R01 §Run manifest (`comparable.harness`) — the repo's experiment files resolve to the
/// `comparable` cf3de30 resolved, byte for byte, and the harness identity holds a name and a
/// version only.
/// Oracle: `comparable/*.json`, recorded at cf3de30 from the same files.
/// Bite: a `driver` entry in `comparable.harness`, or any value of the cell that moves.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn harness_resolution_comparable_equals_cf3de30() {
    let dir = tempfile::tempdir().expect("a directory");
    let experiments = facts::experiments(dir.path());
    assert!(experiments.len() >= 4, "the experiment files are found");
    for (name, file) in &experiments {
        let cells = facts::comparables(file, dir.path()).await;
        assert!(!cells.is_empty(), "{name} plans cells");
        for cell in &cells {
            let harness = cell["harness"].as_object().expect("a harness identity");
            let keys: Vec<&str> = harness.keys().map(String::as_str).collect();
            assert_eq!(keys, ["name", "version"], "{name}: harness identity");
        }
        assert_eq!(
            Value::Array(cells),
            recorded(&format!("comparable/{name}.json")),
            "comparable of {name}"
        );
    }
}
