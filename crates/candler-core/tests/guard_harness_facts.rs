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

use candler_core::isolation::IsolationError;
use candler_core::isolation::container::mounts::{MountError, MountMode, MountSpec};
use candler_core::isolation::container::run::RunArgvError;
use candler_core::isolation::image::ImageError;
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

/// A harness that shares nothing with Claude Code: its own program, version text, state locations
/// and gateway names, which are the neutral pair an OpenCode-shaped driver declares (neither of
/// them is an `ANTHROPIC_*` name).
struct FakeHarness {
    shape: Shape,
}

/// How a fake harness departs from the plain one.
#[derive(Clone, Copy)]
enum Shape {
    /// The harness as declared: a gateway, two state paths, a plain probe.
    Plain,
    /// A harness that names no gateway variables.
    NoGateway,
    /// A harness whose version probe sets an environment variable.
    EnvProbe,
    /// A harness whose version probe sets a working directory.
    CwdProbe,
}

impl FakeHarness {
    const ID: &'static str = "fake-harness";

    const fn plain() -> Self {
        Self::shaped(Shape::Plain)
    }

    const fn shaped(shape: Shape) -> Self {
        Self { shape }
    }
}

impl HarnessDriver for FakeHarness {
    fn id(&self) -> HarnessId {
        HarnessId::new(Self::ID.parse().expect("a harness name"))
    }

    fn supported(&self) -> semver::VersionReq {
        semver::VersionReq::parse(">=7, <8").expect("a requirement")
    }

    fn version_probe(&self) -> CommandSpec {
        let mut builder = CommandSpec::builder()
            .argv(vec![String::from("fakeharness"), String::from("--semver")]);
        match self.shape {
            Shape::EnvProbe => {
                builder = builder.env(EnvEntry::literal(
                    String::from("FAKE_PROBE"),
                    String::from("1"),
                ));
            }
            Shape::CwdProbe => {
                builder = builder.cwd(RelPath::from_str("sub").expect("a path"));
            }
            Shape::Plain | Shape::NoGateway => {}
        }
        builder.build().expect("a command")
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
        if matches!(self.shape, Shape::NoGateway) {
            return None;
        }
        Some(GatewayEnv::new(
            String::from("CANDLER_GATEWAY_URL"),
            String::from("CANDLER_GATEWAY_TOKEN"),
            vec![
                String::from("CANDLER_GATEWAY_URL"),
                String::from("CANDLER_GATEWAY_TOKEN"),
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

/// What a row of the version table expects.
enum VersionWant {
    /// The version the service reports, and the words the control container's argv ends with.
    Read {
        version: semver::Version,
        argv_ends: Vec<String>,
        exact: bool,
    },
    /// A refusal as `ImageError::VersionUnreadable`, whose text holds and omits the given words,
    /// after this many control containers ran.
    Unreadable {
        names: &'static [&'static str],
        omits: &'static [&'static str],
        runs: usize,
    },
}

/// Guards: R01 §Harness drivers — the control container that reads the harness version runs the
/// probe the driver names, and the version the service reports is the driver's parse of what the
/// probe printed: the Claude Code probe is the argv cf3de30 ran; another driver's own probe and
/// format are used, with no change in the core. An output the driver cannot parse is refused as
/// `VersionUnreadable` naming the harness and its probe and no other harness, and a probe that
/// sets an environment or a directory, which a control container cannot honour, is refused before
/// any container runs.
/// Oracle: the argv the PATH-fake docker logged at cf3de30 (`argv.json`, recorded there), the
/// version text the fake printed, and the fake driver's own declared probe, format and id,
/// written here by hand.
/// Bite: a probe argv that differs from `timeout 10 claude --version`, a parse that keeps the
/// `(Claude Code)` suffix, the core's own `claude --version` argv or parse, an error that names a
/// harness other than the driver's, or the probe's environment and directory check removed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn harness_version_comes_from_the_drivers_probe_and_parse() {
    let baseline = recorded("argv.json");
    let claude: Arc<dyn HarnessDriver> = Arc::new(ClaudeCodeDriver::new().expect("the driver"));
    let rows: Vec<(&str, Arc<dyn HarnessDriver>, &str, VersionWant)> = vec![
        (
            "Claude Code's probe equals cf3de30's",
            claude,
            "2.1.311 (Claude Code)\n",
            VersionWant::Read {
                version: baseline["version"]
                    .as_str()
                    .expect("text")
                    .parse()
                    .expect("a version"),
                argv_ends: words(&baseline["control_version_probe"]),
                exact: true,
            },
        ),
        (
            "another driver's probe and parse",
            Arc::new(FakeHarness::plain()),
            "fakeharness 7.8.9\n",
            VersionWant::Read {
                version: semver::Version::new(7, 8, 9),
                argv_ends: ["10", "fakeharness", "--semver"].map(String::from).to_vec(),
                exact: false,
            },
        ),
        (
            "an output the driver cannot parse",
            Arc::new(FakeHarness::plain()),
            "9.9.9\n",
            VersionWant::Unreadable {
                names: &[FakeHarness::ID, "fakeharness --semver"],
                omits: &["Claude"],
                runs: 1,
            },
        ),
        (
            "a probe that sets an environment variable",
            Arc::new(FakeHarness::shaped(Shape::EnvProbe)),
            "fakeharness 7.8.9\n",
            VersionWant::Unreadable {
                names: &[FakeHarness::ID, "environment or a directory"],
                omits: &["Claude"],
                runs: 0,
            },
        ),
        (
            "a probe that sets a directory",
            Arc::new(FakeHarness::shaped(Shape::CwdProbe)),
            "fakeharness 7.8.9\n",
            VersionWant::Unreadable {
                names: &[FakeHarness::ID, "environment or a directory"],
                omits: &["Claude"],
                runs: 0,
            },
        ),
    ];
    for (case, driver, output, want) in rows {
        let rig = facts::VersionRig::new(driver, output);
        let got = rig.version().await;
        let runs = rig.runs();
        match want {
            VersionWant::Read {
                version,
                argv_ends,
                exact,
            } => {
                assert_eq!(got.as_ref().ok(), Some(&version), "{case}: {got:?}");
                assert_eq!(runs.len(), 1, "{case}: one control container ran: {runs:?}");
                assert!(
                    runs[0].ends_with(&argv_ends),
                    "{case}: the control container ran {:?}",
                    runs[0]
                );
                if exact {
                    assert_eq!(runs[0], argv_ends, "{case}: the whole argv");
                }
            }
            VersionWant::Unreadable {
                names,
                omits,
                runs: ran,
            } => {
                let Err(ImageError::VersionUnreadable { detail }) = got else {
                    panic!("{case}: not refused as VersionUnreadable: {got:?}");
                };
                for word in names {
                    assert!(
                        detail.contains(word),
                        "{case}: the error names {word}: {detail}"
                    );
                }
                for word in omits {
                    assert!(
                        !detail.contains(word),
                        "{case}: the error omits {word}: {detail}"
                    );
                }
                assert_eq!(runs.len(), ran, "{case}: control containers run: {runs:?}");
            }
        }
    }
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
    let argv = facts::agent_argv(&scene, &FakeHarness::plain(), &fake_command(Vec::new()))
        .expect("an argv");
    let pair = |name: &str, value: &str| {
        let wanted = format!("{name}={value}");
        argv.windows(2).any(|w| w[0] == "-e" && w[1] == wanted)
    };
    assert!(pair("CANDLER_GATEWAY_URL", facts::BASE_URL), "{argv:?}");
    assert!(
        pair("CANDLER_GATEWAY_TOKEN", facts::DUMMY_TOKEN),
        "{argv:?}"
    );
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

/// What a row of an argv table expects.
enum ArgvWant {
    /// An argv.
    Built,
    /// The overlay sets this reserved name.
    Reserved(&'static str),
    /// The plan already sets this gateway variable.
    Duplicate(&'static str),
    /// The driver names no gateway variables.
    NoGateway,
    /// The plan already sets this state variable.
    StateSet(&'static str),
    /// The plan reaches the `docker run` argv with no gateway variables.
    PlanNoGateway,
    /// No writable mount holds the home or a state path.
    NotWritable,
}

/// Whether `got` is the refusal `want` names, by variant.
fn is_refusal(got: &Result<Vec<String>, facts::ArgvRefusal>, want: &ArgvWant) -> bool {
    match (want, got) {
        (ArgvWant::Built, Ok(_))
        | (
            ArgvWant::NotWritable,
            Err(facts::ArgvRefusal::Run(RunArgvError::Mount(MountError::ConfigDirNotWritable))),
        ) => true,
        (
            ArgvWant::Reserved(name),
            Err(facts::ArgvRefusal::Run(RunArgvError::ReservedName { name: got })),
        )
        | (
            ArgvWant::Duplicate(name),
            Err(facts::ArgvRefusal::Run(RunArgvError::DuplicateName { name: got })),
        ) => got == name,
        (ArgvWant::NoGateway, Err(facts::ArgvRefusal::Env(IsolationError::Plan { detail }))) => {
            detail.contains("names no gateway variables") && detail.contains(FakeHarness::ID)
        }
        (
            ArgvWant::StateSet(name),
            Err(facts::ArgvRefusal::Env(IsolationError::Plan { detail })),
        ) => detail.contains(&format!("state variable {name} is already set")),
        (
            ArgvWant::PlanNoGateway,
            Err(facts::ArgvRefusal::Run(RunArgvError::Request { detail })),
        ) => detail.contains("names no gateway variables"),
        _ => false,
    }
}

/// A row of the refusal table: the case, the driver, the overlay names, the plan variables, whether
/// the plan reaches the `docker run` argv with no gateway, and the expected outcome.
type EnvRow = (
    &'static str,
    FakeHarness,
    Vec<&'static str>,
    Vec<&'static str>,
    bool,
    ArgvWant,
);

/// Guards: R01 §Harness drivers, §Agent environment — what the core refuses of a driver's
/// variables, each by its variant: the names the driver reserves are refused in its plain
/// environment and no other name is reserved; a plan variable that repeats a gateway variable, a
/// state variable the plan already sets, a driver with no gateway variables and a plan that
/// reaches the `docker run` argv with none are refused.
/// Oracle: the fake driver's declared names and reserved list, written here by hand, and the
/// variants of the refusal enums.
/// Bite: a reserved list fixed in the core (the fake's reserved name accepted, or a Claude Code
/// name refused for the fake); the gateway-name collision check in `container/run.rs` removed; the
/// `already set` check or the no-gateway refusal in `apply_harness_env` removed; the no-gateway
/// refusal in `agent_argv` removed.
#[test]
fn harness_env_refusals_name_their_variant() {
    let entry = |name: &str| EnvEntry::literal(name.to_owned(), String::from("v")); // BORROW: the entry owns its name
    let dir = tempfile::tempdir().expect("a directory");
    let scene = facts::scene(dir.path());
    let ordinary = FakeHarness::plain;
    // (case, driver, overlay names, plan variables, reaches the argv with no gateway, expected)
    let rows: Vec<EnvRow> = vec![
        (
            "the driver's reserved name",
            ordinary(),
            vec!["FAKE_GATEWAY_KEY"],
            vec![],
            false,
            ArgvWant::Reserved("FAKE_GATEWAY_KEY"),
        ),
        (
            "a name the driver does not reserve",
            ordinary(),
            vec!["ANTHROPIC_BASE_URL"],
            vec![],
            false,
            ArgvWant::Built,
        ),
        (
            "a plan variable named like the gateway URL",
            ordinary(),
            vec![],
            vec!["CANDLER_GATEWAY_URL"],
            false,
            ArgvWant::Duplicate("CANDLER_GATEWAY_URL"),
        ),
        (
            "a plan variable named like the gateway token",
            ordinary(),
            vec![],
            vec!["CANDLER_GATEWAY_TOKEN"],
            false,
            ArgvWant::Duplicate("CANDLER_GATEWAY_TOKEN"),
        ),
        (
            "a state variable the plan already sets",
            ordinary(),
            vec![],
            vec!["FAKE_STATE"],
            false,
            ArgvWant::StateSet("FAKE_STATE"),
        ),
        (
            "a driver with no gateway variables",
            FakeHarness::shaped(Shape::NoGateway),
            vec![],
            vec![],
            false,
            ArgvWant::NoGateway,
        ),
        (
            "a plan with no gateway reaching the argv",
            ordinary(),
            vec![],
            vec![],
            true,
            ArgvWant::PlanNoGateway,
        ),
    ];
    for (case, driver, overlay, plan_env, direct, want) in rows {
        let mut plan = scene.plan.clone(); // BORROW: each row plans over its own copy
        for name in plan_env {
            plan.env.push((name.to_owned(), String::from("set"))); // BORROW: the plan owns its names
        }
        let command = fake_command(overlay.into_iter().map(entry).collect());
        let got = if direct {
            facts::plan_argv(&scene, &plan, &command)
        } else {
            facts::plan_argv_over(&scene, plan, &driver, &command)
        };
        assert!(is_refusal(&got, &want), "{case}: {got:?}");
    }
}

/// Which mount an argv row adds to the scene.
enum Mounted {
    /// Only the scene's own mounts.
    Own,
    /// A read-only mount over a state path of the fake harness.
    ReadOnlyState,
    /// The home mounted read-only.
    ReadOnlyHome,
}

/// Guards: R01 §Process supervision — the home and every state location the harness writes sit in
/// a writable mount: a state location under a read-only mount is refused, one in the writable home
/// is accepted, a driver with no state path is not concerned by a read-only mount over another
/// driver's path, and a mount list with a read-only home is refused.
/// Oracle: the mount modes the test sets, and the rule that a nested read-only mount hides the
/// writable parent; written here by hand.
/// Bite: the state check dropped (a state path under a read-only mount accepted), the check made
/// over the home alone, or the home check dropped.
#[test]
fn harness_env_state_paths_are_in_writable_mounts() {
    let claude = ClaudeCodeDriver::new().expect("the driver");
    // (case, driver is Claude Code, mount, expected)
    let rows = [
        (
            "a state path in the writable home",
            false,
            Mounted::Own,
            ArgvWant::Built,
        ),
        (
            "a state path under a read-only mount",
            false,
            Mounted::ReadOnlyState,
            ArgvWant::NotWritable,
        ),
        (
            "no state path to check under that mount",
            true,
            Mounted::ReadOnlyState,
            ArgvWant::Built,
        ),
        (
            "a read-only home",
            true,
            Mounted::ReadOnlyHome,
            ArgvWant::NotWritable,
        ),
    ];
    for (case, is_claude, mounted, want) in rows {
        let dir = tempfile::tempdir().expect("a directory");
        let mut scene = facts::scene(dir.path());
        match mounted {
            Mounted::Own => {}
            Mounted::ReadOnlyState => {
                let shadow = scene.home.join(".fake/state");
                std::fs::create_dir_all(&shadow).expect("a directory");
                scene.plan.mounts.push(MountSpec {
                    source: shadow,
                    target: String::from("/home/agent/.fake/state"),
                    mode: MountMode::ReadOnly,
                });
            }
            Mounted::ReadOnlyHome => {
                for mount in &mut scene.plan.mounts {
                    if mount.target == "/home/agent" {
                        mount.mode = MountMode::ReadOnly;
                    }
                }
            }
        }
        let got = if is_claude {
            facts::agent_argv(&scene, &claude, &facts::reference_command())
        } else {
            facts::agent_argv(&scene, &FakeHarness::plain(), &fake_command(Vec::new()))
        };
        assert!(is_refusal(&got, &want), "{case}: {got:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// H3: the census.
// ---------------------------------------------------------------------------------------------

/// Guards: R01 §Run manifest (`census.*`, `run.usage`, `run.cost_usd`, `run.num_turns`) — the three
/// census values and the three run totals of every recorded Claude Code stream are the ones
/// cf3de30 recorded, byte for byte.
/// Oracle: `census.json` and `run_totals.json`, recorded at cf3de30 from the same streams.
/// Bite: the census read by event key in the core with a reordered key, a list of tool names
/// that is not sorted and deduplicated for `census.init_tools`, or a stream recorded as empty; the
/// reader's totals taken from the first `result` line instead of the last, or left empty.
#[test]
fn harness_census_equals_cf3de30_on_every_fixture() {
    let baseline = recorded("census.json");
    let baseline = baseline.as_object().expect("an object");
    let totals = recorded("run_totals.json");
    let totals = totals.as_object().expect("an object");
    let streams = facts::stream_paths();
    assert!(streams.len() >= 8, "the fixtures are found: {streams:?}");
    let mut recorded_some = 0;
    for (name, path) in &streams {
        let actual = facts::census_value(&facts::lines_of(path));
        let expected = baseline
            .get(name)
            .unwrap_or_else(|| panic!("no baseline for {name}"));
        assert_eq!(&actual, expected, "census of {name}");
        let actual_totals = facts::run_totals_value(&facts::lines_of(path));
        let expected_totals = totals
            .get(name)
            .unwrap_or_else(|| panic!("no run totals baseline for {name}"));
        assert_eq!(&actual_totals, expected_totals, "run totals of {name}");
        if !actual.is_null() {
            recorded_some += 1;
        }
    }
    assert_eq!(
        baseline.len(),
        streams.len(),
        "every baseline entry has a fixture"
    );
    assert_eq!(
        totals.len(),
        streams.len(),
        "every run totals entry has a fixture"
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

/// A row of the census table: the case, the inventory, and the pairs recorded (none for no census).
type CensusRow = (
    &'static str,
    Option<InitReport>,
    Option<Vec<(String, Value)>>,
);

/// Guards: R01 §Run manifest (`census.*`) — an inventory records what its source saw: from a
/// source that sees no servers, `census.tools` and `census.init_tools` only, never an empty server
/// list; from the harness's own init, the three values in the order of the baseline with the
/// server list out of `census.init_tools`; from no inventory, nothing, and no empty list.
/// Oracle: the sorted, deduplicated tool names and the order and shape of `census.json`
/// (recorded at cf3de30), with the lists written here by hand.
/// Bite: `census.mcp_servers` recorded as an empty list, `census.init_tools` left unsorted,
/// servers merged into `census.init_tools`, the three values reordered, or an absent inventory
/// recorded as an empty tool list.
#[test]
fn harness_census_records_what_the_source_saw() {
    let servers = json!([{"name": "s", "status": "connected"}]);
    let rows: Vec<CensusRow> = vec![
        (
            "a source that sees no servers",
            Some(inventory(
                json!(["b", "a", "b"]),
                None,
                CensusSource::Gateway,
            )),
            Some(vec![
                (String::from("census.tools"), json!(["b", "a", "b"])),
                (String::from("census.init_tools"), json!(["a", "b"])),
            ]),
        ),
        (
            "the harness's own init",
            Some(inventory(
                json!(["Bash", "Agent"]),
                Some(servers.clone()), // BORROW: the row owns its list
                CensusSource::Harness,
            )),
            Some(vec![
                (String::from("census.tools"), json!(["Bash", "Agent"])),
                (String::from("census.mcp_servers"), servers),
                (String::from("census.init_tools"), json!(["Agent", "Bash"])),
            ]),
        ),
        ("no inventory", None, None),
    ];
    for (case, init, want) in rows {
        match want {
            Some(pairs) => assert_eq!(census_pairs(init.as_ref()), pairs, "{case}"),
            None => assert!(
                matches!(census(init.as_ref()), Err(CensusError::NoInit)),
                "{case}"
            ),
        }
    }
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
    let (mut tap, handle) = live_observer(Box::new(reader), stream);
    for (index, line) in facts::lines_of(&path).iter().enumerate() {
        let frame = RawFrame::new(None, FrameData::from_bytes(line.as_bytes()));
        tap.frame(u64::try_from(index).expect("an index"), &frame);
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
