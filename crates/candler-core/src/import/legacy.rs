//! The names of the legacy format: the keys, files and directories the importer reads, each written once.

/// The `experiment.json` object that holds the cell's plan.
pub(super) const PLAN: &str = "plan";
/// The object that holds the provenance of a cell or of a run's metadata copy.
pub(super) const PROVENANCE: &str = "provenance";
/// The path of the plan's output directory, which is left out of the cell's values.
pub(super) const OUT_DIR: &str = "plan.out_dir";
/// The directory of a run's artifacts.
pub(super) const OUTPUTS: &str = "outputs";
/// The key that names the configuration a row or event belongs to.
pub(super) const CONFIG: &str = "config";
/// The key that holds the run number of a row or event.
pub(super) const RUN_NUMBER: &str = "run_number";
/// The key of a grading entry that names what blocked it.
pub(super) const BLOCKED_BY: &str = "blocked_by";
/// The key of the timing file that lists the inputs copied into the run.
pub(super) const COPIED_INPUTS: &str = "copied_inputs";
/// The key of a row, and of a copied input, that names its role.
pub(super) const ROLE: &str = "role";
/// The key of a copied input that holds its hex digest.
pub(super) const SHA256: &str = "sha256";
/// The key of a grading file that lists its entries.
pub(super) const EXPECTATIONS: &str = "expectations";
/// The file at the folder's root that lists one row per entry.
pub(super) const EXPECTATIONS_FILE: &str = "expectations.jsonl";
/// The file of a run that holds a copy of its metadata.
pub(super) const EVAL_METADATA_FILE: &str = "eval_metadata.json";
/// The file of a run that holds its timing.
pub(super) const TIMING_FILE: &str = "timing.json";
/// The file of a run that holds its completion time.
pub(super) const RUN_COMPLETE_FILE: &str = "run_complete.json";
/// The file of a run that holds the agent's standard error.
pub(super) const STDERR_FILE: &str = "agent_stderr.txt";
/// The key of the folder's provenance file that lists its runs.
pub(super) const RUNS: &str = "runs";
/// The key of a run's legacy integrity block in the provenance file.
pub(super) const ORIGINAL_INTEGRITY: &str = "original_integrity";
/// The key of an integrity verdict.
pub(super) const VERDICT: &str = "verdict";
/// The key of the completion time in a run's completion file.
pub(super) const COMPLETED_AT: &str = "completed_at";
/// The key of the elapsed seconds of a progress event.
pub(super) const ELAPSED_SECONDS: &str = "elapsed_seconds";
/// The kind of the progress event that opens a run.
pub(super) const EVENT_RUN_START: &str = "run_start";
/// The kind of the progress event that closes a run.
pub(super) const EVENT_RUN_COMPLETE: &str = "run_complete";
/// The keys of the timing file that count tokens, in the order a report lists them.
pub(super) const TOKEN_COUNTERS: [&str; 4] = [
    "input_tokens",
    "output_tokens",
    "cache_creation_tokens",
    "cache_read_tokens",
];
/// The key of the timing file that names the outputs the legacy grader collected.
pub(super) const COLLECTED_OUTPUTS: &str = "collected_outputs_paths";
/// The pointer of the evaluator's `name@version` in the folder's provenance file.
pub(super) const PINNED_DESCRIBE: &str = "/pinned/describe";
/// The pointer of the commit the evaluator ran at in the folder's provenance file.
pub(super) const PINNED_COMMIT: &str = "/pinned/commit";
/// The pointer of the flag that says the evaluator's checkout had uncommitted changes.
pub(super) const PINNED_DIRTY: &str = "/pinned/dirty";
/// The pointer of the commit of the legacy repository the corpus was read at.
pub(super) const CORPUS_BASE_COMMIT: &str = "/corpus/base_commit";
/// The file of a cell that lists the contamination catalog's rows.
pub(super) const CONTAMINATION_CATALOG_FILE: &str = "contamination_catalog.jsonl";
/// The file of a cell that lists its progress events.
pub(super) const PROGRESS_FILE: &str = "progress.jsonl";
/// The file of a cell that holds its experiment.
pub(super) const EXPERIMENT_FILE: &str = "experiment.json";
/// The prefix of an eval directory and of a grading file name.
pub(super) const EVAL_PREFIX: &str = "eval-";
/// The suffix of a grading file name.
pub(super) const GRADING_SUFFIX: &str = "-grading.json";
