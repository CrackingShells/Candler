#![cfg(test)]
//! Guard for the refusal of `candler diff --equivalence`: a bad equivalence file reaches
//! `--error-format json` with its own code and side, and the text form is the error's own words.

use std::fs;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

/// The real binary with `args`, run from `home` over an empty store.
fn candler(home: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .current_dir(home)
        .env_remove("CANDLER_STORE")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .arg("--store")
        .arg(home.join("store"))
        .args(args)
        .output()
        .expect("the binary runs")
}

/// Guards: R2 — an equivalence file that is not valid JSON, and one that cannot be read, are
/// refused under `--error-format json` with exit 2, the codes `suite.equivalence_invalid` and
/// `suite.equivalence_unreadable`, the side `suite` and the file named, and the text form of the
/// first still reads `refused: the equivalence file is not valid: ...`.
/// Oracle: the files the test wrote or left missing, and the codes and side the contract's
/// `codes.toml` gives `EquivalenceError::Parse` and `Io`.
/// Bite: the load failure flattened back to `CliError::refused(format!(..))` prints
/// `candler.internal` for both.
#[test]
fn r2_a_bad_equivalence_file_is_refused_with_its_code() {
    let dir = TempDir::new().expect("a directory");
    let bad = dir.path().join("equivalence.json");
    fs::write(&bad, "not json").expect("the file writes");
    let missing = dir.path().join("missing.json");
    for (file, code) in [
        (&bad, "suite.equivalence_invalid"),
        (&missing, "suite.equivalence_unreadable"),
    ] {
        let path = file.to_str().expect("a UTF-8 path");
        let args = [
            "diff",
            "--from",
            "a@1.0.0",
            "--to",
            "b@1.0.0",
            "--format",
            "json",
            "--equivalence",
            path,
        ];
        let mut json_args = vec!["--error-format", "json"];
        json_args.extend_from_slice(&args);
        let out = candler(dir.path(), &json_args);
        assert_eq!(out.status.code(), Some(2), "{out:?}");
        assert!(out.stdout.is_empty(), "{out:?}");
        let record: Value = serde_json::from_slice(&out.stderr).expect("one JSON record");
        assert_eq!(record["code"], code, "{record}");
        assert_eq!(record["side"], "suite", "{record}");
        assert_eq!(record["file"], path, "{record}");
    }
    let out = candler(
        dir.path(),
        &[
            "diff",
            "--from",
            "a@1.0.0",
            "--to",
            "b@1.0.0",
            "--format",
            "json",
            "--equivalence",
            bad.to_str().expect("a UTF-8 path"),
        ],
    );
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(
        text.starts_with("candler: refused: the equivalence file is not valid: "),
        "{text}"
    );
}
