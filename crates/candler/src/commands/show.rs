//! `candler show`: stored outcomes, or their rates.

use std::collections::BTreeMap;

use candler_core::evaluation::selection::rates::{rates, rates_toml};
use serde_json::{Value, json};

use super::{CliError, Exit, common};
use crate::Cli;

/// The arguments of `candler show`.
#[derive(Debug, clap::Args)]
pub struct ShowArgs {
    /// Print the per-cell rates of one evaluator instead of the outcomes; the evaluator is the
    /// one the store holds when `--evaluator` is absent. not an experiment setting: it chooses
    /// what the call prints.
    #[arg(long)]
    pub rates: bool,
    /// The evaluator whose outcomes are shown, as `name@version` or a bare version one evaluator
    /// holds; every evaluator when absent. not an experiment setting: it narrows which stored
    /// outcomes the call prints.
    #[arg(long)]
    pub evaluator: Option<String>,
    /// The output format. not an experiment setting: it is how the output is read.
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
/// Returns [`CliError::Coded`] for an evaluator that is not `name@version` and for a bare version
/// or an absent evaluator that does not name one evaluator of the store, and [`CliError::Failed`] when the store cannot be read or the output cannot be
/// written.
pub async fn run(cli: &Cli, args: &ShowArgs) -> Result<Exit, CliError> {
    let requested = args.evaluator.as_deref();
    if let Some(text) = requested {
        common::check_evaluator_text("--evaluator", text)?;
    }
    let store = common::open_store("show", cli).await?;
    let selection = common::read_selection("show", &store)?;
    let evaluator = if args.rates || requested.is_some() {
        Some(common::evaluator_of(&selection, "--evaluator", requested)?)
    } else {
        None
    };
    let text = if args.rates {
        let evaluator = evaluator.ok_or_else(|| CliError::refused("--rates needs an evaluator"))?;
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
