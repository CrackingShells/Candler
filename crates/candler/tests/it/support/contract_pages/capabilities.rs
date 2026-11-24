//! The capability page: what a run can expect of each driver the binary registers, one column per
//! driver and one row per setting, per fact and per typed spec.
//!
//! The rows come from `AgentSetting::ALL` and `DriverFact::ALL`, one row per entry, plus the rows a
//! driver does not declare words for (gateway route, stop, authoritative trace, skills layout),
//! which read the driver's typed specs. Each cell of a setting or fact row is the text of the
//! driver's `answer` or `fact`; a reader never meets one fact in two spellings.

use std::fmt::Write;
use std::path::Path;
use std::str::FromStr as _;
use std::sync::Arc;

use candler_harness::driver::HarnessDriver;
use candler_harness::spec::{
    Action, AgentSetting, Answer, DriverFact, EffortLevel, PermissionMode, StopSignal, StopSpec,
};
use candler_schemas::execution::{GitCommit, TreatmentRef};
use candler_schemas::primitives::Name;
use candler_schemas::servers::McpServerKind;

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

/// The words of an answer, in a table cell.
fn answer_text(answer: Answer) -> String {
    match answer {
        Answer::Mapped { construct } => construct.to_owned(), // BORROW: .to_owned() makes the owned cell text
        Answer::Candler { how } => format!("Candler: {how}"),
        Answer::Refused { reason } => format!("no ({reason})"),
    }
}

/// The label of the row an agent setting words.
fn setting_label(setting: AgentSetting) -> &'static str {
    match setting {
        AgentSetting::ModelId => "model id",
        AgentSetting::ModelEndpoint => "model endpoint",
        AgentSetting::Effort(level) => match level {
            EffortLevel::Low => "model effort: low",
            EffortLevel::Medium => "model effort: medium",
            EffortLevel::High => "model effort: high",
            EffortLevel::Xhigh => "model effort: xhigh",
            EffortLevel::Max => "model effort: max",
            EffortLevel::Ultracode => "model effort: ultracode",
        },
        AgentSetting::PermissionMode(mode) => match mode {
            PermissionMode::Default => "permission mode: default",
            PermissionMode::AcceptEdits => "permission mode: acceptEdits",
            PermissionMode::Plan => "permission mode: plan",
            PermissionMode::BypassPermissions => "permission mode: bypassPermissions",
            PermissionMode::Auto => "permission mode: auto",
            PermissionMode::DontAsk => "permission mode: dontAsk",
        },
        AgentSetting::ToolsAllowed => "tools allowed",
        AgentSetting::ToolsDisallowed => "tools disallowed",
        AgentSetting::Env => "env",
        AgentSetting::Inputs => "inputs",
        AgentSetting::Skills => "skills",
        AgentSetting::McpTransport(kind) => match kind {
            McpServerKind::Stdio => "MCP transport: stdio",
            McpServerKind::Local => "MCP transport: local",
            McpServerKind::Http => "MCP transport: http",
            McpServerKind::Sse => "MCP transport: sse",
            McpServerKind::Remote => "MCP transport: remote",
        },
        AgentSetting::Codemode(true) => "MCP codemode: on",
        AgentSetting::Codemode(false) => "MCP codemode: off",
        AgentSetting::McpName => "MCP server name",
        AgentSetting::WallBudget => "wall cap",
        AgentSetting::TurnBudget => "turn cap",
        AgentSetting::CostBudget => "cost cap",
        AgentSetting::SessionMode => "session mode",
    }
}

/// The label of the row a driver fact words.
fn fact_label(fact: DriverFact) -> &'static str {
    match fact {
        DriverFact::GracefulStop => "graceful stop",
        DriverFact::LiveEvents => "live events",
        DriverFact::InHarnessGuard => "in-harness guard",
        DriverFact::ExecutionOutsideChild => "execution outside the child",
        DriverFact::Census => "census source",
        DriverFact::UnsetPermissionMode => "permission mode when none is set",
    }
}

/// The rows of the capability table outside the declaration: the capabilities a driver does not
/// word through `answer` or `fact`, read from its typed specs.
#[derive(Debug, Clone, Copy)]
enum SpecRow {
    /// The gateway variables the driver names.
    GatewayRoute,
    /// The stop the driver's `stop()` describes.
    Stop,
    /// The collected or live stream the authoritative trace comes from.
    AuthoritativeTrace,
    /// Where the driver places the treatment's skill tree.
    SkillsLayout,
}

impl SpecRow {
    /// The rows, in the order of the page.
    const ALL: [Self; 4] = [
        Self::GatewayRoute,
        Self::Stop,
        Self::AuthoritativeTrace,
        Self::SkillsLayout,
    ];

    /// The label of the row.
    const fn label(self) -> &'static str {
        match self {
            Self::GatewayRoute => "gateway route",
            Self::Stop => "stop",
            Self::AuthoritativeTrace => "authoritative trace",
            Self::SkillsLayout => "skills layout",
        }
    }

    /// The cell of one driver: `Candler: …` when Candler stands in, otherwise what the driver's
    /// spec says it does.
    ///
    /// # Errors
    /// `Err` with a message about the wording when the driver's spec has no cell for the row:
    /// no registered driver fails this.
    fn cell(
        self,
        name: &str,
        driver: &Arc<dyn HarnessDriver>,
        registry: &Registry,
    ) -> Result<String, String> {
        match self {
            Self::GatewayRoute => {
                let env = driver
                    .gateway()
                    .ok_or("a registered driver names its gateway variables")?;
                Ok(format!("Candler: gateway, address in `{}`", env.base_url))
            }
            Self::Stop => {
                let spec = driver.stop();
                let signal = signal(&spec)?;
                match (
                    &spec.pre_stop,
                    !driver.fact(DriverFact::GracefulStop).is_refused(),
                ) {
                    (None, true) => Ok("yes".to_owned()),
                    (Some(Action::Exec(_)), _) => Ok(format!(
                        "Candler: interrupt command in the container, then {signal}"
                    )),
                    (Some(Action::Command(_)), _) => Ok(format!(
                        "Candler: interrupt command on the host, then {signal}"
                    )),
                    (Some(Action::Http { .. }), _) => {
                        Ok(format!("Candler: interrupt request, then {signal}"))
                    }
                    (None, false) | (Some(_), _) => {
                        Err("a registered driver's stop has no cell wording".to_owned())
                    }
                }
            }
            Self::AuthoritativeTrace => {
                if let Some((stream, _)) = registry.offline(name).first() {
                    return Ok(format!("Candler: collects `{stream}` after the run"));
                }
                if driver.live_reader().is_some()
                    && !driver.fact(DriverFact::LiveEvents).is_refused()
                {
                    return Ok("yes".to_owned());
                }
                Err("a registered driver's trace has no cell wording".to_owned())
            }
            Self::SkillsLayout => {
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
        }
    }
}

fn signal(spec: &StopSpec) -> Result<&'static str, String> {
    match spec.signal {
        StopSignal::Term => Ok("SIGTERM"),
        StopSignal::Int => Ok("SIGINT"),
        StopSignal::Hup => Ok("SIGHUP"),
        _ => Err("a stop signal has no wording".to_owned()),
    }
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

/// One row of the capability table: which key it words and the label of the row.
struct Row {
    label: &'static str,
    key: RowKey,
}

/// The key a row words: an agent setting, a driver fact, or a row read from a typed spec.
enum RowKey {
    /// An agent setting.
    Setting(AgentSetting),
    /// A driver fact.
    Fact(DriverFact),
    /// A row read from a typed spec.
    Spec(SpecRow),
}

/// The rows of the capability table: one row per agent setting, then the rows read from the typed
/// specs, then one row per driver fact. The settings and the facts read the driver's `answer` or
/// `fact`; the others read the driver's specs.
fn rows() -> Vec<Row> {
    let mut out: Vec<Row> =
        Vec::with_capacity(AgentSetting::ALL.len() + DriverFact::ALL.len() + SpecRow::ALL.len());
    for setting in AgentSetting::ALL {
        out.push(Row {
            label: setting_label(setting),
            key: RowKey::Setting(setting),
        });
    }
    for spec in SpecRow::ALL {
        out.push(Row {
            label: spec.label(),
            key: RowKey::Spec(spec),
        });
    }
    for fact in DriverFact::ALL {
        out.push(Row {
            label: fact_label(fact),
            key: RowKey::Fact(fact),
        });
    }
    out
}

/// The cell of one driver for the row's key: the words of its answer to a setting or a fact, or
/// of its typed spec.
///
/// # Errors
/// `Err` with a message about the wording when a driver's spec has no cell for the row; a
/// registered driver never fails a setting or a fact, as it answers every one.
fn row_cell(
    key: &RowKey,
    name: &str,
    driver: &Arc<dyn HarnessDriver>,
    registry: &Registry,
) -> Result<String, String> {
    match key {
        RowKey::Setting(setting) => Ok(answer_text(driver.answer(*setting))),
        RowKey::Fact(fact) => Ok(answer_text(driver.fact(*fact))),
        RowKey::Spec(spec) => spec.cell(name, driver, registry),
    }
}

/// The capability table: one row per setting, spec or fact, one column per registered driver.
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
    for row in rows() {
        let _ = write!(out, "| {} |", row.label);
        for name in &names {
            let driver = registry
                .driver(name)
                .map_err(|_| format!("no driver for `{name}`"))?;
            let text = row_cell(&row.key, name, &driver, registry)
                .map_err(|e| format!("{} of `{name}`: {e}", row.label))?;
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
        &fields_table(registry)?,
        &refusals,
        existing,
    ))
}
