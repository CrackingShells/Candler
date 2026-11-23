//! `candler show`: stored outcomes, or their rates.

use std::collections::BTreeMap;

use candler_core::evaluation::selection::rates::{rates, rates_toml};
use candler_schemas::listing::{CellRate, Rates};
use serde_json::json;

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
    /// Print the output as text, or as TOML, instead of its JSON record. not an experiment
    /// setting: it is how the output is read.
    #[arg(long, value_enum)]
    pub format: Option<ShowFormat>,
}

/// The output formats of `candler show`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ShowFormat {
    /// Lines for a person.
    Text,
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
        if args.format == Some(ShowFormat::Toml) {
            rates_toml(&cells)
        } else {
            let table: BTreeMap<String, CellRate> = cells
                .iter()
                .map(|(cell, rate)| {
                    (
                        cell.to_string(),
                        CellRate {
                            gate: json!(rate.gate()),
                            quality: json!(rate.quality()),
                        },
                    )
                })
                .collect();
            if args.format == Some(ShowFormat::Text) {
                table
                    .iter()
                    .map(|(cell, rate)| format!("{cell}\t{}\t{}\n", rate.gate, rate.quality))
                    .collect::<Vec<_>>()
                    .concat()
            } else {
                let text = serde_json::to_string(&Rates::new(table))
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
            None => {
                let text =
                    serde_json::to_string(&document).map_err(|e| CliError::failed("show", e))?;
                format!("{text}\n")
            }
            Some(ShowFormat::Toml) => {
                toml::to_string(&document).map_err(|e| CliError::failed("show", e))?
            }
            Some(ShowFormat::Text) => counts
                .iter()
                .flat_map(|(evaluator, by_status)| {
                    by_status
                        .iter()
                        .map(move |(status, count)| format!("{evaluator}\t{status}\t{count}\n"))
                })
                .collect::<Vec<_>>()
                .concat(),
        }
    };
    print!("{text}");
    Ok(Exit::Success)
}
