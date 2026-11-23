//! `candler diff`: the outcomes of two evaluator versions, compared.

use candler_core::evaluation::selection::diff::{Equivalence, diff};
use candler_schemas::listing::{
    DiffChange, DiffEquivalence, DiffReport, DiffReportTag, DiffUncompared,
};
use std::path::PathBuf;

use super::refusal::CliRefusal;
use super::{CliError, Exit, OutputArgs, common, write_record};
use crate::Cli;

/// The arguments of `candler diff`.
#[derive(Debug, clap::Args)]
pub struct DiffArgs {
    /// The base evaluator, as `id@version` or a bare version one evaluator holds. cannot be
    /// derived: which versions to compare is the question the caller asks of the store.
    #[arg(long)]
    pub from: String,
    /// The compared evaluator, as `id@version` or a bare version one evaluator holds. cannot be
    /// derived: which versions to compare is the question the caller asks of the store.
    #[arg(long)]
    pub to: String,
    /// An equivalence file naming the rows excused from the comparison. not an experiment
    /// setting: it is the caller's list of rows to excuse from this one comparison.
    #[arg(long)]
    pub equivalence: Option<PathBuf>,
    #[command(flatten)]
    pub output: OutputArgs,
}

/// Compares the two versions and prints the report.
///
/// # Errors
/// Returns [`CliError::Coded`] for an evaluator that is not `name@version`, for a bare version that
/// does not name one evaluator of the store, for a refused equivalence file and when nothing was
/// compared, and [`CliError::Failed`] when the store cannot be read.
pub async fn run(cli: &Cli, args: &DiffArgs) -> Result<Exit, CliError> {
    common::check_evaluator_text("--from", args.from.as_str())?; // BORROW: explicit .as_str()
    common::check_evaluator_text("--to", args.to.as_str())?; // BORROW: explicit .as_str()
    let equivalence = args
        .equivalence
        .as_deref()
        .map(|path| Equivalence::load(path).map_err(|e| CliError::refused_by(e).with_file(path)))
        .transpose()?;
    let store = common::open_store("diff", cli).await?;
    let selection = common::read_selection("diff", &store)?;
    let from = common::evaluator_of(&selection, "--from", Some(args.from.as_str()))?; // BORROW: explicit .as_str()
    let to = common::evaluator_of(&selection, "--to", Some(args.to.as_str()))?; // BORROW: explicit .as_str()
    let report = diff(
        &selection.for_evaluator(&from),
        &selection.for_evaluator(&to),
        equivalence.as_ref(),
    );
    if report.compared == 0 {
        return Err(CliError::refused_by(CliRefusal::NothingCompared {
            from: from.to_string(),
            to: to.to_string(),
        }));
    }
    let record = record_of(&report)?;
    if args.output.text() {
        println!("{}", render_text(&record));
    } else {
        write_record("diff", &mut std::io::stdout().lock(), &record)?;
    }
    if report.changed > 0 {
        Ok(Exit::Changed {
            count: usize::try_from(report.changed).unwrap_or(usize::MAX),
        })
    } else {
        Ok(Exit::Success)
    }
}

/// The record of the core's report.
fn record_of(
    report: &candler_core::evaluation::selection::diff::DiffReport,
) -> Result<DiffReport, CliError> {
    let mut changes = Vec::new();
    for change in &report.changes {
        let class = serde_json::to_value(change.class)
            .map_err(|e| CliError::failed("diff", e))?
            .as_str()
            .unwrap_or_default()
            .to_owned();
        changes.push(DiffChange {
            class,
            run: change.run.to_string(),
            expectation: change.expectation.to_string(),
            from: serde_json::to_value(&change.from).map_err(|e| CliError::failed("diff", e))?,
            to: serde_json::to_value(&change.to).map_err(|e| CliError::failed("diff", e))?,
        });
    }
    Ok(DiffReport {
        schema: DiffReportTag,
        compared: report.compared,
        unchanged: report.unchanged,
        excused: report.excused,
        changed: report.changed,
        changes,
        uncompared: report
            .uncompared
            .iter()
            .map(|left_out| DiffUncompared {
                expectation: left_out.expectation.to_string(),
                count: left_out.count,
            })
            .collect(),
        equivalence: report
            .equivalence
            .as_ref()
            .map(|equivalence| DiffEquivalence {
                sha256: equivalence.sha256.clone(), // BORROW: the record owns its text
                rows: equivalence.rows,
            }),
    })
}

/// The report as plain lines: the counts, then each change and each uncompared expectation.
fn render_text(report: &DiffReport) -> String {
    let mut lines = vec![format!(
        "compared {} unchanged {} excused {} changed {}",
        report.compared, report.unchanged, report.excused, report.changed
    )];
    for change in &report.changes {
        lines.push(format!(
            "changed {} {} {}",
            change.class, change.run, change.expectation
        ));
    }
    for left_out in &report.uncompared {
        lines.push(format!(
            "uncompared {} {}",
            left_out.expectation, left_out.count
        ));
    }
    if let Some(equivalence) = report.equivalence.as_ref() {
        lines.push(format!(
            "equivalence {} {}",
            equivalence.sha256, equivalence.rows
        ));
    }
    lines.join("\n")
}
