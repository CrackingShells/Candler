//! What output discovery leaves out of the files a run collects.
//!
//! [`ClaudeCodeDriver::collect`](crate::driver::ClaudeCodeDriver) hands the core these exclusions
//! as globs; a glob that starts with `!` leaves out the paths it matches.

/// The directories discovery never collects from, at any depth.
pub const EXCLUDED_DIRS: &[&str] = &[".claude"];

/// The glob patterns of the paths discovery never collects: each directory of [`EXCLUDED_DIRS`] at
/// the top of the workspace and at any depth.
#[must_use]
pub fn exclusion_patterns() -> Vec<String> {
    let mut patterns = Vec::new();
    for dir in EXCLUDED_DIRS {
        patterns.push(format!("{dir}/**"));
        patterns.push(format!("**/{dir}/**"));
    }
    patterns
}
