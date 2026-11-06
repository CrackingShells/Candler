//! What output discovery leaves out of the files a run collects.

/// The directories discovery never collects from.
pub const EXCLUDED_DIRS: &[&str] = &[".claude"];

/// The glob patterns of the paths discovery never collects.
#[must_use]
pub fn exclusion_patterns() -> Vec<String> {
    EXCLUDED_DIRS
        .iter()
        .map(|dir| format!("{dir}/**"))
        .collect()
}
