//! Resolves the inputs an experiment declares: walks each one, stores every file and the list of
//! files as blobs, and tells the agent's inputs from the evaluators'.
//!
//! An input is a file or a directory found relative to the experiment file. Its files form a list
//! of `{path, sha256}` entries, sorted by path; the RFC 8785 text of the list is a blob and the
//! blob's key is the input's digest. The agent's inputs are mounted file by file, each from its
//! blob, under the input's `at` directory. An evaluator-only input is stored and listed, and
//! never given a mount.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr as _;

use candler_schemas::canonical::to_jcs;
use candler_schemas::execution::{
    DigestRef, ExecutionError, ExperimentInput, InputAudience, InputEntry, input_list_digest,
};
use candler_schemas::primitives::{FormatError, RelPath};

use crate::runner::evidence::InputDigest;
use crate::store::{Store, StoreError};

/// What a declared set of inputs resolved to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedInputs {
    /// The agent's inputs as `{kind, digest}`, sorted by kind.
    pub agent: Vec<DigestRef>,
    /// The file of every agent input at its workspace path, with the digest of its bytes.
    pub mounts: Vec<InputDigest>,
    /// The evaluator-only inputs as `{kind, digest}`, sorted by kind.
    pub evaluators: Vec<DigestRef>,
}

/// Why the declared inputs were refused.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InputError {
    /// An input cannot be used as declared.
    #[error("input `{kind}` ({path}) {problem}")]
    Refused {
        /// The input's kind.
        kind: String,
        /// The input's path as written, or the path of the entry at fault.
        path: String,
        /// What is wrong.
        problem: String,
    },
    /// A file or a list could not be stored.
    #[error("input `{kind}` ({path}) cannot be stored: {source}")]
    Store {
        /// The input's kind.
        kind: String,
        /// The file or list at fault.
        path: String,
        /// The store's error.
        source: StoreError,
    },
}

/// Why a name is refused: a mount option cannot carry it.
const UNMOUNTABLE: &str = "has a name with a comma or a line break, which a mount cannot carry";

fn refused(input: &ExperimentInput, path: &str, problem: impl Into<String>) -> InputError {
    InputError::Refused {
        kind: input.kind().to_string(),
        // BORROW: .to_owned() gives the error its own path text
        path: path.to_owned(),
        problem: problem.into(),
    }
}

/// Hashes and stores every input of an experiment.
///
/// # Errors
/// [`InputError::Refused`] naming the input's kind and path when two inputs share a kind, a path
/// is empty, absolute or holds `..`, a path is missing or is not a regular file or directory, a
/// symlink is met, a directory holds no file a file name is not a plain relative path, or two
/// agent files would be mounted at one workspace path or one inside the other;
/// [`InputError::Store`] when a blob cannot be written.
pub fn resolve_inputs(
    store: &Store,
    inputs: &[ExperimentInput],
    experiment_dir: &Path,
) -> Result<ResolvedInputs, InputError> {
    let mut resolved = ResolvedInputs::default();
    let mut targets: Vec<(String, String)> = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        if inputs
            .iter()
            .take(index)
            .any(|earlier| earlier.kind() == input.kind())
        {
            return Err(refused(input, input.path(), "is declared twice"));
        }
        if let Some(at) = input.at().filter(|at| unmountable(at)) {
            return Err(refused(
                input,
                at,
                "mounts under a directory whose name holds a comma or a line break",
            ));
        }
        let files = walk(input, experiment_dir)?;
        let (digest, entries) = store_input(store, input, &files)?;
        let reference = DigestRef::new(input.kind().clone(), digest); // BORROW: the reference owns its kind
        match input.audience() {
            InputAudience::Agent => {
                let prefix = at_prefix(input.at());
                for entry in &entries {
                    let target = format!("{prefix}{}", entry.path);
                    targets.push((target.clone(), input.kind().to_string())); // BORROW: the list owns its texts
                    resolved.mounts.push(InputDigest {
                        path: target,
                        digest: entry.sha256.clone(), // BORROW: the mount owns its digest
                    });
                }
                resolved.agent.push(reference);
            }
            InputAudience::Evaluators => resolved.evaluators.push(reference),
            // EXHAUSTIVE: the wire's enum is non-exhaustive; an audience this reader does not know has no place to go
            _ => {
                return Err(refused(
                    input,
                    input.path(),
                    "has an audience this build does not know",
                ));
            }
        }
    }
    check_targets(inputs, &mut targets)?;
    resolved.agent.sort();
    resolved.evaluators.sort();
    Ok(resolved)
}

/// One file of an input, found on disk and stored.
struct Found {
    /// The path relative to the input's root.
    relative: String,
    /// Where the file is.
    source: PathBuf,
}

/// Whether `text` holds a character that `--mount` cannot carry: a comma, a line break or NUL.
fn unmountable(text: &str) -> bool {
    text.contains([',', '\n', '\r', '\0'])
}

/// The `at` directory as a prefix of a workspace path: empty for `.`, else `dir/`.
fn at_prefix(at: Option<&str>) -> String {
    let parts: Vec<String> = Path::new(at.unwrap_or("."))
        .components()
        .filter_map(|part| match part {
            Component::Normal(text) => text.to_str().map(str::to_owned),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => None,
        })
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("{}/", parts.join("/"))
    }
}

/// Refuses two agent files mounted at one workspace path, or one under the path of another.
fn check_targets(
    inputs: &[ExperimentInput],
    targets: &mut [(String, String)],
) -> Result<(), InputError> {
    targets.sort();
    for pair in targets.windows(2) {
        if let [(first, first_kind), (second, second_kind)] = pair {
            let clash = first == second || second.starts_with(&format!("{first}/"));
            if clash
                && let Some(input) = inputs
                    .iter()
                    .find(|i| i.kind().as_str() == second_kind.as_str())
            {
                return Err(refused(
                    input,
                    second,
                    format!("is mounted where input `{first_kind}` is mounted ({first})"),
                ));
            }
        }
    }
    Ok(())
}

/// Lists the files of an input in path order, refusing what cannot be an input.
fn walk(input: &ExperimentInput, experiment_dir: &Path) -> Result<Vec<Found>, InputError> {
    let written = input.path();
    if written.is_empty() {
        return Err(refused(input, written, "has an empty path"));
    }
    if written.starts_with(['/', '\\']) || Path::new(written).is_absolute() {
        return Err(refused(input, written, "has an absolute path"));
    }
    if written.split(['/', '\\']).any(|segment| segment == "..") {
        return Err(refused(
            input,
            written,
            "has a path that leaves the experiment's directory",
        ));
    }
    let root = experiment_dir.join(written);
    let meta = fs::symlink_metadata(&root).map_err(|error| {
        let problem = if error.kind() == std::io::ErrorKind::NotFound {
            "does not exist".to_owned()
        } else {
            format!("cannot be read: {error}")
        };
        refused(input, written, problem)
    })?;
    if meta.file_type().is_symlink() {
        return Err(refused(input, written, "is a symlink"));
    }
    let mut found = Vec::new();
    if meta.is_file() {
        let file_name = root
            .file_name()
            .and_then(|text| text.to_str())
            .ok_or_else(|| refused(input, written, "has a file name that is not UTF-8"))?;
        if unmountable(file_name) {
            return Err(refused(input, written, UNMOUNTABLE));
        }
        found.push(Found {
            relative: file_name.to_owned(), // BORROW: the entry owns its path
            source: root,
        });
    } else if meta.is_dir() {
        walk_dir(input, &root, "", &mut found)?;
        if found.is_empty() {
            return Err(refused(input, written, "is a directory that holds no file"));
        }
    } else {
        return Err(refused(input, written, "is neither a file nor a directory"));
    }
    found.sort_by(|first, second| first.relative.cmp(&second.relative));
    Ok(found)
}

/// Adds the files under `dir`, whose path relative to the input's root is `prefix`.
fn walk_dir(
    input: &ExperimentInput,
    dir: &Path,
    prefix: &str,
    found: &mut Vec<Found>,
) -> Result<(), InputError> {
    let listing = fs::read_dir(dir).map_err(|error| {
        refused(
            input,
            &dir.display().to_string(),
            format!("cannot be read: {error}"),
        )
    })?;
    for entry in listing {
        let entry = entry.map_err(|error| {
            refused(
                input,
                &dir.display().to_string(),
                format!("cannot be read: {error}"),
            )
        })?;
        let file_name = entry.file_name();
        let text = file_name.to_str().ok_or_else(|| {
            refused(
                input,
                &entry.path().display().to_string(),
                "has a name that is not UTF-8",
            )
        })?;
        let relative = format!("{prefix}{text}");
        if unmountable(text) {
            return Err(refused(input, &relative, UNMOUNTABLE));
        }
        let kind = entry
            .file_type()
            .map_err(|error| refused(input, &relative, format!("cannot be read: {error}")))?;
        if kind.is_symlink() {
            return Err(refused(input, &relative, "is a symlink inside the input"));
        } else if kind.is_dir() {
            walk_dir(input, &entry.path(), &format!("{relative}/"), found)?;
        } else if kind.is_file() {
            found.push(Found {
                relative,
                source: entry.path(),
            });
        } else {
            return Err(refused(
                input,
                &relative,
                "is neither a file nor a directory",
            ));
        }
    }
    Ok(())
}

/// Stores the files of an input and its list, and returns the list's digest and entries.
fn store_input(
    store: &Store,
    input: &ExperimentInput,
    files: &[Found],
) -> Result<(candler_schemas::primitives::Digest, Vec<InputEntry>), InputError> {
    let stored = |path: &str, source: StoreError| InputError::Store {
        kind: input.kind().to_string(),
        // BORROW: .to_owned() gives the error its own path text
        path: path.to_owned(),
        source,
    };
    let mut entries = Vec::with_capacity(files.len());
    for file in files {
        let digest = store
            .put_blob_from_path(&file.source, None)
            .map_err(|error| stored(&file.relative, error))?;
        let path = RelPath::from_str(&file.relative)
            .map_err(|error: FormatError| refused(input, &file.relative, error.to_string()))?;
        entries.push(InputEntry::new(path, digest));
    }
    entries.sort_by(|first, second| first.path.cmp(&second.path));
    let digest = input_list_digest(&entries)
        .map_err(|error: ExecutionError| refused(input, input.path(), error.to_string()))?;
    let list = entries
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| refused(input, input.path(), error.to_string()))?;
    let text = to_jcs(&serde_json::Value::Array(list))
        .map_err(|error| refused(input, input.path(), error.to_string()))?;
    store
        .put_blob(text.as_bytes(), Some(&digest))
        .map_err(|error| stored(input.path(), error))?;
    Ok((digest, entries))
}
