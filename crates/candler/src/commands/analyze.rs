//! `candler analyze`: per-arm tables of one evaluator's outcomes and the tests between arms that
//! differ in one declared factor, as the `analysis-report/1` record, or as text or CSV rendered
//! from it. It reads the store and never writes it.

use std::collections::BTreeSet;
use std::path::PathBuf;

use candler_core::analysis::record::analysis_report;
use candler_core::analysis::refusal::ReportError;
use candler_core::analysis::report::{
    Report, ReportInput, RunView, Unsealed, check_factors, choose_evaluator,
};
use candler_core::analysis::tally::RunFacts;
use candler_schemas::analysis::{AnalysisReport, Entry, Reading};
use candler_schemas::execution::{ExperimentFile, varying_paths};
use candler_schemas::primitives::FieldPath;
use serde_json::Value;

use super::{CliError, Exit, common, write_record};
use crate::Cli;

/// How many characters of an experiment digest a line of text shows.
const SHORT: usize = 12;

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
    /// An experiment file, checked against the factors the run manifests record: the call is
    /// refused when they differ, and without the file the manifests' factors declare which
    /// differences between arms are the experiment's own. not an experiment setting: names the
    /// file the recorded factors are checked against.
    #[arg(long)]
    pub experiment: Option<PathBuf>,
    /// Print the report as text, or one table of it as CSV, instead of its JSON record; a refusal
    /// is written as text with `--format text`. not an experiment setting: chooses how the report
    /// is written.
    #[arg(long, value_enum)]
    pub format: Option<Format>,
    /// The table `--format csv` writes; read only with `--format csv`. not an experiment setting:
    /// chooses which table is written.
    #[arg(long, value_enum, default_value_t)]
    pub table: Table,
}

/// The name `main` dispatches on.
pub type Args = AnalyzeArgs;

/// The renderings of `candler analyze` in place of its JSON record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// The arms, the decisions and the entries left out, as lines for a person.
    Text,
    /// One CSV table, chosen with `--table`.
    Csv,
}

/// The tables of `--format csv`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Table {
    /// One row per cell and expectation.
    #[default]
    Arms,
    /// One row per compared pair and expectation, and one row for a pair with no expectation to
    /// show.
    Comparisons,
    /// What belongs to no other row: the evaluator, the I4 violations, the pairs not compared and
    /// the Holm families.
    Summary,
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
    let evaluator = choose_evaluator(&selection, args.evaluator.as_deref()).map_err(refused)?;
    let file_factors = args
        .experiment
        .as_deref()
        .map(experiment_factors)
        .transpose()?;
    let mut runs = Vec::new();
    let mut unsealed = Vec::new();
    for entry in store.runs().map_err(|e| CliError::failed(VERB, e))? {
        if !entry.done {
            // An unsealed run is counted apart; its experiment is known when its manifest is.
            unsealed.push(Unsealed {
                experiment: store
                    .read_manifest(&entry.key)
                    .ok()
                    .map(|manifest| manifest.experiment().clone()), // BORROW: the record owns the digest
                run: entry.key,
            });
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
    check_factors(runs.as_slice(), file_factors.as_deref()).map_err(refused)?;
    let report = Report::build(ReportInput {
        selection,
        evaluator,
        runs,
        unsealed,
    })
    .map_err(|e| CliError::failed(VERB, e))?;
    let record = analysis_report(&report).map_err(|e| CliError::failed(VERB, e))?;
    match args.format {
        None => write_record(VERB, &mut std::io::stdout().lock(), &record)?,
        Some(Format::Text) => print!("{}", render_text(&record)),
        Some(Format::Csv) => print!("{}", render_csv(&record, args.table)?),
    }
    Ok(Exit::Success)
}

/// A refusal of the report as the command line states it.
fn refused(error: ReportError) -> CliError {
    CliError::refused_by(error)
}

/// The paths that vary across the cells of the experiment file: the factors a manifest records.
fn experiment_factors(file: &std::path::Path) -> Result<Vec<FieldPath>, CliError> {
    let unreadable = |detail: String| {
        refused(ReportError::ExperimentUnreadable {
            file: file.display().to_string(),
            detail,
        })
        .with_file(file)
    };
    let text = std::fs::read_to_string(file).map_err(|e| unreadable(e.to_string()))?;
    let experiment =
        ExperimentFile::from_toml_str(text.as_str()).map_err(|e| unreadable(e.to_string()))?;
    Ok(varying_paths(experiment.cells().as_slice()))
}

/// A number as a CSV field, empty when there is none.
fn number(value: Option<f64>) -> String {
    value.map_or_else(String::new, |held| held.to_string())
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

fn arms_csv(report: &AnalysisReport) -> String {
    let mut text = row(&strings(&[
        "cell",
        "experiment",
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
            let (lo, hi) = held.wilson.as_ref().map_or_else(
                || (String::new(), String::new()),
                |interval| (interval.lo.to_string(), interval.hi.to_string()),
            );
            text.push_str(&row(&[
                arm.cell.clone(),       // BORROW: the row owns its text
                arm.experiment.clone(), // BORROW: the row owns its text
                held.role.clone(),      // BORROW: the row owns its text
                held.id.clone(),        // BORROW: the row owns its text
                held.pass.to_string(),
                held.fail.to_string(),
                held.blocked.to_string(),
                held.not_measured.values().sum::<u64>().to_string(),
                held.not_applicable.values().sum::<u64>().to_string(),
                held.zero_weight.to_string(),
                held.weight.num.to_string(),
                held.weight.den.to_string(),
                lo,
                hi,
                held.omitted.clone().unwrap_or_default(),
            ]));
        }
    }
    text
}

/// The name of an interval's reading as the record writes it.
fn reading_name(reading: Reading) -> String {
    serde_json::to_value(reading)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn comparisons_csv(report: &AnalysisReport) -> Result<String, CliError> {
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
        "experiment",
        "causes",
        "level_a",
        "level_b",
        "battery",
        "fallback",
        "blocks",
        "interval_reading",
        "or_mh",
        "or_lo",
        "or_hi",
        "homogeneity_p",
        "per_block",
    ]));
    for comparison in &report.comparisons {
        let head = [
            comparison.a.clone(),                           // BORROW: the row owns its text
            comparison.b.clone(),                           // BORROW: the row owns its text
            comparison.factor.clone(),                      // BORROW: the row owns its text
            comparison.comparability_class.verdict.clone(), // BORROW: the row owns its text
        ];
        let experiment = comparison.experiment.clone(); // BORROW: the row owns its text
        let causes = comparison.comparability_class.causes.join(";");
        let levels: Vec<String> = comparison.levels.as_ref().map_or_else(
            || vec![String::new(); 2],
            |(a, b)| vec![json_text(a), json_text(b)],
        );
        if comparison.entries.is_empty() {
            // A pair with nothing to show, a refused one first of all, still has its row.
            let mut fields: Vec<String> = head.to_vec();
            fields.extend(vec![String::new(); 13]);
            fields.extend([experiment, causes]);
            fields.extend(levels);
            fields.extend(vec![String::new(); 9]);
            text.push_str(&row(fields.as_slice()));
            continue;
        }
        for entry in &comparison.entries {
            let (id, role) = match entry {
                Entry::Tested(tested) => (tested.id.clone(), tested.role.clone()), // BORROW: the row owns its text
                Entry::Omitted(left) => (left.id.clone(), left.role.clone()), // BORROW: the row owns its text
                _ => (String::new(), String::new()),
            };
            let tail: Vec<String> = match entry {
                Entry::Tested(tested) => vec![
                    tested.a.k.to_string(),
                    tested.a.n.to_string(),
                    tested.b.k.to_string(),
                    tested.b.n.to_string(),
                    tested.p.to_string(),
                    tested.p_holm.to_string(),
                    tested.reject.to_string(),
                    number(tested.diff),
                    number(tested.diff_lo),
                    number(tested.diff_hi),
                    String::new(),
                ],
                Entry::Omitted(left) => {
                    let mut empty = vec![String::new(); 10];
                    empty.push(left.omitted.clone()); // BORROW: the row owns its text
                    empty
                }
                _ => vec![String::new(); 11],
            };
            let mut fields: Vec<String> = head.to_vec();
            fields.extend([id, role]);
            fields.extend(tail);
            fields.extend([experiment.clone(), causes.clone()]); // BORROW: each row owns its text
            fields.extend(levels.clone()); // BORROW: each row owns its text
            fields.extend(match entry {
                Entry::Tested(tested) => vec![
                    tested.battery.clone().unwrap_or_default(),
                    tested.fallback.to_string(),
                    tested.blocks.to_string(),
                    reading_name(tested.interval_reading),
                    number(tested.or_mh),
                    number(tested.or_lo),
                    number(tested.or_hi),
                    number(tested.homogeneity_p),
                    json_text(
                        &serde_json::to_value(&tested.per_block)
                            .map_err(|e| CliError::failed(VERB, e))?,
                    ),
                ],
                Entry::Omitted(_) | _ => vec![String::new(); 9],
            });
            text.push_str(&row(fields.as_slice()));
        }
    }
    Ok(text)
}

/// The counts that belong to no other table, one `item,value` row each.
fn summary_csv(report: &AnalysisReport) -> String {
    let mut text = row(&strings(&["item", "value"]));
    let mut put = |item: &str, value: String| text.push_str(&row(&[item.to_owned(), value]));
    put("evaluator", report.evaluator.clone()); // BORROW: the row owns its text
    put("alpha", report.alpha.value.to_string());
    put("i4_violations", report.i4_violations.to_string());
    put(
        "pairs_not_one_factor",
        report.pairs_not_one_factor.to_string(),
    );
    put("pairs_compared", report.comparisons.len().to_string());
    for family in &report.families {
        put(&format!("family.{}", family.role), family.m.to_string());
    }
    text
}

fn runs_csv(report: &AnalysisReport) -> Result<String, CliError> {
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
        let mut fields = vec![held.run.clone(), held.cell.clone(), json_text(&integrity)]; // BORROW: the row owns its text
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
pub fn render_csv(report: &AnalysisReport, table: Table) -> Result<String, CliError> {
    match table {
        Table::Arms => Ok(arms_csv(report)),
        Table::Comparisons => comparisons_csv(report),
        Table::Summary => Ok(summary_csv(report)),
        Table::Runs => runs_csv(report),
    }
}

/// A levels pair as `a vs b`, or `-` when the comparison states none.
fn levels_text(levels: Option<&(Value, Value)>) -> String {
    levels.map_or_else(
        || "-".to_owned(),
        |(a, b)| format!("{} vs {}", json_text(a), json_text(b)),
    )
}

/// The report as lines for a person: the arms, the decisions and the entries left out.
#[must_use]
pub fn render_text(report: &AnalysisReport) -> String {
    let mut lines = vec![format!(
        "evaluator {}  alpha {} ({})  i4 violations {}",
        report.evaluator, report.alpha.value, report.alpha.source, report.i4_violations
    )];
    lines.push(format!("arms ({})", report.arms.len()));
    for arm in &report.arms {
        lines.push(format!(
            "  {} {}  valid {}  unsealed {}  gate {}/{}  quality {}/{}  spent ${} over {} runs ({} without a total)",
            arm.experiment.chars().take(SHORT).collect::<String>(),
            arm.cell,
            arm.runs.valid,
            arm.runs.unsealed,
            arm.rates.gate.num,
            arm.rates.gate.den,
            arm.rates.quality.num,
            arm.rates.quality.den,
            arm.spent.cost_usd,
            arm.spent.runs_counted,
            arm.spent.runs_without_total
        ));
    }
    lines.push(format!("decisions ({})", report.summary.decisions.len()));
    for held in &report.summary.decisions {
        lines.push(format!(
            "  {} [{}] {} {}  diff {} [{}, {}]  p_holm {}  {}",
            held.factor,
            held.role,
            held.id,
            levels_text(held.levels.as_ref()),
            number_text(held.diff),
            number_text(held.diff_lo),
            number_text(held.diff_hi),
            held.p_holm,
            if held.reject { "reject" } else { "keep" }
        ));
    }
    lines.push(format!("omitted ({})", report.summary.omitted.len()));
    for held in &report.summary.omitted {
        lines.push(format!(
            "  {} [{}] {} {}  {}",
            held.factor,
            held.role,
            held.id,
            levels_text(held.levels.as_ref()),
            held.reason
        ));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// A number for a person, `-` when there is none.
fn number_text(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), |held| held.to_string())
}
