//! The `OpenCode` skill layout: a treatment's skill tree is the subtree `.claude/skills/<name>` of
//! the treatment's repository at the pinned commit, written to the project's
//! `.opencode/skills/<name>/`, which `OpenCode` reads.
//!
//! Nothing here touches the file system or runs a process: the core runs the archive command and
//! hands the bytes of the `git archive` to [`OpenCodeSkills::files`].

use std::path::Path;
use std::str::FromStr as _;

use candler_harness::driver::DriverError;
use candler_harness::skills::SkillTree;
use candler_harness::spec::{CommandSpec, PreparedFile};
use candler_schemas::execution::TreatmentRef;
use candler_schemas::primitives::RelPath;

use crate::prepare::{invalid, missing};

const TREATMENT: &str = "treatment";
/// Where a treatment's skill tree lives in its repository.
const SOURCE_ROOT: &str = ".claude/skills";
/// Where `OpenCode` reads project skills.
const SKILLS_ROOT: &str = ".opencode/skills";
const BLOCK: usize = 512;

/// Where a treatment's skill tree lives for `OpenCode`.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenCodeSkills;

fn plain_name(treatment: &TreatmentRef) -> Result<&str, DriverError> {
    let name = treatment.name.as_str(); // BORROW: explicit .as_str()
    let plain = !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if plain {
        Ok(name)
    } else {
        Err(invalid(
            TREATMENT,
            format!("the treatment name `{name}` is not one plain path segment"),
        ))
    }
}

impl SkillTree for OpenCodeSkills {
    fn archive_command(
        &self,
        treatment: &TreatmentRef,
        source: &Path,
    ) -> Result<CommandSpec, DriverError> {
        let commit = treatment
            .commit
            .as_ref()
            .ok_or_else(|| missing("treatment.commit"))?;
        let subtree = format!("{SOURCE_ROOT}/{}", plain_name(treatment)?);
        let source = source
            .to_str()
            .ok_or_else(|| invalid(TREATMENT, "the treatment source is not text".to_owned()))?; // BORROW: .to_owned() makes the owned text
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

    fn files(
        &self,
        treatment: &TreatmentRef,
        archive: &[u8],
    ) -> Result<Vec<PreparedFile>, DriverError> {
        let name = plain_name(treatment)?;
        let scope = format!("{SOURCE_ROOT}/{name}/");
        let target = format!("{SKILLS_ROOT}/{name}/");
        let entry = format!("{target}SKILL.md");
        let mut files = Vec::new();
        for (path, data, executable) in entries(archive)? {
            let Some(inside) = path.strip_prefix(scope.as_str()) else {
                // BORROW: explicit .as_str()
                return Err(invalid(
                    TREATMENT,
                    format!("the entry `{path}` lies outside `{SOURCE_ROOT}/{name}`"),
                ));
            };
            let rel = RelPath::from_str(&format!("{target}{inside}"))
                .map_err(|error| invalid(TREATMENT, error.to_string()))?; // BORROW: .to_string() gives owned text
            files.push(PreparedFile::new(rel, data, executable));
        }
        // BORROW: explicit .as_str() on both sides
        if !files.iter().any(|f| f.path.as_str() == entry.as_str()) {
            return Err(invalid(
                TREATMENT,
                format!("the archive holds no `{SOURCE_ROOT}/{name}/SKILL.md`"),
            ));
        }
        Ok(files)
    }
}

fn cut() -> DriverError {
    invalid(TREATMENT, "the archive is cut short".to_owned()) // BORROW: .to_owned() makes the owned text
}

fn text_field(bytes: &[u8]) -> &[u8] {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    bytes.get(..end).unwrap_or_default()
}

fn octal(bytes: &[u8]) -> Result<usize, DriverError> {
    let text = std::str::from_utf8(text_field(bytes))
        .map_err(|_| invalid(TREATMENT, "a tar number is not text".to_owned()))?; // BORROW: .to_owned() makes the owned text
    usize::from_str_radix(text.trim(), 8)
        .map_err(|_| invalid(TREATMENT, format!("a tar number is not octal: `{text}`")))
}

fn span(header: &[u8], range: std::ops::Range<usize>) -> &[u8] {
    header.get(range).unwrap_or_default()
}

fn pax_path(data: &[u8]) -> Result<Option<String>, DriverError> {
    let bad = || invalid(TREATMENT, "a pax header is malformed".to_owned()); // BORROW: .to_owned() makes the owned text
    let mut path = None;
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
        match key {
            "path" => path = Some(value.to_owned()), // BORROW: .to_owned() makes owned text
            "linkpath" => return Err(invalid(TREATMENT, "an entry is a link".to_owned())), // BORROW: .to_owned() makes the owned text
            // EXPLICIT: other pax keys do not affect the path
            _ => {}
        }
        rest = rest.get(length..).unwrap_or_default();
    }
    Ok(path)
}

fn header_path(header: &[u8]) -> Result<String, DriverError> {
    let text = |bytes| {
        std::str::from_utf8(text_field(bytes))
            .map_err(|_| invalid(TREATMENT, "a path is not text".to_owned())) // BORROW: .to_owned() makes the owned text
    };
    let name = text(span(header, 0..100))?;
    let ustar = span(header, 257..262) == b"ustar";
    let prefix = if ustar {
        text(span(header, 345..500))?
    } else {
        ""
    };
    Ok(if prefix.is_empty() {
        name.to_owned() // BORROW: .to_owned() makes owned text
    } else {
        format!("{prefix}/{name}")
    })
}

/// The regular files of a tar archive: path, contents, executable bit. A link, device or FIFO is
/// refused.
fn entries(tar: &[u8]) -> Result<Vec<(String, Vec<u8>, bool)>, DriverError> {
    let mut files = Vec::new();
    let mut at = 0_usize;
    let mut long_path: Option<String> = None;
    loop {
        let rest = tar.get(at..).unwrap_or_default();
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
            // EXPLICIT: a global pax header or a directory carries no file
            b'g' | b'5' => {}
            b'x' => {
                if let Some(path) = pax_path(data)? {
                    long_path = Some(path);
                }
            }
            b'L' => {
                let name = std::str::from_utf8(text_field(data))
                    .map_err(|_| invalid(TREATMENT, "a long name is not text".to_owned()))?; // BORROW: .to_owned() makes the owned text
                long_path = Some(name.to_owned()); // BORROW: .to_owned() makes owned text
            }
            b'0' | 0 => {
                let path = match long_path.take() {
                    Some(path) => path,
                    None => header_path(header)?,
                };
                let mode = octal(span(header, 100..108))?;
                // BORROW: .to_vec() copies the file out of the archive
                files.push((path, data.to_vec(), mode & 0o111 != 0));
            }
            other => {
                return Err(invalid(
                    TREATMENT,
                    format!("an entry has the type flag {other} (a link, device or FIFO)"),
                ));
            }
        }
    }
    Ok(files)
}
