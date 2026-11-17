//! Directory trees: one walker, one build-context digest, one staging copy.
//!
//! A walk lists the files of a directory below its root, once, sorted by the `/`-joined relative
//! path as a string (byte order, so `a-b` comes before `a/b`; the order of path components would
//! put them the other way). Each caller names how the walk treats the three things callers
//! disagree on: a link or other special file ([`Links`]), a name that is not UTF-8 ([`Names`]),
//! and a name the caller refuses or leaves out ([`Screen`]).
//!
//! | caller | links | names | screen |
//! |:--|:--|:--|:--|
//! | build context digest and staging ([`BuildContext`], [`Listing::stage`]) | record | strict | none |
//! | a declared input of an experiment | refuse | strict | refuse unmountable names |
//! | the files a run produced | skip | strict | none |
//! | the ground-truth exposure check | skip | lossy | none |
//!
//! [`BuildContext`] is what an experiment's own image layer is built from: the digest of the
//! Dockerfile and of each file of its context. Its JSON is hashed into the cell digest, so its
//! keys and the way it reads a tree are frozen.
//!
//! # Limits
//! The digest of a context covers file bytes, link targets and paths. It does not cover file
//! modes or empty directories: an exec-bit change leaves it as it was, and staging does not make
//! an empty directory. Covering them would change a frozen digest and needs a new schema tag.
//! Not walked here: the import readers (they tolerate what they skip and keep typed errors of
//! their own), `candler ls`, and the one-level listers.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use candler_schemas::canonical::CanonicalError;
use candler_schemas::primitives::Digest;
use serde_json::{Map, Value, json};

/// What a walk does with a symbolic link, and with any other entry that is neither a file nor a
/// directory (a socket or a pipe).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Links {
    /// List a link as [`Kind::Link`], not followed. A special file is listed as a file, to be
    /// read (and to fail there if it cannot be).
    Record,
    /// Leave the entry out.
    Skip,
    /// Fail the walk with [`Refusal::Link`] or [`Refusal::Special`].
    Refuse,
}

/// What a walk does with a name that is not UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Names {
    /// Fail the walk with [`WalkError::Name`].
    Strict,
    /// Replace what is not UTF-8 with U+FFFD.
    Lossy,
}

/// What the caller's name rule says about an entry, by its own name (not its path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Screen {
    /// Take the entry as the other rules decide.
    Keep,
    /// Leave the entry out, and a directory with all it holds.
    Skip,
    /// Fail the walk with [`Refusal::Name`].
    Refuse,
}

/// What a listed entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// A regular file.
    File,
    /// A symbolic link, not followed.
    Link,
}

/// One entry of a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The path below the root, with `/` as the separator.
    pub rel: String,
    /// The path on disk.
    pub abs: PathBuf,
    /// What it is.
    pub kind: Kind,
}

/// Why a walk refused an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal {
    /// It is a symbolic link.
    Link,
    /// It is neither a file, a directory nor a link.
    Special,
    /// Its name is one the caller's [`Screen`] refuses.
    Name,
}

/// Why a walk failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WalkError {
    /// A directory or an entry could not be read.
    #[error("{}: {error}", .path.display())]
    Io {
        /// The path that failed.
        path: PathBuf,
        /// The cause.
        error: io::Error,
    },
    /// A name is not UTF-8, and the walk is [`Names::Strict`].
    #[error("a file name is not UTF-8: {}", .path.display())]
    Name {
        /// The path of the entry.
        path: PathBuf,
    },
    /// The walk's policy refused an entry.
    #[error("`{rel}` is refused: {why:?}")]
    Refused {
        /// The path of the entry below the root.
        rel: String,
        /// Why.
        why: Refusal,
    },
}

impl From<WalkError> for io::Error {
    fn from(error: WalkError) -> Self {
        match error {
            WalkError::Io { path, error } => {
                Self::new(error.kind(), format!("{}: {error}", path.display()))
            }
            WalkError::Name { .. } | WalkError::Refused { .. } => {
                Self::new(io::ErrorKind::InvalidInput, error.to_string())
            }
        }
    }
}

/// A recursive walk, set by the policy of its caller.
#[derive(Debug, Clone, Copy)]
pub struct Walk {
    links: Links,
    names: Names,
    skip: Option<fn(&str) -> Screen>,
}

impl Walk {
    /// A walk with the given policy and no name rule.
    #[must_use]
    pub const fn new(links: Links, names: Names) -> Self {
        Self {
            links,
            names,
            skip: None,
        }
    }

    /// Adds a name rule, asked of each entry's name before its kind is read.
    #[must_use]
    pub const fn screen(mut self, rule: fn(&str) -> Screen) -> Self {
        self.skip = Some(rule);
        self
    }

    /// Lists the files below the directory `root`, sorted by their joined relative path.
    ///
    /// # Errors
    /// [`WalkError`] for what cannot be read, a name the policy refuses, or an entry it refuses.
    pub fn list(self, root: &Path) -> Result<Listing, WalkError> {
        let mut entries = Vec::new();
        self.collect(root, "", &mut entries)?;
        entries.sort_by(|a, b| a.rel.cmp(&b.rel));
        Ok(Listing {
            root: root.to_path_buf(), // BORROW: the listing owns its root
            entries,
        })
    }

    fn collect(self, dir: &Path, prefix: &str, out: &mut Vec<Entry>) -> Result<(), WalkError> {
        let at = |path: &Path| {
            let path = path.to_path_buf(); // BORROW: the error owns its path
            move |error| WalkError::Io { path, error }
        };
        for entry in fs::read_dir(dir).map_err(at(dir))? {
            let entry = entry.map_err(at(dir))?;
            let abs = entry.path();
            let name = match self.names {
                Names::Strict => entry
                    .file_name()
                    .into_string()
                    .map_err(|_| WalkError::Name {
                        path: abs.clone(), // BORROW: the error owns the path
                    })?,
                Names::Lossy => entry.file_name().to_string_lossy().into_owned(),
            };
            let rel = if prefix.is_empty() {
                name.clone() // BORROW: the name is kept for the screen
            } else {
                format!("{prefix}/{name}")
            };
            match self.skip.map_or(Screen::Keep, |rule| rule(&name)) {
                Screen::Keep => {}
                Screen::Skip => continue,
                Screen::Refuse => {
                    return Err(WalkError::Refused {
                        rel,
                        why: Refusal::Name,
                    });
                }
            }
            let kind = entry.file_type().map_err(at(&abs))?;
            if kind.is_dir() {
                self.collect(&abs, &rel, out)?;
            } else if kind.is_file() {
                out.push(Entry {
                    rel,
                    abs,
                    kind: Kind::File,
                });
            } else {
                let (is_link, why) = if kind.is_symlink() {
                    (true, Refusal::Link)
                } else {
                    (false, Refusal::Special)
                };
                match self.links {
                    Links::Skip => {}
                    Links::Refuse => return Err(WalkError::Refused { rel, why }),
                    Links::Record => out.push(Entry {
                        rel,
                        abs,
                        kind: if is_link { Kind::Link } else { Kind::File },
                    }),
                }
            }
        }
        Ok(())
    }
}

/// The entries one walk found, below one root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    root: PathBuf,
    entries: Vec<Entry>,
}

impl Listing {
    /// The directory that was walked.
    #[must_use]
    pub fn root<'a>(&'a self) -> &'a Path {
        &self.root
    }

    /// The entries, sorted by their joined relative path.
    #[must_use]
    pub fn entries<'a>(&'a self) -> &'a [Entry] {
        &self.entries
    }

    /// Copies exactly the listed entries below `to`, keeping the layout: a file is copied and a
    /// link is made again with the same target text, so what is staged is what a digest of the
    /// listing covers.
    ///
    /// # Errors
    /// The [`io::Error`] of a file that cannot be copied, or of a link on a platform without them.
    pub fn stage(&self, to: &Path) -> io::Result<()> {
        fs::create_dir_all(to)?;
        for entry in &self.entries {
            let target = to.join(&entry.rel);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            match entry.kind {
                Kind::File => {
                    fs::copy(&entry.abs, &target)?;
                }
                Kind::Link => link(&fs::read_link(&entry.abs)?, &target)?,
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
fn link(target: &Path, at: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(not(unix))]
fn link(_target: &Path, _at: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "a link cannot be staged on this platform",
    ))
}

/// What the experiment's own image layer is built from: the Dockerfile and the files of its
/// context.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BuildContext {
    /// The digest of the Dockerfile's bytes.
    pub dockerfile: Digest,
    /// The digest of each file of the context, by its path relative to the context, with `/` as
    /// the separator. A symbolic link is listed by the digest of its target text, and not
    /// followed.
    pub context: BTreeMap<String, Digest>,
}

impl BuildContext {
    /// The walk that reads a context: links recorded, names strict.
    #[must_use]
    pub const fn walk() -> Walk {
        Walk::new(Links::Record, Names::Strict)
    }

    /// Reads the Dockerfile at `dockerfile` and walks the directory `context`.
    ///
    /// # Errors
    /// The [`io::Error`] of a file that cannot be read, or of a name that is not UTF-8.
    pub fn read(dockerfile: &Path, context: &Path) -> Result<Self, io::Error> {
        Self::of(dockerfile, &Self::walk().list(context)?)
    }

    /// The inputs of the files `listing` names (made by [`BuildContext::walk`]) and the Dockerfile
    /// at `dockerfile`.
    ///
    /// # Errors
    /// The [`io::Error`] of a file that cannot be read.
    pub fn of(dockerfile: &Path, listing: &Listing) -> Result<Self, io::Error> {
        let mut files = BTreeMap::new();
        for entry in listing.entries() {
            let digest = match entry.kind {
                Kind::File => digest_file(&entry.abs)?,
                Kind::Link => {
                    let target = fs::read_link(&entry.abs).map_err(|e| at(&entry.abs, &e))?;
                    let text = format!("symlink:{}", target.to_string_lossy());
                    Digest::of_bytes(text.as_bytes())
                }
            };
            files.insert(entry.rel.clone(), digest); // BORROW: the map owns its keys
        }
        Ok(Self {
            dockerfile: digest_file(dockerfile)?,
            context: files,
        })
    }

    /// The inputs as the JSON object their digest is taken over.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let context: Vec<Value> = self
            .context
            .iter()
            .map(|(path, digest)| json!({"path": path, "digest": digest}))
            .collect();
        let mut object = Map::new();
        object.insert("dockerfile".to_owned(), json!(self.dockerfile)); // BORROW: owned key for the object
        object.insert("context".to_owned(), Value::Array(context)); // BORROW: owned key for the object
        Value::Object(object)
    }

    /// The digest of the inputs: the SHA-256 of their canonical JSON text.
    ///
    /// # Errors
    /// [`CanonicalError`] when the inputs have no canonical text.
    pub fn digest(&self) -> Result<Digest, CanonicalError> {
        Digest::of_canonical(&self.to_json())
    }
}

/// `error` with the path it failed on in its text.
fn at(path: &Path, error: &io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}

fn digest_file(path: &Path) -> Result<Digest, io::Error> {
    let file = fs::File::open(path).map_err(|e| at(path, &e))?;
    Digest::of_reader(file).map_err(|e| at(path, &e))
}
