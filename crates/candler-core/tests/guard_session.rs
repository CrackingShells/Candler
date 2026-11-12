#![cfg(test)]
//! Guards for session mode: how a cell opts in, what the driver's command line holds per cell,
//! when the input of a session closes, what a session records and what the observation hooks
//! may do.
//!
//! The expected values are literals, the recorded session fixtures of the Claude Code crate, or
//! counts made by hand.

use std::path::Path;
use std::sync::Arc;

use candler_core::experiment::paths;
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::{ResolveError, Resolver};
use candler_core::resolve::steps::session::SessionModeStep;
use candler_harness::driver::HarnessDriver as _;
use candler_harness::spec::{Cell, Workspace};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::ExperimentFile;
use candler_schemas::primitives::FieldPath;
use candler_schemas::values::{JsonValue, ValueSource};
use serde_json::{Value, json};

#[path = "support/harness_facts.rs"]
mod facts;
mod support;

fn fp(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

fn js(value: Value) -> JsonValue {
    JsonValue::new(value).expect("a JSON value")
}

/// Resolves `file` with the session step registered for a driver that is, or is not, capable.
async fn resolve(file: &[(&str, Value)], capable: bool) -> Result<Resolution, ResolveError> {
    let cwd = Path::new("/work");
    let config = Arc::new(Config::new(
        cwd.to_owned(),
        None,
        UserConfig::default(),
        Some(cwd.join("state")),
    ));
    let values = file
        .iter()
        .map(|(path, value)| (fp(path), js(value.clone())))
        .collect();
    Resolver::new(config)
        .register(Box::new(SessionModeStep::new(capable).expect("a step")))
        .expect("registers")
        .resolve(ResolveInput::default().file(FileValues::new(values)))
        .await
}

/// The value the resolution holds at `path`, and whether a preflight step derived it.
fn held(resolution: &Resolution, path: &str) -> Option<(Value, bool)> {
    let at = fp(path);
    let value = resolution
        .values
        .iter()
        .find(|value| *value.path() == at)?
        .value()?
        .as_value()
        .clone();
    let derived = resolution
        .sources
        .iter()
        .any(|entry| *entry.path() == at && matches!(entry.source(), ValueSource::Derived { .. }));
    Some((value, derived))
}

// ---------------------------------------------------------------------------------------------
// S5: opting a cell into session mode.
// ---------------------------------------------------------------------------------------------

/// One row of the resolve table: what the file writes, whether the driver can run a session,
/// and what resolves: the `runner.mode` and `runner.idle_s` of the cell with whether the idle was
/// derived, or the refusal.
struct Row {
    name: &'static str,
    file: Vec<(&'static str, Value)>,
    capable: bool,
    expect: Result<(Option<Value>, Option<(Value, bool)>), &'static str>,
}

fn rows() -> Vec<Row> {
    let ok = |mode: Option<Value>, idle: Option<(Value, bool)>| Ok((mode, idle));
    vec![
        Row {
            name: "no runner value: the cell is headless and holds neither path",
            file: vec![("model.id", json!("m"))],
            capable: true,
            expect: ok(None, None),
        },
        Row {
            name: "no runner value on a driver without sessions",
            file: vec![("model.id", json!("m"))],
            capable: false,
            expect: ok(None, None),
        },
        Row {
            name: "explicit headless is a value of the cell",
            file: vec![("runner.mode", json!("headless"))],
            capable: false,
            expect: ok(Some(json!("headless")), None),
        },
        Row {
            name: "session in fixed: idle_s derived as 60",
            file: vec![("runner.mode", json!("session"))],
            capable: true,
            expect: ok(Some(json!("session")), Some((json!(60), true))),
        },
        Row {
            name: "session with its own idle_s keeps it",
            file: vec![
                ("runner.mode", json!("session")),
                ("runner.idle_s", json!(5)),
            ],
            capable: true,
            expect: ok(Some(json!("session")), Some((json!(5), false))),
        },
        Row {
            name: "idle_s zero is whole seconds",
            file: vec![
                ("runner.mode", json!("session")),
                ("runner.idle_s", json!(0)),
            ],
            capable: true,
            expect: ok(Some(json!("session")), Some((json!(0), false))),
        },
        Row {
            name: "idle_s none is accepted",
            file: vec![
                ("runner.mode", json!("session")),
                ("runner.idle_s", json!("none")),
            ],
            capable: true,
            expect: ok(Some(json!("session")), Some((json!("none"), false))),
        },
        Row {
            name: "session on a driver without the capability",
            file: vec![("runner.mode", json!("session"))],
            capable: false,
            expect: Err("session_capable@1"),
        },
        Row {
            name: "idle_s without a mode",
            file: vec![("runner.idle_s", json!(5))],
            capable: true,
            expect: Err("idle_s_needs_session@1"),
        },
        Row {
            name: "idle_s in headless",
            file: vec![
                ("runner.mode", json!("headless")),
                ("runner.idle_s", json!(5)),
            ],
            capable: true,
            expect: Err("idle_s_needs_session@1"),
        },
        Row {
            name: "a mode that is neither",
            file: vec![("runner.mode", json!("batch"))],
            capable: true,
            expect: Err("mode_valid@1"),
        },
        Row {
            name: "a mode that is not text",
            file: vec![("runner.mode", json!(true))],
            capable: true,
            expect: Err("mode_valid@1"),
        },
        Row {
            name: "idle_s negative",
            file: vec![
                ("runner.mode", json!("session")),
                ("runner.idle_s", json!(-1)),
            ],
            capable: true,
            expect: Err("idle_s_valid@1"),
        },
        Row {
            name: "idle_s fractional",
            file: vec![
                ("runner.mode", json!("session")),
                ("runner.idle_s", json!(1.5)),
            ],
            capable: true,
            expect: Err("idle_s_valid@1"),
        },
        Row {
            name: "idle_s other text",
            file: vec![
                ("runner.mode", json!("session")),
                ("runner.idle_s", json!("soon")),
            ],
            capable: true,
            expect: Err("idle_s_valid@1"),
        },
    ]
}

/// Guards: M4 D1 — a cell opts into session mode by `runner.mode` alone: a cell that writes no
/// runner value gets none (headless, as before), `runner.idle_s` is derived as 60 in session
/// mode, `none` and whole seconds are kept, and a bad mode, an idle without a session, or a
/// session on a driver without the capability is refused by naming the check.
/// Oracle: the table above, written by hand from the leaf's rules.
/// Bite: derive `runner.mode = headless` into every cell (the row with no runner value gets a
/// mode), or drop the capability refusal.
#[tokio::test]
async fn session_resolve_table() {
    let table = rows();
    assert!(table.len() >= 10, "the table is not empty");
    for row in table {
        let outcome = resolve(&row.file, row.capable).await;
        match (&row.expect, outcome) {
            (Ok((mode, idle)), Ok(resolution)) => {
                assert_eq!(
                    held(&resolution, paths::RUNNER_MODE).map(|(value, _)| value),
                    *mode,
                    "{}: runner.mode",
                    row.name
                );
                assert_eq!(
                    held(&resolution, paths::RUNNER_IDLE_S),
                    *idle,
                    "{}: idle",
                    row.name
                );
            }
            (Err(check), Err(ResolveError::Preflight { reports, errors })) => {
                assert!(errors.is_empty(), "{}: no step error", row.name);
                let failed: Vec<String> = reports
                    .iter()
                    .flat_map(|report| report.checks.iter())
                    .filter(|found| !found.passed)
                    .map(|found| found.name.to_string())
                    .collect();
                assert_eq!(failed, [(*check).to_owned()], "{}: failed check", row.name);
            }
            (expected, got) => panic!("{}: expected {expected:?}, got {:?}", row.name, got.is_ok()),
        }
    }
}

/// Guards: M4 D1 — `runner.mode` as a factor level builds one cell per level with the mode in
/// the cell, and both paths belong to the configuration surface (the path classes know them in
/// `[fixed]`, as a factor path and as a leaf of a table level).
/// Oracle: the experiment text below and the leaf's rule that an absent mode is headless.
/// Bite: drop `runner.mode` from the known paths, or default it into cells that do not write it.
#[test]
fn session_resolve_factor_levels_and_known_paths() {
    for path in [paths::RUNNER_MODE, paths::RUNNER_IDLE_S] {
        assert!(paths::is_known_path(&fp(path)), "{path} is a known path");
    }
    let text = "schema = \"experiment/1\"\nreplicates = 1\n\n[fixed]\n\"model.id\" = \"m\"\n\n\
                [[factors]]\npath = \"runner.mode\"\nlevels = [\"headless\", \"session\"]\n\n\
                [budget.per_run]\nwall_s = 60\n\n[harness]\nname = \"claude-code\"\n\n\
                [treatment]\nname = \"t\"\n";
    let file = ExperimentFile::from_toml_str(text).expect("an experiment");
    let cells = file.cells();
    let modes: Vec<Option<Value>> = cells
        .iter()
        .map(|cell| {
            cell.iter()
                .find(|value| value.path().as_str() == paths::RUNNER_MODE)
                .and_then(|value| value.value().map(|json| json.as_value().clone()))
        })
        .collect();
    assert_eq!(modes, [Some(json!("headless")), Some(json!("session"))]);
}

// ---------------------------------------------------------------------------------------------
// P1: a model factor of tables.
// ---------------------------------------------------------------------------------------------

/// The argument after `flag` in `argv`.
fn after<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
    let at = argv.iter().position(|word| word == flag)?;
    argv.get(at + 1).map(String::as_str)
}

/// Guards: M4 D6 — a model factor whose levels are tables gives each cell its own flags in the
/// driver's argv, in a headless command and in a session command alike: `--model` and
/// `--effort` follow the level's `id` and `effort`, and a level without `effort` has no
/// `--effort` flag.
/// Oracle: the levels written below; the expected flags are the same words by hand.
/// Bite: flatten the table (every cell takes the first level's values), or drop `model.effort`.
#[test]
fn session_profiles_table_levels_reach_each_cells_argv() {
    let text = "schema = \"experiment/1\"\nreplicates = 1\n\n[fixed]\npermission_mode = \"bypassPermissions\"\n\
                \"prompt.template\" = \"go\"\n\n[[factors]]\npath = \"model\"\nlevels = [\n\
                { id = \"fast\", effort = \"low\" },\n{ id = \"deep\", effort = \"max\" },\n{ id = \"plain\" },\n]\n\n\
                [budget.per_run]\nwall_s = 60\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"t\"\n";
    let file = ExperimentFile::from_toml_str(text).expect("an experiment");
    let cells = file.cells();
    assert_eq!(cells.len(), 3, "one cell per level");
    let driver = ClaudeCodeDriver::new().expect("the driver");
    let workspace = Workspace::new(Path::new("/work/run"));
    let expected = [
        ("fast", Some("low")),
        ("deep", Some("max")),
        ("plain", None),
    ];
    for (values, (model, effort)) in cells.iter().zip(expected) {
        let cell = Cell::new(values);
        let headless = driver.command(&cell, &workspace).expect("a command");
        let session = driver
            .session(&cell, &workspace)
            .expect("a session")
            .expect("a session spec");
        for (kind, argv) in [
            ("headless", headless.argv()),
            ("session", session.command.argv()),
        ] {
            assert_eq!(
                after(argv, "--model"),
                Some(model),
                "{kind} {model}: --model"
            );
            assert_eq!(after(argv, "--effort"), effort, "{kind} {model}: --effort");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The input flag of a session's container.
// ---------------------------------------------------------------------------------------------

/// Guards: M4 D1, R01 §Session runner — a session's container keeps its standard input open
/// (`docker run -i`, a stdin pipe on the request) and a headless one does not: with the plan not
/// interactive the argv is the one cf3de30 recorded, and with it interactive the argv is that
/// argv with `-i` right after `run` and nothing else changed.
/// Oracle: `argv.json`, recorded at cf3de30, and the flag inserted by hand.
/// Bite: `-i` missing from an interactive plan (the container would read end of input at once),
/// or `-i` added to a headless one (a headless cell's argv would move).
#[test]
fn harness_env_session_adds_input_flag_only_when_interactive() {
    let dir = tempfile::tempdir().expect("a directory");
    let scene = facts::scene(dir.path());
    let driver = ClaudeCodeDriver::new().expect("the driver");
    let baseline: Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/harness_baseline/argv.json"),
        )
        .expect("the baseline"),
    )
    .expect("baseline JSON");
    let recorded: Vec<String> = baseline["headless_run"]
        .as_array()
        .expect("an argv")
        .iter()
        .map(|word| word.as_str().expect("a word").to_owned())
        .collect();
    assert_eq!(
        &recorded[..2],
        ["docker", "run"],
        "the recorded argv starts at run"
    );
    let mut with_flag = recorded.clone();
    with_flag.insert(2, "-i".to_owned());
    for (interactive, expected, stdin) in [(false, recorded, "Null"), (true, with_flag, "Pipe")] {
        let mut plan = scene.plan.clone();
        plan.interactive = interactive;
        let argv =
            facts::plan_argv_over(&scene, plan.clone(), &driver, &facts::reference_command())
                .expect("an argv");
        assert_eq!(argv, expected, "interactive = {interactive}");
        // the request the core spawns: stdin is a pipe exactly when the plan is interactive
        let mut over = plan;
        candler_core::runner::exec::apply_harness_env(&mut over, &driver, &scene.home)
            .expect("the driver's variables");
        let dir = tempfile::tempdir().expect("a directory");
        let outputs = candler_core::isolation::RunOutputs::new(
            candler_core::supervise::spawn::OutputSpec::framed(
                dir.path().to_path_buf(),
                "stdout".parse().expect("a stream"),
                candler_harness::spec::Framing::Lines,
            ),
            candler_core::supervise::spawn::OutputSpec::framed(
                dir.path().to_path_buf(),
                "stderr".parse().expect("a stream"),
                candler_harness::spec::Framing::Lines,
            ),
        );
        let request = candler_core::isolation::container::run::build_run_request(
            &over,
            &facts::reference_command(),
            "m4-session".parse().expect("a marker"),
            &scene.key,
            &"http://gateway:8080/run/ref".parse().expect("a url"),
            facts::DUMMY_TOKEN,
            None,
            outputs,
        )
        .expect("a request");
        assert!(
            format!("{request:?}").contains(&format!("stdin: {stdin}")),
            "interactive = {interactive}: the request's stdin is {stdin}"
        );
    }
}
