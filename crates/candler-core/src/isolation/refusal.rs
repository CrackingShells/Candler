//! The refusal codes of the errors of container isolation: the container guard, the mounts, the argument vector, the docker client, the gateway and the images.
//!
//! One `refusal_table!` call per error type: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other.

use std::error::Error;

use crate::isolation::IsolationError;
use crate::isolation::container::guard::GuardError;
use crate::isolation::container::mounts::MountError;
use crate::isolation::container::run::RunArgvError;
use crate::isolation::container::sweeper::SweepError;
use crate::isolation::docker::DockerError;
use crate::isolation::gateway::{GatewayError, NotReadyCause, SecretError};
use crate::isolation::image::ImageError;
use candler_schemas::refusal::{CatalogRow, Catalogued, Refusal, Side};
use candler_schemas::refusal_table;

/// The path of an error that names no place in a document.
fn no_path<E>(_error: &E) -> Option<String> {
    None
}

/// The pointer of the place in a document where a [`GatewayError`] sits, if it names one.
fn gateway_error_path(error: &GatewayError) -> Option<String> {
    match error {
        GatewayError::Secret(inner) => Refusal::path(inner),
        GatewayError::NotReady { cause } => Refusal::path(cause),
        GatewayError::Unavailable { .. }
        | GatewayError::Start { .. }
        | GatewayError::SecretFormat { .. }
        | GatewayError::Config { .. } => None,
    }
}

/// The pointer of the place in a document where a [`RunArgvError`] sits, if it names one.
fn run_argv_error_path(error: &RunArgvError) -> Option<String> {
    match error {
        RunArgvError::Mount(inner) => Refusal::path(inner),
        RunArgvError::SecretRefused { .. }
        | RunArgvError::ReservedName { .. }
        | RunArgvError::DuplicateName { .. }
        | RunArgvError::BadEnvName { .. }
        | RunArgvError::MissingLabel { .. }
        | RunArgvError::BadCidfile
        | RunArgvError::Command { .. }
        | RunArgvError::Request { .. } => None,
    }
}

refusal_table! {
    IsolationError, path = no_path;
    Unavailable { .. } => row(
        "isolation.step_unavailable",
        Side::Environment,
        "The container runtime has no isolation for the step, so the run cannot be placed.",
        "-",
    ),
    Plan { .. } => row(
        "candler.internal",
        Side::Candler,
        "A cell value the container plan needs is missing or unusable.",
        "-",
    ),
    Docker { .. } => row(
        "isolation.docker_step_failed",
        Side::Environment,
        "Docker failed while the run was being placed in a container.",
        "-",
    ),
}

refusal_table! {
    DockerError, path = no_path;
    Unavailable { .. } => row(
        "isolation.docker_unavailable",
        Side::Environment,
        "The docker client cannot be reached or does not offer the call.",
        "-",
    ),
    Failed { .. } => row(
        "isolation.docker_failed",
        Side::Environment,
        "Docker exited with an error.",
        "-",
    ),
    Unreadable { .. } => row(
        "candler.internal",
        Side::Candler,
        "Docker's answer cannot be read.",
        "-",
    ),
    NotFound { .. } => row(
        "isolation.docker_resource_missing",
        Side::Environment,
        "Docker has no such container, network or image.",
        "-",
    ),
    NotAFullId { .. } => row(
        "candler.internal",
        Side::Candler,
        "A container call named something other than a full container id.",
        "-",
    ),
    NotAJobNetwork { .. } => row(
        "candler.internal",
        Side::Candler,
        "A network call named something other than a job's network.",
        "-",
    ),
}

refusal_table! {
    SecretError, path = no_path;
    Missing { .. } => row(
        "isolation.secret_missing",
        Side::Environment,
        "A secret the experiment declares is not set in the environment.",
        "-",
    ),
}

refusal_table! {
    GatewayError, path = gateway_error_path;
    Unavailable { .. } => row(
        "isolation.gateway_unavailable",
        Side::Environment,
        "The gateway the run needs is not available.",
        "-",
    ),
    Start { .. } => row(
        "isolation.gateway_start_failed",
        Side::Environment,
        "The gateway container did not start.",
        "-",
    ),
    Secret (inner) => delegate(inner, Side::Environment),
    SecretFormat { .. } => row(
        "isolation.secret_unsupported_character",
        Side::Environment,
        "A secret holds a character that the gateway's configuration cannot carry.",
        "-",
    ),
    Config { .. } => row(
        "isolation.gateway_misconfigured",
        Side::Environment,
        "The configuration of the gateway is unusable.",
        "-",
    ),
    NotReady { cause } => delegate(cause, Side::Environment),
}

refusal_table! {
    NotReadyCause, path = no_path;
    NoAnswer { .. } => row(
        "isolation.gateway_no_answer",
        Side::Environment,
        "The gateway started and nothing answered within the retries.",
        "-",
    ),
    Unauthorized { .. } => row(
        "isolation.gateway_unauthorized",
        Side::Environment,
        "The upstream refused the credential with status 401.",
        "-",
    ),
    Capped { .. } => row(
        "isolation.gateway_capped",
        Side::Environment,
        "The concurrency cap of the sidecar answered with status 429.",
        "-",
    ),
    Upstream (..) => row(
        "isolation.gateway_upstream_error",
        Side::Environment,
        "The upstream answered with an error status other than 401 and 429.",
        "-",
    ),
}

refusal_table! {
    ImageError, path = no_path;
    Unavailable { .. } => row(
        "isolation.image_service_unavailable",
        Side::Environment,
        "The image service is not available.",
        "-",
    ),
    Build { .. } => row(
        "isolation.image_build_failed",
        Side::Environment,
        "An image build failed.",
        "-",
    ),
    LockMismatch { .. } => row(
        "isolation.image_lock_mismatch",
        Side::Environment,
        "The lock bytes do not have the digest the caller named.",
        "-",
    ),
    VersionMismatch { .. } => row(
        "isolation.image_label_mismatch",
        Side::Environment,
        "The binary in the image and the image's label name different Claude Code versions.",
        "-",
    ),
    VersionUnreadable { .. } => row(
        "isolation.claude_version_unreadable",
        Side::Environment,
        "The version output of Claude Code in the image names no version.",
        "-",
    ),
}

refusal_table! {
    GuardError, path = no_path;
    Refused (..) => row(
        "candler.internal",
        Side::Candler,
        "The container guard refused to destroy something.",
        "-",
    ),
    Docker { .. } => row(
        "isolation.guard_docker_failed",
        Side::Environment,
        "Docker failed under the container guard; nothing was destroyed after that call.",
        "-",
    ),
    View { .. } => row(
        "isolation.guard_job_unreadable",
        Side::Environment,
        "The job directory could not be read, so the guard could not tell what is recorded.",
        "-",
    ),
}

refusal_table! {
    MountError, path = no_path;
    OutsideRoots { .. } => row(
        "isolation.mount_outside_roots",
        Side::Config,
        "A mount source sits outside the project root and the store.",
        "-",
    ),
    SymlinkEscape { .. } => row(
        "isolation.mount_symlink_escape",
        Side::Config,
        "A mount source is a symlink that leaves the project root and the store.",
        "-",
    ),
    DockerSocket { .. } => row(
        "isolation.mount_docker_socket",
        Side::Config,
        "A mount source names the docker socket or sits under a run directory.",
        "-",
    ),
    BadPath { .. } => row(
        "isolation.mount_path_unsupported",
        Side::Config,
        "A mount path holds a comma, a newline or text that is not UTF-8, which cannot be written as a `--mount` option; a project directory with a comma in its name does it.",
        "-",
    ),
    ConfigDirNotWritable { .. } => row(
        "candler.internal",
        Side::Candler,
        "No mount makes the config directory writable.",
        "-",
    ),
    Unreadable { .. } => row(
        "isolation.mount_unresolvable",
        Side::Config,
        "A mount source cannot be resolved on the host.",
        "-",
    ),
}

refusal_table! {
    RunArgvError, path = run_argv_error_path;
    SecretRefused { .. } => row(
        "isolation.env_secret_refused",
        Side::Config,
        "The environment overlay carries a secret, which never enters the agent container.",
        "-",
    ),
    ReservedName { .. } => row(
        "isolation.env_name_reserved",
        Side::Config,
        "The environment overlay sets a name that the wrapper owns.",
        "-",
    ),
    DuplicateName { .. } => row(
        "isolation.env_name_repeated",
        Side::Config,
        "The environment sets one name twice.",
        "-",
    ),
    BadEnvName { .. } => row(
        "isolation.env_name_invalid",
        Side::Config,
        "An environment name is empty or holds `=` or a NUL byte.",
        "-",
    ),
    MissingLabel { .. } => row(
        "candler.internal",
        Side::Candler,
        "The plan lacks a label the container guard decides by.",
        "-",
    ),
    BadCidfile { .. } => row(
        "candler.internal",
        Side::Candler,
        "The plan's cidfile has no directory.",
        "-",
    ),
    Mount (inner) => delegate(inner, Side::Config),
    Command { .. } => row(
        "candler.internal",
        Side::Candler,
        "The wrapped command cannot be built.",
        "-",
    ),
    Request { .. } => row(
        "candler.internal",
        Side::Candler,
        "The spawn request cannot be built.",
        "-",
    ),
}

refusal_table! {
    SweepError, path = no_path;
    Unavailable { .. } => row(
        "candler.internal",
        Side::Candler,
        "The sweeper is not wired in this build.",
        "-",
    ),
    List { .. } => row(
        "isolation.sweep_list_failed",
        Side::Environment,
        "Docker failed while the sweeper listed containers.",
        "-",
    ),
}

/// The rows of every coded error of this module, in the order of the types.
#[must_use]
pub(crate) fn catalog() -> Vec<CatalogRow> {
    let tables: [&'static [CatalogRow]; 10] = [
        <IsolationError as Catalogued>::rows(),
        <DockerError as Catalogued>::rows(),
        <SecretError as Catalogued>::rows(),
        <GatewayError as Catalogued>::rows(),
        <NotReadyCause as Catalogued>::rows(),
        <ImageError as Catalogued>::rows(),
        <GuardError as Catalogued>::rows(),
        <MountError as Catalogued>::rows(),
        <RunArgvError as Catalogued>::rows(),
        <SweepError as Catalogued>::rows(),
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

/// The refusal that answers for a [`GatewayError`]: a secret error or a not-ready cause it wraps
/// answers with its own code, side and path, and every other variant with its own.
fn gateway_answer(error: &GatewayError) -> &dyn Refusal {
    match error {
        GatewayError::Secret(inner) => crate::refusal::through(inner, error),
        GatewayError::NotReady { cause } => crate::refusal::through(cause, error),
        GatewayError::Unavailable { .. }
        | GatewayError::Start { .. }
        | GatewayError::SecretFormat { .. }
        | GatewayError::Config { .. } => error,
    }
}

/// The refusal that answers for a [`RunArgvError`]: a mount error it wraps answers with its own
/// code, side and path, and every other variant with its own.
fn run_argv_answer(error: &RunArgvError) -> &dyn Refusal {
    match error {
        RunArgvError::Mount(inner) => crate::refusal::through(inner, error),
        RunArgvError::SecretRefused { .. }
        | RunArgvError::ReservedName { .. }
        | RunArgvError::DuplicateName { .. }
        | RunArgvError::BadEnvName { .. }
        | RunArgvError::MissingLabel { .. }
        | RunArgvError::BadCidfile
        | RunArgvError::Command { .. }
        | RunArgvError::Request { .. } => error,
    }
}

/// The coded error of this module that `error` is, or `None` when it is none of them.
#[must_use]
pub(crate) fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    error
        .downcast_ref::<IsolationError>()
        .map(as_refusal)
        .or_else(|| {
            error
                .downcast_ref::<DockerError>()
                .map(as_refusal)
                .or_else(|| {
                    error
                        .downcast_ref::<SecretError>()
                        .map(as_refusal)
                        .or_else(|| {
                            error
                                .downcast_ref::<GatewayError>()
                                .map(gateway_answer)
                                .or_else(|| {
                                    error
                                        .downcast_ref::<NotReadyCause>()
                                        .map(as_refusal)
                                        .or_else(|| {
                                            error.downcast_ref::<ImageError>().map(as_refusal)
        .or_else(|| error.downcast_ref::<GuardError>().map(as_refusal)
        .or_else(|| error.downcast_ref::<MountError>().map(as_refusal)
        .or_else(|| error.downcast_ref::<RunArgvError>().map(run_argv_answer)
        .or_else(|| error.downcast_ref::<SweepError>().map(as_refusal)))))
                                        })
                                })
                        })
                })
        })
}
