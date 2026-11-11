//! `candler analyze`: per-arm tables of one evaluator's outcomes and the tests between arms that
//! differ in one declared factor, as JSON or CSV. It reads the store and never writes it.

use std::collections::BTreeSet;
use std::path::PathBuf;

use candler_core::analysis::refusal::ReportError;
use candler_core::analysis::report::{
    ArmReport, Comparison, Entry, Report, ReportInput, RunRow, RunView, choose_evaluator, role_name,
};
use candler_core::analysis::tally::{Rate, RunFacts};
use candler_core::analysis::{ALPHA, alpha_source};
use candler_schemas::execution::ExperimentFile;
use candler_schemas::primitives::FieldPath;
use serde_json::{Value, json};

use super::{CliError, Exit, common};
use crate::Cli;

/// The verb, as errors and the store reader name it.
const VERB: &str = "analyze";

/// The arguments of `candler analyze`.
#[derive(Debug, clap::Args)]
pub struct AnalyzeArgs {
    /// The evaluator whose outcomes are analysed, as `name@version`, or a bare version when one
    /// evaluator holds it; by default the one evaluator the store holds, and a store with several
    /// is refused. not an experiment setting: names which stored outcomes to analyse.
    #[arg(long)]
    pub evaluator: Option<String>,
    /// The experiment file whose factors declare which differences between arms are the
    /// experiment's own; without it, arms are tabled and no pair is compared. cannot be derived:
    /// the declared factors are in the experiment file, and a manifest cannot vouch for its own.
    #[arg(long)]
    pub experiment: Option<PathBuf>,
    /// How the tables are written. not an experiment setting: chooses how the tables are written.
    #[arg(long, value_enum, default_value_t)]
    pub format: Format,
    /// The table `--format csv` writes; read only with `--format csv`. not an experiment setting:
    /// chooses which table is written.
    #[arg(long, value_enum, default_value_t)]
    pub table: Table,
}

/// The name `main` dispatches on.
pub type Args = AnalyzeArgs;

/// The output formats of `candler analyze`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// One JSON document.
    #[default]
    Json,
    /// One CSV table, chosen with `--table`.
    Csv,
}

/// The tables of `--format csv`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Table {
    /// One row per cell and expectation.
    #[default]
    Arms,
    /// One row per compared pair and expectation.
    Comparisons,
    /// One row per run, with a column for every recorded path.
    Runs,
}

/// Prints the report of one evaluator.
///
/// # Errors
/// Returns [`CliError::Refused`] for the refusals of [`ReportError`] and [`CliError::Failed`]
/// when the store cannot be read or the report cannot be computed.
pub async fn run(cli: &Cli, args: &AnalyzeArgs) -> Result<Exit, CliError> {
    let store = common::open_store(VERB, cli).await?;
    let selection = common::read_selection(VERB, &store)?;
    let evaluator =
        choose_evaluator(&selection, args.evaluator.as_deref()).map_err(|e| refused(&e))?;
    let declared = args
        .experiment
        .as_deref()
        .map(declared_factors)
        .transpose()?;
    let mut runs = Vec::new();
    for entry in store.runs().map_err(|e| CliError::failed(VERB, e))? {
        if !entry.done {
            continue;
        }
        let manifest = store
            .read_manifest(&entry.key)
            .map_err(|e| CliError::failed(VERB, e))?;
        runs.push(RunView {
            facts: RunFacts::new(entry.key, manifest.integrity().clone()), // BORROW: the view owns its integrity
            manifest,
        });
    }
    let report = Report::build(ReportInput {
        selection,
        evaluator,
        runs,
        declared,
    })
    .map_err(|e| CliError::failed(VERB, e))?;
    let text = match args.format {
        Format::Json => render_json(&report)?,
        Format::Csv => render_csv(&report, args.table)?,
    };
    print!("{text}");
    Ok(Exit::Success)
}

/// A refusal of the report as the command line states it.
fn refused(error: &ReportError) -> CliError {
    CliError::refused(error.to_string())
}

/// The paths of the factors of the experiment file.
fn declared_factors(file: &std::path::Path) -> Result<Vec<FieldPath>, CliError> {
    let unreadable = |detail: String| {
        refused(&ReportError::ExperimentUnreadable {
            file: file.display().to_string(),
            detail,
        })
    };
    let text = std::fs::read_to_string(file).map_err(|e| unreadable(e.to_string()))?;
    let experiment =
        ExperimentFile::from_toml_str(text.as_str()).map_err(|e| unreadable(e.to_string()))?;
    Ok(experiment
        .factors()
        .iter()
        .map(|factor| factor.path.clone()) // BORROW: the list owns its paths
        .collect())
}

/// A rate as `{num, den}`.
fn rate_json(rate: Rate) -> Value {
    json!({"num": rate.num(), "den": rate.den()})
}

fn map_json(map: &std::collections::BTreeMap<String, u64>) -> Value {
    Value::Object(
        map.iter()
            .map(|(key, count)| (key.clone(), Value::from(*count))) // BORROW: the document owns its keys
            .collect(),
    )
}

fn arm_json(arm: &ArmReport) -> Value {
    let expectations: Vec<Value> = arm
        .expectations
        .iter()
        .map(|held| {
            let tally = &held.tally;
            let (wilson, omitted) = match &held.wilson {
                Ok(interval) => (
                    json!({"lo": interval.lo(), "hi": interval.hi()}),
                    Value::Null,
                ),
                Err(reason) => (Value::Null, Value::from(reason.as_str())),
            };
            json!({
                "id": held.id.to_string(),
                "role": role_name(tally.role),
                "pass": tally.pass,
                "fail": tally.fail,
                "blocked": tally.blocked,
                "not_measured": map_json(&tally.not_measured),
                "not_applicable": map_json(&tally.not_applicable),
                "zero_weight": tally.zero_weight,
                "weight": {"num": tally.num, "den": tally.den},
                "wilson": wilson,
                "omitted": omitted,
            })
        })
        .collect();
    json!({
        "cell": arm.cell.to_string(),
        "runs": {
            "valid": arm.tally.valid_runs,
            "invalid": map_json(&arm.tally.invalid),
            "experimenter_ends": map_json(&arm.tally.experimenter_ends),
        },
        "rates": {"gate": rate_json(arm.gate), "quality": rate_json(arm.quality)},
        "status": {
            "pass": arm.status.pass,
            "fail": arm.status.fail,
            "blocked": arm.status.blocked,
            "not_measured": arm.status.not_measured,
            "not_applicable": arm.status.not_applicable,
        },
        "expectations": expectations,
    })
}

fn entry_json(entry: &Entry) -> Value {
    match entry {
        Entry::Tested(tested) => json!({
            "id": tested.id.to_string(),
            "role": role_name(tested.role),
            "a": {"k": tested.a.k(), "n": tested.a.n()},
            "b": {"k": tested.b.k(), "n": tested.b.n()},
            "p": tested.p,
            "p_holm": tested.p_holm,
            "reject": tested.reject,
            "diff": tested.diff,
            "diff_lo": tested.diff_lo,
            "diff_hi": tested.diff_hi,
        }),
        Entry::Omitted { id, role, reason } => json!({
            "id": id.to_string(),
            "role": role_name(*role),
            "omitted": reason.as_str(),
        }),
        _ => json!({"id": entry.id().to_string(), "role": role_name(entry.role())}),
    }
}

fn comparison_json(comparison: &Comparison) -> Value {
    json!({
        "a": comparison.a.to_string(),
        "b": comparison.b.to_string(),
        "factor": comparison.factor.to_string(),
        "stratum": {
            "verdict": comparison.verdict.as_str(),
            "causes": comparison.verdict.causes(),
        },
        "entries": comparison.entries.iter().map(entry_json).collect::<Vec<Value>>(),
    })
}

fn run_json(row: &RunRow) -> Result<Value, CliError> {
    let integrity = serde_json::to_value(&row.integrity).map_err(|e| CliError::failed(VERB, e))?;
    Ok(json!({
        "run": row.run.to_string(),
        "cell": row.cell.to_string(),
        "integrity": integrity,
        "comparable": row.comparable,
        "measured": row.measured,
    }))
}

/// The significance level with the place it comes from.
fn alpha_json() -> Result<Value, CliError> {
    let source = serde_json::to_value(alpha_source()).map_err(|e| CliError::failed(VERB, e))?;
    let kind = source
        .get("kind")
        .cloned()
        .ok_or_else(|| CliError::Runtime {
            detail: "the source of alpha has no kind".to_owned(),
        })?;
    Ok(json!({"value": ALPHA, "source": kind}))
}

/// The report as one JSON document and a final newline.
///
/// # Errors
/// Returns [`CliError::Failed`] when a stored value cannot be written as JSON.
pub fn render_json(report: &Report) -> Result<String, CliError> {
    let comparisons = report.comparisons.as_ref().map_or_else(
        || json!({"reason": "no_experiment"}),
        |pairs| Value::Array(pairs.iter().map(comparison_json).collect()),
    );
    let runs = report
        .runs
        .iter()
        .map(run_json)
        .collect::<Result<Vec<Value>, CliError>>()?;
    let document = json!({
        "evaluator": report.evaluator.to_string(),
        "alpha": alpha_json()?,
        "i4_violations": report.i4_violations,
        "arms": report.arms.iter().map(arm_json).collect::<Vec<Value>>(),
        "comparisons": comparisons,
        "families": report
            .families
            .iter()
            .map(|family| json!({"role": role_name(family.role), "m": family.m}))
            .collect::<Vec<Value>>(),
        "pairs_not_one_factor": report.pairs_not_one_factor,
        "runs": runs,
    });
    let mut text = serde_json::to_string(&document).map_err(|e| CliError::failed(VERB, e))?;
    text.push('\n');
    Ok(text)
}

/// One CSV field: quoted when it holds a comma, a quote or a line break (RFC 4180).
fn field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned() // BORROW: the row owns its text
    }
}

fn row(fields: &[String]) -> String {
    let quoted: Vec<String> = fields.iter().map(|text| field(text.as_str())).collect();
    format!("{}\r\n", quoted.join(","))
}

fn json_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(), // BORROW: the row owns its text
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
            value.to_string()
        }
    }
}

fn arms_csv(report: &Report) -> String {
    let mut text = row(&strings(&[
        "cell",
        "role",
        "expectation",
        "pass",
        "fail",
        "blocked",
        "not_measured",
        "not_applicable",
        "zero_weight",
        "num",
        "den",
        "wilson_lo",
        "wilson_hi",
        "omitted",
    ]));
    for arm in &report.arms {
        for held in &arm.expectations {
            let tally = &held.tally;
            let (lo, hi, omitted) = match &held.wilson {
                Ok(interval) => (
                    interval.lo().to_string(),
                    interval.hi().to_string(),
                    String::new(),
                ),
                Err(reason) => (String::new(), String::new(), reason.as_str().to_owned()),
            };
            text.push_str(&row(&[
                arm.cell.to_string(),
                role_name(tally.role).to_owned(),
                held.id.to_string(),
                tally.pass.to_string(),
                tally.fail.to_string(),
                tally.blocked.to_string(),
                tally.not_measured.values().sum::<u64>().to_string(),
                tally.not_applicable.values().sum::<u64>().to_string(),
                tally.zero_weight.to_string(),
                tally.num.to_string(),
                tally.den.to_string(),
                lo,
                hi,
                omitted,
            ]));
        }
    }
    text
}

fn comparisons_csv(report: &Report) -> String {
    let mut text = row(&strings(&[
        "a",
        "b",
        "factor",
        "verdict",
        "expectation",
        "role",
        "a_k",
        "a_n",
        "b_k",
        "b_n",
        "p",
        "p_holm",
        "reject",
        "diff",
        "diff_lo",
        "diff_hi",
        "omitted",
    ]));
    for comparison in report.comparisons.iter().flatten() {
        for entry in &comparison.entries {
            let head = [
                comparison.a.to_string(),
                comparison.b.to_string(),
                comparison.factor.to_string(),
                comparison.verdict.as_str().to_owned(),
                entry.id().to_string(),
                role_name(entry.role()).to_owned(),
            ];
            let tail: Vec<String> = match entry {
                Entry::Tested(tested) => vec![
                    tested.a.k().to_string(),
                    tested.a.n().to_string(),
                    tested.b.k().to_string(),
                    tested.b.n().to_string(),
                    tested.p.to_string(),
                    tested.p_holm.to_string(),
                    tested.reject.to_string(),
                    tested.diff.to_string(),
                    tested.diff_lo.to_string(),
                    tested.diff_hi.to_string(),
                    String::new(),
                ],
                Entry::Omitted { reason, .. } => {
                    let mut empty = vec![String::new(); 10];
                    empty.push(reason.as_str().to_owned());
                    empty
                }
                _ => vec![String::new(); 11],
            };
            let fields: Vec<String> = head.into_iter().chain(tail).collect();
            text.push_str(&row(fields.as_slice()));
        }
    }
    text
}

fn runs_csv(report: &Report) -> Result<String, CliError> {
    let comparable: BTreeSet<&String> = report
        .runs
        .iter()
        .flat_map(|run| run.comparable.keys())
        .collect();
    let measured: BTreeSet<&String> = report
        .runs
        .iter()
        .flat_map(|run| run.measured.keys())
        .collect();
    let mut header = strings(&["run", "cell", "integrity"]);
    header.extend(comparable.iter().map(|path| format!("comparable.{path}")));
    header.extend(measured.iter().map(|path| format!("measured.{path}")));
    let mut text = row(header.as_slice());
    for held in &report.runs {
        let integrity =
            serde_json::to_value(&held.integrity).map_err(|e| CliError::failed(VERB, e))?;
        let mut fields = vec![
            held.run.to_string(),
            held.cell.to_string(),
            json_text(&integrity),
        ];
        fields.extend(comparable.iter().map(|path| {
            held.comparable
                .get(*path)
                .map(json_text)
                .unwrap_or_default()
        }));
        fields.extend(
            measured
                .iter()
                .map(|path| held.measured.get(*path).map(json_text).unwrap_or_default()),
        );
        text.push_str(&row(fields.as_slice()));
    }
    Ok(text)
}

fn strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect() // BORROW: the row owns its text
}

/// One table of the report as CSV: the header first, then a row per line, each ended `\r\n`.
///
/// # Errors
/// Returns [`CliError::Failed`] when a stored value cannot be written as JSON.
pub fn render_csv(report: &Report, table: Table) -> Result<String, CliError> {
    match table {
        Table::Arms => Ok(arms_csv(report)),
        Table::Comparisons => Ok(comparisons_csv(report)),
        Table::Runs => runs_csv(report),
    }
}
