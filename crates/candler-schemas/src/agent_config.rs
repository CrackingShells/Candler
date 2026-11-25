//! The instruction texts and hooks an experiment declares for the agent, in the shape the drivers
//! read them.
//!
//! An experiment writes them as tables at `instructions.global` and `hooks`, in `[fixed]` or as
//! factor levels, and the keys below those two paths are the harness's own words: a harness id
//! such as `claude-code`, a hook event such as `SessionStart`, a file name such as `start.sh`
//! (see [`crate::execution::TABLE_VALUED_PATHS`]). The reader (`candler_harness::agent_config`)
//! reads the tables as the types of this module, each published as its own schema as
//! `mcp-server` is, and refuses what does not fit them.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::generate::GeneratedSchema;
use crate::primitives::RelPath;

/// The wire root schemas this module publishes.
#[must_use]
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    vec![
        GeneratedSchema::of::<Instructions>("instructions"),
        GeneratedSchema::of::<ClaudeCodeHooks>("claude-code-hooks"),
        GeneratedSchema::of::<OpenCodeHooks>("opencode-hooks"),
    ]
}

/// The instruction texts of a cell: one global text per harness and one text for the workspace.
///
/// The global text is the one the harness reads from the agent's home; the workspace text is the
/// one it reads at the root of the workspace. Their contents are free.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Instructions {
    /// The global text of each harness, by harness id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub global: BTreeMap<String, String>,
    /// The text at the root of the workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}

/// The hooks a cell gives Claude Code: the `hooks` configuration and the files it runs.
///
/// Their contents are free; Claude Code judges them.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ClaudeCodeHooks {
    /// The object that goes under the `hooks` key of `.claude/settings.json`, verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<Map<String, Value>>,
    /// The hook files, each at its path under `.claude/hooks/`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<RelPath, String>,
}

/// The hooks a cell gives `OpenCode`: its plugins, which are code.
///
/// Their contents are free; `OpenCode` judges them.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct OpenCodeHooks {
    /// The plugin files, each by its file name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plugins: BTreeMap<RelPath, String>,
}
