//! `candler import`: a legacy results folder into the store, under rules the caller supplies as
//! a TOML file.

use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use candler_core::import::assemble::import;
use candler_core::import::digest_of;
use candler_core::import::rules::{
    HarnessRule, ImportRules, MarkerCode, MarkerRule, SnapshotRule, TreatmentRule,
};
use candler_core::import::streams::ReaderFactory;
use candler_core::store::Store;
use candler_harness::reader::TraceReader;
use candler_harness_claude_code::ClaudeCodeReader;
use candler_schemas::primitives::{FieldPath, Name, NameToken, ReasonCode};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;

use super::{CliError, Exit, common};
use crate::Cli;

/// The arguments of `candler import`.
#[derive(Debug, clap::Args)]
pub struct ImportArgs {
    /// The legacy results folder.
    pub corpus: PathBuf,
    /// The TOML file that holds the import rules.
    #[arg(long)]
    pub rules: PathBuf,
}

/// The rules file: one field per import rule.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RulesFile {
    format: String,
    instrument: String,
    suite_kind: String,
    #[serde(default)]
    metadata: Vec<String>,
    harness: HarnessSection,
    treatment: TreatmentSection,
    #[serde(default)]
    snapshot: Vec<SnapshotSection>,
    #[serde(default)]
    marker: Vec<MarkerSection>,
}

/// `[harness]`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HarnessSection {
    name: String,
    version_pointer: String,
}

/// `[treatment]`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TreatmentSection {
    pointer: String,
    resolved: String,
}

/// `[[snapshot]]`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotSection {
    kind: String,
    pointer: String,
}

/// `[[marker]]`: a pattern and exactly one of `code` and `capture`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MarkerSection {
    pattern: String,
    code: Option<String>,
    capture: Option<String>,
}

/// A primitive read from the text of `field`.
fn primitive<T>(field: &str, text: &str) -> Result<T, CliError>
where
    T: FromStr,
    T::Err: Display,
{
    text.parse::<T>()
        .map_err(|e| CliError::refused(format!("candler import: rules `{field}`: {e}")))
}

/// The import rules the file at `path` states.
fn load_rules(path: &Path) -> Result<(ImportRules, String), CliError> {
    let bytes = fs::read(path).map_err(|e| {
        CliError::refused(format!(
            "candler import: cannot read the rules file {}: {e}",
            path.display()
        ))
    })?;
    // The digest is of the file's bytes, whatever they parse to: it goes into every import record.
    let digest = digest_of(bytes.as_slice()).map_err(|e| CliError::failed("import", e))?; // BORROW: explicit .as_slice()
    let text = String::from_utf8(bytes).map_err(|e| {
        CliError::refused(format!(
            "candler import: the rules file {} is not UTF-8: {e}",
            path.display()
        ))
    })?;
    // BORROW: explicit .as_str()
    let file: RulesFile = toml::from_str(text.as_str())
        .map_err(|e| CliError::refused(format!("candler import: rules file: {e}")))?;
    let mut builder = ImportRules::builder()
        .source_digest(digest)
        .format(primitive::<NameToken>("format", file.format.as_str())?) // BORROW: explicit .as_str()
        .instrument(primitive::<Name>("instrument", file.instrument.as_str())?) // BORROW: explicit .as_str()
        .suite_kind(primitive::<Name>("suite_kind", file.suite_kind.as_str())?) // BORROW: explicit .as_str()
        .harness(HarnessRule::new(
            primitive::<Name>("harness.name", file.harness.name.as_str())?, // BORROW: explicit .as_str()
            file.harness.version_pointer.clone(), // BORROW: the rule owns the pointer
        ))
        .treatment(TreatmentRule::new(
            file.treatment.pointer.clone(), // BORROW: the rule owns the pointer
            primitive::<FieldPath>("treatment.resolved", file.treatment.resolved.as_str())?, // BORROW: explicit .as_str()
        ));
    for item in &file.metadata {
        builder = builder.metadata(primitive::<FieldPath>("metadata", item.as_str())?); // BORROW: explicit .as_str()
    }
    for item in &file.snapshot {
        builder = builder.snapshot(SnapshotRule::new(
            primitive::<Name>("snapshot.kind", item.kind.as_str())?, // BORROW: explicit .as_str()
            item.pointer.clone(), // BORROW: the rule owns the pointer
        ));
    }
    for item in &file.marker {
        let pattern = Regex::new(item.pattern.as_str()) // BORROW: explicit .as_str()
            .map_err(|e| {
                CliError::refused(format!("candler import: rules `marker.pattern`: {e}"))
            })?;
        let code = match (item.code.as_deref(), item.capture.as_deref()) {
            (Some(code), None) => MarkerCode::Fixed(primitive::<ReasonCode>("marker.code", code)?),
            (None, Some(group)) => MarkerCode::Capture(group.to_owned()), // BORROW: the rule owns the group name
            (Some(_), Some(_)) | (None, None) => {
                return Err(CliError::refused(
                    "candler import: rules `marker`: exactly one of `code` and `capture` is required",
                ));
            }
        };
        builder = builder.marker(MarkerRule::new(pattern, code));
    }
    let rules = builder
        .build()
        .map_err(|e| CliError::refused(format!("candler import: rules: {e}")))?;
    Ok((rules, file.harness.name))
}

/// Checks that the reader of the harness `name` can be built, so that the factory cannot fail.
/// The table holds the readers this binary ships.
fn check_reader(name: &str) -> Result<(), CliError> {
    match name {
        "claude-code" => ClaudeCodeReader::new()
            .map(drop)
            .map_err(|e| CliError::failed("import", e)),
        other => Err(CliError::refused(format!(
            "candler import: rules `harness.name`: no reader for `{other}`"
        ))),
    }
}

/// A new reader of the harness `name`, which [`check_reader`] accepted.
// TRAIT_OBJECT: the import's factory type returns boxed readers
fn new_reader(name: &str) -> Box<dyn TraceReader> {
    if let ("claude-code", Ok(reader)) = (name, ClaudeCodeReader::new()) {
        Box::new(reader)
    } else {
        // EXPLICIT: construction succeeded once in `check_reader` and its inputs are constants
        eprintln!("candler import: cannot build the reader for `{name}`");
        std::process::exit(2)
    }
}

/// Imports `args.corpus` into the store under the rules of `args.rules` and prints
/// `{"runs": n, "records": n}`.
///
/// # Errors
/// Returns [`CliError::Refused`] for a rules file that cannot be read or does not hold, naming
/// the field, and for a harness without a reader; and [`CliError::Failed`] when the store or the
/// import fails.
#[allow(
    clippy::unused_async,
    reason = "every verb has the async entry `main` calls"
)]
pub async fn run(cli: &Cli, args: &ImportArgs) -> Result<Exit, CliError> {
    let (rules, harness) = load_rules(args.rules.as_path())?; // BORROW: explicit .as_path()
    check_reader(harness.as_str())?; // BORROW: explicit .as_str()
    let config = common::load_config("import")?;
    let resolver = common::plain_resolver(&config);
    let resolution = common::resolve("import", cli, &config, &resolver).await?;
    let root = common::store_root(&resolution)?;
    let store = Store::init(root.as_path()).map_err(|e| CliError::failed("import", e))?; // BORROW: explicit .as_path()
    let factory: Box<ReaderFactory> = Box::new(move || new_reader(harness.as_str())); // BORROW: explicit .as_str()
    let report = import(
        args.corpus.as_path(), // BORROW: explicit .as_path()
        &rules,
        factory.as_ref(),
        &store,
    )
    .map_err(|e| CliError::failed("import", e))?;
    println!(
        "{}",
        json!({"runs": report.runs, "records": report.outcomes})
    );
    Ok(Exit::Success)
}
