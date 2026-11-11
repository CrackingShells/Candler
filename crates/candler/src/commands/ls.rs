//! `candler ls`: lists the experiments of a project.
//!
//! The roots are `<project root>/experiments/`, the project being the one `candler.toml` names,
//! else `<working directory>/experiments/`, and each path given. A directory is walked for
//! `*.toml` in sorted order and a file is read as it is. Nothing is skipped silently: a file that
//! does not read as `experiment/1` is a `skipped` line with the error's text, and so is a
//! symbolic link, which is never followed out of a root.
//!
//! Each experiment line is `<path>\t<cells>\t<runs>`: the number of cells the file declares and
//! the number of sealed stored runs that match it. A run matches when it records, for every value
//! one cell of the file declares, that value at that path; a setting is no part of a cell and is
//! not compared. The experiment digest a manifest records needs a preflight, which `ls` does not
//! run, so two files that declare the same values count the same runs. A skipped line is
//! `<path>\tskipped\t<reason>`.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use candler_core::resolve::config::{Config, ConfigEnv};
use candler_core::resolve::layers::is_setting;
use candler_core::store::Store;
use candler_schemas::execution::ExperimentFile;
use candler_schemas::primitives::FieldPath;
use serde_json::Value;

use super::term::Paint;
use super::{CliError, Exit, common};

/// The directory of a project that holds its experiments.
const EXPERIMENTS: &str = "experiments";

/// The arguments of `candler ls`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Files or directories to list besides the project's `experiments/` directory.
    pub paths: Vec<PathBuf>,
}

/// One line of the listing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Listing {
    /// A file that reads as an experiment.
    Experiment {
        /// The file.
        path: PathBuf,
        /// How many cells it declares.
        cells: usize,
        /// How many sealed stored runs match it.
        runs: usize,
    },
    /// A file that is no experiment, or could not be read.
    Skipped {
        /// The file.
        path: PathBuf,
        /// Why it was skipped.
        reason: String,
    },
}

impl Listing {
    /// The line, without a trailing line feed.
    #[must_use]
    pub fn line(&self, paint: Paint) -> String {
        match self {
            Self::Experiment { path, cells, runs } => {
                let runs = if *runs > 0 {
                    paint.green(&runs.to_string())
                } else {
                    runs.to_string()
                };
                format!("{}\t{cells}\t{runs}", path.display())
            }
            Self::Skipped { path, reason } => format!(
                "{}\t{}\t{}",
                path.display(),
                paint.dim("skipped"),
                reason.split_whitespace().collect::<Vec<_>>().join(" ")
            ),
        }
    }
}

/// The roots to list: the project's `experiments/` (or the working directory's, with no project)
/// and each path given, made absolute against `cwd`.
#[must_use]
pub fn list_roots(project_root: Option<&Path>, cwd: &Path, given: &[PathBuf]) -> Vec<PathBuf> {
    let base = project_root.unwrap_or(cwd);
    let mut roots = vec![base.join(EXPERIMENTS)];
    roots.extend(given.iter().map(|path| cwd.join(path)));
    roots
}

/// Adds the `*.toml` files under `dir` to `found`, each with the reason it is skipped before it is
/// read, if there is one. A symbolic link is listed and not followed.
fn walk(dir: &Path, found: &mut BTreeMap<PathBuf, Option<String>>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = read.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let is_toml = path.extension().is_some_and(|ext| ext == "toml");
        if meta.file_type().is_symlink() {
            if is_toml || path.is_dir() {
                found.insert(
                    path,
                    Some("a symbolic link, which is not followed".to_owned()), // BORROW: the entry owns its text
                );
            }
        } else if meta.is_dir() {
            walk(&path, found);
        } else if is_toml {
            found.insert(path, None);
        }
    }
}

/// The recorded values of every sealed run of the store, by path. No store is no run.
fn stored_runs(root: &Path) -> Vec<BTreeMap<FieldPath, Value>> {
    let Ok(store) = Store::open(root) else {
        return Vec::new();
    };
    let Ok(entries) = store.runs() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|entry| entry.done)
        .filter_map(|entry| store.read_manifest(&entry.key).ok())
        .map(|manifest| {
            manifest
                .comparable()
                .values()
                .iter()
                .filter_map(|resolved| {
                    // BORROW: .clone() gives the map its own path and value
                    let value = resolved.value()?.as_value().clone();
                    Some((resolved.path().clone(), value))
                })
                .collect()
        })
        .collect()
}

/// How many of `runs` match a cell of `file`.
fn matching_runs(file: &ExperimentFile, runs: &[BTreeMap<FieldPath, Value>]) -> usize {
    let cells = file.cells();
    runs.iter()
        .filter(|run| {
            cells.iter().any(|cell| {
                cell.iter().all(|declared| {
                    is_setting(declared.path())
                        || declared
                            .value()
                            .is_some_and(|value| run.get(declared.path()) == Some(value.as_value()))
                })
            })
        })
        .count()
}

/// The listing of `files`, each read as an experiment, in the order of `files`.
#[must_use]
pub fn list_files(
    files: &BTreeMap<PathBuf, Option<String>>,
    runs: &[BTreeMap<FieldPath, Value>],
) -> Vec<Listing> {
    files
        .iter()
        .map(|(path, skipped)| {
            let skip = |reason: String| Listing::Skipped {
                path: path.clone(), // BORROW: the line owns its path
                reason,
            };
            if let Some(reason) = skipped {
                return skip(reason.clone()); // BORROW: the line owns its reason
            }
            let text = match std::fs::read_to_string(path) {
                Ok(text) => text,
                Err(error) => return skip(error.to_string()),
            };
            match ExperimentFile::from_toml_str(text.as_str()) {
                // BORROW: .as_str() views the text as a `&str`
                Ok(file) => Listing::Experiment {
                    path: path.clone(), // BORROW: the line owns its path
                    cells: file.cells().len(),
                    runs: matching_runs(&file, runs),
                },
                Err(error) => skip(error.to_string()),
            }
        })
        .collect()
}

/// Lists the experiments of the project and of the paths given.
///
/// # Errors
/// [`CliError::Failed`] when the configuration is unreadable or the output cannot be written.
pub async fn run(args: Args, store: Option<&Path>) -> Result<Exit, CliError> {
    let cwd = std::env::current_dir()
        .map_err(|e| CliError::refused(format!("no working directory: {e}")))?;
    let config =
        Config::load(&cwd, &ConfigEnv::from_process()).map_err(|e| CliError::failed("ls", e))?;
    let project_root = config
        .project
        .as_ref()
        .map(|project| project.root.as_path());
    let mut files: BTreeMap<PathBuf, Option<String>> = BTreeMap::new();
    for root in list_roots(project_root, &cwd, &args.paths) {
        if root.is_dir() {
            walk(&root, &mut files);
        } else if root.exists() || args.paths.iter().any(|given| cwd.join(given) == root) {
            files.entry(root).or_insert(None);
        }
    }
    let runs = match common::store_root("ls", store).await {
        Ok(root) => stored_runs(&root),
        Err(_) => Vec::new(),
    };
    let paint = Paint::detect();
    let mut stdout = std::io::stdout().lock();
    for listing in list_files(&files, &runs) {
        writeln!(stdout, "{}", listing.line(paint)).map_err(|e| CliError::failed("ls", e))?;
    }
    Ok(Exit::Success)
}
