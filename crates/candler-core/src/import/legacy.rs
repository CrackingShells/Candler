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
