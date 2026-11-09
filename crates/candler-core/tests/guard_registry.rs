#![cfg(test)]
//! Guards for the plug-in registry: the refusals of `check_suite` and the reuse of a built
//! environment.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use candler_core::evaluation::registry::{
    EnvironmentStep, NeedError, Registry, RegistryError, default_env_root,
};
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::ResolveInput;
use candler_core::resolve::preflight::Resolver;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::evaluation::{DescribeResult, SuiteFile};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{Name, RunKey};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/registry");

fn fixture(name: &str) -> String {
    fs::read_to_string(Path::new(FIXTURES).join(name)).expect("a fixture")
}

/// The fixture text with its single occurrence of `from` replaced by `to`.
fn edit(text: &str, from: &str, to: &str) -> String {
    assert_eq!(text.matches(from).count(), 1, "{from:?} must occur once");
    text.replace(from, to)
}

fn describe_map(text: &str) -> BTreeMap<Name, DescribeResult> {
    let result: DescribeResult = serde_json::from_str(text).expect("a describe result");
    BTreeMap::from([(result.name.clone(), result)])
}

fn registry() -> Registry {
    Registry::load(&[Path::new(FIXTURES).join("plugin").join("plugin.toml")]).expect("loads")
}

/// Guards: R8 — a valid manifest, description and suite are accepted, and single-fault copies are
/// refused: an unknown kind, params that fail the kind's schema (at the pointer of the fault), an
/// artifact glob outside `outputs/`, a `snapshots` need and an `inputs` need, the last two with
/// the text of R03 §Scope move. Oracle: the spec text of R03 §Needs and §Scope move.
#[test]
fn the_registry_refuses_what_the_spec_refuses() {
    let registry = registry();
    let describe = fixture("describe.json");
    let suite = fixture("suite.toml");
    let check = |describe: &str, suite: &str| {
        registry.check_suite(
            &SuiteFile::from_toml_str(suite).expect("a suite"),
            &describe_map(describe),
        )
    };

    // The positive control.
    let ok = check(&describe, &suite).expect("the valid base passes");
    assert_eq!(ok.entries().len(), 1);

    let unknown = check(&describe, &edit(&suite, "grep_present", "grep_absent"))
        .expect_err("an unknown kind is refused");
    assert!(
        matches!(unknown, RegistryError::UnknownKind { .. }),
        "{unknown}"
    );

    // The described version 1.0.0 lies just below this range.
    let outside_range = check(&describe, &edit(&suite, "\"^1\"", "\">=1.0.1\""))
        .expect_err("a described version outside the range is refused");
    assert!(
        matches!(&outside_range, RegistryError::VersionOutsideRange { described, .. } if described == "1.0.0"),
        "{outside_range}"
    );
    let text = outside_range.to_string();
    assert!(text.contains("synthetic") && text.contains("1.0.0") && text.contains(">=1.0.1"));

    let bad = check(&describe, &edit(&suite, "\"domain\"", "3")).expect_err("bad params");
    assert!(
        matches!(&bad, RegistryError::Params { pointer, .. } if pointer == "/pattern"),
        "{bad}"
    );
    assert!(bad.to_string().contains("A1") && bad.to_string().contains("grep_present"));

    let outside = check(&edit(&describe, "outputs/**", "../elsewhere/**"), &suite)
        .expect_err("an unmeetable need is refused");
    assert!(
        matches!(
            &outside,
            RegistryError::Need {
                source: NeedError::OutsideOutputs { .. },
                ..
            }
        ),
        "{outside}"
    );

    for (field, text) in [
        ("snapshots", "unsupported in this version: snapshots"),
        ("inputs", "unsupported in this version: inputs"),
    ] {
        let faulty = edit(
            &describe,
            &format!("\"{field}\": []"),
            &format!("\"{field}\": [\"tree\"]"),
        );
        let error = check(&faulty, &suite).expect_err(field);
        let RegistryError::Need { source, .. } = &error else {
            panic!("{field}: not a need error: {error}");
        };
        assert_eq!(source.to_string(), text);
    }
}

/// An event sink that drops what it is told.
struct Quiet;

impl EventSink for Quiet {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// A directory with a `uv` script that appends one line to `counter` and succeeds.
fn stub_uv(dir: &Path, counter: &Path) {
    let script = dir.join("uv");
    fs::write(
        &script,
        format!("#!/bin/sh\necho run >> '{}'\nexit 0\n", counter.display()),
    )
    .expect("written");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("executable");
}

fn lines(file: &Path) -> usize {
    fs::read_to_string(file).map_or(0, |text| text.lines().count())
}

/// Copies `from` into `to`, files and directories.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a dir");
    for entry in fs::read_dir(from).expect("a listing") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copied");
        }
    }
}

/// Guards: R6 — two preflights under the same lock digest run `uv` once, and the environment
/// directory's inode and modification time are the same after the second as before it. Oracle:
/// the operating system's `stat` of the directory and the stub's own counter file.
#[tokio::test]
async fn a_second_preflight_under_one_digest_spawns_nothing() {
    let scratch = tempfile::tempdir().expect("a dir");
    let root = scratch.path();
    let plugin_dir = root.join("plugin");
    copy_tree(&Path::new(FIXTURES).join("plugin"), &plugin_dir);
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("a dir");
    let counter = root.join("counter");
    stub_uv(&bin, &counter);

    let config = Config::new(
        root.to_owned(),
        None,
        UserConfig::default(),
        Some(root.join("state")),
    );
    let env_root = default_env_root(&config).expect("a state home");
    assert_eq!(env_root, root.join("state/candler/environments"));
    let registry = Registry::load(&[plugin_dir.join("plugin.toml")]).expect("loads");
    let plugin = registry.plugins().values().next().expect("one plug-in");
    let supervisor = Arc::new(Supervisor::new(Arc::new(Quiet)));
    let path = format!("{}:/usr/bin:/bin", bin.display());

    let preflight = || async {
        let step = EnvironmentStep::for_plugin(plugin, &env_root, Arc::clone(&supervisor))
            .expect("a step name")
            .expect("a uv environment")
            .with_path(&path);
        Resolver::new(Arc::new(config.clone()))
            .register(Box::new(step))
            .expect("registered")
            .resolve(ResolveInput::default())
            .await
            .expect("the preflight passes")
    };

    preflight().await;
    assert_eq!(lines(&counter), 1, "the first preflight builds");
    let dirs: Vec<PathBuf> = fs::read_dir(&env_root)
        .expect("the env root exists")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(dirs.len(), 1, "one environment per digest");
    let env_dir = dirs.first().expect("one directory");
    assert!(env_dir.join("ready").exists());
    let before = fs::metadata(env_dir).expect("stat");

    preflight().await;
    let after = fs::metadata(env_dir).expect("stat");
    assert_eq!(lines(&counter), 1, "the second preflight spawns nothing");
    assert_eq!(before.ino(), after.ino());
    assert_eq!(before.mtime(), after.mtime());
    assert_eq!(before.mtime_nsec(), after.mtime_nsec());
}
