//! The typed model of a legacy results folder: every file is a source, an artifact, a stream or a skipped entry with its reason.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::BufRead as _;
use std::path::{Path, PathBuf};

use candler_schemas::primitives::{Digest, RelPath};
use flate2::read::GzDecoder;
use serde_json::Value;

use super::ImportError;
use super::legacy::{
    CONTAMINATION_CATALOG_FILE, EVAL_METADATA_FILE, EVAL_PREFIX, EXPECTATIONS_FILE,
    EXPERIMENT_FILE, GRADING_SUFFIX, OUTPUTS, PROGRESS_FILE, RUN_COMPLETE_FILE, STDERR_FILE,
    TIMING_FILE,
};
use super::rules::ImportRules;

/// A file the import read: its path relative to the folder, the digest of its bytes, the bytes and its `executable` flag.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SourceFile {
    /// The path of the file relative to the folder's root.
    pub path: RelPath,
    /// The digest of the file's bytes.
    pub sha256: Digest,
    /// The bytes of the file.
    pub bytes: Vec<u8>,
    /// Whether the file has an execute bit for its owner, its group or others.
    pub executable: bool,
}

/// A JSON file the model interprets: its path relative to the folder's root and its parsed value. The same file is also kept as a [`SourceFile`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct JsonFile {
    /// The path of the file relative to the folder's root.
    pub path: RelPath,
    /// The parsed document.
    pub value: Value,
}

/// Why a file or directory is not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SkipReason {
    /// An `AppleDouble` file, whose name starts with `._`.
    AppleDouble,
    /// A `.DS_Store` file.
    FinderMetadata,
    /// An archive's `__MACOSX` directory.
    ArchiveMetadata,
    /// A `shared_<config>` directory: inputs the runs of a cell share, copied from the repository.
    SharedInputs,
}

impl SkipReason {
    /// The spelling of the reason: `apple_double`, `finder_metadata`, `archive_metadata` or `shared_inputs`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AppleDouble => "apple_double",
            Self::FinderMetadata => "finder_metadata",
            Self::ArchiveMetadata => "archive_metadata",
            Self::SharedInputs => "shared_inputs",
        }
    }
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // BORROW: explicit .as_str() instead of Deref coercion
        f.write_str(self.as_str())
    }
}

/// A file or directory the model leaves out, with its reason; a skipped directory is one entry that covers every file below it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Skipped {
    /// The path relative to the folder's root.
    pub path: RelPath,
    /// Why it is not read.
    pub reason: SkipReason,
}

/// One run of a cell: the files under `outputs/`, the stream, the stderr and the files the model reads as the run's sources.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct LegacyRun {
    /// The configuration directory name the run belongs to.
    pub config: String,
    /// The run number: the `N` of `run-N`.
    pub number: u32,
    /// The run as errors name it: `<cell>/<config>/run-N`.
    pub label: String,
    /// The harness version read at the harness rule's pointer from the first line of the stream.
    pub harness_version: String,
    /// The compressed stream of the agent.
    pub stream: SourceFile,
    /// The agent's standard error, when the run has it.
    pub stderr: Option<SourceFile>,
    /// Every file under `outputs/`, sorted by path.
    pub outputs: Vec<SourceFile>,
    /// Every directory under `outputs/` that holds nothing, sorted by path.
    pub empty_dirs: Vec<RelPath>,
    /// The run's timing file: the first copy, every copy holding the same document.
    pub timing: JsonFile,
    /// The run's completion file, when the run has it.
    pub run_complete: Option<JsonFile>,
    /// Every copy of the run's metadata file, sorted by path.
    pub eval_metadata: Vec<JsonFile>,
    /// Every grading file found by name under the run's eval directories, sorted by path.
    pub gradings: Vec<JsonFile>,
    /// The other files of the run that the model keeps: the interpreted ones among them are also in the typed fields above.
    pub sources: Vec<SourceFile>,
}

/// One cell: its experiment file, its progress events, its contamination catalog, its sources and its runs.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct LegacyCell {
    /// The cell's directory name under the folder's root.
    pub name: String,
    /// The cell's experiment file.
    pub experiment: JsonFile,
    /// The events of the cell's progress file, in order; empty when the cell has none.
    pub progress: Vec<Value>,
    /// The rows of the cell's contamination catalog; empty when the cell has none.
    pub catalog: Vec<Value>,
    /// Every file of the cell that belongs to no run, the interpreted ones included, sorted by path.
    pub sources: Vec<SourceFile>,
    /// The cell's runs, sorted by configuration and number.
    pub runs: Vec<LegacyRun>,
}

impl LegacyCell {
    /// The path of `path` relative to the cell's directory, or `None` if the path is outside it.
    #[must_use]
    pub fn relative<'a>(&'a self, path: &'a RelPath) -> Option<&'a str> {
        // BORROW: explicit .as_str() instead of Deref coercion
        let rest = path.as_str().strip_prefix(self.name.as_str())?;
        rest.strip_prefix('/')
    }
}

/// A legacy results folder read into a model: its provenance file, its expectation rows, its cells, its other files and what it skipped.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct LegacyFolder {
    /// The directory that was read.
    pub root: PathBuf,
    /// The folder's provenance file.
    pub provenance: JsonFile,
    /// The rows of `expectations.jsonl`, in order.
    pub expectations: Vec<Value>,
    /// Every file at the folder's root, the interpreted ones included, sorted by path.
    pub sources: Vec<SourceFile>,
    /// The cells, sorted by directory name.
    pub cells: Vec<LegacyCell>,
    /// What the read left out, sorted by path.
    pub skipped: Vec<Skipped>,
}

impl LegacyFolder {
    /// Reads the folder at `root`: one pass over every file, each becoming a source, an artifact, a stream or a skipped entry.
    ///
    /// # Memory
    /// The whole folder is held in memory at once: every file's bytes stay in the returned model
    /// until the import ends, so the peak is about the size of the folder plus its decompressed
    /// streams (about 379 MB for the 240-run golden corpus in a debug build). There is no cap and
    /// no streaming in this version.
    ///
    /// # Errors
    /// Returns [`ImportError::Io`] for a file or directory that cannot be read,
    /// [`ImportError::Path`] for a path that is not a run-relative path,
    /// [`ImportError::Json`] for a file that is not valid JSON or JSONL where the format requires
    /// it, [`ImportError::NoOutputs`] for a run without `outputs/`,
    /// [`ImportError::DuplicateGrading`] for a grading file name found in two eval directories of
    /// one run, and [`ImportError::Layout`] for a file the format requires and the folder lacks.
    pub fn read(root: &Path, rules: &ImportRules) -> Result<Self, ImportError> {
        let mut reader = Reader {
            root,
            rules,
            skipped: Vec::new(),
        };
        let (provenance, expectations, mut sources, cells) = reader.folder()?;
        let mut skipped = reader.skipped;
        sources.sort_by(|a, b| a.path.cmp(&b.path));
        skipped.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Self {
            // BORROW: .to_owned() makes the owned path
            root: root.to_owned(),
            provenance,
            expectations,
            sources,
            cells,
            skipped,
        })
    }

    /// Every run of the folder with its cell: the cells in order, then each cell's runs in order. A run's index in this order is the `run` of a mapped run.
    pub fn runs<'a>(&'a self) -> impl Iterator<Item = (&'a LegacyCell, &'a LegacyRun)> {
        self.cells
            .iter()
            .flat_map(|cell| cell.runs.iter().map(move |run| (cell, run)))
    }

    /// The run at `index` of [`runs`](Self::runs), with its cell.
    #[must_use]
    pub fn run<'a>(&'a self, index: usize) -> Option<(&'a LegacyCell, &'a LegacyRun)> {
        self.runs().nth(index)
    }
}

/// The digest of `bytes`: `sha256:` and the hex of the SHA-256 of the bytes.
///
/// # Errors
/// Returns [`ImportError::Format`] if the hex digits do not form a digest, which they always do.
pub fn digest_of(bytes: &[u8]) -> Result<Digest, ImportError> {
    use sha2::Digest as _;
    let mut hex = String::with_capacity(64);
    for byte in sha2::Sha256::digest(bytes) {
        hex.extend(char::from_digit(u32::from(byte >> 4), 16));
        hex.extend(char::from_digit(u32::from(byte & 0xf), 16));
    }
    Ok(Digest::from_hex(&hex)?)
}

/// A directory entry that is a file or a directory.
struct Entry {
    name: String,
    is_dir: bool,
}

/// Joins a folder-relative directory and a name.
fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.into()
    } else {
        format!("{parent}/{name}")
    }
}

/// Reads `text` as a run-relative path.
pub(super) fn rel_path(text: &str) -> Result<RelPath, ImportError> {
    text.parse::<RelPath>().map_err(|error| ImportError::Path {
        path: text.into(),
        detail: error.to_string(),
    })
}

/// Reports whether `meta` has an execute bit for the owner, the group or others.
#[cfg(unix)]
fn is_executable(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    meta.permissions().mode() & 0o111 != 0
}

/// Reports whether `meta` has an execute bit; a platform without mode bits has none.
#[cfg(not(unix))]
fn is_executable(_meta: &fs::Metadata) -> bool {
    false
}

/// The reason a file or directory is skipped, if it is.
fn skip_reason(entry: &Entry) -> Option<SkipReason> {
    // BORROW: explicit .as_str() instead of Deref coercion
    let name = entry.name.as_str();
    if name.starts_with("._") {
        Some(SkipReason::AppleDouble)
    } else if !entry.is_dir && name == ".DS_Store" {
        Some(SkipReason::FinderMetadata)
    } else if entry.is_dir && name == "__MACOSX" {
        Some(SkipReason::ArchiveMetadata)
    } else if entry.is_dir && name.starts_with("shared_") {
        Some(SkipReason::SharedInputs)
    } else {
        None
    }
}

/// The number of a directory named `run-N`, with `N` a plain decimal.
fn run_number(name: &str) -> Option<u32> {
    let digits = name.strip_prefix("run-")?;
    let plain = !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (digits.len() == 1 || !digits.starts_with('0'));
    if plain { digits.parse().ok() } else { None }
}

/// Reports whether `name` is `eval-K-<title>`, the directory of one eval.
fn is_eval_dir(name: &str) -> bool {
    name.strip_prefix(EVAL_PREFIX)
        .and_then(|rest| rest.split_once('-'))
        .is_some_and(|(k, _)| !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit()))
}

/// Reports whether `name` is `eval-K-grading.json`.
fn is_grading_name(name: &str) -> bool {
    name.strip_prefix(EVAL_PREFIX)
        .and_then(|rest| rest.strip_suffix(GRADING_SUFFIX))
        .is_some_and(|k| !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit()))
}

/// The last segment of a path.
fn file_name<'a>(path: &'a RelPath) -> &'a str {
    // BORROW: explicit .as_str() instead of Deref coercion
    path.as_str().rsplit('/').next().unwrap_or_default()
}

/// Parses a JSON file.
fn json_file(file: &SourceFile) -> Result<JsonFile, ImportError> {
    // BORROW: .as_slice() views the vector as a slice
    let value = serde_json::from_slice::<Value>(file.bytes.as_slice()).map_err(|error| {
        ImportError::Json {
            file: file.path.to_string(),
            detail: error.to_string(),
        }
    })?;
    Ok(JsonFile {
        // BORROW: .clone() copies the path into the parsed file
        path: file.path.clone(),
        value,
    })
}

/// Parses a JSONL file whose every line is a JSON object; blank lines are ignored.
fn jsonl_rows(file: &SourceFile) -> Result<Vec<Value>, ImportError> {
    let json = |detail: String| ImportError::Json {
        file: file.path.to_string(),
        detail,
    };
    // BORROW: .as_slice() views the vector as a slice
    let text = std::str::from_utf8(file.bytes.as_slice()).map_err(|e| json(e.to_string()))?;
    let mut rows = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let number = index.saturating_add(1);
        let row =
            serde_json::from_str::<Value>(line).map_err(|e| json(format!("line {number}: {e}")))?;
        if !row.is_object() {
            return Err(json(format!("line {number} is not a JSON object")));
        }
        rows.push(row);
    }
    Ok(rows)
}

/// The files of a run while its directories are being read.
struct RunBuilder {
    config: String,
    number: u32,
    label: String,
    has_outputs: bool,
    outputs: Vec<SourceFile>,
    empty_dirs: Vec<RelPath>,
    stream: Option<SourceFile>,
    stderr: Option<SourceFile>,
    sources: Vec<SourceFile>,
    timing: Option<JsonFile>,
    run_complete: Option<JsonFile>,
    eval_metadata: Vec<JsonFile>,
    gradings: Vec<JsonFile>,
}

impl RunBuilder {
    fn new(cell: &str, config: &str, number: u32) -> Self {
        Self {
            config: config.into(),
            number,
            label: format!("{cell}/{config}/run-{number}"),
            has_outputs: false,
            outputs: Vec::new(),
            empty_dirs: Vec::new(),
            stream: None,
            stderr: None,
            sources: Vec::new(),
            timing: None,
            run_complete: None,
            eval_metadata: Vec::new(),
            gradings: Vec::new(),
        }
    }
}

type Runs = BTreeMap<(String, u32), RunBuilder>;

/// What `Reader::folder` finds: the provenance file, the expectation rows, the root files and the cells.
type FolderParts = (JsonFile, Vec<Value>, Vec<SourceFile>, Vec<LegacyCell>);

/// One pass over a folder.
struct Reader<'a> {
    root: &'a Path,
    rules: &'a ImportRules,
    skipped: Vec<Skipped>,
}

impl Reader<'_> {
    /// Lists a directory, sorted by name.
    fn list(&self, rel: &str) -> Result<Vec<Entry>, ImportError> {
        let dir = self.root.join(rel);
        let io = |error: std::io::Error| ImportError::Io {
            path: dir.display().to_string(),
            detail: error.to_string(),
        };
        let mut entries = Vec::new();
        for item in fs::read_dir(&dir).map_err(io)? {
            let item = item.map_err(io)?;
            let kind = item.file_type().map_err(io)?;
            let name = item
                .file_name()
                .into_string()
                .map_err(|name| ImportError::Path {
                    path: name.to_string_lossy().into_owned(),
                    detail: "is not UTF-8".into(),
                })?;
            if !kind.is_dir() && !kind.is_file() {
                return Err(ImportError::Layout {
                    file: join(rel, &name),
                    run: "folder".into(),
                    detail: "is neither a file nor a directory".into(),
                });
            }
            entries.push(Entry {
                name,
                is_dir: kind.is_dir(),
            });
        }
        // BORROW: explicit .as_str() instead of Deref coercion
        entries.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
        Ok(entries)
    }

    /// Reads a file's bytes.
    fn source(&self, rel: &str) -> Result<SourceFile, ImportError> {
        let path = self.root.join(rel);
        let bytes = fs::read(&path).map_err(|error| ImportError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
        let executable = fs::metadata(&path)
            .map_err(|error| ImportError::Io {
                path: path.display().to_string(),
                detail: error.to_string(),
            })
            .map(|meta| is_executable(&meta))?;
        Ok(SourceFile {
            path: rel_path(rel)?,
            // BORROW: explicit .as_slice() instead of Deref coercion
            sha256: digest_of(bytes.as_slice())?,
            bytes,
            executable,
        })
    }

    /// Records a skipped entry when `entry` is one, and says whether it was.
    fn skip(&mut self, rel: &str, entry: &Entry) -> Result<bool, ImportError> {
        let Some(reason) = skip_reason(entry) else {
            return Ok(false);
        };
        self.skipped.push(Skipped {
            path: rel_path(&join(rel, &entry.name))?,
            reason,
        });
        Ok(true)
    }

    /// Reads every file below `rel` into `files`, skipping what the format skips.
    fn walk(&mut self, rel: &str, files: &mut Vec<SourceFile>) -> Result<(), ImportError> {
        for entry in self.list(rel)? {
            if self.skip(rel, &entry)? {
                continue;
            }
            let path = join(rel, &entry.name);
            if entry.is_dir {
                self.walk(&path, files)?;
            } else {
                files.push(self.source(&path)?);
            }
        }
        Ok(())
    }

    /// Reads every file below `rel` into `files` and records each directory that holds nothing in `empty`.
    fn walk_outputs(
        &mut self,
        rel: &str,
        files: &mut Vec<SourceFile>,
        empty: &mut Vec<RelPath>,
    ) -> Result<(), ImportError> {
        let entries = self.list(rel)?;
        if entries.is_empty() {
            empty.push(rel_path(rel)?);
        }
        for entry in entries {
            if self.skip(rel, &entry)? {
                continue;
            }
            let path = join(rel, &entry.name);
            if entry.is_dir {
                self.walk_outputs(&path, files, empty)?;
            } else {
                files.push(self.source(&path)?);
            }
        }
        Ok(())
    }

    fn folder(&mut self) -> Result<FolderParts, ImportError> {
        let mut provenance = None;
        let mut expectations = None;
        let mut sources = Vec::new();
        let mut cells = Vec::new();
        for entry in self.list("")? {
            if self.skip("", &entry)? {
                continue;
            }
            if entry.is_dir {
                cells.push(self.cell(&entry.name)?);
                continue;
            }
            let file = self.source(&entry.name)?;
            // BORROW: explicit .as_str() instead of Deref coercion
            match entry.name.as_str() {
                "PROVENANCE.json" => provenance = Some(json_file(&file)?),
                EXPECTATIONS_FILE => expectations = Some(jsonl_rows(&file)?),
                // EXPLICIT: any other root file is kept as a folder source and read no further
                _ => {}
            }
            sources.push(file);
        }
        let missing = |file: &str| ImportError::Layout {
            file: file.into(),
            run: "folder".into(),
            detail: "is missing from the folder's root".into(),
        };
        let provenance = provenance.ok_or_else(|| missing("PROVENANCE.json"))?;
        let expectations = expectations.ok_or_else(|| missing(EXPECTATIONS_FILE))?;
        Ok((provenance, expectations, sources, cells))
    }

    fn cell(&mut self, name: &str) -> Result<LegacyCell, ImportError> {
        let mut experiment = None;
        let mut progress = None;
        let mut catalog = None;
        let mut sources = Vec::new();
        let mut runs = Runs::new();
        for entry in self.list(name)? {
            if self.skip(name, &entry)? {
                continue;
            }
            let path = join(name, &entry.name);
            // BORROW: explicit .as_str() instead of Deref coercion
            let entry_name = entry.name.as_str();
            if entry.is_dir {
                if entry_name == "_run_cache" {
                    self.cache_dir(name, &path, &mut runs, &mut sources)?;
                } else if is_eval_dir(entry_name) {
                    self.eval_dir(name, &path, &mut runs, &mut sources)?;
                } else {
                    self.walk(&path, &mut sources)?;
                }
                continue;
            }
            let file = self.source(&path)?;
            match entry_name {
                EXPERIMENT_FILE => experiment = Some(json_file(&file)?),
                PROGRESS_FILE => progress = Some(jsonl_rows(&file)?),
                CONTAMINATION_CATALOG_FILE => catalog = Some(jsonl_rows(&file)?),
                // EXPLICIT: any other cell file is kept as a cell source and read no further
                _ => {}
            }
            sources.push(file);
        }
        let experiment = experiment.ok_or_else(|| ImportError::Layout {
            file: join(name, "experiment.json"),
            run: name.into(),
            detail: "is missing from the cell".into(),
        })?;
        let runs = runs
            .into_values()
            .map(|run| self.finish(run))
            .collect::<Result<Vec<_>, _>>()?;
        sources.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(LegacyCell {
            name: name.into(),
            experiment,
            progress: progress.unwrap_or_default(),
            catalog: catalog.unwrap_or_default(),
            sources,
            runs,
        })
    }

    /// Reads `_run_cache`: one directory per configuration, one per run.
    fn cache_dir(
        &mut self,
        cell: &str,
        rel: &str,
        runs: &mut Runs,
        loose: &mut Vec<SourceFile>,
    ) -> Result<(), ImportError> {
        self.run_dirs(cell, rel, runs, loose, true)
    }

    /// Reads one `eval-K-<title>` directory: one directory per configuration, one per run.
    fn eval_dir(
        &mut self,
        cell: &str,
        rel: &str,
        runs: &mut Runs,
        loose: &mut Vec<SourceFile>,
    ) -> Result<(), ImportError> {
        self.run_dirs(cell, rel, runs, loose, false)
    }

    fn run_dirs(
        &mut self,
        cell: &str,
        rel: &str,
        runs: &mut Runs,
        loose: &mut Vec<SourceFile>,
        cache: bool,
    ) -> Result<(), ImportError> {
        for config in self.list(rel)? {
            if self.skip(rel, &config)? {
                continue;
            }
            let config_rel = join(rel, &config.name);
            if !config.is_dir {
                loose.push(self.source(&config_rel)?);
                continue;
            }
            for run in self.list(&config_rel)? {
                if self.skip(&config_rel, &run)? {
                    continue;
                }
                let run_rel = join(&config_rel, &run.name);
                // BORROW: explicit .as_str() instead of Deref coercion
                let number = run_number(run.name.as_str()).filter(|_| run.is_dir);
                let Some(number) = number else {
                    if run.is_dir {
                        self.walk(&run_rel, loose)?;
                    } else {
                        loose.push(self.source(&run_rel)?);
                    }
                    continue;
                };
                let builder = runs
                    // BORROW: .clone() copies the configuration name into the key
                    .entry((config.name.clone(), number))
                    .or_insert_with(|| RunBuilder::new(cell, &config.name, number));
                self.run_files(&run_rel, builder, cache)?;
            }
        }
        Ok(())
    }

    /// Reads the files of one run directory, of `_run_cache` when `cache` and of an eval directory otherwise.
    fn run_files(
        &mut self,
        rel: &str,
        run: &mut RunBuilder,
        cache: bool,
    ) -> Result<(), ImportError> {
        for entry in self.list(rel)? {
            if self.skip(rel, &entry)? {
                continue;
            }
            let path = join(rel, &entry.name);
            // BORROW: explicit .as_str() instead of Deref coercion
            let name = entry.name.as_str();
            if entry.is_dir {
                if cache && name == OUTPUTS {
                    run.has_outputs = true;
                    self.walk_outputs(&path, &mut run.outputs, &mut run.empty_dirs)?;
                } else {
                    self.walk(&path, &mut run.sources)?;
                }
                continue;
            }
            let file = self.source(&path)?;
            match (cache, name) {
                (true, "agent_stream.jsonl.gz") => {
                    run.stream = Some(file);
                    continue;
                }
                (true, STDERR_FILE) => {
                    run.stderr = Some(file);
                    continue;
                }
                (true, RUN_COMPLETE_FILE) => run.run_complete = Some(json_file(&file)?),
                (false, EVAL_METADATA_FILE) => run.eval_metadata.push(json_file(&file)?),
                (false, TIMING_FILE) => {
                    let copy = json_file(&file)?;
                    match &run.timing {
                        Some(first) if first.value != copy.value => {
                            return Err(ImportError::Disagree {
                                what: "timing files",
                                // BORROW: .clone() copies the run label into the error
                                run: run.label.clone(),
                                first: first.path.to_string(),
                                second: copy.path.to_string(),
                                detail: "the copies of one run hold different documents".into(),
                            });
                        }
                        // EXPLICIT: a copy equal to the first adds nothing to keep
                        Some(_) => {}
                        None => run.timing = Some(copy),
                    }
                }
                (false, grading) if is_grading_name(grading) => {
                    let parsed = json_file(&file)?;
                    if let Some(first) = run
                        .gradings
                        .iter()
                        .find(|other| file_name(&other.path) == grading)
                    {
                        return Err(ImportError::DuplicateGrading {
                            // BORROW: .clone() copies the run label into the error
                            run: run.label.clone(),
                            name: grading.into(),
                            first: first.path.to_string(),
                            second: file.path.to_string(),
                        });
                    }
                    run.gradings.push(parsed);
                }
                // EXPLICIT: any other run file is kept as a run source and read no further
                _ => {}
            }
            run.sources.push(file);
        }
        Ok(())
    }

    /// Checks that a run holds what the format requires and makes it a [`LegacyRun`].
    fn finish(&self, run: RunBuilder) -> Result<LegacyRun, ImportError> {
        let RunBuilder {
            config,
            number,
            label,
            has_outputs,
            mut outputs,
            mut empty_dirs,
            stream,
            stderr,
            mut sources,
            timing,
            run_complete,
            mut eval_metadata,
            mut gradings,
        } = run;
        if !has_outputs {
            return Err(ImportError::NoOutputs { run: label });
        }
        let required = |file: &str| ImportError::Layout {
            file: file.into(),
            // BORROW: .clone() copies the run label into the error
            run: label.clone(),
            detail: "is missing from the run".into(),
        };
        let stream = stream.ok_or_else(|| required("agent_stream.jsonl.gz"))?;
        let timing = timing.ok_or_else(|| required(TIMING_FILE))?;
        let harness_version = self.harness_version(&stream, &label)?;
        outputs.sort_by(|a, b| a.path.cmp(&b.path));
        empty_dirs.sort();
        sources.sort_by(|a, b| a.path.cmp(&b.path));
        eval_metadata.sort_by(|a, b| a.path.cmp(&b.path));
        gradings.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(LegacyRun {
            config,
            number,
            label,
            harness_version,
            stream,
            stderr,
            outputs,
            empty_dirs,
            timing,
            run_complete,
            eval_metadata,
            gradings,
            sources,
        })
    }

    /// The harness version: the string at the rule's pointer in the first line of the stream.
    fn harness_version(&self, stream: &SourceFile, run: &str) -> Result<String, ImportError> {
        let layout = |detail: String| ImportError::Layout {
            file: stream.path.to_string(),
            run: run.into(),
            detail,
        };
        // BORROW: explicit .as_slice() instead of Deref coercion
        let mut lines = std::io::BufReader::new(GzDecoder::new(stream.bytes.as_slice()));
        let mut line = String::new();
        lines
            .read_line(&mut line)
            .map_err(|e| layout(format!("cannot read the first line ({e})")))?;
        let head = serde_json::from_str::<Value>(line.trim())
            .map_err(|e| layout(format!("first line is not JSON ({e})")))?;
        // BORROW: explicit .as_str() instead of Deref coercion
        let pointer = self.rules.harness.version_pointer.as_str();
        match head.pointer(pointer) {
            Some(Value::String(version)) => Ok(version.into()),
            Some(
                Value::Null
                | Value::Bool(_)
                | Value::Number(_)
                | Value::Array(_)
                | Value::Object(_),
            )
            | None => Err(layout(format!(
                "first line has no string at the pointer `{pointer}`"
            ))),
        }
    }
}
