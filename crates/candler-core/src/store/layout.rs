//! Every path of a store, built in one place.

use std::path::{Path, PathBuf};

use candler_schemas::primitives::{Digest, JobId, NameVersion, RequestId, RunKey};
use candler_schemas::raw::StreamName;

/// The name of the marker file at the root of a store.
const MARKER: &str = "store.json";
/// The extension of a finished outcome file.
pub(crate) const OUTCOME_EXTENSION: &str = "jsonl";
/// The suffix of an outcome file that is still being written.
const PARTIAL_SUFFIX: &str = ".partial";

/// The paths of a store rooted at one directory; it is the only code that builds a store path.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct StoreLayout {
    root: PathBuf,
}

impl StoreLayout {
    /// Lays out a store under `root`.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// The root directory.
    #[must_use]
    pub fn root<'a>(&'a self) -> &'a Path {
        self.root.as_path()
    }

    /// The marker file `store.json`.
    #[must_use]
    pub fn marker(&self) -> PathBuf {
        self.root.join(MARKER)
    }

    /// The directory that holds the run directories.
    #[must_use]
    pub fn runs_dir(&self) -> PathBuf {
        self.root.join("runs")
    }

    /// The directory of one run: `runs/<cell hex>-r<replicate>-a<attempt>`.
    #[must_use]
    pub fn run_dir(&self, run: &RunKey) -> PathBuf {
        self.runs_dir().join(run.to_string())
    }

    /// The directory that receives the unfinished runs a retry moved aside: `runs/.replaced/<key>-<n>`.
    #[must_use]
    pub fn replaced_dir(&self, run: &RunKey, n: u32) -> PathBuf {
        self.runs_dir().join(".replaced").join(format!("{run}-{n}"))
    }

    /// The manifest of a run.
    #[must_use]
    pub fn manifest(&self, run: &RunKey) -> PathBuf {
        self.run_dir(run).join("manifest.json")
    }

    /// The workspace tree of a run.
    #[must_use]
    pub fn tree(&self, run: &RunKey) -> PathBuf {
        self.run_dir(run).join("tree.json")
    }

    /// The normalised trace of a run.
    #[must_use]
    pub fn trace(&self, run: &RunKey) -> PathBuf {
        self.run_dir(run).join("trace.jsonl")
    }

    /// The seal of a run: present once the run is complete.
    #[must_use]
    pub fn done(&self, run: &RunKey) -> PathBuf {
        self.run_dir(run).join("run.done")
    }

    /// The directory of a run's raw streams.
    #[must_use]
    pub fn raw_dir(&self, run: &RunKey) -> PathBuf {
        self.run_dir(run).join("raw")
    }

    /// The finished raw file of one stream of a run.
    #[must_use]
    pub fn raw(&self, run: &RunKey, stream: &StreamName) -> PathBuf {
        self.raw_dir(run).join(format!("{stream}.jsonl.gz"))
    }

    /// The directory that holds the blobs.
    #[must_use]
    pub fn blobs_dir(&self) -> PathBuf {
        self.root.join("blobs")
    }

    /// The file of a blob: `blobs/<hex>`.
    #[must_use]
    pub fn blob(&self, digest: &Digest) -> PathBuf {
        self.blobs_dir().join(digest.hex())
    }

    /// The directory that holds the outcome files of every run.
    #[must_use]
    pub fn outcomes_dir(&self) -> PathBuf {
        self.root.join("outcomes")
    }

    /// The directory of one run's outcome files, one subdirectory per evaluator.
    #[must_use]
    pub fn run_outcomes_dir(&self, run: &RunKey) -> PathBuf {
        self.outcomes_dir().join(run.to_string())
    }

    /// The directory of one evaluator's outcome files for a run.
    #[must_use]
    pub fn evaluator_dir(&self, run: &RunKey, evaluator: &NameVersion) -> PathBuf {
        self.run_outcomes_dir(run).join(evaluator.as_str()) // BORROW: explicit .as_str() instead of Deref coercion
    }

    /// The finished outcome file of one request.
    #[must_use]
    pub fn outcome(&self, run: &RunKey, evaluator: &NameVersion, request: &RequestId) -> PathBuf {
        self.evaluator_dir(run, evaluator)
            .join(format!("{request}.{OUTCOME_EXTENSION}"))
    }

    /// The outcome file of one request while it is being written.
    #[must_use]
    pub fn outcome_partial(
        &self,
        run: &RunKey,
        evaluator: &NameVersion,
        request: &RequestId,
    ) -> PathBuf {
        self.evaluator_dir(run, evaluator)
            .join(format!("{request}.{OUTCOME_EXTENSION}{PARTIAL_SUFFIX}"))
    }

    /// The directory of one job.
    #[must_use]
    pub fn job_dir(&self, job: &JobId) -> PathBuf {
        self.root.join("jobs").join(job.as_str()) // BORROW: explicit .as_str() instead of Deref coercion
    }

    /// The lock file of the supervisor of one experiment.
    #[must_use]
    pub fn experiment_lock(&self, experiment: &Digest) -> PathBuf {
        self.root
            .join("experiments")
            .join(experiment.hex())
            .join("lock")
    }
}
