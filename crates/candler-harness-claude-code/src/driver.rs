//! The Claude Code harness driver: the command line, the stop spec and the exit classification.

/// The driver of Claude Code.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ClaudeCodeDriver;

impl ClaudeCodeDriver {
    /// A driver.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}
