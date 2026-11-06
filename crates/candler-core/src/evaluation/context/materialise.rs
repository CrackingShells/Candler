//! Step two of a context: private copies of the artifacts, and the digest of what was written.

use std::fs::{self, OpenOptions, Permissions};
use std::io;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use candler_schemas::evaluation::{ContextArtifact, RunContext};

use super::ContextError;
use super::assemble::{Assembled, AssembledArtifact};
use crate::store::blob::digest_of_file;

/// Wraps a failed call on `path` as a [`ContextError::Io`] of `op`.
fn io_error<'a>(op: &'static str, path: &'a Path) -> impl FnOnce(io::Error) -> ContextError + 'a {
    move |source| ContextError::Io {
        op,
        path: path.to_path_buf(),
        source,
    }
}

/// The mode of a materialised file: nobody can write it.
const READ_ONLY: u32 = 0o444;
/// The mode of a materialised file that had an execute bit.
const READ_ONLY_EXEC: u32 = 0o555;
/// The mode a file has while it is being written, before it is made read-only.
const WRITING: u32 = 0o600;

/// A request's workspace: a directory outside the store with an `outputs/` that this call made.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Workspace {
    root: PathBuf,
}

/// The real path of `path`, resolving every symbolic link of its deepest existing ancestor.
fn resolved(path: &Path) -> Result<PathBuf, ContextError> {
    let mut tail = Vec::new();
    let mut head = path.to_path_buf();
    loop {
        match head.canonicalize() {
            Ok(mut real) => {
                for part in tail.iter().rev() {
                    real.push(part);
                }
                return Ok(real);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match (head.file_name(), head.parent()) {
                    (Some(name), Some(parent)) => {
                        tail.push(name.to_owned()); // BORROW: the tail owns its parts
                        head = parent.to_path_buf();
                    }
                    (None, _) | (_, None) => {
                        return Err(io_error("resolve the workspace", path)(error));
                    }
                }
            }
            Err(error) => return Err(io_error("resolve the workspace", &head)(error)),
        }
    }
}

impl Workspace {
    /// Prepares `root` as a workspace: it must be absolute, valid UTF-8 and outside the store at
    /// `store_root`, and the `outputs/` below it must not exist yet, so nothing but the
    /// materialised files can be there.
    ///
    /// # Errors
    /// Returns [`ContextError::Workspace`] for a relative, non-UTF-8 or in-store path and
    /// [`ContextError::Io`] when a directory cannot be made, which includes an existing `outputs/`.
    pub fn prepare(root: &Path, store_root: &Path) -> Result<Self, ContextError> {
        let refuse = |detail| ContextError::Workspace {
            path: root.to_path_buf(),
            detail,
        };
        if !root.is_absolute() {
            return Err(refuse("is not an absolute path"));
        }
        if root.to_str().is_none() {
            return Err(refuse("is not valid UTF-8"));
        }
        let store = resolved(store_root)?;
        if resolved(root)?.starts_with(&store) {
            return Err(refuse("lies inside the store"));
        }
        fs::create_dir_all(root).map_err(io_error("create the workspace", root))?;
        let workspace = Self {
            root: root.to_path_buf(),
        };
        let outputs = workspace.outputs_dir();
        fs::create_dir(&outputs).map_err(io_error("create the workspace outputs", &outputs))?;
        Ok(workspace)
    }

    /// The workspace directory.
    #[must_use]
    pub fn root<'a>(&'a self) -> &'a Path {
        self.root.as_path()
    }

    /// The directory the artifacts are copied under.
    #[must_use]
    pub fn outputs_dir(&self) -> PathBuf {
        self.root.join("outputs")
    }
}

/// Copies one artifact's blob to a new file under `outputs`, makes it read-only and returns the
/// digest-checked context entry for it.
fn copy_artifact(
    outputs: &Path,
    artifact: &AssembledArtifact,
) -> Result<ContextArtifact, ContextError> {
    let target = outputs.join(artifact.path.as_str()); // BORROW: explicit .as_str()
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(io_error("create an artifact directory", parent))?;
    }
    let mut source =
        fs::File::open(&artifact.blob).map_err(io_error("open a blob", &artifact.blob))?;
    // `create_new` refuses an existing name, a dangling link included, so the copy is a new inode.
    let mut copy = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(WRITING)
        .open(&target)
        .map_err(io_error("create an artifact copy", &target))?;
    io::copy(&mut source, &mut copy).map_err(io_error("copy an artifact", &target))?;
    copy.sync_all()
        .map_err(io_error("sync an artifact copy", &target))?;
    drop(copy);
    let mode = if artifact.executable {
        READ_ONLY_EXEC
    } else {
        READ_ONLY
    };
    fs::set_permissions(&target, Permissions::from_mode(mode))
        .map_err(io_error("make an artifact read-only", &target))?;
    let written = digest_of_file(&target)?;
    if written != artifact.sha256 {
        return Err(ContextError::CorruptBlob {
            path: artifact.path.to_string(),
            expected: artifact.sha256.to_string(),
            actual: written.to_string(),
        });
    }
    let abs = target
        .to_str()
        .ok_or_else(|| ContextError::Workspace {
            path: target.clone(), // BORROW: the error owns the path
            detail: "is not valid UTF-8",
        })?
        .to_owned(); // BORROW: the context owns its path text
    Ok(ContextArtifact::new(artifact.path.clone(), abs, written)) // BORROW: the context owns the path
}

/// Copies the assembled artifacts into `<workspace>/outputs/` and builds the run's context from
/// the files as written.
///
/// Each file is a fresh copy, never a hard link or a symbolic link to a store blob, and is made
/// read-only once written. The digest of the context is computed from the hash of each written
/// file, and a file whose hash differs from the store's record is an error. `workspace` must be
/// an absolute directory outside the store whose `outputs/` does not exist; its removal is the
/// engine's job.
///
/// # Errors
/// Returns [`ContextError::Workspace`] when `workspace` is unfit, [`ContextError::Io`] when a copy
/// fails, [`ContextError::CorruptBlob`] when a copy does not have the recorded digest, and
/// [`ContextError::Build`] when the context cannot be built.
pub fn materialise(assembled: &Assembled, workspace: &Path) -> Result<RunContext, ContextError> {
    let space = Workspace::prepare(workspace, assembled.store_root())?;
    let outputs = space.outputs_dir();
    let root = space
        .root()
        .to_str()
        .ok_or_else(|| ContextError::Workspace {
            path: space.root().to_path_buf(), // BORROW: the error owns the path
            detail: "is not valid UTF-8",
        })?
        .to_owned(); // BORROW: the context owns its root text
    let mut builder = RunContext::builder()
        .run(assembled.run().clone()) // BORROW: the context owns the key
        .root(root);
    for artifact in assembled.artifacts() {
        builder = builder.artifact(copy_artifact(&outputs, artifact)?);
    }
    for (path, value) in assembled.metadata() {
        builder = builder.metadata(path.clone(), value.clone()); // BORROW: the context owns its entries
    }
    Ok(builder.build()?)
}
