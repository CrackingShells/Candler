//! `candler diff`: the outcomes of two evaluator versions, compared.

use candler_core::evaluation::selection::diff::{DiffReport, Equivalence, diff};
use std::path::PathBuf;

use super::{CliError, Exit, common};
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
    /// The output format. not an experiment setting: it is how the output is read.
    #[arg(long, value_enum)]
    pub format: DiffFormat,
}

/// The output formats of `candler diff`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum DiffFormat {
    /// JSON.
    Json,
    /// Text.
    Text,
}

/// Compares the two versions and prints the report.
///
/// # Errors
/// Returns [`CliError::Refused`] for an evaluator that is not `name@version`, [`CliError::Coded`]
/// for a bare version that does not name one evaluator of the store and for a refused equivalence
/// file, and [`CliError::Refused`] when nothing was compared, and [`CliError::Failed`] when the store cannot
/// be read.
pub async fn run(cli: &Cli, args: &DiffArgs) -> Result<Exit, CliError> {
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
        return Err(CliError::refused(format!(
            "nothing compared between {from} and {to}"
        )));
    }
    let text = match args.format {
        DiffFormat::Json => {
            serde_json::to_string(&report).map_err(|e| CliError::failed("diff", e))?
        }
        DiffFormat::Text => render_text(&report),
    };
    println!("{text}");
    if report.changed > 0 {
        Ok(Exit::Changed {
            count: usize::try_from(report.changed).unwrap_or(usize::MAX),
        })
    } else {
        Ok(Exit::Success)
    }
}

/// The report as plain lines: the counts, then each change and each uncompared expectation.
fn render_text(report: &DiffReport) -> String {
    let mut lines = vec![format!(
        "compared {} unchanged {} excused {} changed {}",
        report.compared, report.unchanged, report.excused, report.changed
    )];
    for change in &report.changes {
        let class = serde_json::to_value(change.class)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        lines.push(format!(
            "changed {class} {} {}",
            change.run, change.expectation
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
