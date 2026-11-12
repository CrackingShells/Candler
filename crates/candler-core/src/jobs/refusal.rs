//! The refusal codes of the errors of jobs: the job directory, the lost-job recovery, the
//! orchestration of a job's runs, the supervisor's runtime, the slots and the job view.
//!
//! One `refusal_table!` call per error type: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other.
//!
//! A variant that only wraps another error is a `delegate` row: it has no code and returns the
//! wrapped error's code and path. [`coded`] gives the walk of causes the wrapped error in its
//! place, so a record carries the wrapped error's own side.

use std::error::Error;

use crate::jobs::dir::JobError;
use crate::jobs::lost::RecoverError;
use crate::jobs::orchestrate::session::OrchestrateError;
use crate::jobs::runtime::RuntimeError;
use crate::jobs::slots::SlotError;
use crate::jobs::view::JobViewError;
use crate::refusal::wrapper_table;
use candler_schemas::refusal::{CatalogRow, Catalogued, Refusal, RefusalCode, Side};
use candler_schemas::refusal_table;

/// The path of an error that names no place in a document.
fn no_path<E>(_error: &E) -> Option<String> {
    None
}

/// The pointer of the place in a document where a [`RecoverError`] sits, if it names one.
fn recover_error_path(error: &RecoverError) -> Option<String> {
    match error {
        RecoverError::Job(inner) => Refusal::path(inner),
        RecoverError::Sweep(inner) => Refusal::path(inner),
        RecoverError::Unavailable { .. }
        | RecoverError::Reap { .. }
        | RecoverError::Store { .. } => None,
    }
}

/// The pointer of the place in a document where an [`OrchestrateError`] sits, if it names one.
fn orchestrate_error_path(error: &OrchestrateError) -> Option<String> {
    match error {
        OrchestrateError::Plan(inner) => Refusal::path(inner),
        OrchestrateError::TokenSource { .. }
        | OrchestrateError::Gateway { .. }
        | OrchestrateError::Treatment
        | OrchestrateError::TokenSetting
        | OrchestrateError::Docker { .. }
        | OrchestrateError::Job { .. } => None,
    }
}

/// The rows of [`JobError`], in the order of its variants; the impl below picks one by the
/// variant and, for two of them, by what the error says, which a `refusal_table!` row cannot see.
static JOB_ROWS: [CatalogRow; 9] = [
    CatalogRow {
        enum_name: "JobError",
        variant: "Unavailable",
        code: "candler.internal",
        side: Side::Candler,
        when: "The job directory port is not wired in this build.",
        at: "-",
        delegates: false,
    },
    CatalogRow {
        enum_name: "JobError",
        variant: "Held",
        code: "jobs.job_held",
        side: Side::Environment,
        when: "Another process holds the job, so this one cannot take it.",
        at: "-",
        delegates: false,
    },
    CatalogRow {
        enum_name: "JobError",
        variant: "Io",
        code: "jobs.job_not_found",
        side: Side::Config,
        when: "No directory of the store holds the job whose id was given.",
        at: "-",
        delegates: false,
    },
    CatalogRow {
        enum_name: "JobError",
        variant: "Io",
        code: "jobs.io_failed",
        side: Side::Environment,
        when: "A file system call on the job's directory failed, such as a read, a write or a rename, or the store's `jobs` directory cannot be entered.",
        at: "-",
        delegates: false,
    },
    CatalogRow {
        enum_name: "JobError",
        variant: "Parse",
        code: "jobs.job_not_a_directory",
        side: Side::Environment,
        when: "The path of a job in the store is not a directory.",
        at: "-",
        delegates: false,
    },
    CatalogRow {
        enum_name: "JobError",
        variant: "Parse",
        code: "candler.internal",
        side: Side::Candler,
        when: "A file of the job directory is not a valid record.",
        at: "-",
        delegates: false,
    },
    CatalogRow {
        enum_name: "JobError",
        variant: "Exists",
        code: "jobs.directory_exists",
        side: Side::Environment,
        when: "The directory of a job to create already exists.",
        at: "-",
        delegates: false,
    },
    CatalogRow {
        enum_name: "JobError",
        variant: "Mismatch",
        code: "candler.internal",
        side: Side::Candler,
        when: "The record in a job directory names another job.",
        at: "-",
        delegates: false,
    },
    CatalogRow {
        enum_name: "JobError",
        variant: "Corrupt",
        code: "candler.internal",
        side: Side::Candler,
        when: "A line of a job's event log is not an event.",
        at: "-",
        delegates: false,
    },
];

/// The operation of `JobDir::open` that finds the directory of a job.
const OPEN_JOB: &str = "open the job directory";

/// The text of the error that `ENOENT` gives, which `JobError::Io` keeps as its `detail`.
const NO_SUCH_ENTRY: &str = "(os error 2)";

/// The row of `error`: the kind of the file system error decides between a job that is not there
/// and a call that failed, and the one text that `JobDir::open` gives a path that is no directory
/// decides between the environment's and Candler's own.
fn job_row(error: &JobError) -> &'static CatalogRow {
    match error {
        JobError::Unavailable { .. } => &JOB_ROWS[0],
        JobError::Held => &JOB_ROWS[1],
        JobError::Io { op, detail, .. } if *op == OPEN_JOB && detail.ends_with(NO_SUCH_ENTRY) => {
            &JOB_ROWS[2]
        }
        JobError::Io { .. } => &JOB_ROWS[3],
        JobError::Parse { detail, .. } if detail == "not a directory" => &JOB_ROWS[4],
        JobError::Parse { .. } => &JOB_ROWS[5],
        JobError::Exists { .. } => &JOB_ROWS[6],
        JobError::Mismatch { .. } => &JOB_ROWS[7],
        JobError::Corrupt { .. } => &JOB_ROWS[8],
    }
}

impl Refusal for JobError {
    fn code(&self) -> RefusalCode {
        RefusalCode::from_checked(job_row(self).code)
    }

    fn side(&self) -> Side {
        job_row(self).side
    }

    fn path(&self) -> Option<String> {
        None
    }
}

impl Catalogued for JobError {
    fn rows() -> &'static [CatalogRow] {
        &JOB_ROWS
    }
}

wrapper_table! {
    RecoverError, path = recover_error_path;
    Unavailable { .. } => row(
        "candler.internal",
        Side::Candler,
        "The recovery port is not wired in this build.",
        "-",
    ),
    Job (inner) => delegate(inner, Side::Environment),
    Reap { .. } => row(
        "jobs.child_not_reaped",
        Side::Environment,
        "A host process left behind by a lost job could not be reaped.",
        "-",
    ),
    Sweep (inner) => delegate(inner, Side::Environment),
    Store { .. } => row(
        "jobs.recovery_store_failed",
        Side::Environment,
        "The store could not be read or written while a lost job was recovered.",
        "-",
    ),
}

wrapper_table! {
    OrchestrateError, path = orchestrate_error_path;
    TokenSource { .. } => row(
        "jobs.token_secret_unreadable",
        Side::Environment,
        "The secret that `model.token_secret` names cannot be read.",
        "-",
    ),
    Gateway { .. } => row(
        "jobs.gateway_not_serving",
        Side::Environment,
        "The gateway of the job does not serve, for example because it did not start or refused the token.",
        "-",
    ),
    Treatment { .. } => row(
        "jobs.treatment_unset",
        Side::Config,
        "The setting `request.treatment` is not set to a treatment.",
        "-",
    ),
    TokenSetting { .. } => row(
        "jobs.token_secret_unset",
        Side::Config,
        "The setting `model.token_secret` names no secret.",
        "-",
    ),
    Plan (inner) => delegate(inner, Side::Experiment),
    Docker { .. } => row(
        "jobs.docker_unavailable",
        Side::Environment,
        "The docker client cannot be reached when a job's session starts.",
        "-",
    ),
    Job { .. } => row(
        "jobs.job_directory_unavailable",
        Side::Environment,
        "The job's directory cannot be opened or read when its session starts.",
        "-",
    ),
}

refusal_table! {
    RuntimeError, path = no_path;
    SpecDigest { .. } => row(
        "candler.internal",
        Side::Candler,
        "The spec of a job is not the one whose digest the launcher recorded, or the digest cannot be written down.",
        "-",
    ),
    AlreadySupervised { .. } => row(
        "jobs.already_supervised",
        Side::Environment,
        "Another supervisor already holds the lock of the job.",
        "-",
    ),
    Session { .. } => row(
        "candler.internal",
        Side::Candler,
        "The supervisor could not take its signal handlers, its session or the subreaper role.",
        "-",
    ),
    Job { .. } => row(
        "jobs.job_directory_failed",
        Side::Environment,
        "The job's directory, its log or its state failed while the supervisor ran.",
        "-",
    ),
    Setup { .. } => row(
        "candler.internal",
        Side::Candler,
        "The job could not be set up: the cause is text only, such as a configuration, a slot or a plug-in directory.",
        "-",
    ),
    SuiteChanged { .. } => row(
        "evaluation.suite_changed",
        Side::Suite,
        "The suite file of an evaluate job changed between the launcher reading it and the supervisor running the job, so the job refuses to grade with another suite.",
        "-",
    ),
    Body { .. } => row(
        "candler.internal",
        Side::Candler,
        "The body of the job failed; the cause is text only and is in the job's log.",
        "-",
    ),
    DidNotStart { .. } => row(
        "jobs.supervisor_did_not_start",
        Side::Environment,
        "The detached supervisor did not take the job's lock by the deadline, or exited first; its log says why.",
        "-",
    ),
    Detach { .. } => row(
        "jobs.supervisor_spawn_failed",
        Side::Environment,
        "The supervisor process could not be started or given the job's spec.",
        "-",
    ),
}

refusal_table! {
    SlotError, path = no_path;
    Unavailable { .. } => row(
        "candler.internal",
        Side::Candler,
        "The slot port is not wired in this build.",
        "-",
    ),
    Io { .. } => row(
        "jobs.slot_io_failed",
        Side::Environment,
        "A file system call on the slots directory failed, such as opening or locking a slot file.",
        "-",
    ),
}

refusal_table! {
    JobViewError, path = no_path;
    Unavailable { .. } => row(
        "candler.internal",
        Side::Candler,
        "The job view port is not wired in this build.",
        "-",
    ),
    Read { .. } => row(
        "jobs.job_unreadable",
        Side::Environment,
        "The files of a job's directory cannot be read.",
        "-",
    ),
}

/// The rows of every coded error of this module, in the order of the types.
#[must_use]
pub(crate) fn catalog() -> Vec<CatalogRow> {
    let tables: [&'static [CatalogRow]; 6] = [
        <JobError as Catalogued>::rows(),
        <RecoverError as Catalogued>::rows(),
        <OrchestrateError as Catalogued>::rows(),
        <RuntimeError as Catalogued>::rows(),
        <SlotError as Catalogued>::rows(),
        <JobViewError as Catalogued>::rows(),
    ];
    tables
        .iter()
        .flat_map(|rows| rows.iter().copied())
        .collect()
}

/// Views a coded error as its [`Refusal`].
fn as_refusal<'e, E: Refusal>(found: &'e E) -> &'e dyn Refusal {
    found
}

/// The refusal that answers for a [`RecoverError`].
fn recover_answer(error: &RecoverError) -> &dyn Refusal {
    match error {
        RecoverError::Job(inner) => crate::refusal::through(inner, error),
        RecoverError::Sweep(inner) => crate::refusal::through(inner, error),
        RecoverError::Unavailable { .. }
        | RecoverError::Reap { .. }
        | RecoverError::Store { .. } => error,
    }
}

/// The refusal that answers for an [`OrchestrateError`].
fn orchestrate_answer(error: &OrchestrateError) -> &dyn Refusal {
    match error {
        OrchestrateError::Plan(inner) => crate::refusal::through(inner, error),
        OrchestrateError::TokenSource { .. }
        | OrchestrateError::Gateway { .. }
        | OrchestrateError::Treatment
        | OrchestrateError::TokenSetting
        | OrchestrateError::Docker { .. }
        | OrchestrateError::Job { .. } => error,
    }
}

/// The coded error of this module that `error` is, or `None` when it is none of them.
///
/// A variant that only wraps another coded error answers with the wrapped error's refusal, so
/// the side of a record is the one of the error that arose and not of the wrapper.
#[must_use]
pub(crate) fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    if let Some(found) = error.downcast_ref::<RecoverError>() {
        return Some(recover_answer(found));
    }
    if let Some(found) = error.downcast_ref::<OrchestrateError>() {
        return Some(orchestrate_answer(found));
    }
    error
        .downcast_ref::<JobError>()
        .map(as_refusal)
        .or_else(|| error.downcast_ref::<RuntimeError>().map(as_refusal))
        .or_else(|| error.downcast_ref::<SlotError>().map(as_refusal))
        .or_else(|| error.downcast_ref::<JobViewError>().map(as_refusal))
}
