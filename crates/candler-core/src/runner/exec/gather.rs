//! What the core reads off a finished attempt: the files it produced, what the trace says it
//! spent, and what the reader made of its output.

use std::fs;
use std::io;
use std::path::Path;

use candler_harness::spec::CollectSpec;
use candler_schemas::outcome::Finite;
use candler_schemas::trace::{EventKind, SessionEvent, SessionId, UsageScope};
use regex::Regex;

use super::ends::TraceFacts;
use crate::runner::Usage;
use crate::runner::evidence::OutputDigest;
use crate::store::blob::digest_of_file;
use candler_schemas::primitives::Digest;

/// The tokens and cost a trace reports.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[non_exhaustive]
pub struct Spent {
    /// Input tokens, cache reads and writes excluded.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// The cost in US dollars, when the harness reports it.
    pub cost_usd: f64,
}

impl Spent {
    /// The usage the job's meter takes: input and output tokens, and the cost.
    #[must_use]
    pub const fn usage(&self) -> Usage {
        Usage {
            tokens: self.input.saturating_add(self.output),
            cost_usd: self.cost_usd,
        }
    }

    /// What the reader saw of the run, as the ends table reads it.
    #[must_use]
    pub const fn facts_tokens(&self) -> u64 {
        self.output
    }
}

/// One session's counts: its last running total, else the sum of its messages.
#[derive(Default)]
struct Counts {
    total: Option<Spent>,
    messages: Spent,
}

/// The tokens and cost of `events`. A session counts by its last running total when it reports
/// one, and by the sum of its per-message usage otherwise; sessions add up.
#[must_use]
pub fn spent(events: &[SessionEvent]) -> Spent {
    let mut sessions: Vec<(&SessionId, Counts)> = Vec::new();
    for entry in events {
        let EventKind::Usage {
            scope,
            input_tokens,
            output_tokens,
            cost_usd,
            ..
        } = &entry.event.kind
        else {
            continue;
        };
        let at = sessions
            .iter()
            .position(|(id, _)| **id == entry.session)
            .unwrap_or_else(|| {
                sessions.push((&entry.session, Counts::default()));
                sessions.len().saturating_sub(1)
            });
        let Some((_, counts)) = sessions.get_mut(at) else {
            continue;
        };
        let seen = Spent {
            input: *input_tokens,
            output: *output_tokens,
            cost_usd: cost_usd.map_or(0.0, Finite::get),
        };
        if matches!(scope, UsageScope::SessionTotal) {
            counts.total = Some(seen);
        } else {
            counts.messages = Spent {
                input: counts.messages.input.saturating_add(seen.input),
                output: counts.messages.output.saturating_add(seen.output),
                cost_usd: counts.messages.cost_usd + seen.cost_usd,
            };
        }
    }
    sessions
        .into_iter()
        .fold(Spent::default(), |sum, (_, counts)| {
            let one = counts.total.unwrap_or(counts.messages);
            Spent {
                input: sum.input.saturating_add(one.input),
                output: sum.output.saturating_add(one.output),
                cost_usd: sum.cost_usd + one.cost_usd,
            }
        })
}

/// The trace facts of a reader that finished with `sessions` ended as `ended_by`.
#[must_use]
pub const fn read_facts(ended_by: candler_schemas::trace::EndedBy, spent: &Spent) -> TraceFacts {
    TraceFacts::Read {
        ended_by,
        tokens: spent.facts_tokens(),
    }
}

/// The regular expression a glob stands for: `**/` is any directories, `**` anything, `*` anything
/// but a slash, `?` one character but a slash.
///
/// # Errors
/// The regex error when the pattern cannot be compiled.
pub fn glob_regex(glob: &str) -> Result<Regex, regex::Error> {
    let mut pattern = String::from("^");
    let mut chars = glob.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                if chars.peek() == Some(&'/') {
                    chars.next();
                    pattern.push_str("(?:.*/)?");
                } else {
                    pattern.push_str(".*");
                }
            }
            '*' => pattern.push_str("[^/]*"),
            '?' => pattern.push_str("[^/]"),
            other => pattern.push_str(&regex::escape(&other.to_string())),
        }
    }
    pattern.push('$');
    Regex::new(&pattern)
}

/// The files under `root` that the file specs pick, by relative path, each with its digest. A
/// spec whose glob starts with `!` leaves out what it matches. Links are not followed. Command
/// specs name streams the harness collects itself and pick no file.
///
/// # Errors
/// An I/O error when the tree cannot be read, and an invalid-input error for a bad glob or a
/// path that is not text.
pub fn collect_files(root: &Path, specs: &[CollectSpec]) -> io::Result<Vec<OutputDigest>> {
    let bad = |error: regex::Error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string());
    let mut keep: Vec<Regex> = Vec::new();
    let mut drop: Vec<Regex> = Vec::new();
    for spec in specs {
        if let CollectSpec::File { glob, .. } = spec {
            match glob.strip_prefix('!') {
                Some(rest) => drop.push(glob_regex(rest).map_err(bad)?),
                None => keep.push(glob_regex(glob).map_err(bad)?),
            }
        }
    }
    let mut found: Vec<(String, Digest)> = Vec::new();
    walk(root, "", &mut |path, file| {
        if keep.iter().any(|re| re.is_match(path)) && !drop.iter().any(|re| re.is_match(path)) {
            let digest = digest_of_file(file).map_err(|e| io::Error::other(e.to_string()))?;
            found.push((path.to_owned(), digest)); // BORROW: the list owns its paths
        }
        Ok(())
    })?;
    found.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(found
        .into_iter()
        .map(|(path, digest)| OutputDigest { path, digest })
        .collect())
}

/// Calls `each` with the relative path and the file of every regular file under `dir`.
fn walk(
    dir: &Path,
    prefix: &str,
    each: &mut dyn FnMut(&str, &Path) -> io::Result<()>, // TRAIT_OBJECT: the visitor is a closure, held by reference to keep the recursion non-generic
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "a file name is not text")
        })?;
        let rel = if prefix.is_empty() {
            name.to_owned() // BORROW: the path is built from the name
        } else {
            format!("{prefix}/{name}")
        };
        if kind.is_dir() {
            walk(&entry.path(), &rel, each)?;
        } else if kind.is_file() {
            each(&rel, &entry.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_forms() {
        let all = glob_regex("**/*").expect("glob");
        assert!(all.is_match("a") && all.is_match("a/b/c.txt"));
        let hidden = glob_regex("**/.claude/**").expect("glob");
        assert!(hidden.is_match("x/.claude/s.json") && hidden.is_match(".claude/s.json"));
        assert!(!hidden.is_match("x/claude/s.json"));
        let top = glob_regex(".claude/**").expect("glob");
        assert!(top.is_match(".claude/a/b") && !top.is_match("x/.claude/a"));
    }
}
