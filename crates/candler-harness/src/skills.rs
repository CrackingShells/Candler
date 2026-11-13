//! Where a treatment's skill tree comes from and where it goes: the subtree
//! `<source_root>/<name>` of the treatment's repository at the pinned commit, written to the
//! workspace under `<target_root>/<name>/`.
//!
//! Harnesses lay skills out in their own directories and differ in nothing else, so one
//! [`SkillTree`] holds both roots. Nothing here touches the file system or runs a process: the core
//! runs the archive command and hands the bytes of the `git archive` to [`SkillTree::files`].

use std::path::Path;
use std::str::FromStr as _;

use candler_schemas::execution::TreatmentRef;
use candler_schemas::primitives::{FieldPath, RelPath, plain_segment};

use crate::driver::DriverError;
use crate::spec::{CommandSpec, PreparedFile};

/// The cell path of the treatment the skill tree comes from.
const TREATMENT: &str = "treatment";
/// The size of a tar block.
const BLOCK: usize = 512;

/// The error for a treatment that cannot be used.
fn invalid(detail: String) -> DriverError {
    match FieldPath::from_str(TREATMENT) {
        Ok(path) => DriverError::InvalidValue { path, detail },
        Err(error) => DriverError::VersionOutput {
            detail: error.to_string(), // BORROW: .to_string() makes the owned text the error keeps
        },
    }
}

/// Where a treatment's skill tree lives in its repository and in the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SkillTree {
    /// The directory of the repository that holds the skills, such as `.claude/skills`.
    source_root: &'static str,
    /// The directory of the workspace the harness reads skills from.
    target_root: &'static str,
}

impl SkillTree {
    /// A layout that reads skills from `source_root` of a repository and writes them under
    /// `target_root` of the workspace.
    #[must_use]
    pub const fn new(source_root: &'static str, target_root: &'static str) -> Self {
        Self {
            source_root,
            target_root,
        }
    }

    /// The name of `treatment`, which must be one plain path segment.
    fn name<'t>(treatment: &'t TreatmentRef) -> Result<&'t str, DriverError> {
        // BORROW: explicit .as_str() instead of Deref coercion
        let name = treatment.name.as_str();
        if plain_segment(name) {
            Ok(name)
        } else {
            Err(invalid(format!(
                "the treatment name `{name}` is not one plain path segment"
            )))
        }
    }

    /// The command that archives the skill tree at the commit `treatment` pins:
    /// `git -C <source> archive --format=tar <commit> -- <source_root>/<name>`.
    ///
    /// # Errors
    /// [`DriverError::MissingValue`] when the treatment pins no commit.
    /// [`DriverError::InvalidValue`] when `source` is not text or the treatment name is not one
    /// plain path segment.
    /// [`DriverError::Command`] when the command cannot be built.
    pub fn archive_command(
        &self,
        treatment: &TreatmentRef,
        source: &Path,
    ) -> Result<CommandSpec, DriverError> {
        let commit = treatment.commit.as_ref().ok_or_else(|| {
            match FieldPath::from_str("treatment.commit") {
                Ok(path) => DriverError::MissingValue { path },
                Err(error) => DriverError::VersionOutput {
                    detail: error.to_string(), // BORROW: .to_string() makes the owned text
                },
            }
        })?;
        let subtree = format!("{}/{}", self.source_root, Self::name(treatment)?);
        let source = source
            .to_str()
            .ok_or_else(|| invalid("the treatment source is not text".to_owned()))?; // BORROW: .to_owned() makes the owned text
        // BORROW: .to_owned() makes the owned arguments the command keeps
        let argv = vec![
            "git".to_owned(),
            "-C".to_owned(),
            source.to_owned(),
            "archive".to_owned(),
            "--format=tar".to_owned(),
            commit.to_string(), // BORROW: .to_string() gives owned text
            "--".to_owned(),
            subtree,
        ];
        Ok(CommandSpec::builder().argv(argv).build()?)
    }

    /// The files of `archive`, the output of the archive command for `treatment`: regular files with
    /// their executable bit, at `<target_root>/<name>/` followed by the path the archive gives
    /// them inside `<source_root>/<name>/`.
    ///
    /// Directories and the pax global header are skipped. The reader takes ustar and pax headers
    /// and GNU long names.
    ///
    /// # Errors
    /// [`DriverError::InvalidValue`] when the treatment name is not one plain path segment, when an
    /// entry is a link, a device or a FIFO, when a path is absolute, empty, leaves the root or lies
    /// outside the skill subtree, when `SKILL.md` is absent from the subtree, or when the archive
    /// is cut short.
    pub fn files(
        &self,
        treatment: &TreatmentRef,
        archive: &[u8],
    ) -> Result<Vec<PreparedFile>, DriverError> {
        let name = Self::name(treatment)?;
        let scope = format!("{}/{name}/", self.source_root);
        let target = format!("{}/{name}/", self.target_root);
        let entry = format!("{target}SKILL.md");
        let cut = || invalid("the archive is cut short".to_owned()); // BORROW: .to_owned() makes the owned text
        let mut files = Vec::new();
        let mut at = 0_usize;
        let mut long_path: Option<String> = None;
        loop {
            let rest = archive.get(at..).unwrap_or_default();
            if rest.is_empty() {
                break;
            }
            let header = rest.get(..BLOCK).ok_or_else(cut)?;
            if header.iter().all(|b| *b == 0) {
                break;
            }
            let size = octal(span(header, 124..136))?;
            let kind = header.get(156).copied().unwrap_or(0);
            let data_end = BLOCK.checked_add(size).ok_or_else(cut)?;
            let data = rest.get(BLOCK..data_end).ok_or_else(cut)?;
            let padded = data_end
                .div_ceil(BLOCK)
                .checked_mul(BLOCK)
                .ok_or_else(cut)?;
            at = at.checked_add(padded).ok_or_else(cut)?;
            match kind {
                b'g' | b'5' => {}
                b'x' => {
                    for (key, value) in pax_records(data)? {
                        // BORROW: explicit .as_str()
                        match key.as_str() {
                            "path" => long_path = Some(value),
                            "linkpath" => {
                                return Err(invalid("an entry is a link".to_owned())); // BORROW: .to_owned() makes the owned text
                            }
                            _ => {}
                        }
                    }
                }
                b'L' => {
                    let long = std::str::from_utf8(text_field(data))
                        .map_err(|_| invalid("a long name is not text".to_owned()))?; // BORROW: .to_owned() makes the owned text
                    long_path = Some(long.to_owned()); // BORROW: .to_owned() makes the owned text
                }
                b'0' | 0 => {
                    let path = match long_path.take() {
                        Some(path) => path,
                        None => entry_path(header)?,
                    };
                    // BORROW: explicit .as_str()
                    let Some(inside) = path.strip_prefix(scope.as_str()) else {
                        return Err(invalid(format!(
                            "the entry `{path}` lies outside `{}/{name}`",
                            self.source_root
                        )));
                    };
                    let rel = RelPath::from_str(&format!("{target}{inside}"))
                        .map_err(|error| invalid(error.to_string()))?; // BORROW: .to_string() gives owned text
                    let mode = octal(span(header, 100..108))?;
                    // BORROW: .to_vec() copies the file out of the archive
                    files.push(PreparedFile::new(rel, data.to_vec(), mode & 0o111 != 0));
                }
                other => {
                    return Err(invalid(format!(
                        "an entry has the type flag {other} (a link, device or FIFO)"
                    )));
                }
            }
        }
        // BORROW: explicit .as_str() on both sides
        if !files
            .iter()
            .any(|file| file.path.as_str() == entry.as_str())
        {
            return Err(invalid(format!("the archive holds no `{entry}`")));
        }
        Ok(files)
    }
}

/// The text of a header field up to its first NUL.
fn text_field(bytes: &[u8]) -> &[u8] {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    bytes.get(..end).unwrap_or_default()
}

/// The octal number a header field holds.
fn octal(bytes: &[u8]) -> Result<usize, DriverError> {
    let text = std::str::from_utf8(text_field(bytes))
        .map_err(|_| invalid("a tar number is not text".to_owned()))?; // BORROW: .to_owned() makes the owned text
    usize::from_str_radix(text.trim(), 8)
        .map_err(|_| invalid(format!("a tar number is not octal: `{text}`")))
}

/// The bytes `range` of `header`.
fn span(header: &[u8], range: std::ops::Range<usize>) -> &[u8] {
    header.get(range).unwrap_or_default()
}

/// The `path` and `linkpath` records of a pax extended header.
fn pax_records(data: &[u8]) -> Result<Vec<(String, String)>, DriverError> {
    let bad = || invalid("a pax header is malformed".to_owned()); // BORROW: .to_owned() makes the owned text
    let mut records = Vec::new();
    let mut rest = data;
    while !rest.is_empty() {
        let space = rest.iter().position(|b| *b == b' ').ok_or_else(bad)?;
        let length: usize = std::str::from_utf8(rest.get(..space).unwrap_or_default())
            .ok()
            .and_then(|text| text.parse().ok())
            .ok_or_else(bad)?;
        let record = rest.get(..length).ok_or_else(bad)?;
        let body = record.get(space.saturating_add(1)..).ok_or_else(bad)?;
        let body = std::str::from_utf8(body).map_err(|_| bad())?;
        let (key, value) = body
            .trim_end_matches('\n')
            .split_once('=')
            .ok_or_else(bad)?;
        // BORROW: .to_owned() makes the owned strings the list keeps
        records.push((key.to_owned(), value.to_owned()));
        rest = rest.get(length..).unwrap_or_default();
    }
    Ok(records)
}

/// The path of the entry a ustar `header` describes: the prefix, a `/` and the name.
fn entry_path(header: &[u8]) -> Result<String, DriverError> {
    let text = |bytes| {
        std::str::from_utf8(text_field(bytes)).map_err(|_| invalid("a path is not text".to_owned())) // BORROW: .to_owned() makes the owned text
    };
    let name = text(span(header, 0..100))?;
    let ustar = span(header, 257..262) == b"ustar";
    let prefix = if ustar {
        text(span(header, 345..500))?
    } else {
        ""
    };
    Ok(if prefix.is_empty() {
        name.to_owned() // BORROW: .to_owned() makes the owned text
    } else {
        format!("{prefix}/{name}")
    })
}
