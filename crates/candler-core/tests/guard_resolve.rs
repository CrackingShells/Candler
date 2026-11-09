#![cfg(test)]
//! Guards for configuration loading, value layering and preflight steps of `resolve`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use candler_core::resolve::config::{Config, ConfigEnv, ConfigError, ProjectConfig, UserConfig};
use candler_core::resolve::layers::{FileValues, Layered, Resolution, ResolveInput};
use candler_core::resolve::preflight::{
    CheckResult, PreflightError, PreflightStep, ResolveContext, ResolveError, Resolver, StepReport,
};
use candler_schemas::primitives::{FieldPath, NameToken};
use candler_schemas::values::{JsonValue, ResolvedValue, SourceEntry, ValueSource};
use futures::future::BoxFuture;
use serde_json::json;

fn fp(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

fn nt(text: &str) -> NameToken {
    text.parse().expect("a name@token")
}

fn js(text: &str) -> JsonValue {
    JsonValue::new(json!(text)).expect("a JSON value")
}

fn text_of(value: &ResolvedValue) -> Option<&str> {
    // BORROW: `.as_str()` views the JSON string as a `&str`
    value.value().and_then(|v| v.as_value().as_str())
}

/// A config with no project whose user state lives in `<cwd>/state`.
fn empty_config(cwd: &Path) -> Config {
    Config::new(
        // BORROW: `.to_owned()` makes the owned path the config holds
        cwd.to_owned(),
        None,
        UserConfig::default(),
        Some(cwd.join("state")),
    )
}

fn num(n: u32) -> JsonValue {
    JsonValue::new(json!(n)).expect("a JSON value")
}

fn setting_paths(resolution: &Resolution) -> Vec<&str> {
    resolution
        .settings
        .iter()
        .map(|v| v.path().as_str()) // BORROW: `.as_str()` views the path as a `&str`
        .collect()
}

fn value_paths(resolution: &Resolution) -> Vec<&str> {
    resolution
        .values
        .iter()
        .map(|v| v.path().as_str()) // BORROW: `.as_str()` views the path as a `&str`
        .collect()
}

/// Guards: I12 — the six layers of R01's resolution order: for a path set at levels k..6 only
/// (explicit argument, experiment or suite file, project config, user config, derived value, core
/// default), level k wins and names its source; the sources cover exactly the values and settings,
/// and the store root and an explicit path are settings. Oracle: R01 §Operation API, resolution
/// order and value sources, written out as the table below.
#[test]
fn the_highest_layer_that_sets_a_path_wins_and_names_its_source() {
    let root = tempfile::tempdir().expect("a temp dir");
    let cwd = root.path();
    let config = empty_config(cwd);
    let seed = fp("run.seed");
    let step = nt("probe@1");
    let table: [(&str, ValueSource); 6] = [
        ("v1", ValueSource::ExplicitArgument),
        ("v2", ValueSource::ExperimentFile),
        ("v3", ValueSource::ProjectConfig),
        ("v4", ValueSource::UserConfig),
        // BORROW: `.clone()` makes the owned step the source holds
        ("v5", ValueSource::derived(step.clone())),
        ("v6", ValueSource::CoreDefault),
    ];
    for (row, (winner, source)) in table.iter().enumerate() {
        let level = row + 1;
        let input = ResolveInput::default()
            .explicit_path(&config, fp("run.input_dir"), Path::new("in/file"))
            .expect("a UTF-8 path");
        let mut layered = Layered::new(&config, input).expect("layers");
        let at = |n: usize| (level <= n).then(|| (seed.clone(), js(&format!("v{n}")))); // BORROW: owned path for the layer entry
        layered.explicit.extend(at(1));
        layered.file.extend(at(2));
        layered.project.extend(at(3));
        layered.user.extend(at(4));
        // BORROW: `.clone()` makes the owned entries the layer holds
        layered
            .derived
            // BORROW: each derived entry owns a copy of the step name
            .extend(at(5).map(|(path, value)| (step.clone(), path, value)));
        layered.core_defaults.extend(at(6));

        let resolution = layered.resolve();
        let values: Vec<&FieldPath> = resolution.values.iter().map(ResolvedValue::path).collect();
        assert_eq!(values, vec![&seed], "row {level}: values");
        let settings: Vec<&FieldPath> = resolution
            .settings
            .iter()
            .map(ResolvedValue::path)
            .collect();
        let (input_dir, store_root) = (fp("run.input_dir"), fp("store.root"));
        assert_eq!(settings, vec![&input_dir, &store_root], "row {level}");
        let sources: Vec<&FieldPath> = resolution.sources.iter().map(SourceEntry::path).collect();
        assert_eq!(
            sources,
            vec![&input_dir, &seed, &store_root],
            "row {level}: sources cover values and settings"
        );

        assert_eq!(
            resolution.values.first().and_then(text_of),
            Some(*winner),
            "row {level}: value"
        );
        let source_of = |path: &FieldPath| {
            resolution
                .sources
                .iter()
                .find(|entry| entry.path() == path)
                .map(SourceEntry::source)
        };
        assert_eq!(source_of(&seed), Some(source), "row {level}: source");
        assert_eq!(
            source_of(&input_dir),
            Some(&ValueSource::ExplicitArgument),
            "row {level}"
        );
        assert_eq!(
            source_of(&store_root),
            Some(&ValueSource::CoreDefault),
            "row {level}"
        );
        let cwd_text = cwd.display();
        assert_eq!(
            resolution.settings.first().and_then(text_of),
            // BORROW: `.as_str()` views the expected text as a `&str`
            Some(format!("{cwd_text}/in/file").as_str())
        );
        assert_eq!(
            resolution.settings.get(1).and_then(text_of),
            // BORROW: `.as_str()` views the expected text as a `&str`
            Some(format!("{cwd_text}/state/candler/store").as_str())
        );
    }
}

/// One refusal: a file's text, and the key the error must name.
struct Refusal {
    text: &'static str,
    key: &'static str,
}

fn refusal_cases(valid: &'static str, refusals: &[Refusal]) -> Vec<(String, &'static str)> {
    let mut cases = vec![(valid.to_owned(), "")]; // BORROW: owned text for the case
    for refusal in refusals {
        cases.push((format!("{valid}{}", refusal.text), refusal.key));
    }
    cases
}

/// Guards: D8 — a key that no table of R01 §Configuration defines is refused in each table of
/// both files, naming the file and the dotted key, and `slots.count` is refused in a project
/// config naming the file; the valid file loads. Oracle: the tables and keys of R01
/// §Configuration (project: `store.root`, `plugins.dirs[]`; user: `slots.count`), listed by hand.
#[test]
fn unknown_keys_are_refused_naming_the_file_and_key() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project_file = dir.path().join("candler.toml");
    let valid_project = "[store]\nroot = \"data/store\"\n[plugins]\ndirs = [\"plug\"]\n";
    let project_cases = refusal_cases(
        valid_project,
        &[
            Refusal {
                text: "[store.extra]\n",
                key: "store.extra",
            },
            Refusal {
                text: "[extra]\nx = 1\n",
                key: "extra",
            },
        ],
    );
    let mut cases: Vec<(String, &str)> = project_cases;
    // A key added to a table that is already open must be read inside that table.
    cases.push((
        "[store]\nroot = \"r\"\nbogus = 1\n".to_owned(), // BORROW: owned text for the case
        "store.bogus",
    ));
    cases.push((
        "[plugins]\ndirs = []\nbogus = 1\n".to_owned(), // BORROW: owned text for the case
        "plugins.bogus",
    ));
    cases.push(("toplevel = 1\n".to_owned(), "toplevel")); // BORROW: owned text for the case
    for (text, key) in &cases {
        fs::write(&project_file, text).expect("write the project file");
        let loaded = ProjectConfig::discover(dir.path());
        if key.is_empty() {
            assert!(loaded.is_ok(), "the valid file is the positive control");
            continue;
        }
        let error = loaded.expect_err("an unknown key is refused");
        assert!(
            matches!(&error, ConfigError::UnknownKey { file, key: named } if *file == project_file && named == key),
            "{text:?}: {error}"
        );
        let message = error.to_string();
        assert!(
            message.contains(&project_file.display().to_string()),
            "{message}"
        );
        assert!(message.contains(key), "{message}");
    }

    fs::write(
        &project_file,
        format!("{valid_project}[slots]\ncount = 2\n"),
    )
    .expect("write the project file");
    let error = ProjectConfig::discover(dir.path()).expect_err("slots is machine-wide");
    assert!(
        matches!(&error, ConfigError::MachineWideKey { file, key } if *file == project_file && key == "slots.count"),
        "{error}"
    );
    assert!(
        error
            .to_string()
            .contains(&project_file.display().to_string())
    );

    let xdg = dir.path().join("xdg");
    fs::create_dir_all(xdg.join("candler")).expect("create the user config dir");
    let user_file = xdg.join("candler").join("config.toml");
    let env = ConfigEnv::new(Some(xdg), None, None);
    let valid_user = "[slots]\ncount = 4\n";
    let user_cases = refusal_cases(
        valid_user,
        &[
            Refusal {
                text: "bogus = 1\n",
                key: "slots.bogus",
            },
            Refusal {
                text: "[store]\nroot = \"x\"\n",
                key: "store",
            },
            Refusal {
                text: "[extra]\nx = 1\n",
                key: "extra",
            },
        ],
    );
    for (text, key) in &user_cases {
        fs::write(&user_file, text).expect("write the user file");
        let loaded = UserConfig::load(&env);
        if key.is_empty() {
            assert_eq!(loaded.expect("the valid file").slots_count, Some(4));
            continue;
        }
        let error = loaded.expect_err("an unknown key is refused");
        assert!(
            matches!(&error, ConfigError::UnknownKey { file, key: named } if *file == user_file && named == key),
            "{text:?}: {error}"
        );
        let message = error.to_string();
        assert!(
            message.contains(&user_file.display().to_string()),
            "{message}"
        );
        assert!(message.contains(key), "{message}");
    }
}

/// Guards: I14 — a path written in a config file or an experiment file resolves against that
/// file's directory, and a path given as an argument against the working directory.
/// Oracle: I14 of R01 §Invariants, with the expected paths written out by hand.
/// Bite: in `FileValues::relative_to`, join the text to `std::env::current_dir()` instead of `dir`.
#[tokio::test]
async fn paths_resolve_against_the_file_or_the_working_directory() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = dir.path();
    let cwd = root.join("a").join("b");
    fs::create_dir_all(&cwd).expect("create the working directory");
    fs::write(
        root.join("candler.toml"),
        "[store]\nroot = \"data/store\"\n",
    )
    .expect("write the project file");
    let env = ConfigEnv::default();
    let config = Arc::new(Config::load(&cwd, &env).expect("the config loads"));
    let resolver = Resolver::new(Arc::clone(&config));
    let root_text = root.display();

    let from_file = resolver
        .resolve(ResolveInput::default())
        .await
        .expect("resolves");
    let store = |resolution: &Resolution| {
        resolution
            .settings
            .iter()
            .find(|v| v.path() == &fp("store.root"))
            .and_then(text_of)
            .map(str::to_owned) // BORROW: owned text outlives the resolution
    };
    assert_eq!(store(&from_file), Some(format!("{root_text}/data/store")));

    let input = ResolveInput::default()
        .explicit_path(&config, fp("store.root"), Path::new("data/store"))
        .expect("a UTF-8 path");
    let from_argument = resolver.resolve(input).await.expect("resolves");
    assert_eq!(
        store(&from_argument),
        Some(format!("{root_text}/a/b/data/store"))
    );

    // An experiment file in `exp/` writes build paths; its directory is not the working directory.
    let file_dir = root.join("exp");
    let file = FileValues::new(vec![
        (fp("isolation.build.lock"), js("env.lock")),
        (
            fp("isolation.build.requirements"),
            JsonValue::new(json!([
                "reqs/base.txt",
                "treatment:reqs/skill.txt",
                "/abs/pin.txt"
            ]))
            .expect("a JSON value"),
        ),
    ])
    .relative_to(&file_dir)
    .expect("a UTF-8 path");
    let input = ResolveInput::default()
        .file(file)
        .explicit_path(
            &config,
            fp("isolation.build.dockerfile"),
            Path::new("Dockerfile"),
        )
        .expect("a UTF-8 path");
    let built = resolver.resolve(input).await.expect("resolves");
    let setting = |name: &str| {
        built
            .settings
            .iter()
            .find(|v| v.path() == &fp(name))
            .and_then(|v| v.value())
            .map(|v| v.as_value().clone()) // BORROW: owned JSON outlives the resolution
    };
    assert_eq!(
        setting("isolation.build.lock"),
        Some(json!(format!("{root_text}/exp/env.lock")))
    );
    assert_eq!(
        setting("isolation.build.requirements"),
        Some(json!([
            format!("{root_text}/exp/reqs/base.txt"),
            "treatment:reqs/skill.txt",
            "/abs/pin.txt"
        ]))
    );
    assert_eq!(
        setting("isolation.build.dockerfile"),
        Some(json!(format!("{root_text}/a/b/Dockerfile")))
    );
}

/// A step that derives values, checks, and records what it saw.
struct Probe {
    name: NameToken,
    derived: Vec<(FieldPath, JsonValue)>,
    outcome: Outcome,
    ran: Arc<Mutex<Vec<String>>>,
    seen: Arc<Mutex<Vec<Arc<ResolveContext>>>>,
}

#[derive(Clone, Copy)]
enum Outcome {
    Pass,
    FailCheck,
    Error,
}

impl PreflightStep for Probe {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        // BORROW: owned values move into the future
        let (name, derived, outcome) = (self.name.clone(), self.derived.clone(), self.outcome);
        let (ran, seen) = (Arc::clone(&self.ran), Arc::clone(&self.seen));
        Box::pin(async move {
            ran.lock().expect("lock").push(name.to_string());
            seen.lock().expect("lock").push(ctx);
            match outcome {
                Outcome::Error => Err(PreflightError::Failed {
                    detail: "cannot run".to_owned(), // BORROW: owned text for the error
                }),
                Outcome::Pass | Outcome::FailCheck => {
                    let passed = matches!(outcome, Outcome::Pass);
                    let check = CheckResult::new(nt("probe_check@1"), passed, "probed".to_owned()); // BORROW: owned text
                    Ok(derived
                        .into_iter()
                        .fold(StepReport::new(name), |report, (p, v)| report.derive(p, v))
                        .check(check))
                }
            }
        })
    }
}

type Log = Arc<Mutex<Vec<String>>>;
type Seen = Arc<Mutex<Vec<Arc<ResolveContext>>>>;

fn probe(
    name: &str,
    derived: Vec<(FieldPath, JsonValue)>,
    outcome: Outcome,
    ran: &Log,
    seen: &Seen,
) -> Box<dyn PreflightStep> {
    // TRAIT_OBJECT: steps of different types are registered side by side
    Box::new(Probe {
        name: nt(name),
        derived,
        outcome,
        ran: Arc::clone(ran),
        seen: Arc::clone(seen),
    })
}

/// Guards: Cond 5 (`PreflightStep` and `ResolveContext`) — a step sees the value an earlier step
/// derived, with source `derived {step}`; a
/// duplicate step name is refused; of three steps, two failing ones (a failed check and an
/// error) are both reported, the third still runs without seeing what the failed one derived,
/// and the resolution fails. Oracle: R01 §Operation API, preflight steps (registration order,
/// duplicate refusal, every failure reported before resolution fails).
#[tokio::test]
async fn steps_run_in_order_and_every_failure_is_reported() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let config = Arc::new(empty_config(dir.path()));
    let ran: Log = Arc::default();
    let seen: Seen = Arc::default();
    let derived_path = fp("plugin.version");

    let resolver = Resolver::new(Arc::clone(&config))
        .register(probe(
            "first@1",
            // BORROW: `.clone()` makes the owned path the entry holds
            vec![(derived_path.clone(), js("1.2"))],
            Outcome::Pass,
            &ran,
            &seen,
        ))
        .expect("first registers")
        .register(probe("second@1", vec![], Outcome::Pass, &ran, &seen))
        .expect("second registers");
    let resolution = resolver
        .resolve(ResolveInput::default())
        .await
        .expect("both steps pass");
    {
        let contexts = seen.lock().expect("lock");
        let second_saw = contexts.get(1).expect("the second step ran");
        assert_eq!(second_saw.get(&derived_path), Some(&js("1.2")));
        let source = second_saw
            .sources
            .iter()
            .find(|entry| entry.path() == &derived_path)
            .map(SourceEntry::source);
        assert_eq!(source, Some(&ValueSource::derived(nt("first@1"))));
        let reported: Vec<&NameToken> = resolution.preflight.iter().map(|r| &r.step).collect();
        assert_eq!(reported, vec![&nt("first@1"), &nt("second@1")]);
    }

    let duplicate = resolver.register(probe("first@1", vec![], Outcome::Pass, &ran, &seen));
    assert!(matches!(
        duplicate,
        Err(ResolveError::DuplicateStep { name }) if name == nt("first@1")
    ));

    // The version token does not make a step a different step.
    let other_version = Resolver::new(Arc::clone(&config))
        .register(probe("first@1", vec![], Outcome::Pass, &ran, &seen))
        .expect("registers")
        .register(probe("first@2", vec![], Outcome::Pass, &ran, &seen));
    assert!(matches!(
        other_version,
        Err(ResolveError::DuplicateStep { name }) if name == nt("first@2")
    ));

    // Two passing steps that derive one path are refused, naming both steps and the path.
    let twice = Resolver::new(Arc::clone(&config))
        .register(probe(
            "left@1",
            // BORROW: `.clone()` makes the owned path the entry holds
            vec![(derived_path.clone(), js("1.2"))],
            Outcome::Pass,
            &ran,
            &seen,
        ))
        .expect("registers")
        .register(probe(
            "right@1",
            // BORROW: `.clone()` makes the owned path the entry holds
            vec![(derived_path.clone(), js("1.3"))],
            Outcome::Pass,
            &ran,
            &seen,
        ))
        .expect("registers");
    let error = twice
        .resolve(ResolveInput::default())
        .await
        .expect_err("one path, two deriving steps");
    assert!(
        matches!(
            &error,
            ResolveError::DerivedTwice { path, first, second }
                if *path == derived_path && first == "left@1" && second == "right@1"
        ),
        "{error}"
    );
    let message = error.to_string();
    assert!(
        message.contains("left@1")
            && message.contains("right@1")
            && message.contains("plugin.version"),
        "{message}"
    );

    ran.lock().expect("lock").clear();
    seen.lock().expect("lock").clear();
    let resolver = Resolver::new(config)
        .register(probe(
            "bad_check@1",
            vec![(fp("dropped.one"), js("x"))],
            Outcome::FailCheck,
            &ran,
            &seen,
        ))
        .expect("registers")
        .register(probe("bad_run@1", vec![], Outcome::Error, &ran, &seen))
        .expect("registers")
        .register(probe("last@1", vec![], Outcome::Pass, &ran, &seen))
        .expect("registers");
    let error = resolver
        .resolve(ResolveInput::default())
        .await
        .expect_err("two steps fail");
    let ResolveError::Preflight { reports, errors } = error else {
        panic!("expected a preflight failure");
    };
    let failed_checks: Vec<&NameToken> = reports.iter().map(|r| &r.step).collect();
    assert_eq!(failed_checks, vec![&nt("bad_check@1")]);
    let failed_runs: Vec<&NameToken> = errors.iter().map(|e| &e.step).collect();
    assert_eq!(failed_runs, vec![&nt("bad_run@1")]);
    assert_eq!(
        *ran.lock().expect("lock"),
        vec!["bad_check@1", "bad_run@1", "last@1"]
    );
    let last_saw = seen
        .lock()
        .expect("lock")
        .get(2)
        .cloned()
        .expect("the third ran");
    assert_eq!(last_saw.get(&fp("dropped.one")), None);
}

/// Guards: Cond 1 (cell digest content) — a job cap (`budget.*` except `budget.per_run.*`) and
/// `replicates` resolve into settings, whether given as an argument or in the experiment file,
/// so they never reach a cell digest; a per-run cap stays a value. Oracle: R01 §Run manifest,
/// "the per-run caps are cell values under `budget.per_run.*`; the job caps (every other
/// `budget.*` path) and `replicates` are operational settings".
#[test]
fn job_caps_and_replicates_are_settings_and_per_run_caps_are_values() {
    let root = tempfile::tempdir().expect("a temp dir");
    let config = empty_config(root.path());
    let input = ResolveInput::default()
        .explicit(fp("budget.max_runs"), num(5))
        .explicit(fp("budget.per_run.wall_s"), num(600))
        .file(FileValues::new(vec![
            (fp("budget.max_runs"), num(9)),
            (fp("budget.max_cost_usd"), num(20)),
            (fp("budget.max_parallel"), num(2)),
            (fp("replicates"), num(3)),
            (fp("budget.per_run.turns"), num(40)),
            (fp("run.seed"), num(7)),
        ]));
    let layered = Layered::new(&config, input).expect("layers");
    let resolution = layered.resolve();
    assert_eq!(
        setting_paths(&resolution),
        vec![
            "budget.max_cost_usd",
            "budget.max_parallel",
            "budget.max_runs",
            "replicates",
            "store.root",
        ]
    );
    assert_eq!(
        value_paths(&resolution),
        vec!["budget.per_run.turns", "budget.per_run.wall_s", "run.seed"]
    );
    // The explicit `budget.max_runs` wins over the file's, inside settings.
    let max_runs = resolution
        .settings
        .iter()
        .find(|v| v.path() == &fp("budget.max_runs"))
        .expect("a setting");
    assert_eq!(max_runs.value(), Some(&num(5)));
}

/// Guards: D8 — the default store root is `<project root>/.candler/store` in a project, and
/// otherwise `$XDG_STATE_HOME/candler/store`, else `~/.local/state/candler/store`, an empty
/// `XDG_STATE_HOME` counting as unset; with no state directory at all there is no default.
/// Oracle: R01 §Configuration and §Run store, the paths written out by hand.
#[tokio::test]
async fn the_default_store_root_follows_the_project_or_the_user_state_directory() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let base = dir.path();
    let work = base.join("work");
    fs::create_dir_all(&work).expect("create the working directory");
    let store = |config: Config| async move {
        let resolution = Resolver::new(Arc::new(config))
            .resolve(ResolveInput::default())
            .await
            .expect("resolves");
        let found = resolution
            .settings
            .iter()
            .find(|v| v.path() == &fp("store.root"))
            .and_then(text_of)
            .map(str::to_owned); // BORROW: owned text outlives the resolution
        let source = resolution
            .sources
            .iter()
            .find(|e| e.path() == &fp("store.root"))
            .map(|e| e.source().clone()); // BORROW: owned source outlives the resolution
        (found, source)
    };
    let base_text = base.display();

    let xdg = ConfigEnv::new(None, Some(base.join("xdg-state")), Some(base.join("home")));
    let config = Config::load(&work, &xdg).expect("loads");
    assert_eq!(
        store(config).await,
        (
            Some(format!("{base_text}/xdg-state/candler/store")),
            Some(ValueSource::CoreDefault)
        )
    );

    let empty = ConfigEnv::new(None, Some(PathBuf::new()), Some(base.join("home")));
    let config = Config::load(&work, &empty).expect("loads");
    assert_eq!(
        store(config).await.0,
        Some(format!("{base_text}/home/.local/state/candler/store"))
    );

    let none = ConfigEnv::default();
    let config = Config::load(&work, &none).expect("loads");
    assert_eq!(store(config).await, (None, None));

    // In a project the default stays inside it, whatever the state directory is.
    fs::write(work.join("candler.toml"), "[plugins]\ndirs = []\n").expect("write the project file");
    let config = Config::load(&work, &xdg).expect("loads");
    assert_eq!(
        store(config).await.0,
        Some(format!("{base_text}/work/.candler/store"))
    );
}
