#![cfg(test)]
//! Guards for session mode: how a cell opts in, what the driver's command line holds per cell,
//! when the input of a session closes, what a session records and what the observation hooks
//! may do.
//!
//! The expected values are literals, the recorded session fixtures of the Claude Code crate, or
//! counts made by hand.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use candler_core::experiment::paths;
use candler_core::jobs::model::JobSpec;
use candler_core::jobs::orchestrate::session::cells_of;
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::{ResolveError, Resolver};
use candler_core::resolve::steps::session::SessionModeStep;
use candler_core::runner::plan::{Force, PlanCtx, PlanError, PlanMode, plan_runs};
use candler_harness::driver::HarnessDriver as _;
use candler_harness::spec::{Cell, Workspace};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::ExperimentFile;
use candler_schemas::primitives::FieldPath;
use candler_schemas::refusal::{Refusal, Side};
use candler_schemas::values::{JsonValue, ResolvedValue, ValueSource};
use serde_json::{Value, json};

#[path = "support/harness_facts.rs"]
mod facts;
#[path = "support/session_rig.rs"]
mod rig;
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
/// What a resolve row expects: the mode and the idle with whether it was derived, or the refusal.
type Resolved = Result<(Option<Value>, Option<(Value, bool)>), &'static str>;

struct Row {
    name: &'static str,
    file: Vec<(&'static str, Value)>,
    capable: bool,
    expect: Resolved,
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
            (Err(check), Err(ResolveError::Session(refusal))) => {
                assert_eq!(refusal.check(), *check, "{}: failed check", row.name);
                assert_eq!(
                    Refusal::code(&refusal).as_str(),
                    "experiment.session_refused",
                    "{}: the author's code",
                    row.name
                );
                assert_eq!(Refusal::side(&refusal), Side::Experiment);
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

/// The experiment text with the factors `factors` (already TOML) and no `[fixed]` value.
fn experiment_with(factors: &str) -> ExperimentFile {
    let text = format!(
        "schema = \"experiment/1\"\nreplicates = 1\n\n{factors}\n[budget.per_run]\nwall_s = 60\n\n\
         [harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"t\"\n"
    );
    ExperimentFile::from_toml_str(&text).expect("an experiment")
}

/// Plans `file` over the resolution of `fixed`, on a driver that is, or is not, capable, and
/// returns the cells' values and, per cell, the sources a job would record.
async fn planned(
    file: &ExperimentFile,
    fixed: &[(&str, Value)],
    capable: bool,
) -> Result<Vec<(Vec<(String, Value)>, Vec<(String, bool)>)>, PlanError> {
    let mut all = vec![
        (
            "harness",
            json!({"name": "claude-code", "version": "2.1.311"}),
        ),
        ("treatment", json!({"name": "t"})),
    ];
    all.extend(fixed.iter().map(|(path, value)| (*path, value.clone())));
    let mut resolution = resolve(&all, true).await.expect("the fixed values resolve");
    resolution.settings.push(ResolvedValue::recorded(
        fp(paths::REQUEST_TREATMENT),
        js(json!({"name": "t"})),
    ));
    let (dir, store, lock) = support::run_plan::fresh_store();
    let ctx = PlanCtx {
        store,
        experiment_lock: lock,
        secrets_present: BTreeSet::new(),
        session_capable: Some(capable),
    };
    let outcome = plan_runs(&ctx, file, &resolution, Force::No, PlanMode::Check)?;
    let spec = JobSpec {
        experiment: file.clone(),
        resolution,
        plans: outcome.plans.clone(),
        budget: file.budget().clone(),
        force: false,
        inputs: Vec::new(),
        evaluation_inputs: Vec::new(),
    };
    let cells = cells_of(&spec, &outcome, &BTreeSet::new(), &BTreeMap::new()).expect("cells");
    drop(dir);
    Ok(cells
        .values()
        .map(|cell| {
            let values = cell
                .comparable
                .values()
                .iter()
                .filter_map(|value| {
                    Some((value.path().to_string(), value.value()?.as_value().clone()))
                })
                .collect();
            let sources = cell
                .sources
                .iter()
                .map(|entry| {
                    (
                        entry.path().to_string(),
                        matches!(entry.source(), ValueSource::Derived { .. }),
                    )
                })
                .collect();
            (values, sources)
        })
        .collect())
}

fn value_of(values: &[(String, Value)], path: &str) -> Option<Value> {
    values
        .iter()
        .find(|(own, _)| own == path)
        .map(|(_, value)| value.clone())
}

/// Guards: M4 D1, I12 — the session rule is settled per cell: a session cell whose mode is a
/// factor level is the same cell as its `[fixed]` twin (the same values, `runner.idle_s = 60`
/// derived in both), a bad factor-level mode and a factor-level session on a driver that cannot
/// run one are refused when planning with the author's code `experiment.session_refused`, never
/// when running, and `runner.idle_s` in `[fixed]` with factor modes `headless` and `session` is
/// refused as a whole, at resolve, because it applies to session cells only.
/// Oracle: the experiment texts below; the twin's values are the `[fixed]` cell's own.
/// Bite: leave the factor-level cells out of the session rule (no idle derived, no refusal).
#[tokio::test]
async fn session_resolve_settles_each_cell() {
    let twin_fixed = planned(
        &experiment_with(""),
        &[(paths::RUNNER_MODE, json!("session"))],
        true,
    )
    .await
    .expect("plans");
    let twin_factor = planned(
        &experiment_with("[[factors]]\npath = \"runner.mode\"\nlevels = [\"session\"]\n"),
        &[],
        true,
    )
    .await
    .expect("plans");
    assert_eq!(twin_fixed.len(), 1);
    assert_eq!(
        twin_factor, twin_fixed,
        "the factor level is its fixed twin, values and sources"
    );
    let (values, sources) = twin_fixed.first().expect("a cell");
    assert_eq!(value_of(values, paths::RUNNER_IDLE_S), Some(json!(60)));
    assert!(
        sources.contains(&(paths::RUNNER_IDLE_S.to_owned(), true)),
        "the default is derived: {sources:?}"
    );
    let table_level = planned(
        &experiment_with("[[factors]]\npath = \"runner\"\nlevels = [{ mode = \"session\" }]\n"),
        &[],
        true,
    )
    .await
    .expect("plans");
    assert_eq!(
        table_level, twin_fixed,
        "a table-valued level is the same cell and labels the idle the same"
    );
    let both = planned(
        &experiment_with(
            "[[factors]]\npath = \"runner.mode\"\nlevels = [\"headless\", \"session\"]\n",
        ),
        &[],
        true,
    )
    .await
    .expect("plans");
    let mut idles: Vec<Option<Value>> = both
        .iter()
        .map(|(values, _)| value_of(values, paths::RUNNER_IDLE_S))
        .collect();
    idles.sort_by_key(Option::is_some);
    assert_eq!(
        idles,
        [None, Some(json!(60))],
        "only the session cell holds an idle"
    );
    let factor_modes = |levels: &str| {
        experiment_with(&format!(
            "[[factors]]\npath = \"runner.mode\"\nlevels = {levels}\n"
        ))
    };
    for (name, levels, capable, check) in [
        (
            "a bad mode",
            "[\"headless\", \"sesion\"]",
            true,
            "mode_valid@1",
        ),
        (
            "no capability",
            "[\"headless\", \"session\"]",
            false,
            "session_capable@1",
        ),
    ] {
        match planned(&factor_modes(levels), &[], capable).await {
            Err(PlanError::Session(refusal)) => {
                assert_eq!(refusal.check(), check, "{name}");
                assert_eq!(
                    Refusal::code(&refusal).as_str(),
                    "experiment.session_refused"
                );
            }
            other => panic!(
                "{name}: expected a session refusal, got {:?}",
                other.map(|cells| cells.len())
            ),
        }
    }
    match resolve(&[(paths::RUNNER_IDLE_S, json!(5))], true).await {
        Err(ResolveError::Session(refusal)) => {
            assert_eq!(refusal.check(), "idle_s_needs_session@1");
        }
        other => panic!("idle in [fixed] without a mode: {:?}", other.is_ok()),
    }
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

// ---------------------------------------------------------------------------------------------
// S1: when the input closes.
// ---------------------------------------------------------------------------------------------

use std::time::Duration;

use candler_core::runner::exec::session::{CloseRule, HOOK_SETTLE, Idle};
use candler_harness::spec::{SessionRecord, SessionTracker};

/// One record of a replay.
#[derive(Clone)]
enum Rec {
    Stream(String),
    Hook(String),
}

/// A time of `ms` milliseconds.
fn ms(value: f64) -> Duration {
    Duration::from_secs_f64(value / 1000.0)
}

/// Whether two times are the same to the microsecond.
fn same(left: Duration, right: Duration) -> bool {
    left.abs_diff(right) < Duration::from_micros(1)
}

/// The records of the recorded session `session` with the host times `timings.toml` gives them:
/// each stdout line at its `t_ms`, each hook line at its `seen_ms`, in time order.
fn recorded(session: &str, with_hooks: bool) -> Vec<(Duration, Rec)> {
    let dir = rig::sessions_dir().join(session);
    let timings: toml::Table = std::fs::read_to_string(dir.join("timings.toml"))
        .expect("timings")
        .parse()
        .expect("timings TOML");
    let lines = rig::fixture_lines(session, "stdout.jsonl");
    let hooks = rig::fixture_lines(session, "hooks.jsonl");
    let float = |entry: &toml::Value, key: &str| entry[key].as_float().expect("a time");
    let mut events: Vec<(Duration, Rec)> = Vec::new();
    for entry in timings["stdout"].as_array().expect("stdout timings") {
        let number = usize::try_from(entry["line"].as_integer().expect("a line")).expect("a line");
        events.push((
            ms(float(entry, "t_ms")),
            Rec::Stream(lines[number - 1].clone()),
        ));
    }
    if with_hooks {
        for (entry, line) in timings["hook"]
            .as_array()
            .expect("hook timings")
            .iter()
            .zip(&hooks)
        {
            events.push((ms(float(entry, "seen_ms")), Rec::Hook(line.clone())));
        }
    }
    events.sort_by_key(|(at, _)| *at);
    events
}

/// The state of a replay after its last record: the rule's due time and whether the tracker was
/// quiet at every moment the rule was due.
struct Replayed {
    due: Option<Duration>,
    quiet_when_due: bool,
}

/// Feeds `events` to the Claude Code tracker and a rule, as the runner does.
fn replay(events: &[(Duration, Rec)], idle: Idle) -> Replayed {
    let mut tracker = candler_harness_claude_code::session::Tracker::default();
    let mut rule = CloseRule::new(idle, HOOK_SETTLE);
    let mut quiet_when_due = true;
    for (at, record) in events {
        // EXPLICIT: an unreadable record leaves the tracker's state unknown, which the rule reads
        let _ = match record {
            Rec::Stream(line) => tracker.feed(SessionRecord::Stream(line.as_bytes())),
            Rec::Hook(line) => tracker.feed(SessionRecord::Hook(line.as_bytes())),
        };
        let state = tracker.state();
        rule.observe(*at, &state);
        if rule.due_at().is_some() {
            quiet_when_due &= state.turn_ended && state.known && state.pending.is_empty();
        }
    }
    Replayed {
        due: rule.due_at(),
        quiet_when_due,
    }
}

/// Guards: R01 §Session runner, M4 D2 — over the recorded sessions c1 and c2, with the times the
/// host saw each line, the input closes `idle_s` after the turn's end (at least the hook settle),
/// never while the turn runs, never while a background task is pending, never when the pending
/// list was not read (no hook line), and never with `idle_s = none`.
/// Oracle: `timings.toml` of the recorded sessions, by hand: c2's `result` at 1062.01 ms, c1's
/// first `result` at 430.86 ms with one task listed by its Stop line, c1's second at 20532.47 ms.
/// Bite: count the idle from the last record instead of the turn's end (see the stray-frame rows
/// of `session_close_ignores_records_that_do_not_change_the_state`), or close with a task pending.
#[test]
fn session_close_replays_the_recorded_sessions() {
    struct Row {
        name: &'static str,
        session: &'static str,
        hooks: bool,
        upto_line: Option<usize>,
        idle: Idle,
        due_ms: Option<f64>,
    }
    let rows = [
        Row {
            name: "c2, two seconds idle: due two seconds after the result",
            session: "c2",
            hooks: true,
            upto_line: None,
            idle: Idle::Seconds(2),
            due_ms: Some(1062.01 + 2000.0),
        },
        Row {
            name: "c2, zero idle: the hook settle holds the close back",
            session: "c2",
            hooks: true,
            upto_line: None,
            idle: Idle::Seconds(0),
            due_ms: Some(1062.01 + 250.0),
        },
        Row {
            name: "c2, none: never",
            session: "c2",
            hooks: true,
            upto_line: None,
            idle: Idle::Never,
            due_ms: None,
        },
        Row {
            name: "c2 without a hook line: the pending list is unread, never",
            session: "c2",
            hooks: false,
            upto_line: None,
            idle: Idle::Seconds(2),
            due_ms: None,
        },
        Row {
            name: "c1 after its first result: a task is pending",
            session: "c1",
            hooks: true,
            upto_line: Some(8),
            idle: Idle::Seconds(1),
            due_ms: None,
        },
        Row {
            name: "c1 once the notification arrived and before its turn's first event",
            session: "c1",
            hooks: true,
            upto_line: Some(11),
            idle: Idle::Seconds(1),
            due_ms: None,
        },
        Row {
            name: "c1 in the middle of its second turn",
            session: "c1",
            hooks: true,
            upto_line: Some(13),
            idle: Idle::Seconds(1),
            due_ms: None,
        },
        Row {
            name: "c1 whole, three seconds idle: due after the second result",
            session: "c1",
            hooks: true,
            upto_line: None,
            idle: Idle::Seconds(3),
            due_ms: Some(20532.47 + 3000.0),
        },
    ];
    for row in rows {
        let mut events = recorded(row.session, row.hooks);
        if let Some(line) = row.upto_line {
            // the first `line` stdout records, and the hook lines seen by then
            let stdout_at = recorded(row.session, false)[line - 1].0;
            events.retain(|(at, _)| *at <= stdout_at);
        }
        assert!(!events.is_empty(), "{}: a replay has records", row.name);
        let replayed = replay(&events, row.idle);
        assert!(replayed.quiet_when_due, "{}: due only when quiet", row.name);
        match (row.due_ms, replayed.due) {
            (None, None) => {}
            (Some(expected), Some(due)) => assert!(
                same(due, ms(expected)),
                "{}: due at {due:?}, expected {expected} ms",
                row.name
            ),
            (expected, due) => panic!("{}: due {due:?}, expected {expected:?}", row.name),
        }
    }
}

/// Guards: M4 D2 — an event that does not change what the tracker reports never moves the close
/// (a stray frame after the turn's end), a new turn takes the close away until it ends, a later
/// turn end restarts the idle from that end, a wake-up listed by the Stop line and a hook line
/// that cannot be read both keep the session open.
/// Oracle: c2's lines with hand-written additions at the times in the table.
/// Bite: count the idle from the last event (the stray-frame row due at the frame's time plus
/// One row of the pure close table: a name, the timed records and the expected close in ms.
type CloseRow = (&'static str, Vec<(Duration, Rec)>, Option<f64>);

/// idle), or ignore the cron list of the Stop payload.
#[test]
fn session_close_ignores_records_that_do_not_change_the_state() {
    let base = recorded("c2", true);
    let stream = |at: f64, line: &str| (ms(at), Rec::Stream(line.to_owned()));
    let idle = Idle::Seconds(2);
    // The Stop line of c2 with a one-shot wake-up listed.
    let hook_line = rig::fixture_lines("c2", "hooks.jsonl")[0].clone();
    let with_wakeup = hook_line.replace(
        "\"session_crons\":[]",
        "\"session_crons\":[{\"id\":\"w1\",\"schedule\":\"+1m\",\"recurring\":false,\"prompt\":\"p\"}]",
    );
    assert_ne!(
        with_wakeup, hook_line,
        "the fixture's cron list is replaced"
    );
    let mut wake: Vec<(Duration, Rec)> = base
        .iter()
        .filter(|(_, rec)| !matches!(rec, Rec::Hook(_)))
        .cloned() // BORROW: the row owns its records
        .collect();
    wake.push((ms(1018.81), Rec::Hook(with_wakeup)));
    wake.sort_by_key(|(at, _)| *at);
    let mut garbled: Vec<(Duration, Rec)> = base
        .iter()
        .filter(|(_, rec)| !matches!(rec, Rec::Hook(_)))
        .cloned() // BORROW: the row owns its records
        .collect();
    garbled.push((ms(1018.81), Rec::Hook("{\"payload\": ".to_owned())));
    garbled.sort_by_key(|(at, _)| *at);
    let stray = r#"{"type":"system","subtype":"task_updated","session_id":"s"}"#;
    let started =
        r#"{"type":"assistant","session_id":"s","message":{"role":"assistant","content":[]}}"#;
    let ended = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":"hello","session_id":"s"}"#;
    let second_stop = hook_line.replace("746d51f1-9ffb-45ce-8416-088580bcf6de", "second-turn");
    let with = |extra: Vec<(Duration, Rec)>| {
        let mut events = base.clone(); // BORROW: the row owns its records
        events.extend(extra);
        events.sort_by_key(|(at, _)| *at);
        events
    };
    let rows: Vec<CloseRow> = vec![
        ("no extra record", with(Vec::new()), Some(1062.01 + 2000.0)),
        (
            "a stray frame 600 ms after the turn's end",
            with(vec![stream(1662.01, stray)]),
            Some(1062.01 + 2000.0),
        ),
        (
            "a new turn starts 600 ms after the end",
            with(vec![stream(1662.01, started)]),
            None,
        ),
        (
            "a second turn ends 600 ms after the first, its Stop line 11 ms later",
            with(vec![
                stream(1662.01, started),
                stream(1700.0, ended),
                (ms(1711.0), Rec::Hook(second_stop)),
            ]),
            Some(1711.0 + 2000.0),
        ),
        ("a wake-up pending in the Stop line", wake, None),
        ("a hook line that is not JSON", garbled, None),
    ];
    for (name, events, expected) in rows {
        let replayed = replay(&events, idle);
        assert!(replayed.quiet_when_due, "{name}: due only when quiet");
        match (expected, replayed.due) {
            (None, None) => {}
            (Some(want), Some(due)) => {
                assert!(
                    same(due, ms(want)),
                    "{name}: due at {due:?}, expected {want} ms"
                );
            }
            (want, due) => panic!("{name}: due {due:?}, expected {want:?}"),
        }
    }
}

/// The stub's script for c2's single turn, then end of input, then exit 0.
fn c2_script(after_result: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    let mut steps = rig::replay_of("c2", &BTreeMap::new());
    steps.extend(after_result);
    steps.push(rig::wait_eof(None));
    steps.push(rig::exit(0));
    steps
}

/// Guards: R01 §Session runner, M4 D2 — the input never closes on a turn's pending list that was
/// not read for that turn: in the order the host sees in production (a turn's `result` line
/// before its `Stop` line) the state is unknown until the `Stop` line arrives, however late; a
/// turn that ends with no `Stop` line (the turn limit) after a quiet turn leaves the state
/// unknown, so the wake-up it scheduled is not dropped; a `Stop` line that lists a wake-up keeps
/// the session open; a `Stop` line read before its `result` still counts for that turn; a
/// `Stop` line counts once (its `prompt_id`), for the earliest ended turn without one, and only
/// when its message is that turn's result text.
/// Oracle: the records below, by hand; the times are the ones the rows say.
/// Bite: count the `Stop` lines instead of tying each to its turn.
#[test]
fn session_close_needs_a_stop_for_every_turn() {
    let stream = |text: &str| Rec::Stream(text.to_owned()); // BORROW: the row owns its records
    let stop = |id: &str, crons: &str| {
        Rec::Hook(format!(
            r#"{{"written_ns":1,"payload":{{"prompt_id":"{id}","hook_event_name":"Stop","background_tasks":[],"session_crons":{crons}}}}}"#
        ))
    };
    let assistant = || stream(r#"{"type":"assistant","message":{"content":[]}}"#);
    let result = || stream(r#"{"type":"result","subtype":"success","num_turns":1}"#);
    let max_turns = || stream(r#"{"type":"result","subtype":"error_max_turns","num_turns":2}"#);
    let tasks_changed =
        || stream(r#"{"type":"system","subtype":"background_tasks_changed","tasks":[]}"#);
    let quiet_turn = |from: f64| {
        vec![
            (ms(from), assistant()),
            (ms(from + 10.0), result()),
            (ms(from + 21.0), stop("p1", "[]")),
        ]
    };
    let rows: Vec<CloseRow> = vec![
        (
            "the result is read and the Stop line is not yet: unknown",
            vec![(ms(0.0), assistant()), (ms(10.0), result())],
            None,
        ),
        (
            "the Stop line arrives 11 ms after the result: due one second after it",
            quiet_turn(0.0),
            Some(21.0 + 1000.0),
        ),
        (
            "the Stop line is read before its result: still that turn's",
            vec![
                (ms(0.0), assistant()),
                (ms(5.0), stop("p1", "[]")),
                (ms(10.0), result()),
            ],
            Some(10.0 + 1000.0),
        ),
        (
            "a second turn ends with no Stop line: unknown, not the first turn's list",
            {
                let mut events = quiet_turn(0.0);
                events.push((ms(2000.0), assistant()));
                events.push((ms(2100.0), max_turns()));
                events.push((ms(2110.0), tasks_changed()));
                events
            },
            None,
        ),
        (
            "a late Stop line of the second turn lists a wake-up: pending, not due",
            {
                let mut events = quiet_turn(0.0);
                events.push((ms(2000.0), assistant()));
                events.push((ms(2100.0), result()));
                events.push((ms(2111.0), stop("p2", r#"[{"id":"w1","recurring":false}]"#)));
                events
            },
            None,
        ),
        (
            "the same Stop line twice, then a second result with no Stop line yet",
            {
                let mut events = quiet_turn(0.0);
                events.push((ms(40.0), stop("p1", "[]")));
                events.push((ms(2000.0), assistant()));
                events.push((ms(2100.0), result()));
                events
            },
            None,
        ),
        (
            "a Stop line of the first turn read after the second result: the second has none",
            vec![
                (ms(0.0), assistant()),
                (ms(10.0), result()),
                (ms(1000.0), assistant()),
                (ms(1010.0), result()),
                (ms(1021.0), stop("p1", "[]")),
            ],
            None,
        ),
        (
            "then the second turn's own Stop line arrives: both are covered",
            vec![
                (ms(0.0), assistant()),
                (ms(10.0), result()),
                (ms(1000.0), assistant()),
                (ms(1010.0), result()),
                (ms(1021.0), stop("p1", "[]")),
                (ms(1032.0), stop("p2", "[]")),
            ],
            Some(1032.0 + 1000.0),
        ),
        (
            "a Stop line whose message is not the turn's result text covers no turn",
            vec![
                (ms(0.0), assistant()),
                (
                    ms(10.0),
                    stream(r#"{"type":"result","subtype":"success","result":"done"}"#),
                ),
                (ms(21.0), stop("p1", "[]")),
            ],
            None,
        ),
    ];
    for (name, events, due_ms) in rows {
        let replayed = replay(&events, Idle::Seconds(1));
        match due_ms {
            None => assert_eq!(replayed.due, None, "{name}"),
            Some(expected) => {
                let due = replayed
                    .due
                    .unwrap_or_else(|| panic!("{name}: no due time"));
                assert!(
                    same(due, ms(expected)),
                    "{name}: due at {due:?}, expected {expected} ms"
                );
            }
        }
        assert!(replayed.quiet_when_due, "{name}: quiet whenever due");
    }
}

/// The stub's script for c1 with a 2.5 s pause where the background job runs, then end of input,
/// then exit 0. A pause is inserted before the notification's first line (line 9).
fn c1_script() -> Vec<serde_json::Value> {
    let mut between = BTreeMap::new();
    between.insert(9, vec![rig::sleep(2.5)]);
    let mut steps = rig::replay_of("c1", &between);
    steps.push(rig::wait_eof(None));
    steps.push(rig::exit(0));
    steps
}

/// Guards: R01 §Session runner, M4 D2 — through the run loop (a stub harness replaying the
/// recorded sessions over a fake isolation), the harness's stdin stays open until the agent has
/// yielded with nothing pending and `idle_s` has passed since the turn's end: the end of input
/// reaches the harness `idle_s` after the last turn's `result` (not after a later stray frame),
/// not while a background task is pending, not during a turn; `idle_s = none` never closes it.
/// Oracle: the stub's own trace of when it wrote each line and when its stdin ended, on one
/// monotonic clock; the expected delay is `idle_s` plus at most the runner's polling slack.
/// Bite: count the idle from the last frame (stray-frame row 2 closes 0.6 s late), close at the
/// first turn end (row 3 closes with a task pending), or never close.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_close_wired_through_the_run_loop() {
    let values = rig::cell_values(&[]);
    // Row 1: one turn, nothing pending.
    let one = rig::run(
        &c2_script(Vec::new()),
        &values,
        rig::Driver::new(true),
        None,
    )
    .await;
    let _ = one.outcome();
    let result = one.last_at("out", "result").expect("a result");
    let eof = one.at("eof", "").expect("the input closed");
    assert!(
        (1.0..1.4).contains(&(eof - result)),
        "row 1: the input closed {:.3} s after the result",
        eof - result
    );
    // Row 2: a stray frame 0.6 s after the turn's end.
    let stray = r#"{"type":"system","subtype":"task_updated","session_id":"s"}"#;
    let two = rig::run(
        &c2_script(vec![rig::sleep(0.6), rig::out(stray, "stray")]),
        &values,
        rig::Driver::new(true),
        None,
    )
    .await;
    let result = two.last_at("out", "result").expect("a result");
    let eof = two.at("eof", "").expect("the input closed");
    assert!(
        (1.0..1.4).contains(&(eof - result)),
        "row 2: the input closed {:.3} s after the result, with a frame at 0.6 s",
        eof - result
    );
    // Row 3: a task is pending after the first turn and its notification starts a second turn.
    let three = rig::run(&c1_script(), &values, rig::Driver::new(true), None).await;
    let results: Vec<f64> = three
        .trace
        .iter()
        .filter(|(_, kind, detail)| kind == "out" && detail == "result")
        .map(|(at, _, _)| *at)
        .collect();
    assert_eq!(results.len(), 2, "row 3: two turns ended");
    let eofs = three
        .trace
        .iter()
        .filter(|(_, kind, _)| kind == "eof")
        .count();
    assert_eq!(eofs, 1, "row 3: the input closed once");
    let eof = three.at("eof", "").expect("the input closed");
    assert!(
        eof > results[1] && (1.0..1.4).contains(&(eof - results[1])),
        "row 3: the input closed {:.3} s after the second result (first at {:.3}, closed at {eof:.3})",
        eof - results[1],
        results[0]
    );
    // Row 4: idle_s = none runs to the wall.
    let never = rig::cell_values(&[
        ("runner.idle_s", serde_json::json!("none")),
        ("budget.per_run.wall_s", serde_json::json!(3)),
    ]);
    let four = rig::run(&c2_script(Vec::new()), &never, rig::Driver::new(true), None).await;
    assert!(
        four.at("eof", "").is_none(),
        "row 4: the input never closed"
    );
    assert_eq!(
        four.measured("run.ended_by"),
        Some(serde_json::json!({"kind": "cap", "cap": "wall"})),
        "row 4: the wall ended the run"
    );
}

// ---------------------------------------------------------------------------------------------
// S2: what a session records.
// ---------------------------------------------------------------------------------------------

use candler_core::jobs::model::JobCause;
use serde_json::json as j;

/// The tool names of the init line of the recorded session `session`, sorted and deduplicated.
fn init_tools(session: &str) -> serde_json::Value {
    let line = rig::fixture_lines(session, "stdout.jsonl")
        .into_iter()
        .find(|line| line.contains("\"subtype\":\"init\""))
        .expect("an init line");
    let value: serde_json::Value = serde_json::from_str(&line).expect("init JSON");
    let mut names: Vec<String> = value["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| tool.as_str().expect("a name").to_owned())
        .collect();
    names.sort();
    names.dedup();
    serde_json::Value::from(names)
}

/// Turns of c2 repeated: init, an assistant line and a result that counts two turns, `count`
/// times, `gap` seconds apart, each result preceded by c2's Stop line when `with_stops` is set.
fn looping_script(count: usize, gap: f64, with_stops: bool) -> Vec<serde_json::Value> {
    let lines = rig::fixture_lines("c2", "stdout.jsonl");
    let stop_line = rig::fixture_lines("c2", "hooks.jsonl")
        .into_iter()
        .find(|line| line.contains("\"hook_event_name\":\"Stop\""))
        .expect("a Stop line");
    let mut steps = Vec::new();
    for turn in 0..count {
        for line in &lines {
            let line = line.replace("\"num_turns\":1", "\"num_turns\":2");
            if with_stops && line.contains("\"type\":\"result\"") {
                steps.push(rig::hook(&stop_line.replace(
                    "746d51f1-9ffb-45ce-8416-088580bcf6de",
                    &format!("turn-{turn}"),
                )));
                steps.push(rig::sleep(0.01));
            }
            steps.push(rig::out_of(&line));
        }
        steps.push(rig::sleep(gap));
    }
    steps.push(rig::wait_eof(None));
    steps
}

/// Turns of c2 repeated as `looping_script` does, with the Stop line of the second turn missing,
/// so the pending state was known after the first turn and not after the second.
fn lost_stop_script() -> Vec<serde_json::Value> {
    let lines = rig::fixture_lines("c2", "stdout.jsonl");
    let stop_line = rig::fixture_lines("c2", "hooks.jsonl")
        .into_iter()
        .find(|line| line.contains("\"hook_event_name\":\"Stop\""))
        .expect("a Stop line");
    let mut steps = Vec::new();
    for turn in 0..40 {
        for line in &lines {
            let line = line.replace("\"num_turns\":1", "\"num_turns\":2");
            if turn == 0 && line.contains("\"type\":\"result\"") {
                steps.push(rig::hook(&stop_line));
                steps.push(rig::sleep(0.01));
            }
            steps.push(rig::out_of(&line));
        }
        steps.push(rig::sleep(0.3));
    }
    steps.push(rig::wait_eof(None));
    steps
}

/// One row of the measures table: its name, the stub's script, the cell's values, a job stop with
/// the second it comes at, and what the manifest holds.
type MeasureRow<'a> = (
    &'static str,
    Vec<serde_json::Value>,
    &'a [(String, serde_json::Value)],
    Option<(JobCause, f64)>,
    Expect,
);

/// What a row of the measures table expects of the manifest.
struct Expect {
    ended_by: serde_json::Value,
    end: Option<&'static str>,
    forced: bool,
    valid: bool,
}

/// Guards: R01 §Session runner, M4 D3, I20 — a closed session records `session.turns`,
/// `session.yielded_at_ms`, `session.pending_at_end`, `session.end = closed`, `run.ended_by =
/// completed` and `census.init_tools`; a session that loops to the turn limit or the wall records
/// `session.end = cap`; a harness that leaves first is `harness_exit`; one that outlives its grace
/// after the close is stopped and records `session.close_forced`; a job kill or a budget stop
/// mid-session is the experimenter's end: the run is invalid, its `ended_by` is that stop and no
/// `session.end` is recorded for the arm.
/// Oracle: the scripts below, by hand; the init tools read from the fixture's init line with
/// `serde_json`; the turn counts are the `num_turns` the script writes.
/// Bite: record a closed session as a cap, charge a kill to the arm as a session end, or count the
/// turn limit with `>=`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_measures_by_how_the_session_ended() {
    let values = rig::cell_values(&[]);
    let capped = rig::cell_values(&[("budget.per_run.turns", j!(3))]);
    let limit_two = rig::cell_values(&[("budget.per_run.turns", j!(2))]);
    let wall = rig::cell_values(&[
        ("runner.idle_s", j!("none")),
        ("budget.per_run.wall_s", j!(3)),
    ]);
    let long_idle = rig::cell_values(&[("runner.idle_s", j!(60))]);
    let mut exits_first = rig::replay_of("c2", &BTreeMap::new());
    exits_first.push(rig::exit(0));
    let mut ignores_eof = rig::replay_of("c2", &BTreeMap::new());
    ignores_eof.extend([rig::wait_eof(None), rig::sleep(30.0)]);
    let rows: Vec<MeasureRow<'_>> = vec![
        (
            "closed",
            c2_script(Vec::new()),
            &values,
            None,
            Expect {
                ended_by: j!({"kind": "completed"}),
                end: Some("closed"),
                forced: false,
                valid: true,
            },
        ),
        (
            "the turn limit across the session",
            looping_script(6, 0.3, true),
            &capped,
            None,
            Expect {
                ended_by: j!({"kind": "cap", "cap": "turns"}),
                end: Some("cap"),
                forced: false,
                valid: true,
            },
        ),
        (
            "the wall",
            looping_script(40, 0.3, true),
            &wall,
            None,
            Expect {
                ended_by: j!({"kind": "cap", "cap": "wall"}),
                end: Some("cap"),
                forced: false,
                valid: true,
            },
        ),
        (
            "the wall with a hook channel that never delivers a Stop line (no turn is covered)",
            looping_script(40, 0.3, false),
            &wall,
            None,
            Expect {
                ended_by: j!({"kind": "unknown"}),
                end: Some("hooks_incomplete"),
                forced: false,
                valid: false,
            },
        ),
        (
            "one Stop line lost mid-session, after the state was known once",
            lost_stop_script(),
            &wall,
            None,
            Expect {
                ended_by: j!({"kind": "unknown"}),
                end: Some("hooks_incomplete"),
                forced: false,
                valid: false,
            },
        ),
        (
            "the turn limit equals the sum of one result: the session goes on and closes",
            looping_script(1, 0.1, true),
            &limit_two,
            None,
            Expect {
                ended_by: j!({"kind": "completed"}),
                end: Some("closed"),
                forced: false,
                valid: true,
            },
        ),
        (
            "the harness leaves first",
            exits_first,
            &long_idle,
            None,
            Expect {
                ended_by: j!({"kind": "completed"}),
                end: Some("harness_exit"),
                forced: false,
                valid: true,
            },
        ),
        (
            "a harness that outlives its grace after the close",
            ignores_eof,
            &values,
            None,
            Expect {
                ended_by: j!({"kind": "completed"}),
                end: Some("closed"),
                forced: true,
                valid: true,
            },
        ),
        (
            "the experimenter kills the job",
            looping_script(40, 0.3, true),
            &long_idle,
            Some((JobCause::Killed, 0.8)),
            Expect {
                ended_by: j!({"kind": "killed"}),
                end: None,
                forced: false,
                valid: false,
            },
        ),
        (
            "the job's budget stops the run",
            looping_script(40, 0.3, true),
            &long_idle,
            Some((JobCause::BudgetStop, 0.8)),
            Expect {
                ended_by: j!({"kind": "budget_stop"}),
                end: None,
                forced: false,
                valid: false,
            },
        ),
    ];
    assert!(rows.len() >= 7, "the table is not empty");
    for (name, steps, cell, cancel, expect) in rows {
        let ran = rig::run(&steps, cell, rig::Driver::new(true), cancel).await;
        let outcome = ran.outcome();
        assert_eq!(
            ran.measured("run.ended_by"),
            Some(expect.ended_by.clone()),
            "{name}: run.ended_by ({:?})",
            outcome.invalid
        );
        assert_eq!(
            outcome.invalid.is_none(),
            expect.valid,
            "{name}: the run is valid"
        );
        assert_eq!(
            ran.measured("session.end"),
            expect.end.map(serde_json::Value::from),
            "{name}: session.end"
        );
        assert_eq!(
            ran.measured("session.close_forced"),
            Some(serde_json::Value::Bool(expect.forced)),
            "{name}: session.close_forced"
        );
        assert!(
            ran.measured("session.turns")
                .and_then(|turns| turns.as_u64())
                .is_some_and(|turns| turns >= 1),
            "{name}: session.turns"
        );
        assert!(
            ran.run_dir.join("run.done").is_file(),
            "{name}: the run is sealed"
        );
        assert_eq!(
            ran.measured("census.init_tools"),
            Some(init_tools("c2")),
            "{name}: census.init_tools"
        );
        match name {
            "closed" => {
                assert_eq!(ran.measured("session.turns"), Some(j!(1)));
                assert_eq!(ran.measured("session.pending_at_end"), Some(j!([])));
                assert_eq!(ran.measured("session.idle_s"), Some(j!(1)));
                let result = ran.last_at("out", "result").expect("a result");
                let opened = ran.at("opened", "").expect("opened");
                let yielded = ran
                    .measured("session.yielded_at_ms")
                    .and_then(|ms| ms.as_f64())
                    .expect("yielded_at_ms");
                let floor = (result - opened) * 1000.0;
                assert!(
                    yielded >= floor && yielded < floor + 2000.0,
                    "closed: yielded at {yielded} ms, the stub's result {floor} ms after it opened"
                );
            }
            "the turn limit across the session" => {
                let turns = ran
                    .measured("session.turns")
                    .and_then(|t| t.as_u64())
                    .expect("turns");
                assert_eq!(
                    ran.measured("run.num_turns").and_then(|t| t.as_u64()),
                    Some(turns * 2),
                    "the run's turns are the sum over the session's results"
                );
                assert_eq!(
                    turns, 2,
                    "two turns of two exceed a limit of three, and no more ran"
                );
            }
            _ => {}
        }
    }
}

/// Guards: R01 §Session runner, M4 D3 — in session mode every `run.*` value is the session's
/// total: `run.usage` sums the usage of every result (the harness reports each result's own),
/// `run.num_turns` sums the turns, and `run.cost_usd` is the last result's (the harness reports
/// the cost cumulatively).
/// Oracle: the recorded session c1 by hand: results with usage 200+40 and 100+20 tokens, turns 2
/// and 1, total cost 0.0016 then 0.0024.
/// Bite: record the last result's usage as the session's.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_measures_run_values_are_session_totals() {
    let values = rig::cell_values(&[("runner.idle_s", j!(1))]);
    let ran = rig::run(&c1_script(), &values, rig::Driver::new(true), None).await;
    let _ = ran.outcome();
    let usage = ran.measured("run.usage").expect("run.usage");
    assert_eq!(usage["input_tokens"], j!(300), "input tokens summed");
    assert_eq!(usage["output_tokens"], j!(60), "output tokens summed");
    assert_eq!(ran.measured("run.num_turns"), Some(j!(3)));
    let cost = ran
        .measured("run.cost_usd")
        .and_then(|cost| cost.as_f64())
        .expect("a cost");
    assert!(
        (cost - 0.0024).abs() < 1e-9,
        "the cost is the cumulative one: {cost}"
    );
    assert_eq!(ran.measured("session.turns"), Some(j!(2)));
}

/// Guards: R01 §Session runner, M4 D3 — the turns of a session are counted from each `result`
/// frame itself: a result that gives no `num_turns` adds none, and never repeats the count of an
/// earlier result.
/// Oracle: the frames below; 2 + 1 = 3 by hand, and the result with no count between them.
/// Bite: read the count from the reader's latest totals (a stale count is added again).
#[test]
fn session_measures_turns_come_from_each_result() {
    let watch = candler_core::runner::exec::session::SessionWatch::new(
        Box::new(candler_harness_claude_code::session::Tracker::default()),
        CloseRule::new(Idle::Never, HOOK_SETTLE),
        None,
        candler_core::runner::exec::session::monotonic_clock(),
    );
    let assistant = br#"{"type":"assistant","message":{"content":[]}}"#;
    for (frame, expected) in [
        (
            &br#"{"type":"result","subtype":"success","num_turns":2}"#[..],
            2,
        ),
        (&br#"{"type":"result","subtype":"success"}"#[..], 2),
        (
            &br#"{"type":"result","subtype":"success","num_turns":1}"#[..],
            3,
        ),
    ] {
        watch.feed_stream(assistant);
        watch.feed_stream(frame);
        assert_eq!(
            watch.snapshot().2,
            expected,
            "after {}",
            String::from_utf8_lossy(frame)
        );
    }
}

// ---------------------------------------------------------------------------------------------
// S3: the observation hooks.
// ---------------------------------------------------------------------------------------------

use std::process::{Command, Stdio};

/// Every hook command of the settings file the driver installs.
fn hook_commands() -> Vec<(String, String)> {
    let values = vec![
        ("model.id", j!("m")),
        ("permission_mode", j!("bypassPermissions")),
        ("prompt.template", j!("go")),
    ];
    let resolved: Vec<candler_schemas::values::ResolvedValue> = values
        .into_iter()
        .map(|(path, value)| candler_schemas::values::ResolvedValue::recorded(fp(path), js(value)))
        .collect();
    let driver = ClaudeCodeDriver::new().expect("the driver");
    let spec = driver
        .session(
            &Cell::new(&resolved),
            &Workspace::new(Path::new("/work/run")),
        )
        .expect("a session")
        .expect("a session spec");
    let hooks = spec.hooks.expect("the driver installs hooks");
    assert_eq!(hooks.files.len(), 1, "one settings file");
    let settings: serde_json::Value =
        serde_json::from_slice(&hooks.files[0].contents).expect("settings JSON");
    let mut commands = Vec::new();
    for (event, entries) in settings["hooks"].as_object().expect("hooks") {
        for entry in entries.as_array().expect("entries") {
            for hook in entry["hooks"].as_array().expect("hooks") {
                commands.push((
                    event.clone(), // BORROW: the list owns its event names
                    hook["command"].as_str().expect("a command").to_owned(), // BORROW: the list owns its commands
                ));
            }
        }
    }
    commands
}

/// Guards: R01 §Session runner, I9 — a hook the driver installs never decides: whatever its input
/// and wherever its log write goes (a home that does not exist, a home that is a file, a read-only
/// home), it exits 0 and prints nothing on stdout, which is where a harness reads a decision.
/// Oracle: the exit status and stdout of `sh -c <command>` run by the test, with `HOME` and the
/// payload set by hand; the harness reads exit 2 as a block and stdout as a decision.
/// Bite: a hook command that ends in `exit 2`, or one that prints its payload to stdout.
#[test]
fn session_hooks_never_decide_whatever_the_log_does() {
    let commands = hook_commands();
    assert!(commands.len() >= 2, "Stop and SessionEnd are installed");
    let dir = tempfile::tempdir().expect("a directory");
    let file = dir.path().join("a-file");
    std::fs::write(&file, "x").expect("a file");
    let readonly = dir.path().join("readonly");
    std::fs::create_dir_all(readonly.join(".candler")).expect("a directory");
    let mut perms = std::fs::metadata(readonly.join(".candler"))
        .expect("metadata")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o500);
    std::fs::set_permissions(readonly.join(".candler"), perms).expect("a mode");
    let homes: [(&str, std::path::PathBuf); 3] = [
        ("a home that does not exist", dir.path().join("missing")),
        ("a home that is a file", file),
        ("a log directory that cannot be written", readonly),
    ];
    let payloads: [&[u8]; 3] = [
        br#"{"hook_event_name":"Stop","background_tasks":[],"session_crons":[]}"#,
        b"\xff\xfe not json at all",
        b"",
    ];
    for (event, command) in &commands {
        for (home_name, home) in &homes {
            for payload in payloads {
                let mut child = Command::new("sh")
                    .args(["-c", command])
                    .env("HOME", home)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .expect("sh runs");
                std::io::Write::write_all(&mut child.stdin.take().expect("stdin"), payload)
                    .expect("the payload");
                let output = child.wait_with_output().expect("the hook ends");
                assert_eq!(
                    output.status.code(),
                    Some(0),
                    "{event} with {home_name}: the hook exits 0"
                );
                assert!(
                    output.stdout.is_empty(),
                    "{event} with {home_name}: the hook prints nothing on stdout"
                );
                assert!(
                    output.stderr.is_empty(),
                    "{event} with {home_name}: the hook prints nothing on stderr: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }
    // the read-only directory is made writable again so the temporary directory can be removed
    let mut perms = std::fs::metadata(dir.path().join("readonly/.candler"))
        .expect("metadata")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o700);
    std::fs::set_permissions(dir.path().join("readonly/.candler"), perms).expect("a mode");
}

/// The files under `dir`, relative, sorted.
fn files_under(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).expect("a directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.push(
                    path.strip_prefix(dir)
                        .expect("below")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    found.sort();
    found
}

/// Guards: R01 §Session runner, I9 — the same stub run with the observation hooks installed and
/// without them calls the same tools, runs the same turns and leaves the same files in the
/// workspace: the hooks are installed in the agent's home, never in the workspace.
/// Oracle: the run without hooks, which has no observer at all; the tool-call count is c1's
/// recorded Bash call.
/// Bite: install the hook settings in the workspace (a `.candler` entry appears), or let the
/// hooked run's measures differ (a hook that alters the stream).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_hooks_leave_the_run_as_it_is() {
    let cell = rig::cell_values(&[
        ("runner.idle_s", j!("none")),
        ("budget.per_run.wall_s", j!(4)),
    ]);
    let hooked_steps = {
        let mut steps = rig::replay_of("c1", &BTreeMap::new());
        steps.push(rig::wait_eof(None));
        steps
    };
    let bare_steps: Vec<serde_json::Value> = hooked_steps
        .iter()
        .filter(|step| step.get("hook").is_none())
        .cloned() // BORROW: the run owns its script
        .collect();
    assert!(
        bare_steps.len() < hooked_steps.len(),
        "the hooked script has hook lines"
    );
    let hooked = rig::run(&hooked_steps, &cell, rig::Driver::new(true), None).await;
    let bare = rig::run(&bare_steps, &cell, rig::Driver::new(false), None).await;
    let _ = (hooked.outcome(), bare.outcome());
    for path in ["run.tool_calls", "session.turns", "run.num_turns"] {
        assert_eq!(
            hooked.measured(path),
            bare.measured(path),
            "{path} with and without hooks"
        );
    }
    assert_eq!(hooked.measured("run.tool_calls"), Some(j!({"Bash": 1})));
    assert_eq!(hooked.measured("session.turns"), Some(j!(2)));
    let (with_files, without_files) = (
        files_under(&hooked.run_dir.join("workspace")),
        files_under(&bare.run_dir.join("workspace")),
    );
    assert_eq!(
        with_files, without_files,
        "the workspace holds the same files"
    );
    assert!(
        with_files.iter().all(|file| !file.contains(".candler")),
        "no hook file in the workspace: {with_files:?}"
    );
    assert!(
        hooked.run_dir.join("home/.candler/settings.json").is_file(),
        "the hooks are installed in the home"
    );
    assert!(
        !bare.run_dir.join("home/.candler/settings.json").exists(),
        "no hook is installed without hooks"
    );
    assert!(
        hooked.run_dir.join("raw/hooks.jsonl.gz").is_file(),
        "the hook log is archived as a raw stream: {:?}",
        files_under(&hooked.run_dir)
    );
}
