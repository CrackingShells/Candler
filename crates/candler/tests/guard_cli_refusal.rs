//! Guards for how the binary reports a refusal: one `refusal/1` record on one line of stderr
//! under `--error-format json`, with exit 2 and nothing on stdout, and the unchanged text form
//! without the flag. The binary is the real one, run from a directory outside the project with
//! the PATH-fake docker client of `support/cli_run.rs`.
#![cfg(test)]

#[expect(
    dead_code,
    reason = "this guard uses the world and its launch, not every helper of the shared module"
)]
#[path = "support/cli_run.rs"]
mod cli_run;
#[cfg(unix)]
#[expect(
    dead_code,
    reason = "this guard builds the helper's environment, not every helper of the shared module"
)]
#[path = "support/seam.rs"]
mod seam;

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

use cli_run::world;

/// The record a refusal is expected to print, written by hand from the contract: the code, the
/// side and the pointer, and the start of the message.
struct Expect {
    code: &'static str,
    side: &'static str,
    path: Option<&'static str>,
    message_starts: &'static str,
}

/// The one line of stderr as a JSON document, after checking the shape every refusal shares:
/// exit 2, nothing on stdout, and exactly one line on stderr.
fn record_of(out: &Output) -> Value {
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(2), "exit status\nstderr: {stderr}");
    assert!(
        out.stdout.is_empty(),
        "stdout is empty, it holds {:?}",
        out.stdout
    );
    assert!(
        stderr.ends_with('\n'),
        "stderr ends with a newline: {stderr:?}"
    );
    let line = stderr.trim_end_matches('\n');
    assert!(
        !line.contains('\n'),
        "one line on stderr, it holds: {stderr:?}"
    );
    serde_json::from_str(line).unwrap_or_else(|e| panic!("stderr is JSON ({e}): {line:?}"))
}

/// Panics unless `out` is the refusal `expect` describes.
fn assert_record(out: &Output, expect: &Expect) {
    let record = record_of(out);
    let object = record.as_object().expect("the record is an object");
    assert_eq!(object["schema"], "refusal/1", "{record}");
    assert_eq!(object["code"], expect.code, "{record}");
    assert_eq!(object["side"], expect.side, "{record}");
    match expect.path {
        Some(path) => assert_eq!(object["path"], json!(path), "{record}"),
        None => assert!(!object.contains_key("path"), "no path: {record}"),
    }
    assert!(!object.contains_key("file"), "no file: {record}");
    let message = object["message"].as_str().expect("a message");
    assert!(
        message.starts_with(expect.message_starts),
        "message starts with {:?}: {message:?}",
        expect.message_starts
    );
    assert!(object["causes"].is_array(), "causes is a list: {record}");
}

/// The binary with `args`, from `dir`, with the PATH `path`, a home inside `dir` and nothing else
/// of the caller's environment.
fn candler(dir: &Path, path: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", path)
        .env("HOME", dir.join("home"))
        .env("XDG_STATE_HOME", dir.join("home"))
        .stdin(Stdio::null())
        .output()
        .expect("the binary runs")
}

/// A plug-in manifest with a key the manifest does not have, in a temporary directory.
fn bad_manifest() -> TempDir {
    let dir = TempDir::new().expect("a temporary directory");
    fs::create_dir_all(dir.path().join("home")).expect("a home");
    fs::write(
        dir.path().join("plugin.toml"),
        "schema = \"plugin-manifest/1\"\nname = \"seam\"\nversion = \"0.1.0\"\ncommand = [\"true\"]\n\
         zzz = 1\n\n[protocol]\nmajor = 1\nminor = 0\n\n[environment]\nkind = \"uv\"\n\
         project = \"env\"\nlock = \"env/uv.lock\"\n",
    )
    .expect("the manifest writes");
    fs::write(dir.path().join("suite.toml"), "").expect("the suite writes");
    dir
}

/// The arguments of `evaluate` over the manifest of [`bad_manifest`].
fn evaluate_args<'a>(format: &'a [&'a str]) -> Vec<&'a str> {
    let mut args = format.to_vec(); // BORROW: the call line is extended
    args.extend([
        "evaluate",
        "--plugin",
        "plugin.toml",
        "--suite",
        "suite.toml",
        "--store",
        "store",
    ]);
    args
}

/// The refusal a manifest with an unknown key is.
const MANIFEST: Expect = Expect {
    code: "plugin.unknown_field",
    side: "plugin",
    path: Some("/zzz"),
    message_starts: "`evaluate` failed: the plug-in manifest plugin.toml is not valid: ",
};

/// Adds a second factor that varies the setting `slots.count` to the world's experiment.
fn vary_a_setting(world: &cli_run::World) {
    let mut text = fs::read_to_string(&world.experiment).expect("the experiment reads");
    text.push_str("\n[[factors]]\npath = \"slots.count\"\nlevels = [1, 2]\n");
    fs::write(&world.experiment, text).expect("the experiment writes");
}

/// The refusal a factor over a setting is.
const SETTING_AS_FACTOR: Expect = Expect {
    code: "experiment.setting_as_factor",
    side: "experiment",
    path: Some("/factors/1/path"),
    message_starts: "refused: the setting slots.count cannot be a factor; write it in [fixed]",
};

/// Guards: R2 — a malformed plug-in manifest prints as one `refusal/1` record under
/// `--error-format json`: the code, side and pointer of the schemas' own error, found in the
/// chain of causes of a verb that failed.
///
/// Oracle: a record written by hand from the contract (code `plugin.unknown_field`, side
/// `plugin`, pointer `/zzz`), compared field by field.
///
/// Bite: drop the path from the record; blame the wrong side; print the record on stdout; print a
/// second line for one error.
#[test]
fn r2_a_malformed_manifest_prints_as_a_record() {
    let dir = bad_manifest();
    let out = candler(
        dir.path(),
        "/usr/bin:/bin",
        &evaluate_args(&["--error-format", "json"]),
    );
    assert_record(&out, &MANIFEST);
}

/// Guards: R2 — a factor that varies a setting prints as the record of
/// `experiment.setting_as_factor`, at the `path` of the factor in the experiment file.
///
/// Oracle: a record written by hand from the contract, compared field by field.
///
/// Bite: leave the pointer out of the record; give the refusal the side `candler`.
#[test]
fn r2_a_setting_as_factor_prints_as_a_record() {
    let world = world();
    vary_a_setting(&world);
    let out = world.launch_with(&["--error-format", "json"]);
    assert_record(&out, &SETTING_AS_FACTOR);
}

/// Guards: R2 — an experiment file that is not TOML prints as the record of
/// `experiment.toml_invalid`, which names the file because no pointer into a document exists.
///
/// Oracle: a record written by hand from the contract, compared field by field.
///
/// Bite: leave the file out of the record.
#[test]
fn r2_a_broken_experiment_file_names_the_file() {
    let world = world();
    fs::write(&world.experiment, "replicates = [\n").expect("the experiment writes");
    let out = world.launch_with(&["--error-format", "json"]);
    let record = record_of(&out);
    let file = world.experiment.display().to_string();
    assert_eq!(record["code"], "experiment.toml_invalid", "{record}");
    assert_eq!(record["side"], "experiment", "{record}");
    assert_eq!(record["file"], json!(file), "{record}");
    assert!(record.get("path").is_none(), "no path: {record}");
    let message = record["message"].as_str().expect("a message");
    assert!(
        message.starts_with(format!("refused: {file} is not an experiment: ").as_str()),
        "{message:?}"
    );
}

/// Guards: R2 — without `--error-format` (or with `text`) every refusal prints as it always did:
/// `candler: <message>` on one line, with exit 2.
///
/// Oracle: the text written by hand.
///
/// Bite: print the record under the text format.
#[test]
fn r2_the_text_form_is_unchanged_without_the_flag() {
    let world = world();
    vary_a_setting(&world);
    for extra in [&[][..], &["--error-format", "text"][..]] {
        let out = world.launch_with(extra);
        assert_eq!(out.status.code(), Some(2));
        assert_eq!(out.stdout, b"");
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(
            stderr,
            "candler: refused: the setting slots.count cannot be a factor; write it in [fixed]\n"
        );
    }
    let dir = bad_manifest();
    let out = candler(dir.path(), "/usr/bin:/bin", &evaluate_args(&[]));
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        stderr.starts_with(
            "candler: `evaluate` failed: the plug-in manifest plugin.toml is not valid: "
        ),
        "{stderr:?}"
    );
}

/// The record a refusal that no test of the refusal's own verb reaches is expected to print.
fn assert_refusal(out: &Output, code: &str, side: &str, path: Option<&str>) {
    let record = record_of(out);
    assert_eq!(record["schema"], "refusal/1", "{record}");
    assert_eq!(record["code"], code, "{record}");
    assert_eq!(record["side"], side, "{record}");
    match path {
        Some(pointer) => assert_eq!(record["path"], json!(pointer), "{record}"),
        None => assert!(record.get("path").is_none(), "no path: {record}"),
    }
}

/// Guards: R2 — a plug-in manifest that does not exist prints as the record of
/// `plugin.manifest_unreadable`, side `plugin`, with no pointer because no document was read.
///
/// Oracle: a record written by hand from the contract; the cause is a file that is not there.
///
/// Bite: name the code after the function (`evaluate.failed`); leave the registry's `Read` error
/// without a code, so the record falls back to `candler.internal`.
#[test]
fn r2_a_missing_plugin_manifest_prints_as_a_record() {
    let dir = bad_manifest();
    let out = candler(
        dir.path(),
        "/usr/bin:/bin",
        &[
            "--error-format",
            "json",
            "evaluate",
            "--plugin",
            "absent/plugin.toml",
            "--suite",
            "suite.toml",
            "--store",
            "store",
        ],
    );
    assert_refusal(&out, "plugin.manifest_unreadable", "plugin", None);
}

/// Guards: R2 — an environment with neither `XDG_STATE_HOME` nor `HOME` prints as the record of
/// `evaluation.state_home_unset`, side `environment`.
///
/// Oracle: a record written by hand from the contract; the process environment is empty.
///
/// Bite: give the variant side `plugin`.
#[test]
fn r2_no_state_home_prints_as_an_environment_record() {
    let dir = bad_manifest();
    let out = Command::new(env!("CARGO_BIN_EXE_candler"))
        .args([
            "--error-format",
            "json",
            "evaluate",
            "--plugin",
            "plugin.toml",
        ])
        .args(["--suite", "suite.toml", "--store", "store"])
        .current_dir(dir.path())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .output()
        .expect("the binary runs");
    // The manifest has an unknown key, so the registry refuses first: the empty environment is
    // asserted by a manifest that loads.
    assert_refusal(&out, "plugin.unknown_field", "plugin", Some("/zzz"));
    fs::write(
        dir.path().join("plugin.toml"),
        "schema = \"plugin-manifest/1\"\nname = \"seam\"\nversion = \"0.1.0\"\n\
         command = [\"true\"]\n\n[protocol]\nmajor = 1\nminor = 0\n\n[environment]\n\
         kind = \"uv\"\nproject = \"env\"\nlock = \"env/uv.lock\"\n",
    )
    .expect("the manifest writes");
    let out = Command::new(env!("CARGO_BIN_EXE_candler"))
        .args([
            "--error-format",
            "json",
            "evaluate",
            "--plugin",
            "plugin.toml",
        ])
        .args(["--suite", "suite.toml", "--store", "store"])
        .current_dir(dir.path())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .output()
        .expect("the binary runs");
    assert_refusal(&out, "evaluation.state_home_unset", "environment", None);
}

/// A plug-in of one kind whose params schema takes one integer `n`, set up beside `world` with a
/// link to the helper's project, and a suite that gives `n` a string. Returns the manifest and
/// the suite.
#[cfg(unix)]
fn params_plugin(world: &cli_run::World) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = world.root.join("params-plugin");
    let kinds = dir.join("kinds");
    fs::create_dir_all(&kinds).expect("a kinds directory");
    let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../python/candler-evaluator");
    std::os::unix::fs::symlink(helper, dir.join("env")).expect("a link to the helper");
    fs::write(
        kinds.join("params_kind.py"),
        "NAME = \"seam\"\nVERSION = \"0.1.0\"\n\
         KIND_SPECS = {\"check\": {\"params_schema\": {\"type\": \"object\", \
         \"properties\": {\"n\": {\"type\": \"integer\"}}, \"additionalProperties\": False}}}\n\n\n\
         def check(params, context, prior):\n    return {\"status\": \"pass\", \"evidence\": [{\"note\": \"x\"}]}\n\n\n\
         KINDS = {\"check\": check}\n",
    )
    .expect("the kind writes");
    let manifest = dir.join("plugin.toml");
    fs::write(
        &manifest,
        format!(
            "schema = \"plugin-manifest/1\"\nname = \"seam\"\nversion = \"0.1.0\"\n\
             command = [\"env\", \"PYTHONPATH={}\", \"uv\", \"run\", \"--frozen\", \"python\", \
             \"-m\", \"candler_evaluator\", \"params_kind\"]\n\n[protocol]\nmajor = 1\nminor = 0\n\n\
             [environment]\nkind = \"uv\"\nproject = \"env\"\nlock = \"env/uv.lock\"\n",
            kinds.display()
        ),
    )
    .expect("the manifest writes");
    let suite = dir.join("suite.toml");
    fs::write(
        &suite,
        "schema = \"suite/1\"\n\n[[expectations]]\nid = \"R1\"\nrole = \"gate\"\nkind = \"check\"\n\
         depends_on = []\n\n[expectations.evaluator]\nname = \"seam\"\nrange = \"^0.1\"\n\n\
         [expectations.params]\nn = \"three\"\n",
    )
    .expect("the suite writes");
    (manifest, suite)
}

/// Guards: R2 — a suite whose params the plug-in's schema refuses prints as the record of
/// `suite.params_invalid`, side `suite`, with the pointer of the refused value inside `params`.
/// The plug-in is real: its environment is built and it answers `describe` itself.
///
/// Oracle: a record written by hand from the contract; the schema takes an integer `n` and the
/// suite gives it a string, so the pointer is `/n`.
///
/// Bite: name the code after the function (`evaluate.failed`); drop the pointer from the path of
/// `RegistryError::Params`; give the variant side `plugin`.
#[cfg(unix)]
#[test]
fn r2_refused_suite_params_print_as_a_suite_record() {
    let world = world();
    seam::sync_environment(&world);
    let (manifest, suite) = params_plugin(&world);
    let out = Command::new(env!("CARGO_BIN_EXE_candler"))
        .args(["--error-format", "json", "--store"])
        .arg(world.store())
        .arg("evaluate")
        .arg("--plugin")
        .arg(&manifest)
        .arg("--suite")
        .arg(&suite)
        .current_dir(&world.work)
        .env_remove("CANDLER_STORE")
        .env("HOME", world.root.join("home"))
        .env("XDG_CONFIG_HOME", world.root.join("config"))
        .env("XDG_STATE_HOME", &world.state)
        .stdin(Stdio::null())
        .output()
        .expect("the binary runs");
    // The supervisor reports the evaluator it spawned on stderr first; the refusal is the last line.
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let last = stderr.lines().last().expect("a line of stderr");
    let out = Output {
        stderr: format!("{last}\n").into_bytes(),
        ..out
    };
    assert_refusal(&out, "suite.params_invalid", "suite", Some("/n"));
}

/// Guards: R2 — the refusals of `ingest` print as records that name the file given: a directory
/// with no stream is `import.no_stream` (side `environment`), a legacy results folder given
/// without `--rules` is `import.rules_required` (side `config`) and a path that does not exist is
/// `import.unreadable_path` (side `environment`); none has a pointer, because no document was
/// read.
///
/// Oracle: records written by hand from the contract, compared field by field; the three inputs
/// are an empty directory, a directory holding an `outputs/` directory, and an absent path.
///
/// Bite: give `RulesRequired` the side `environment`; name `NoStream` after the verb
/// (`ingest.failed`).
#[test]
fn r2_ingest_refusals_print_as_records_naming_the_file() {
    let dir = TempDir::new().expect("a temporary directory");
    fs::create_dir_all(dir.path().join("home")).expect("a home");
    fs::create_dir_all(dir.path().join("empty")).expect("an empty directory");
    fs::create_dir_all(dir.path().join("results/run-1/outputs")).expect("a results folder");
    for (path, code, side) in [
        ("empty", "import.no_stream", "environment"),
        ("results", "import.rules_required", "config"),
        ("absent", "import.unreadable_path", "environment"),
    ] {
        let out = candler(
            dir.path(),
            "/usr/bin:/bin",
            &["--error-format", "json", "--store", "store", "ingest", path],
        );
        let record = record_of(&out);
        assert_eq!(record["code"], code, "{path}: {record}");
        assert_eq!(record["side"], side, "{path}: {record}");
        assert_eq!(record["file"], json!(path), "{path}: {record}");
        assert!(record.get("path").is_none(), "no pointer: {record}");
    }
}

/// The refusal a `--set` key under `request` that no setting names is.
const UNKNOWN_REQUEST_KEY: Expect = Expect {
    code: "experiment.unknown_request_key",
    side: "experiment",
    path: None,
    message_starts: "refused: `request.foo` is not a request setting: the known ones are `request.harness`, `request.treatment`, `request.inputs`",
};

/// Guards: R3 — `--set request.<key>` names a request setting or is refused by name: the key
/// `request.foo` prints as the record of `experiment.unknown_request_key`, each setting under
/// `request` (`harness`, `treatment`, `inputs`) is still accepted, and the retired
/// `request.treatment_source` keeps its own refusal.
///
/// Oracle: a record written by hand from the contract; the three keys of `request` the
/// resolver reads. A key is accepted when the run goes on to the next step, which in a world
/// with no docker client is a refusal of another code.
///
/// Bite: accept the unknown key; refuse every key under `request`.
#[test]
fn r3_unknown_request_key_is_refused() {
    let world = world();
    let out = world.launch_with(&["--error-format", "json", "--set", "request.foo=1"]);
    assert_record(&out, &UNKNOWN_REQUEST_KEY);

    let retired = world.launch_with(&[
        "--error-format",
        "json",
        "--set",
        "request.treatment_source=somewhere",
    ]);
    let record = record_of(&retired);
    assert_eq!(record["code"], "config.retired_setting", "{record}");

    let empty = TempDir::new().expect("a temporary directory");
    fs::create_dir_all(empty.path().join("home")).expect("a home");
    for (key, value) in [
        ("request.harness", r#"{"name":"claude-code"}"#),
        ("request.treatment", r#"{"name":"demo"}"#),
        ("request.inputs", "{}"),
    ] {
        let setting = format!("{key}={value}");
        let out = Command::new(env!("CARGO_BIN_EXE_candler"))
            .arg("run")
            .arg(&world.experiment)
            .args(["--error-format", "json", "--set", setting.as_str()])
            .current_dir(&world.work)
            .env_clear()
            .env("PATH", empty.path())
            .env("HOME", empty.path().join("home"))
            .env("XDG_STATE_HOME", &world.state)
            .stdin(Stdio::null())
            .output()
            .expect("the binary runs");
        let record = record_of(&out);
        assert_ne!(
            record["code"], "experiment.unknown_request_key",
            "{key} is a request setting: {record}"
        );
    }
}

/// Guards: R3 — a factor that varies a setting is refused before any image work: the verb is
/// `Refused` with the factor's pointer, and the fake docker client was never called, so the
/// sidecar image is neither looked up nor built for an experiment that cannot run.
///
/// Oracle: the call log of the PATH-fake docker client (`World::calls`), which holds one line
/// per call; the record written by hand from the contract.
///
/// Bite: move the factor check after `ensure_sidecar`; check the factors in `plan_runs` only.
#[test]
fn r3_factor_refusal_precedes_docker() {
    let world = world();
    vary_a_setting(&world);
    let out = world.launch_with(&["--error-format", "json"]);
    assert_record(&out, &SETTING_AS_FACTOR);
    assert_eq!(
        world.calls(),
        "",
        "the refusal comes before any docker call"
    );
}

/// Adds `entry` to the world's experiment: a line of `[fixed]`, or a block at the end.
fn add_to_fixed(world: &cli_run::World, line: &str) {
    let text = fs::read_to_string(&world.experiment).expect("the experiment reads");
    let marker = "[fixed]\n";
    assert!(text.contains(marker), "the experiment has a [fixed] table");
    fs::write(
        &world.experiment,
        text.replace(marker, format!("{marker}{line}\n").as_str()),
    )
    .expect("the experiment writes");
}

/// The refusal an unknown path in `[fixed]` is.
const UNKNOWN_IN_FIXED: Expect = Expect {
    code: "experiment.unknown_path",
    side: "experiment",
    path: Some("/fixed/zzz.unknown_thing"),
    message_starts: "refused: `zzz.unknown_thing` is not a path of the experiment surface; it is written in `[fixed]`",
};

/// The refusal an unknown path in a factor is.
const UNKNOWN_IN_FACTOR: Expect = Expect {
    code: "experiment.unknown_path",
    side: "experiment",
    path: Some("/factors/1/path"),
    message_starts: "refused: `zzz.other` is not a path of the experiment surface; it is written in the factor at `/factors/1`",
};

/// The refusal an unknown path in the level of a factor is.
const UNKNOWN_IN_LEVEL: Expect = Expect {
    code: "experiment.unknown_path",
    side: "experiment",
    path: Some("/factors/1/levels/0/limits/zzz"),
    message_starts: "refused: `isolation.limits.zzz` is not a path of the experiment surface",
};

/// The refusal an unknown path in `--set` is: no pointer, because `--set` is no document.
const UNKNOWN_IN_SET: Expect = Expect {
    code: "experiment.unknown_path",
    side: "experiment",
    path: None,
    message_starts: "refused: `zzz.third` is not a path of the experiment surface; it is written in `--set`",
};

/// The paths of the surface an experiment may write: every path of the oracle `field_paths.toml`
/// (a wildcard row stands for members chosen here) and a child of each open root.
fn surface_paths() -> Vec<String> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../candler-core/tests/oracle/vectors/field_paths.toml");
    let table = fs::read_to_string(file)
        .expect("field_paths.toml is committed")
        .parse::<toml::Table>()
        .expect("field_paths.toml is TOML");
    let mut paths = Vec::new();
    for row in table["path"].as_array().expect("a [[path]] array") {
        let path = row["path"].as_str().expect("a path");
        match path {
            "mcp.*" => paths.push("mcp.filesystem".to_owned()),
            "isolation.limits.*" => {
                paths.extend(["isolation.limits.pids", "isolation.limits.cpus"].map(str::to_owned));
            }
            other => paths.push(other.trim_end_matches("[]").to_owned()),
        }
    }
    paths.extend(["mcp.other.nested", "prompt.values.name"].map(str::to_owned));
    paths
}

/// Guards: R3 — a dotted path that no path class knows is refused with its path and where it was
/// written: under `[fixed]`, in a factor (its `path` or a leaf of a level) and in `--set`, as the
/// record of `experiment.unknown_path`; and every path of the oracle `field_paths.toml` and every
/// child of an open root (`mcp`, `isolation.limits`, `prompt.values`) is still accepted, so the
/// refusal swallows no valid path.
///
/// Oracle: records written by hand from the contract; `field_paths.toml`, transcribed from the
/// design by hand and never read by the code under test. A path is accepted when the run goes on
/// to the next step, which in a world with no docker client is a refusal of another code.
///
/// Bite: skip the check for factors; skip it for `--set`; refuse every path under `isolation`.
#[test]
fn r3_unknown_field_path_is_refused() {
    let world = world();
    add_to_fixed(&world, "\"zzz.unknown_thing\" = 1");
    let out = world.launch_with(&["--error-format", "json"]);
    assert_record(&out, &UNKNOWN_IN_FIXED);

    let world = cli_run::world();
    let mut text = fs::read_to_string(&world.experiment).expect("the experiment reads");
    text.push_str("\n[[factors]]\npath = \"zzz.other\"\nlevels = [1, 2]\n");
    fs::write(&world.experiment, text).expect("the experiment writes");
    let out = world.launch_with(&["--error-format", "json"]);
    assert_record(&out, &UNKNOWN_IN_FACTOR);

    let world = cli_run::world();
    let mut text = fs::read_to_string(&world.experiment).expect("the experiment reads");
    text.push_str("\n[[factors]]\npath = \"isolation\"\nlevels = [{ limits = { zzz = 1 } }]\n");
    fs::write(&world.experiment, text).expect("the experiment writes");
    let out = world.launch_with(&["--error-format", "json"]);
    assert_record(&out, &UNKNOWN_IN_LEVEL);

    let world = cli_run::world();
    let out = world.launch_with(&["--error-format", "json", "--set", "zzz.third=1"]);
    assert_record(&out, &UNKNOWN_IN_SET);

    let empty = TempDir::new().expect("a temporary directory");
    fs::create_dir_all(empty.path().join("home")).expect("a home");
    let world = cli_run::world();
    let mut command = Command::new(env!("CARGO_BIN_EXE_candler"));
    command
        .arg("run")
        .arg(&world.experiment)
        .args(["--error-format", "json"]);
    let paths = surface_paths();
    assert!(
        paths.len() > 40,
        "the oracle lists the surface: {}",
        paths.len()
    );
    for path in &paths {
        command.arg("--set").arg(format!("{path}=1"));
    }
    let out = command
        .current_dir(&world.work)
        .env_clear()
        .env("PATH", empty.path())
        .env("HOME", empty.path().join("home"))
        .env("XDG_STATE_HOME", &world.state)
        .stdin(Stdio::null())
        .output()
        .expect("the binary runs");
    let record = record_of(&out);
    assert_ne!(
        record["code"], "experiment.unknown_path",
        "a path of the surface is refused: {record}"
    );
    assert_ne!(record["code"], "experiment.unknown_request_key", "{record}");
}
