//! `candler ingest`: raw trace directories into the store, one run per stream, or, with `--rules`,
//! legacy results folders under rules the caller supplies as a TOML file.

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, PoisonError};

use candler_core::import::assemble::import;
use candler_core::import::digest_of;
use candler_core::import::raw::{RawIngestError, ingest_raw_with};
use candler_core::import::rules::{
    HarnessRule, ImportRules, MarkerCode, MarkerRule, SnapshotRule, TreatmentRule,
};
use candler_core::import::streams::ReaderFactory;
use candler_core::store::Store;
use candler_harness::reader::TraceReader;
use candler_schemas::primitives::{FieldPath, FormatError, Name, NameToken, ReasonCode};
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};

use super::refusal::CliRefusal;
use super::{CliError, Exit, common};
use crate::Cli;
use crate::drivers::Registry;

/// The arguments of `candler ingest`.
#[derive(Debug, clap::Args)]
pub struct IngestArgs {
    /// A raw trace directory, or a results folder when `--rules` is given.
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
    /// The TOML file that holds the import rules, which makes each path a results folder. cannot be
    /// derived: how a legacy folder maps to runs is the importer's input, written by whoever owns
    /// the results.
    #[arg(long)]
    pub rules: Option<PathBuf>,
}

/// The rules file: one field per import rule.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RulesFile {
    format: String,
    instrument: String,
    suite_kind: String,
    entry_params: Vec<String>,
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
    T: FromStr<Err = FormatError>,
{
    text.parse::<T>()
        .map_err(|e| CliError::refused_by(e).led_by(format!("rules `{field}`: ")))
}

/// A refusal of the rules file, whose text is `detail`.
fn invalid(detail: String) -> CliError {
    CliError::refused_by(CliRefusal::RulesInvalid { detail })
}

/// The import rules the file at `path` states; a coded refusal names the file.
fn load_rules(path: &Path) -> Result<(ImportRules, String), CliError> {
    rules_from(path).map_err(|error| error.with_file(path))
}

/// The steps of [`load_rules`].
fn rules_from(path: &Path) -> Result<(ImportRules, String), CliError> {
    let bytes = fs::read(path).map_err(|e| {
        invalid(format!(
            "cannot read the rules file {}: {e}",
            path.display()
        ))
    })?;
    // The digest is of the file's bytes, whatever they parse to: it goes into every import record.
    let digest = digest_of(bytes.as_slice()).map_err(|e| CliError::failed("import", e))?; // BORROW: explicit .as_slice()
    let text = String::from_utf8(bytes).map_err(|e| {
        invalid(format!(
            "the rules file {} is not UTF-8: {e}",
            path.display()
        ))
    })?;
    // BORROW: explicit .as_str()
    let file: RulesFile = toml::from_str(text.as_str()).map_err(|e| {
        CliError::refused_by(CliRefusal::RulesInvalid {
            detail: e.to_string(),
        })
        .led_by(String::from("rules file: "))
    })?;
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
    builder = builder.entry_params(
        file.entry_params
            .iter()
            .map(|item| primitive::<Name>("entry_params", item.as_str())) // BORROW: explicit .as_str()
            .collect::<Result<Vec<_>, _>>()?,
    );
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
            .map_err(|e| invalid(format!("rules `marker.pattern`: {e}")))?;
        let code = match (item.code.as_deref(), item.capture.as_deref()) {
            (Some(code), None) => MarkerCode::Fixed(primitive::<ReasonCode>("marker.code", code)?),
            (None, Some(group)) => MarkerCode::Capture(group.to_owned()), // BORROW: the rule owns the group name
            (Some(_), Some(_)) | (None, None) => {
                return Err(invalid(
                    "rules `marker`: exactly one of `code` and `capture` is required".to_owned(),
                ));
            }
        };
        builder = builder.marker(MarkerRule::new(pattern, code));
    }
    let rules = builder
        .build()
        .map_err(|e| CliError::refused_by(e).led_by(String::from("rules: ")))?;
    Ok((rules, file.harness.name))
}

/// Checks that the reader of the harness `name` can be built, so that the factory cannot fail.
/// The table holds the readers this binary ships.
fn check_reader(name: &str) -> Result<(), CliError> {
    let registry = Registry::builtin().map_err(|e| CliError::failed("import", e))?;
    let entry = registry
        .entry(name)
        .map_err(|_| invalid(format!("rules `harness.name`: no reader for `{name}`")))?;
    (entry.reader)()
        .map(drop)
        .map_err(|e| CliError::failed("import", e))
}

/// A new reader of the harness `name`, which [`check_reader`] accepted.
// TRAIT_OBJECT: the import's factory type returns boxed readers
fn new_reader(name: &str) -> Box<dyn TraceReader> {
    let built = Registry::builtin()
        .ok()
        .and_then(|registry| registry.entry(name).ok())
        .and_then(|entry| (entry.reader)().ok());
    if let Some(reader) = built {
        reader
    } else {
        // EXPLICIT: construction succeeded once in `check_reader` and its inputs are constants
        eprintln!("candler ingest: cannot build the reader for `{name}`");
        std::process::exit(2)
    }
}

/// Imports each results folder of `args.paths` into the store under the rules of `rules` and
/// prints `{"runs": n, "records": n}` for each.
async fn run_folders(cli: &Cli, args: &IngestArgs, rules: &Path) -> Result<Exit, CliError> {
    let (loaded, harness) = load_rules(rules)?;
    check_reader(harness.as_str()).map_err(|e| e.with_file(rules))?; // BORROW: explicit .as_str()
    let rules = loaded;
    let root = common::store_root("import", cli.store.as_deref()).await?;
    let store = Store::init(root.as_path()).map_err(|e| CliError::failed("import", e))?; // BORROW: explicit .as_path()
    let factory: Box<ReaderFactory> = Box::new(move || new_reader(harness.as_str())); // BORROW: explicit .as_str()
    for folder in &args.paths {
        let report = import(
            folder.as_path(), // BORROW: explicit .as_path()
            &rules,
            factory.as_ref(),
            &store,
        )
        .map_err(|e| CliError::failed("import", e))?;
        println!(
            "{}",
            json!({"runs": report.runs, "records": report.outcomes})
        );
    }
    Ok(Exit::Success)
}

/// Ingests each raw trace directory of `args.paths` and prints one line summed over them.
async fn run_traces(cli: &Cli, args: &IngestArgs) -> Result<Exit, CliError> {
    let registry = Arc::new(Registry::builtin().map_err(|e| CliError::failed("ingest", e))?);
    let names = registry.names();
    for name in &names {
        check_reader(name.as_str())?; // BORROW: explicit .as_str()
    }
    let root = common::store_root("ingest", cli.store.as_deref()).await?;
    let store = Store::init(root.as_path()).map_err(|e| CliError::failed("ingest", e))?; // BORROW: explicit .as_path()
    let (mut created, mut existing) = (0_u64, 0_u64);
    let mut skipped: Vec<Value> = Vec::new();
    for path in &args.paths {
        // The streams of one path share a harness: the readers are those of the first stream the
        // registry recognised in this path, which the ingest has seen before it builds any
        // reader. Another path starts from nothing.
        let recognised: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&recognised); // BORROW: the factory shares the list the recogniser fills
        let fallback = names.first().cloned().unwrap_or_default(); // BORROW: the factory owns the name
        let factory: Box<ReaderFactory> = Box::new(move || {
            let first = seen
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .first()
                .cloned(); // BORROW: the factory owns the name
            new_reader(first.unwrap_or_else(|| fallback.clone()).as_str()) // BORROW: .clone() copies the fallback name
        });
        let recognise = {
            let registry = Arc::clone(&registry); // BORROW: the recogniser owns a handle on the registry
            move |sample: &[u8]| {
                let found = registry.recognise(sample)?;
                recognised
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(found.harness.to_string());
                Some(found)
            }
        };
        let offline = {
            let registry = Arc::clone(&registry); // BORROW: the callback owns a handle on the registry
            move |harness: &str| registry.offline(harness)
        };
        let report = ingest_raw_with(
            &store,
            path.as_path(), // BORROW: explicit .as_path()
            &recognise,
            factory.as_ref(),
            &offline,
        )
        .map_err(|error| {
            if matches!(
                error,
                RawIngestError::NoStream { .. }
                    | RawIngestError::Unreadable { .. }
                    | RawIngestError::RulesRequired { .. }
            ) {
                CliError::refused_by(error).with_file(path.as_path()) // BORROW: explicit .as_path()
            } else {
                CliError::failed("ingest", error)
            }
        })?;
        created = created.saturating_add(report.created);
        existing = existing.saturating_add(report.existing);
        skipped.extend(
            report
                .skipped
                .iter()
                .map(|entry| json!({"path": entry.path, "reason": entry.reason.as_str()})),
        );
    }
    println!(
        "{}",
        json!({
            "runs": created.saturating_add(existing),
            "created": created,
            "existing": existing,
            "skipped": skipped,
        })
    );
    Ok(Exit::Success)
}

/// Ingests the paths of `args` into the store and prints what it did.
///
/// Without `--rules` each path is a raw trace directory, and the verb prints
/// `{"runs": n, "created": c, "existing": e, "skipped": [...]}` summed over the paths. With
/// `--rules` each path is a legacy results folder and the verb prints `{"runs": n, "records": n}`
/// once per path.
///
/// # Errors
/// Returns [`CliError::Refused`] for a rules file that cannot be read or does not hold, naming
/// the field, and for a harness without a reader; a coded refusal for a path that holds no stream
/// or cannot be read; and [`CliError::Failed`] when the store or the ingest fails.
pub async fn run(cli: &Cli, args: &IngestArgs) -> Result<Exit, CliError> {
    match args.rules.as_deref() {
        Some(rules) => run_folders(cli, args, rules).await,
        None => run_traces(cli, args).await,
    }
}
