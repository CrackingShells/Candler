//! The version numbers of the three manifests, and the `<!-- versions -->` blocks that quote them.

use std::fs;
use std::path::Path;

use regex::Regex;

/// The opening marker of a block of version numbers.
pub const OPEN: &str = "<!-- versions -->";
/// The closing marker of a block of version numbers.
pub const CLOSE: &str = "<!-- /versions -->";

/// The three release lines, read from their manifests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Versions {
    /// The workspace `version` of `Cargo.toml`: the core line, tagged `v<version>`.
    pub core: String,
    /// The `version` of `crates/candler-schemas/Cargo.toml`: tagged `schemas@<version>`.
    pub schemas: String,
    /// The `version` of `python/candler-evaluator/pyproject.toml`: `candler-evaluator@<version>`.
    pub evaluator: String,
}

impl Versions {
    /// Reads the manifests under the workspace root.
    pub fn read(root: &Path) -> Result<Versions, String> {
        let read = |file: &str, keys: &[&str]| -> Result<String, String> {
            let path = root.join(file);
            let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let document: toml::Value =
                toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            let mut value = &document;
            for key in keys {
                value = value
                    .get(*key)
                    .ok_or_else(|| format!("{file}: no `{key}`"))?;
            }
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{file}: the version is not a string"))
        };
        Ok(Versions {
            core: read("Cargo.toml", &["workspace", "package", "version"])?,
            schemas: read("crates/candler-schemas/Cargo.toml", &["package", "version"])?,
            evaluator: read(
                "python/candler-evaluator/pyproject.toml",
                &["project", "version"],
            )?,
        })
    }

    /// The code span of a line's tag: `` `v0.6.0` ``, `` `schemas@0.6.0` ``.
    pub fn tag(&self, line: &str) -> Option<String> {
        match line {
            "v" => Some(format!("`v{}`", self.core)),
            "schemas@" => Some(format!("`schemas@{}`", self.schemas)),
            "candler-evaluator@" => Some(format!("`candler-evaluator@{}`", self.evaluator)),
            _ => None,
        }
    }
}

fn token() -> Result<Regex, String> {
    Regex::new(r"`(v|schemas@|candler-evaluator@)(\d+\.\d+\.\d+[0-9A-Za-z.+-]*)`")
        .map_err(|e| e.to_string())
}

/// Every version token of the text, as (line, version), in order.
pub fn tokens(text: &str) -> Result<Vec<(String, String)>, String> {
    let re = token()?;
    Ok(re
        .captures_iter(text)
        .filter_map(|c| Some((c.get(1)?.as_str().to_owned(), c.get(2)?.as_str().to_owned())))
        .collect())
}

/// The text between the markers of every block, or an error for a marker with no partner.
pub fn blocks(text: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(OPEN) {
        let after = rest.get(start + OPEN.len()..).unwrap_or_default();
        let end = after
            .find(CLOSE)
            .ok_or_else(|| "a `<!-- versions -->` block is never closed".to_owned())?;
        let inside = after.get(..end).unwrap_or_default();
        if inside.contains(OPEN) {
            return Err("a `<!-- versions -->` block opens inside another".to_owned());
        }
        out.push(inside.to_owned());
        rest = after.get(end + CLOSE.len()..).unwrap_or_default();
    }
    if rest.contains(CLOSE) {
        return Err("a `<!-- /versions -->` marker has no opening".to_owned());
    }
    Ok(out)
}

/// The tokens that sit outside every block of the text.
pub fn tokens_outside_blocks(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut outside = String::new();
    let mut rest = text;
    while let Some(start) = rest.find(OPEN) {
        outside.push_str(rest.get(..start).unwrap_or_default());
        let after = rest.get(start..).unwrap_or_default();
        let end = after
            .find(CLOSE)
            .ok_or_else(|| "a `<!-- versions -->` block is never closed".to_owned())?;
        rest = after.get(end + CLOSE.len()..).unwrap_or_default();
    }
    outside.push_str(rest);
    tokens(&outside)
}

/// Whether every token inside the blocks of `text` equals its manifest's version.
pub fn check_blocks(text: &str, versions: &Versions) -> Result<(), String> {
    for block in blocks(text)? {
        for (line, found) in tokens(&block)? {
            let expected = match line.as_str() {
                "v" => &versions.core,
                "schemas@" => &versions.schemas,
                _ => &versions.evaluator,
            };
            if &found != expected {
                return Err(format!(
                    "`{line}{found}` is stale, the manifest says {expected}"
                ));
            }
        }
    }
    Ok(())
}
