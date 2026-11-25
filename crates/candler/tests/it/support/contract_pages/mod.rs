//! Support of the contract-page guards: the walk of a committed schema, the renderer of a page and
//! of the index, the frame check and the version blocks. The schema pages are read from the
//! committed schema files and the three manifests, never from the Rust types; the harness pages
//! are read from the trait definitions and the binary's driver registry.

pub mod capabilities;
pub mod config_pages;
pub mod frame;
pub mod hand;
pub mod harness;
pub mod index;
pub mod pages;
pub mod refusals;
pub mod schema;
pub mod versions;

use std::fs;
use std::path::{Path, PathBuf};

use candler_harness::spec::{AgentSetting, Answer, EffortLevel, InstructionScope, PermissionMode};
use candler_schemas::servers::McpServerKind;

/// The environment variable that makes the guards write the pages instead of comparing them.
pub const BLESS_ENV: &str = "CANDLER_BLESS_CONTRACT";

/// The workspace root.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The committed schema files.
pub fn schema_dir() -> PathBuf {
    repo_root().join("crates/candler-schemas/schema")
}

/// The contract pages.
pub fn docs_dir() -> PathBuf {
    repo_root().join("docs/contract")
}

/// Whether the guards write instead of compare.
pub fn blessing() -> bool {
    std::env::var(BLESS_ENV).is_ok_and(|value| value == "1")
}

/// Writes the file through a rename, so a guard reading it in another process never sees half.
pub fn write_atomically(path: &Path, text: &str) -> Result<(), String> {
    let temporary = path.with_extension(format!("md.{}.tmp", std::process::id()));
    fs::write(&temporary, text).map_err(|e| format!("{}: {e}", temporary.display()))?;
    fs::rename(&temporary, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The words of a driver's answer, as a page writes them: the construct, the Candler action behind
/// `Candler: `, or the refusal behind `no (…)`. The one producer every per-driver cell reads.
#[must_use]
pub fn answer_words(answer: Answer) -> String {
    match answer {
        Answer::Mapped { construct } => construct.to_owned(), // BORROW: .to_owned() makes the owned cell text
        Answer::Candler { how } => format!("Candler: {how}"),
        Answer::Refused { reason } => format!("no ({reason})"),
    }
}

/// The label of the row an agent setting words.
#[must_use]
pub const fn setting_label(setting: AgentSetting) -> &'static str {
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
        AgentSetting::Instructions(InstructionScope::Global) => "instructions: global",
        AgentSetting::Instructions(InstructionScope::Workspace) => "instructions: workspace",
        AgentSetting::Hooks => "hooks",
    }
}
