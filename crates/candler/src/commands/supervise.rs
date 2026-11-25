//! `candler supervise`: the detached supervisor of a job. `candler run` starts it by re-executing
//! the binary in a new process group; the supervisor calls `setsid` on itself.
//!
//! The verb reads the job's spec from its standard input, checks it against the digest in
//! `job.json`, records its own identity there, and runs the job through the orchestration. The
//! store comes from [`STORE_ENV`], which the launcher sets; without it the store is found from
//! the working directory as `ps` finds it.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::model::{JobBody, JobControl, JobOutcome, JobSpec, JobSpecFile};
use candler_core::jobs::orchestrate::controls::ControlContexts;
use candler_core::jobs::orchestrate::orchestrate_with;
use candler_core::jobs::orchestrate::session::DockerHost;
use candler_core::jobs::runtime::evaluate::{EvaluateEntry, supervise_evaluate};
use candler_core::jobs::runtime::launch::{own_identity, spec_digest};
use candler_core::jobs::runtime::session::read_spec;
use candler_core::jobs::runtime::{JobEntry, JobServices, RuntimeError, supervise_entry};
use candler_core::resolve::config::{ConfigEnv, UserConfig};
use candler_core::runner::Runtime;
use candler_core::runner::evidence::provenance::EvalInfra;
use candler_core::store::Store;
use candler_schemas::primitives::JobId;

use super::preflight::asset_paths;
use super::run::STORE_ENV;
use super::{CliError, Exit, common};
use crate::drivers::Registry;

/// The build identity's reader, shared with `build.rs` and its guard. The build side of the file
/// is not used here.
#[path = "../../build_infra.rs"]
#[expect(dead_code, reason = "the build script's half of the shared file")]
mod build_infra;
use build_infra::read_recorded;

/// The arguments of `candler supervise`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The job to supervise.
    #[arg(long)]
    pub job: String,
}

/// The build of the evaluation infrastructure this binary is, as `build.rs` proved it. A field
/// the build could not prove is unrecorded.
fn eval_infra() -> EvalInfra {
    let recorded = read_recorded(
        option_env!("CANDLER_BUILT_TAG"),
        option_env!("CANDLER_BUILT_COMMITS_AHEAD"),
        option_env!("CANDLER_BUILT_COMMIT"),
        option_env!("CANDLER_BUILT_DIRTY"),
    );
    EvalInfra {
        tag: recorded.tag,
        commits_ahead: recorded.commits_ahead,
        commit: recorded.commit,
        dirty: recorded.dirty,
    }
}

/// Why the supervisor of a job failed, as the error of the verb.
fn failed(error: impl std::error::Error + Send + Sync + 'static) -> CliError {
    CliError::Failed {
        verb: "supervise",
        source: Box::new(error),
    }
}

/// The store root: the launcher's, or else the one `--store` or the working directory names.
async fn root(store: Option<&Path>) -> Result<PathBuf, CliError> {
    match std::env::var_os(STORE_ENV) {
        Some(root) => Ok(PathBuf::from(root)),
        None => common::store_root("supervise", store).await,
    }
}

/// The bytes of the spec on standard input, once their digest is the one `job.json` records.
fn spec_bytes(dir: &JobDir) -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    std::io::stdin().read_to_end(&mut bytes).map_err(failed)?;
    let record = dir.record().map_err(failed)?;
    let digest = spec_digest(bytes.as_slice()).map_err(failed)?; // BORROW: explicit .as_slice()
    if &digest != record.spec_digest() {
        return Err(failed(RuntimeError::SpecDigest {
            detail: format!(
                "the spec's digest is {digest}, the job records {}",
                record.spec_digest()
            ),
        }));
    }
    Ok(bytes)
}

/// Supervises the job: reads its spec, takes its place in `job.json` and runs the job its spec
/// names: the orchestration of a run, or the evaluation of an evaluate job.
///
/// # Errors
/// [`CliError::Failed`] when the spec is refused, the store or the job's directory cannot be
/// opened, or the supervisor fails; the exit is 2.
pub async fn run(args: Args, store: Option<&Path>) -> Result<Exit, CliError> {
    let job: JobId = args.job.parse().map_err(failed)?;
    let store = Store::open(root(store).await?.as_path()).map_err(failed)?;
    let dir = JobDir::open(store.layout(), &job).map_err(failed)?;
    let bytes = spec_bytes(&dir)?;
    let file: JobSpecFile = serde_json::from_slice(bytes.as_slice()).map_err(failed)?; // BORROW: explicit .as_slice()
    dir.set_supervisor(own_identity().map_err(failed)?)
        .map_err(failed)?;
    match file {
        JobSpecFile::Run(_) => {
            // The digest was checked above; the reader checks it again and keeps `spec.json`.
            let spec = read_spec(dir.path(), std::io::Cursor::new(bytes)).map_err(failed)?;
            run_job(store, job, spec).await
        }
        JobSpecFile::Evaluate(spec) => {
            let entry = EvaluateEntry { store, job, spec };
            supervise_evaluate(entry).await.map_err(failed)?;
            Ok(Exit::Success)
        }
    }
}

/// The name of the harness of the job's cells: the `harness` value of its resolution.
fn harness_of(spec: &JobSpec) -> Result<String, CliError> {
    spec.resolution
        .values
        .iter()
        .find(|value| value.path().as_str() == "harness") // BORROW: explicit .as_str()
        .and_then(|value| value.value())
        .and_then(|value| value.as_value().get("name"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned) // BORROW: the caller owns the name
        .ok_or_else(|| CliError::refused("the job's resolution names no harness"))
}

/// The build contexts the session controls of the job's harness need, or why they cannot be had.
fn control_contexts(rt: &Runtime, spec: &JobSpec) -> Result<ControlContexts, String> {
    let name = harness_of(spec).map_err(|e| e.to_string())?;
    let driver = Registry::builtin()
        .driver(&name)
        .map_err(|e| e.to_string())?;
    let base = driver.base_image();
    let assets = asset_paths(rt.store.layout().root(), base.context).map_err(|e| e.to_string())?;
    Ok(ControlContexts {
        base: assets.images.base_context,
        base_build_args: base.build_args,
        fakemodel: assets.fakemodel,
    })
}

/// The orchestration of a run job on this machine's docker, with the build contexts the session
/// controls need. A machine whose contexts cannot be had gets a host without controls, which
/// refuses a job that has a session cell and says why.
fn orchestrate_here(
    rt: Runtime,
    spec: JobSpec,
    ctl: JobControl,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = JobOutcome> + Send>> {
    // TRAIT_OBJECT: the job body is a boxed future, as `JobBody` declares
    let contexts = control_contexts(&rt, &spec);
    // The user configuration was read and checked when the job was started; the supervisor reads
    // the same file from the same environment. A read that fails now is not a reason to run with
    // the system bundle: the job refuses and says why.
    let ca =
        upstream_ca_of(UserConfig::load(&ConfigEnv::from_process()).map(|u| u.gateway_upstream_ca));
    let host = match (contexts, ca) {
        (_, Err(why)) => DockerHost::without_controls(&why),
        (Ok(contexts), Ok(ca)) => DockerHost::with_controls(contexts).with_upstream_ca(ca),
        (Err(why), Ok(ca)) => DockerHost::without_controls(&why).with_upstream_ca(ca),
    };
    orchestrate_with(rt, spec, ctl, std::sync::Arc::new(host))
}

/// The upstream CA the supervisor's read of the user configuration holds, or the reason the read
/// failed, worded for the job's refusal.
fn upstream_ca_of<E: std::fmt::Display>(
    read: Result<Option<Vec<u8>>, E>,
) -> Result<Option<Vec<u8>>, String> {
    read.map_err(|why| format!("the supervisor cannot read the user configuration: {why}"))
}

/// Runs a run job through the orchestration, with the driver and skill layout of the harness its
/// cells name.
async fn run_job(store: Store, job: JobId, spec: JobSpec) -> Result<Exit, CliError> {
    let driver = Registry::builtin().driver(&harness_of(&spec)?)?;
    let entry = JobEntry {
        store,
        job,
        spec,
        services: JobServices {
            driver,
            harnesses: Registry::builtin().names(),
            eval_infra: eval_infra(),
        },
    };
    let body: JobBody = orchestrate_here;
    supervise_entry(entry, body).await.map_err(failed)?;
    Ok(Exit::Success)
}

#[cfg(test)]
mod upstream_ca_read_tests {
    use super::upstream_ca_of;

    /// Guards: a failed second read of the user configuration is an error the job reports, never a
    /// silent run on the system bundle.
    ///
    /// Oracle: the failure text and the bytes of a good read, as given.
    ///
    /// Bite: the failure turned into `Ok(None)` (red at the first assertion).
    #[test]
    fn a_failed_read_is_reported_and_a_good_one_passes_the_bytes() {
        let refused = upstream_ca_of::<&str>(Err("the file is gone"));
        assert!(refused.is_err_and(|why| why.contains("the file is gone")));
        assert_eq!(
            upstream_ca_of::<&str>(Ok(Some(b"pem".to_vec()))),
            Ok(Some(b"pem".to_vec()))
        );
        assert_eq!(upstream_ca_of::<&str>(Ok(None)), Ok(None));
    }
}
