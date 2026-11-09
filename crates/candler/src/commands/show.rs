//! `candler show`: stored outcomes, or their rates.

use std::collections::BTreeMap;

use candler_core::evaluation::selection::rates::{rates, rates_toml};
use candler_schemas::primitives::NameVersion;
use serde_json::{Value, json};

use super::{CliError, Exit, common};
use crate::Cli;

/// The arguments of `candler show`.
#[derive(Debug, clap::Args)]
pub struct ShowArgs {
    /// Print the per-cell rates of one evaluator instead of the outcomes.
    #[arg(long, requires = "evaluator")]
    pub rates: bool,
    /// The evaluator whose outcomes are shown, as `name@version`; every evaluator when absent.
    #[arg(long)]
    pub evaluator: Option<String>,
    /// The output format.
    #[arg(long, value_enum)]
    pub format: ShowFormat,
}

/// The output formats of `candler show`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ShowFormat {
    /// JSON.
    Json,
    /// TOML.
    Toml,
}

/// Prints the outcomes or the rates.
///
/// # Errors
/// Returns [`CliError::Refused`] for an evaluator that is not `name@version` and for `--rates`
/// without one, and [`CliError::Failed`] when the store cannot be read or the output cannot be
/// written.
pub async fn run(cli: &Cli, args: &ShowArgs) -> Result<Exit, CliError> {
    let evaluator = args
        .evaluator
        .as_deref()
        .map(|text| {
            text.parse::<NameVersion>()
                .map_err(|e| CliError::refused(format!("candler show: --evaluator `{text}`: {e}")))
        })
        .transpose()?;
    let store = common::open_store("show", cli).await?;
    let selection = common::read_selection("show", &store)?;
    let text = if args.rates {
        let evaluator = evaluator
            .ok_or_else(|| CliError::refused("candler show: --rates requires --evaluator"))?;
        let cells = rates(&selection, &evaluator);
        match args.format {
            ShowFormat::Toml => rates_toml(&cells),
            ShowFormat::Json => {
                let table: BTreeMap<String, Value> = cells
                    .iter()
                    .map(|(cell, rate)| {
                        (
                            cell.to_string(),
                            json!({"gate": rate.gate(), "quality": rate.quality()}),
                        )
                    })
                    .collect();
                let text = serde_json::to_string(&json!({ "cell": table }))
                    .map_err(|e| CliError::failed("show", e))?;
                format!("{text}\n")
            }
        }
    } else {
        let mut counts: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
        for record in selection.current() {
            if evaluator
                .as_ref()
                .is_some_and(|wanted| record.evaluator() != wanted)
            {
                continue;
            }
            let status = serde_json::to_value(record.status())
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            let slot = counts
                .entry(record.evaluator().to_string())
                .or_default()
                .entry(status)
                .or_insert(0);
            *slot = slot.saturating_add(1);
        }
        let document = json!({ "counts": counts });
        match args.format {
            ShowFormat::Json => {
                let text =
                    serde_json::to_string(&document).map_err(|e| CliError::failed("show", e))?;
                format!("{text}\n")
            }
            ShowFormat::Toml => {
                toml::to_string(&document).map_err(|e| CliError::failed("show", e))?
            }
        }
    };
    print!("{text}");
    Ok(Exit::Success)
}
