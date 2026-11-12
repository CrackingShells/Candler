//! The file rule: which of a run's stored `outputs/` files a context carries.
//!
//! A file is an artifact when an artifact glob matches `outputs/<path>` and every component of
//! `<path>` that starts with `.` is named literally by that same glob. The rule therefore drops
//! `.cache/`, `AppleDouble` `._*` files and `.DS_Store` under a glob such as `outputs/**`, and
//! keeps a dot path only for a glob that spells the dot component out, as in `outputs/.env`.

use candler_schemas::primitives::RelPath;
use globset::{GlobBuilder, GlobMatcher};

use super::ContextError;

/// The directory of a run that artifact globs are written against.
const OUTPUTS: &str = "outputs";

/// One compiled artifact glob and the segments of it that name a path component literally.
#[derive(Debug, Clone)]
struct Rule {
    matcher: GlobMatcher,
    literal: Vec<String>,
}

/// The file rule over a set of artifact globs.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct FileRule {
    rules: Vec<Rule>,
}

/// Reports whether a glob segment has no glob syntax, so that it names one component exactly.
fn is_literal(segment: &str) -> bool {
    !segment.contains(['*', '?', '[', ']', '{', '}', '\\', '!'])
}

impl FileRule {
    /// Compiles the artifact globs of a run's needs. A `*` does not cross `/` and a `**` does.
    ///
    /// # Errors
    /// Returns [`ContextError::Glob`] for a glob that does not compile.
    pub fn new(globs: &[String]) -> Result<Self, ContextError> {
        let mut rules = Vec::with_capacity(globs.len());
        for glob in globs {
            let compiled = GlobBuilder::new(glob.as_str()) // BORROW: explicit .as_str()
                .literal_separator(true)
                .build()
                .map_err(|e| ContextError::Glob {
                    glob: glob.clone(), // BORROW: the error owns the text
                    detail: e.to_string(),
                })?;
            let literal = glob
                .split('/')
                .filter(|segment| is_literal(segment))
                .map(str::to_owned) // BORROW: the rule owns its segments
                .collect();
            rules.push(Rule {
                matcher: compiled.compile_matcher(),
                literal,
            });
        }
        Ok(Self { rules })
    }

    /// Reports whether the rule keeps the file at `path`, relative to `outputs/`.
    #[must_use]
    pub fn keeps(&self, path: &RelPath) -> bool {
        let full = format!("{OUTPUTS}/{path}");
        self.rules.iter().any(|rule| {
            rule.matcher.is_match(full.as_str()) // BORROW: explicit .as_str()
                && path
                    .as_str() // BORROW: explicit .as_str()
                    .split('/')
                    .filter(|component| component.starts_with('.'))
                    .all(|component| rule.literal.iter().any(|named| named == component))
        })
    }
}

/// Reports whether the file rule of `globs` keeps the file at `path`, relative to `outputs/`.
///
/// # Errors
/// Returns [`ContextError::Glob`] for a glob that does not compile.
pub fn file_rule_keeps(globs: &[String], path: &RelPath) -> Result<bool, ContextError> {
    Ok(FileRule::new(globs)?.keeps(path))
}
