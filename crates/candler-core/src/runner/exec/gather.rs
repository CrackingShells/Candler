//! What the core reads off a finished attempt: the files it produced, what the trace says it
//! spent, and what the reader made of its output.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use candler_harness::reader::{FrameIn, ReaderError, TraceReader};
use candler_harness::spec::{
    CollectSpec, CommandSpec, EnvValue, FileRoot, Framing, Observed, PreparedFile,
};
use candler_schemas::outcome::Finite;
use candler_schemas::raw::{Clock, RawRef, StreamName};
use candler_schemas::trace::{EventKind, Session, SessionEvent, SessionId, UsageScope};
use regex::Regex;
use serde_json::{Value, json};

use super::ends::TraceFacts;
use crate::frames::{FrameReader, FrameWriter, RawFileInfo};
use crate::isolation::model::ContainerPlan;
use crate::isolation::{Isolation, IsolationError};
use crate::runner::Usage;
use crate::runner::evidence::{Inputs, OutputDigest};
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

/// What one collect command printed, under the stream name its spec gives.
#[derive(Debug)]
#[non_exhaustive]
pub struct Collected {
    /// The stream the output is archived as.
    pub name: StreamName,
    /// The standard output of the command, or why it could not be had.
    pub output: Result<Vec<u8>, IsolationError>,
}

/// Runs each command the driver's collect specs name, inside the run's isolation, in order, and
/// returns what each printed. A failure is that command's own result, never raised: the others
/// still run. Specs that name files, or a command for the host, pick nothing here.
pub async fn collect_commands(
    isolation: &dyn Isolation, // TRAIT_OBJECT: the isolation is docker, or a fake in tests
    plan: &ContainerPlan,
    specs: &[CollectSpec],
) -> Vec<Collected> {
    let mut found = Vec::new();
    for spec in specs {
        if let CollectSpec::Exec { spec, name, .. } = spec {
            found.push(Collected {
                name: name.clone(), // BORROW: the result owns the stream name
                output: isolation.collect_exec(plan, spec).await,
            });
        }
    }
    found
}

/// A stream stored as a raw file and read by a reader.
#[non_exhaustive]
pub struct StreamRead {
    /// The raw file the bytes were stored as.
    pub file: RawFileInfo,
    /// What the reader saw of the stored frames.
    pub observed: Observed,
    /// The identity the reader names in a trace header.
    pub reader: candler_schemas::primitives::NameVersion,
    /// The sessions and events the reader made of the stream.
    pub trace: Result<(Vec<Session>, Vec<SessionEvent>), ReaderError>,
}

/// Frames `bytes` as the stream `name` into `dir` the way `framing` says, and returns the file.
///
/// # Errors
/// The I/O error of writing the frame file.
pub fn store_stream(
    dir: &Path,
    name: &StreamName,
    framing: Framing,
    bytes: &[u8],
) -> io::Result<RawFileInfo> {
    let mut writer = FrameWriter::create(dir, name.clone(), framing, Clock::Unrecorded, None)?; // BORROW: the writer owns the stream name
    writer.write(bytes, None)?;
    writer.finish()
}

/// Stores `bytes` as the stream `name` in `dir`, then reads the stored frames, and only those,
/// with `reader`: what the reader makes is a function of the raw file.
///
/// # Errors
/// The I/O error of storing or reading back the frame file. A frame the reader cannot read is the
/// reader's own result in [`StreamRead::trace`].
pub fn read_stream(
    dir: &Path,
    name: &StreamName,
    bytes: &[u8],
    mut reader: Box<dyn TraceReader>, // TRAIT_OBJECT: the reader is the driver's
) -> io::Result<StreamRead> {
    let file = store_stream(dir, name, reader.framing(), bytes)?;
    let frames = FrameReader::open(&file.path).map_err(io::Error::other)?;
    let identity = reader.id();
    let mut events: Vec<SessionEvent> = Vec::new();
    let mut failure: Option<ReaderError> = None;
    for (index, frame) in frames.enumerate() {
        let frame = frame.map_err(io::Error::other)?;
        let at = RawRef::new(name.clone(), u64::try_from(index).unwrap_or(u64::MAX)); // BORROW: the reference owns the stream name
        match reader.feed(FrameIn::new(at, None, &frame)) {
            Ok(mut made) => events.append(&mut made),
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
    }
    let observed = reader.observed();
    let trace = match failure {
        Some(error) => Err(error),
        None => reader.finish().map(|sessions| (sessions, events)),
    };
    Ok(StreamRead {
        file,
        observed,
        reader: identity,
        trace,
    })
}

/// The name a prepared file's root is recorded under.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a root this core does not know is the workspace, as the writer takes it"
)]
const fn root_label(root: FileRoot) -> &'static str {
    match root {
        FileRoot::Home => "home",
        _ => "workspace",
    }
}

/// The record of what the driver resolved from the cell: the harness argv and the names of the
/// variables its overlay sets, and each prepared file with its contents (as text when it is
/// text) and the digest of its bytes. It holds no value of a variable and no secret: a secret in
/// the overlay is a name, and a driver's file refers to secrets by the name of a variable.
#[must_use]
pub fn resolved_record(
    command: &CommandSpec,
    files: &[PreparedFile],
    digests: &BTreeMap<String, Digest>,
) -> Vec<u8> {
    let env: Vec<Value> = command
        .env_overlay()
        .iter()
        .map(|entry| {
            #[expect(
                clippy::wildcard_enum_match_arm,
                reason = "a value that is not a literal is a secret reference, or one no kind names"
            )]
            let kind = match &entry.value {
                EnvValue::Literal(_) => "literal",
                _ => "secret",
            };
            json!({"name": entry.name, "kind": kind})
        })
        .collect();
    let files: Vec<Value> = files
        .iter()
        .map(|file| {
            let digest = digests.get(file.path.as_str()).map(ToString::to_string); // BORROW: explicit .as_str()
            json!({
                "path": file.path.as_str(), // BORROW: explicit .as_str()
                "root": root_label(file.root),
                "executable": file.executable,
                "sha256": digest,
                "text": std::str::from_utf8(&file.contents).ok(),
            })
        })
        .collect();
    let record = json!({
        "argv": command.argv(),
        "cwd": command.cwd().map(ToString::to_string),
        "env": env,
        "files": files,
    });
    // EXPLICIT: a JSON value always serialises
    let mut bytes = serde_json::to_vec(&record).unwrap_or_default();
    bytes.push(b'\n');
    bytes
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

/// The files the run produced: [`collect_files`] without the mount point of any input. The mount
/// point of an input file is an empty file the host made in the workspace, so it is not an output.
///
/// # Errors
/// The errors of [`collect_files`].
pub fn collect_outputs(
    root: &Path,
    specs: &[CollectSpec],
    inputs: &Inputs,
) -> io::Result<Vec<OutputDigest>> {
    let mut found = collect_files(root, specs)?;
    found.retain(|out| inputs.digests.iter().all(|input| input.path != out.path));
    Ok(found)
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
        let hidden = glob_regex("**/.hidden/**").expect("glob");
        assert!(hidden.is_match("x/.hidden/s.json") && hidden.is_match(".hidden/s.json"));
        assert!(!hidden.is_match("x/hidden/s.json"));
        let top = glob_regex(".hidden/**").expect("glob");
        assert!(top.is_match(".hidden/a/b") && !top.is_match("x/.hidden/a"));
    }
}
