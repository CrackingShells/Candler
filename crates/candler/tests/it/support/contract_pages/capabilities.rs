//! The capability page: what a run can expect of each driver the binary registers, one column per
//! driver and one row per capability.
//!
//! Each row has one mapping from what a driver declares to the words of its cell, so a reader never
//! meets two spellings of one fact.

use std::fmt::Write;
use std::path::Path;
use std::str::FromStr as _;
use std::sync::Arc;

use candler_harness::driver::{Disposition, HarnessDriver};
use candler_harness::spec::{Action, StopSignal, StopSpec};
use candler_schemas::execution::{GitCommit, TreatmentRef};
use candler_schemas::primitives::Name;

use super::harness::{HarnessPage, assemble, cell, version_section};
use super::refusals::{Entry, catalogue_entries};
use super::versions::Versions;
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

/// A cell of the capability table, from one driver.
type Mapping = fn(&str, &Arc<dyn HarnessDriver>, &Registry) -> Result<String, String>;

/// The rows of the capability table: the row's name and the one mapping that words its cells.
const ROWS: [(&str, Mapping); 8] = [
    ("gateway route", gateway_route),
    ("census source", census_source),
    ("session mode", session_mode),
    ("turn cap", turn_cap),
    ("cost cap", cost_cap),
    ("stop", stop),
    ("authoritative trace", authoritative_trace),
    ("skills layout", skills_layout),
];

fn gateway_route(_: &str, driver: &Arc<dyn HarnessDriver>, _: &Registry) -> Result<String, String> {
    let env = driver
        .gateway()
        .ok_or("a registered driver names its gateway variables")?;
    Ok(format!("Candler: gateway, address in `{}`", env.base_url))
}

fn census_source(_: &str, driver: &Arc<dyn HarnessDriver>, _: &Registry) -> Result<String, String> {
    match (driver.capabilities().census, driver.census_stream()) {
        (true, Some(_)) => Ok("Candler: gateway, first request with tools".to_owned()),
        (true, None) => Ok("yes".to_owned()),
        (false, _) => Err("a registered driver without a census has no cell wording".to_owned()),
    }
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "every row of the table has the one mapping type"
)]
fn session_mode(_: &str, driver: &Arc<dyn HarnessDriver>, _: &Registry) -> Result<String, String> {
    Ok(if driver.capabilities().session {
        "yes".to_owned()
    } else {
        "no (refused)".to_owned()
    })
}

/// A cap the harness enforces is `yes`; one it lacks must be a refused row of its settings table.
fn cap(driver: &Arc<dyn HarnessDriver>, enforced: bool, setting: &str) -> Result<String, String> {
    if enforced {
        return Ok("yes".to_owned());
    }
    let refused = driver
        .settings_table()
        .iter()
        .any(|row| row.setting == setting && row.disposition == Disposition::Refused);
    if refused {
        Ok("no (refused)".to_owned())
    } else {
        Err(format!(
            "a driver that lacks `{setting}` declares a refused row for it"
        ))
    }
}

fn turn_cap(_: &str, driver: &Arc<dyn HarnessDriver>, _: &Registry) -> Result<String, String> {
    cap(
        driver,
        driver.capabilities().budgets.turns,
        "budget.per_run.turns",
    )
}

fn cost_cap(_: &str, driver: &Arc<dyn HarnessDriver>, _: &Registry) -> Result<String, String> {
    cap(
        driver,
        driver.capabilities().budgets.cost,
        "budget.per_run.max_cost_usd",
    )
}

fn signal(spec: &StopSpec) -> Result<&'static str, String> {
    match spec.signal {
        StopSignal::Term => Ok("SIGTERM"),
        StopSignal::Int => Ok("SIGINT"),
        StopSignal::Hup => Ok("SIGHUP"),
        _ => Err("a stop signal has no wording".to_owned()),
    }
}

fn stop(_: &str, driver: &Arc<dyn HarnessDriver>, _: &Registry) -> Result<String, String> {
    let spec = driver.stop();
    let signal = signal(&spec)?;
    match (&spec.pre_stop, driver.capabilities().graceful_stop) {
        (None, true) => Ok("yes".to_owned()),
        (Some(Action::Exec(_)), _) => Ok(format!(
            "Candler: interrupt command in the container, then {signal}"
        )),
        (Some(Action::Command(_)), _) => Ok(format!(
            "Candler: interrupt command on the host, then {signal}"
        )),
        (Some(Action::Http { .. }), _) => Ok(format!("Candler: interrupt request, then {signal}")),
        (None, false) | (Some(_), _) => {
            Err("a registered driver's stop has no cell wording".to_owned())
        }
    }
}

fn authoritative_trace(
    name: &str,
    driver: &Arc<dyn HarnessDriver>,
    registry: &Registry,
) -> Result<String, String> {
    if let Some((stream, _)) = registry.offline(name).first() {
        return Ok(format!("Candler: collects `{stream}` after the run"));
    }
    if driver.live_reader().is_some() && driver.capabilities().live_events {
        return Ok("yes".to_owned());
    }
    Err("a registered driver's trace has no cell wording".to_owned())
}

/// A treatment whose tree the driver's skill layout is asked about.
fn probe_treatment() -> Result<TreatmentRef, String> {
    let name = Name::from_str("probe").map_err(|e| e.to_string())?;
    let mut treatment = TreatmentRef::new(name);
    treatment.commit = Some(GitCommit::from_str(&"0".repeat(40)).map_err(|e| e.to_string())?);
    Ok(treatment)
}

/// A tar archive with one regular file `path` that holds `x`.
fn tar_of(path: &str) -> Vec<u8> {
    let mut header = [0_u8; 512];
    header[..path.len()].copy_from_slice(path.as_bytes());
    header[100..107].copy_from_slice(b"0000644");
    header[124..135].copy_from_slice(b"00000000001");
    header[136..147].copy_from_slice(b"00000000000");
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[148..156].copy_from_slice(b"        ");
    let sum: u32 = header.iter().map(|byte| u32::from(*byte)).sum();
    header[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
    let mut out = header.to_vec();
    let mut data = [0_u8; 512];
    data[0] = b'x';
    out.extend_from_slice(&data);
    out.extend_from_slice(&[0_u8; 1024]);
    out
}

fn skills_layout(_: &str, driver: &Arc<dyn HarnessDriver>, _: &Registry) -> Result<String, String> {
    let treatment = probe_treatment()?;
    let skills = driver.skills();
    let archive = skills
        .archive_command(&treatment, Path::new("/probe"))
        .map_err(|e| e.to_string())?;
    let subtree = archive
        .argv()
        .last()
        .ok_or("the archive command has no pathspec")?
        .clone();
    let files = skills
        .files(&treatment, &tar_of(&format!("{subtree}/SKILL.md")))
        .map_err(|e| e.to_string())?;
    let first = files
        .first()
        .map(|file| file.path.as_str())
        .ok_or("the layout places no file")?;
    let tree = first
        .strip_suffix("/SKILL.md")
        .ok_or("the layout does not place SKILL.md")?;
    if tree == subtree {
        Ok("yes".to_owned())
    } else {
        Ok(format!(
            "Candler: places the tree at `{}`",
            tree.replace("probe", "<name>")
        ))
    }
}

/// The capability table: one row per capability, one column per registered driver.
fn capability_fields(registry: &Registry) -> Result<String, String> {
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
    for (label, mapping) in ROWS {
        let _ = write!(out, "| {label} |");
        for name in &names {
            let driver = registry
                .driver(name)
                .map_err(|_| format!("no driver for `{name}`"))?;
            let text = mapping(name, &driver, registry)
                .map_err(|e| format!("{label} of `{name}`: {e}"))?;
            let _ = write!(out, " {} |", cell(&text));
        }
        out.push('\n');
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
        &capability_fields(registry)?,
        &refusals,
        existing,
    ))
}
