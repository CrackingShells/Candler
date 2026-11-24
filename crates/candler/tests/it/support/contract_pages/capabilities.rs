//! The capability page: what a run can expect of each driver the binary registers, one column per
//! driver and one row per setting, per fact and per typed spec.
//!
//! The rows come from `AgentSetting::ALL` and `DriverFact::ALL`, one row per entry, plus the rows a
//! driver does not declare words for (gateway route, authoritative trace), which read the driver's
//! typed specs. Each cell of a setting or fact row is [`answer_words`] of the driver's `answer` or
//! `fact`; a reader never meets one fact in two spellings.

use std::fmt::Write;
use std::sync::Arc;

use candler_harness::driver::HarnessDriver;
use candler_harness::spec::{AgentSetting, DriverFact};

use super::harness::{HarnessPage, assemble, cell, version_section};
use super::refusals::{Entry, catalogue_entries};
use super::versions::Versions;
use super::{answer_words, setting_label};
use crate::drivers::Registry;

/// The page of the capabilities of the registered drivers.
pub const CAPABILITY_PAGE: HarnessPage = HarnessPage {
    file: "harness-capabilities.md",
    title: "Harness capabilities",
    writer: "experimenter, reading",
};

const CAPABILITY_SURFACE: &str = "What a run can expect of each registered harness: what the harness does itself, what Candler supplies in its place, and what resolving refuses.";

/// The codes of the refusals that a capability a harness lacks causes in resolving.
const CAPABILITY_CODES: [&str; 2] = [
    "experiment.session_refused",
    "experiment.setting_unsupported",
];

/// The label of the row a driver fact words.
const fn fact_label(fact: DriverFact) -> &'static str {
    match fact {
        DriverFact::GracefulStop => "graceful stop",
        DriverFact::LiveEvents => "live events",
        DriverFact::InHarnessGuard => "in-harness guard",
        DriverFact::ExecutionOutsideChild => "execution outside the child",
        DriverFact::Census => "census source",
        DriverFact::UnsetPermissionMode => "permission mode when none is set",
    }
}

/// The cell of one driver for a row read from a typed spec.
type SpecCell = fn(&str, &Arc<dyn HarnessDriver>, &Registry) -> Result<String, String>;

/// The rows of the capability table outside the declaration: the capabilities a driver does not
/// word through `answer` or `fact`, read from its typed specs.
///
/// # Errors
/// `Err` with a message about the wording when a driver's spec has no cell for the row; no
/// registered driver fails this.
const SPEC_ROWS: [(&str, SpecCell); 2] = [
    ("gateway route", gateway_route),
    ("authoritative trace", authoritative_trace),
];

/// The cell of one driver for its gateway variables.
///
/// # Errors
/// `Err` when the driver names no gateway variables; no registered driver fails this.
fn gateway_route(_: &str, driver: &Arc<dyn HarnessDriver>, _: &Registry) -> Result<String, String> {
    let env = driver
        .gateway()
        .ok_or("a registered driver names its gateway variables")?;
    Ok(format!("Candler: gateway, address in `{}`", env.base_url))
}

/// The cell of one driver for its authoritative trace: the offline stream the registry collects,
/// or the driver's live reader when it streams events itself.
///
/// # Errors
/// `Err` when the driver's trace has no cell wording; no registered driver fails this.
fn authoritative_trace(
    name: &str,
    driver: &Arc<dyn HarnessDriver>,
    registry: &Registry,
) -> Result<String, String> {
    if let Some((stream, _)) = registry.offline(name).first() {
        return Ok(format!("Candler: collects `{stream}` after the run"));
    }
    if driver.live_reader().is_some() && !driver.fact(DriverFact::LiveEvents).is_refused() {
        return Ok("yes".to_owned());
    }
    Err("a registered driver's trace has no cell wording".to_owned())
}

/// Writes one row: the label and each driver's cell.
///
/// # Errors
/// [`CliError`](crate::commands::CliError) of the registry, when no registered driver has the
/// name; no registered driver fails a cell.
fn write_row(
    out: &mut String,
    label: &str,
    names: &[String],
    registry: &Registry,
    cells: impl Fn(&str, &Arc<dyn HarnessDriver>) -> Result<String, String>,
) -> Result<(), String> {
    let _ = write!(out, "| {label} |");
    for name in names {
        let driver = registry
            .driver(name)
            .map_err(|_| format!("no driver for `{name}`"))?;
        let text = cells(name, &driver).map_err(|e| format!("{label} of `{name}`: {e}"))?;
        let _ = write!(out, " {} |", cell(&text));
    }
    out.push('\n');
    Ok(())
}

/// The capability table: one row per setting, per typed spec or per fact, one column per
/// registered driver.
fn fields_table(registry: &Registry) -> Result<String, String> {
    let names = registry.names();
    let mut out = String::from("| Capability |");
    for name in &names {
        let _ = write!(out, " `{name}` |");
    }
    out.push_str("\n|---|");
    for _ in &names {
        out.push_str("---|");
    }
    out.push('\n');
    for setting in AgentSetting::ALL {
        write_row(
            &mut out,
            setting_label(setting),
            &names,
            registry,
            |_, driver| Ok(answer_words(driver.answer(setting))),
        )?;
    }
    for (label, cells) in SPEC_ROWS {
        write_row(&mut out, label, &names, registry, |name, driver| {
            cells(name, driver, registry)
        })?;
    }
    for fact in DriverFact::ALL {
        write_row(&mut out, fact_label(fact), &names, registry, |_, driver| {
            Ok(answer_words(driver.fact(fact)))
        })?;
    }
    Ok(out)
}

/// The capability page.
pub fn render_capabilities(
    versions: &Versions,
    registry: &Registry,
    existing: Option<&str>,
) -> Result<String, String> {
    let known = catalogue_entries();
    let refusals: Vec<Entry> = CAPABILITY_CODES
        .iter()
        .map(|code| {
            known
                .iter()
                .find(|row| row.code == *code)
                .cloned()
                .ok_or_else(|| format!("`{code}` is no row of the catalogue"))
        })
        .collect::<Result<_, _>>()?;
    Ok(assemble(
        &CAPABILITY_PAGE,
        CAPABILITY_SURFACE,
        &version_section(versions)?,
        &fields_table(registry)?,
        &refusals,
        existing,
    ))
}
