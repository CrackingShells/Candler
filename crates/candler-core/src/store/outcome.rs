//! Outcome files: one file per request, visible all at once and never replaced.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::{NameVersion, RequestId, RunKey};

use super::layout::OUTCOME_EXTENSION;
use super::sync::sync_file;
use super::{Store, StoreError, io_error, sync_dir, visible_names};

/// The name of one outcome file: the run, the evaluator and the request it answers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub struct OutcomeFileRef {
    /// The run the outcomes grade.
    pub run: RunKey,
    /// The evaluator that produced them.
    pub evaluator: NameVersion,
    /// The request they answer.
    pub request: RequestId,
}

impl OutcomeFileRef {
    /// Names the outcome file of `request` for `evaluator` on `run`.
    #[must_use]
    pub const fn new(run: RunKey, evaluator: NameVersion, request: RequestId) -> Self {
        Self {
            run,
            evaluator,
            request,
        }
    }
}

/// Writes the outcomes of one request: lines go to a `.partial` file that readers never see, and
/// [`OutcomeWriter::commit`] makes the whole file visible at once.
#[derive(Debug)]
#[non_exhaustive]
pub struct OutcomeWriter {
    store: Store,
    target: OutcomeFileRef,
    partial: PathBuf,
    file: File,
}

impl OutcomeWriter {
    /// Opens the `.partial` file of a request as a new file, replacing one an earlier writer left;
    /// a writer holds its own file, so no later writer reaches a file already published.
    ///
    /// # Errors
    /// Returns [`StoreError::OutcomeExists`] when the final file exists, and [`StoreError::Io`]
    /// when the file cannot be opened.
    pub fn create(
        store: &Store,
        run: &RunKey,
        evaluator: &NameVersion,
        request: &RequestId,
    ) -> Result<Self, StoreError> {
        let layout = store.layout();
        let last = layout.outcome(run, evaluator, request);
        if last.exists() {
            return Err(StoreError::OutcomeExists { path: last });
        }
        let dir = layout.evaluator_dir(run, evaluator);
        fs::create_dir_all(&dir).map_err(io_error("create an outcome directory", &dir))?;
        let partial = layout.outcome_partial(run, evaluator, request);
        match fs::remove_file(&partial) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(io_error("remove a stale partial file", &partial)(source)),
        }
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
            .map_err(io_error("open a partial outcome file", &partial))?;
        Ok(Self {
            store: store.clone(), // BORROW: the writer owns a handle on the store
            target: OutcomeFileRef::new(
                run.clone(),       // BORROW: the writer owns the key
                evaluator.clone(), // BORROW: the writer owns the evaluator
                request.clone(),   // BORROW: the writer owns the request id
            ),
            partial,
            file,
        })
    }

    /// Appends one record as a line and flushes it.
    ///
    /// # Errors
    /// Returns [`StoreError::RecordMismatch`] when the record is for another run, evaluator or
    /// request, and [`StoreError::Io`] when the write fails.
    pub fn append(&mut self, record: &OutcomeRecord) -> Result<(), StoreError> {
        if record.run() != &self.target.run {
            return Err(StoreError::RecordMismatch { detail: "run" });
        }
        if record.evaluator() != &self.target.evaluator {
            return Err(StoreError::RecordMismatch {
                detail: "evaluator",
            });
        }
        if record.request_id() != &self.target.request {
            return Err(StoreError::RecordMismatch { detail: "request" });
        }
        let mut line = serde_json::to_vec(record).map_err(|e| StoreError::Parse {
            path: self.partial.clone(), // BORROW: the error owns the path
            detail: e.to_string(),
        })?;
        line.push(b'\n');
        self.file
            .write_all(&line)
            .and_then(|()| self.file.flush())
            .map_err(io_error("append an outcome", &self.partial))
    }

    /// Syncs the file and publishes it under its final name, which is never replaced.
    ///
    /// # Errors
    /// Returns [`StoreError::OutcomeExists`] when the final file exists, and [`StoreError::Io`]
    /// when a sync or the publication fails.
    pub fn commit(self) -> Result<(), StoreError> {
        let layout = self.store.layout();
        let last = layout.outcome(
            &self.target.run,
            &self.target.evaluator,
            &self.target.request,
        );
        sync_file(&self.file).map_err(io_error("sync an outcome file", &self.partial))?;
        if last.exists() {
            return Err(StoreError::OutcomeExists { path: last });
        }
        // A hard link fails when the name is taken, where a rename would replace it.
        match fs::hard_link(&self.partial, &last) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                return Err(StoreError::OutcomeExists { path: last });
            }
            Err(source) => return Err(io_error("publish an outcome file", &last)(source)),
        }
        fs::remove_file(&self.partial)
            .map_err(io_error("remove a published partial file", &self.partial))?;
        match last.parent() {
            Some(dir) => sync_dir(dir),
            None => Ok(()),
        }
    }
}

impl Store {
    /// The finished outcome files of a run, in evaluator then request order. Partial files are not
    /// listed.
    ///
    /// # Errors
    /// Returns [`StoreError::Io`] when a directory cannot be listed.
    pub fn outcome_files(&self, run: &RunKey) -> Result<Vec<OutcomeFileRef>, StoreError> {
        let layout = self.layout();
        let mut files = Vec::new();
        for name in visible_names(&layout.run_outcomes_dir(run))? {
            // BORROW: explicit .as_str() instead of Deref coercion
            let Ok(evaluator) = name.as_str().parse::<NameVersion>() else {
                // BORROW: explicit .as_str() instead of Deref coercion
                continue;
            };
            for file in visible_names(&layout.evaluator_dir(run, &evaluator))? {
                let stem = file
                    // BORROW: explicit .as_str() instead of Deref coercion
                    .as_str() // BORROW: explicit .as_str() instead of Deref coercion
                    .strip_suffix(&format!(".{OUTCOME_EXTENSION}"))
                    .and_then(|text| text.parse::<RequestId>().ok());
                if let Some(request) = stem {
                    files.push(OutcomeFileRef::new(
                        run.clone(),       // BORROW: each reference owns its key
                        evaluator.clone(), // BORROW: each reference owns its evaluator
                        request,
                    ));
                }
            }
        }
        Ok(files)
    }

    /// The records of one outcome file, in order.
    ///
    /// # Errors
    /// Returns [`StoreError::Io`] when the file cannot be read and [`StoreError::Parse`] when a
    /// line is not an outcome record.
    pub fn read_outcomes(&self, file: &OutcomeFileRef) -> Result<Vec<OutcomeRecord>, StoreError> {
        let path = self
            .layout()
            .outcome(&file.run, &file.evaluator, &file.request);
        let text = fs::read_to_string(&path).map_err(io_error("read an outcome file", &path))?;
        text.lines()
            .enumerate()
            .map(|(index, line)| {
                serde_json::from_str::<OutcomeRecord>(line).map_err(|e| StoreError::Parse {
                    path: path.clone(), // BORROW: the error owns the path
                    detail: format!("line {}: {e}", index.saturating_add(1)),
                })
            })
            .collect()
    }
}
