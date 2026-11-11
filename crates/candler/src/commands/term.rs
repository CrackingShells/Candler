//! What every verb shares about the terminal: when colour is on, and the few ways to paint.
//!
//! Colour is on only when standard output is a terminal and `NO_COLOR` is unset or empty
//! (no-color.org: a non-empty value turns colour off). There is no flag for it. [`Paint`] is the
//! only place an escape byte is written.

use std::ffi::OsStr;
use std::io::IsTerminal as _;

/// Whether colour is on for a terminal or a pipe and the value of `NO_COLOR`, which is `None` when
/// the variable is unset.
#[must_use]
pub fn color_enabled(stdout_is_terminal: bool, no_color: Option<&OsStr>) -> bool {
    stdout_is_terminal && no_color.is_none_or(OsStr::is_empty)
}

/// Whether colour is on for this process: standard output is a terminal and `NO_COLOR` is unset
/// or empty.
#[must_use]
pub fn stdout_color() -> bool {
    color_enabled(
        std::io::stdout().is_terminal(),
        std::env::var_os("NO_COLOR").as_deref(),
    )
}

/// Wraps text in the colour codes when colour is on, and returns it unchanged when it is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paint {
    on: bool,
}

impl Paint {
    /// A painter that colours when `on` is true.
    #[must_use]
    pub const fn new(on: bool) -> Self {
        Self { on }
    }

    /// A painter that follows the terminal and `NO_COLOR`.
    #[must_use]
    pub fn detect() -> Self {
        Self::new(stdout_color())
    }

    fn wrap(self, code: &str, text: &str) -> String {
        if self.on {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned() // BORROW: the caller owns the text it prints
        }
    }

    /// `text` as a line that replaces the one on the terminal: a return to the start of the line,
    /// and the old line cleared. With colour off the old line is overwritten with spaces.
    #[must_use]
    pub fn rewrite(self, text: &str) -> String {
        if self.on {
            format!("\r\x1b[2K{}", self.bold(text))
        } else {
            format!("\r{text:<72}")
        }
    }

    /// `text` in bold.
    #[must_use]
    pub fn bold(self, text: &str) -> String {
        self.wrap("1", text)
    }

    /// `text` dimmed.
    #[must_use]
    pub fn dim(self, text: &str) -> String {
        self.wrap("2", text)
    }

    /// `text` in red.
    #[must_use]
    pub fn red(self, text: &str) -> String {
        self.wrap("31", text)
    }

    /// `text` in green.
    #[must_use]
    pub fn green(self, text: &str) -> String {
        self.wrap("32", text)
    }
}
