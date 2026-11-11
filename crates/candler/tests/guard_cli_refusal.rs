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
    path: Some("/factors"),
    message_starts: "`run` failed: the setting slots.count cannot be a factor; write it in [fixed]",
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
/// `experiment.setting_as_factor`, at the experiment file's `factors`.
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
            "candler: `run` failed: the setting slots.count cannot be a factor; write it in [fixed]\n"
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

/// The rows of the refusals that a later leaf codes: each runs today and reports the fallback
/// code, so a leaf that codes one fails this guard until it replaces the row with the record it
/// expects.
const PENDING: &[(&str, &[&str])] = &[(
    "a missing plug-in directory",
    &[
        "evaluate",
        "--plugin",
        "absent/plugin.toml",
        "--suite",
        "suite.toml",
        "--store",
        "store",
    ],
)];

/// Guards: R2 — the refusals of the evaluation registry are listed here and still print as
/// `candler.internal`, so the leaf that codes them replaces each row by the record it expects.
/// The row for a suite whose params the plug-in's schema refuses needs a live plug-in
/// environment and is added by that leaf.
///
/// Oracle: the fallback code of `docs/contract/codes.toml`, `[internal]`.
///
/// Bite: code the registry's `Read` error: the row turns red.
#[test]
fn r2_the_pending_rows_still_print_as_internal() {
    let dir = bad_manifest();
    for (name, args) in PENDING {
        let mut all = vec!["--error-format", "json"];
        all.extend_from_slice(args);
        let out = candler(dir.path(), "/usr/bin:/bin", &all);
        let record = record_of(&out);
        assert_eq!(
            record["code"], "candler.internal",
            "{name} is coded: replace its row with the record it expects\n{record}"
        );
        assert_eq!(record["side"], "candler", "{name}: {record}");
    }
}
