//! `candler resolve`: resolves an experiment and prints every value with its source.
//!
//! The verb runs the foreground preflight `run` runs, with the secrets step and the gateway step
//! added, and then stops. It never calls `ensure_sidecar`, `plan_job`, `JobDir::create` or
//! `detach`, so no job and no agent starts and the store gets no job directory. Every failure is
//! collected before the exit: the failed checks and errors of the preflight, and the entries of
//! `plugins.dirs` that are not directories. Each failure is a line with its check id, which is
//! `<step>.<check>` for a check, `<step>.error` for a step that returned an error, and
//! `plugins.dir_missing` for a directory. The ids are check ids, not the codes of
//! `docs/contract/codes.toml`.
//!
//! The values are those of the experiment before its agent inputs are stored: `resolve` writes
//! nothing to the store, so the setting `request.inputs` that `run` adds from the experiment's
//! declared inputs is not part of what it prints.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use candler_core::jobs::orchestrate::session::EnvSecrets;
use candler_core::resolve::config::{Config, ConfigEnv};
use candler_core::resolve::layers::Resolution;
use candler_core::resolve::preflight::{ResolveError, StepError, StepReport};
use candler_core::resolve::steps::gateway::register_gateway_step;
use candler_core::resolve::steps::secrets::register_secret_step;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::{EventPayload, ExperimentFile};
use candler_schemas::primitives::{FieldPath, RunKey};
use candler_schemas::values::{SourceEntry, ValueSource};

use super::preflight::{check_factors, foreground, request_settings};
use super::refusal::CliRefusal;
use super::term::Paint;
use super::{CliError, Exit, common};

/// The arguments of `candler resolve`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The experiment file.
    pub experiment: PathBuf,
    /// A value that replaces the file's: `PATH=VALUE`, the value read as JSON when it is JSON
    /// and as text otherwise. Repeatable. cannot be derived: an override is what the caller types.
    #[arg(long = "set", value_name = "PATH=VALUE", value_parser = parse_field)]
    pub fields: Vec<(FieldPath, String)>,
    /// Print the resolution as one JSON document. not an experiment setting: it is how the output
    /// is read.
    #[arg(long)]
    pub json: bool,
}

/// Reads `PATH=VALUE` from the command line.
fn parse_field(text: &str) -> Result<(FieldPath, String), String> {
    let Some((path, value)) = text.split_once('=') else {
        return Err("expected PATH=VALUE".to_owned()); // BORROW: the parser owns its message
    };
    let path = FieldPath::from_str(path).map_err(|e| e.to_string())?;
    Ok((path, value.to_owned())) // BORROW: the argument owns its value
}

/// An event sink that drops what it is given: a resolution belongs to no job.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// One failure of the verb: the check id and what was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// `<step>.<check>`, `<step>.error` or `plugins.dir_missing`.
    pub id: String,
    /// What was checked and what was found.
    pub explanation: String,
}

/// The text a source is printed as: its kind, and for a derived value its step and the location
/// it read; for an imported value the file.
#[must_use]
pub fn render_source(source: &ValueSource) -> String {
    match source {
        ValueSource::ExplicitArgument => "explicit_argument".to_owned(), // BORROW: the line owns its text
        ValueSource::ExperimentFile => "experiment_file".to_owned(), // BORROW: the line owns its text
        ValueSource::ProjectConfig => "project_config".to_owned(), // BORROW: the line owns its text
        ValueSource::UserConfig => "user_config".to_owned(),       // BORROW: the line owns its text
        ValueSource::Derived { step, from, .. } => match from {
            Some(from) => format!("derived {step} from {from}"),
            None => format!("derived {step}"),
        },
        ValueSource::CoreDefault => "core_default".to_owned(), // BORROW: the line owns its text
        ValueSource::Imported { file, .. } => format!("imported {file}"),
        ValueSource::Unrecorded => "unrecorded".to_owned(), // BORROW: the line owns its text
        _ => "unknown".to_owned(),                          // BORROW: the line owns its text
    }
}

/// The text of each entry's source, in the order of `entries`.
#[must_use]
pub fn render_sources(entries: &[SourceEntry]) -> Vec<String> {
    entries
        .iter()
        .map(|entry| render_source(entry.source()))
        .collect()
}

/// The failures of a resolution error: every failed check and every step error, not the five
/// that the error's own text names.
#[must_use]
pub fn render_failures(reports: &[StepReport], errors: &[StepError]) -> Vec<Failure> {
    let mut found = Vec::new();
    for report in reports {
        for check in report.checks.iter().filter(|check| !check.passed) {
            found.push(Failure {
                id: format!("{}.{}", report.step.name(), check.name.name()),
                explanation: check.explanation.clone(), // BORROW: the failure owns its text
            });
        }
    }
    for error in errors {
        found.push(Failure {
            id: format!("{}.error", error.step.name()),
            explanation: error.error.to_string(),
        });
    }
    found
}

/// The entries of `plugins.dirs` that are not directories, each as a failure.
#[must_use]
pub fn plugin_dir_failures(config: &Config) -> Vec<Failure> {
    let Some(project) = config.project.as_ref() else {
        return Vec::new();
    };
    project
        .plugin_dirs
        .iter()
        .filter(|dir| !dir.is_dir())
        .map(|dir| Failure {
            id: "plugins.dir_missing".to_owned(), // BORROW: the failure owns its text
            explanation: format!(
                "`plugins.dirs` names {}, which is not a directory",
                dir.display()
            ),
        })
        .collect()
}

/// The lines of the text form: `<path>\t<value as JSON>\t<source>`, one per value and per setting,
/// sorted by path.
#[must_use]
pub fn render_values(resolution: &Resolution, paint: Paint) -> Vec<String> {
    let values: BTreeMap<&FieldPath, String> = resolution
        .values
        .iter()
        .chain(resolution.settings.iter())
        .map(|resolved| {
            let text = resolved
                .value()
                .map_or_else(|| "-".to_owned(), |value| value.as_value().to_string()); // BORROW: the line owns its text
            (resolved.path(), text)
        })
        .collect();
    let texts = render_sources(&resolution.sources);
    resolution
        .sources
        .iter()
        .zip(texts)
        .map(|(entry, source)| {
            let value = values.get(entry.path()).map_or("-", |text| text.as_str()); // BORROW: explicit .as_str()
            format!(
                "{}\t{value}\t{}",
                paint.bold(entry.path().as_str()), // BORROW: explicit .as_str()
                paint.dim(&source)
            )
        })
        .collect()
}

/// The experiment file at `path`.
fn read_experiment(path: &Path) -> Result<ExperimentFile, CliError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        CliError::refused_by(CliRefusal::ExperimentUnreadable {
            path: path.display().to_string(),
            detail: e.to_string(),
        })
        .with_file(path)
    })?;
    ExperimentFile::from_toml_str(text.as_str()) // BORROW: .as_str() views the text as a `&str`
        .map_err(|e| {
            CliError::refused_by(e)
                .led_by(format!("{} is not an experiment: ", path.display()))
                .with_file(path)
        })
}

fn failed_here(error: impl std::error::Error + Send + Sync + 'static) -> CliError {
    CliError::failed("resolve", error)
}

/// Prints the failures, one line each.
fn print_failures(
    failures: &[Failure],
    json: bool,
    out: &mut dyn std::io::Write, // TRAIT_OBJECT: stdout, or a buffer
) -> Result<(), CliError> {
    let paint = Paint::detect();
    let written = if json {
        let list: Vec<serde_json::Value> = failures
            .iter()
            .map(|f| serde_json::json!({"id": f.id, "explanation": f.explanation}))
            .collect();
        writeln!(out, "{}", serde_json::json!({ "failures": list }))
    } else {
        failures
            .iter()
            .try_for_each(|f| writeln!(out, "{}\t{}\t{}", paint.red("failed"), f.id, f.explanation))
    };
    written.map_err(failed_here)
}

/// Resolves the experiment and prints what its settings come to.
///
/// # Errors
/// [`CliError::Refused`] or [`CliError::Coded`] when the file or a `--set` is refused, and the
/// failure of the preflight (the exit is 2) when a check fails or `plugins.dirs` names something
/// that is not a directory; every failure is printed before the exit.
pub async fn run(args: Args, store: Option<&Path>) -> Result<Exit, CliError> {
    let Args {
        experiment,
        fields,
        json,
    } = args;
    let cwd = std::env::current_dir()
        .map_err(|e| CliError::refused(format!("no working directory: {e}")))?;
    let file = read_experiment(&experiment)?;
    check_factors(&file)?;
    let experiment_dir = cwd
        .join(&experiment)
        .parent()
        .map_or_else(|| cwd.clone(), Path::to_path_buf); // BORROW: the directory is owned
    let root = common::store_root("resolve", store).await?;
    let config = Arc::new(Config::load(&cwd, &ConfigEnv::from_process()).map_err(failed_here)?);
    let input = request_settings(&file, experiment_dir.as_path(), &fields, &cwd)?;
    let services = foreground(Arc::clone(&config), &root)?;
    let supervisor = {
        // TRAIT_OBJECT: the check belongs to no job and reports no event
        let sink: Arc<dyn EventSink> = Arc::new(Silent);
        Arc::new(Supervisor::new(sink))
    };
    let present = EnvSecrets::new(file.secrets()).present();
    let resolver = register_secret_step(services.resolver, file.secrets().to_vec(), present)
        .and_then(|resolver| register_gateway_step(resolver, supervisor))
        .map_err(failed_here)?;

    let mut failures = plugin_dir_failures(&config);
    let mut resolution = None;
    let mut summary = None;
    match resolver.resolve(input).await {
        Ok(found) => resolution = Some(found),
        Err(ResolveError::Preflight { reports, errors }) => {
            failures.extend(render_failures(&reports, &errors));
            summary = Some(ResolveError::Preflight { reports, errors });
        }
        Err(other) => return Err(CliError::refused_by(other)),
    }

    let mut stdout = std::io::stdout().lock();
    if !failures.is_empty() {
        print_failures(&failures, json, &mut stdout)?;
        return Err(match summary {
            Some(error) => CliError::refused_by(error),
            None => CliError::refused(format!(
                "{} check(s) failed; see the lines above",
                failures.len()
            )),
        });
    }
    let Some(resolution) = resolution else {
        return Ok(Exit::Success);
    };
    if json {
        let text = serde_json::to_string_pretty(&resolution).map_err(failed_here)?;
        writeln!(stdout, "{text}").map_err(failed_here)?;
    } else {
        for line in render_values(&resolution, Paint::detect()) {
            writeln!(stdout, "{line}").map_err(failed_here)?;
        }
    }
    Ok(Exit::Success)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use candler_schemas::primitives::{FieldPath, NameToken, RelPath};
    use candler_schemas::values::{SourceEntry, ValueSource};

    use super::render_sources;

    /// Guards: O1 — the source printer gives each of the eight kinds a text of its own, and a
    /// derived source names its step and, when it has one, the location it read. A live
    /// resolution reaches six of the eight; `imported` and `unrecorded` come only from imported
    /// runs, so this row alone reaches them. Oracle: hand-built entries of the eight kinds.
    /// Bite: a kind that shares another's text, or a derived source that drops its step or its
    /// `from`.
    #[test]
    fn o1_the_source_printer_names_all_eight_kinds() {
        let path: FieldPath = "model.id".parse().expect("a path");
        let step: NameToken = "image_id@1".parse().expect("a step");
        let file: RelPath = "runs/a/manifest.json".parse().expect("a file");
        let sources = [
            ValueSource::ExplicitArgument,
            ValueSource::ExperimentFile,
            ValueSource::ProjectConfig,
            ValueSource::UserConfig,
            ValueSource::derived(step.clone()), // BORROW: the next entry needs the step too
            ValueSource::derived_from(step, "treatment:lock".to_owned()), // BORROW: the entry owns its text
            ValueSource::CoreDefault,
            ValueSource::imported(file),
            ValueSource::Unrecorded,
        ];
        let entries: Vec<SourceEntry> = sources
            .into_iter()
            .map(|source| SourceEntry::new(path.clone(), source)) // BORROW: each entry owns its path
            .collect();
        let texts = render_sources(&entries);
        let distinct: BTreeSet<&String> = texts.iter().collect();
        assert_eq!(distinct.len(), 9, "nine distinct texts: {texts:?}");
        let derived = texts.get(4).expect("the derived text");
        assert!(derived.contains("image_id@1"), "{derived}");
        let from = texts.get(5).expect("the derived-from text");
        assert!(
            from.contains("image_id@1") && from.contains("treatment:lock"),
            "{from}"
        );
        assert!(!derived.contains("treatment:lock"), "{derived}");
    }
}
