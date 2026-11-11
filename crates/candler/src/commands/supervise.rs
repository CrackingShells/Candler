//! `candler supervise`: the detached supervisor of a job. `candler run` starts it by re-executing
//! the binary in a new process group; the supervisor calls `setsid` on itself.
//!
//! The verb reads the job's spec from its standard input, checks it against the digest in
//! `job.json`, records its own identity there, and runs the job through the orchestration. The
//! store comes from [`STORE_ENV`], which the launcher sets; without it the store is found from
//! the working directory as `ps` finds it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::model::JobBody;
use candler_core::jobs::orchestrate::orchestrate;
use candler_core::jobs::runtime::launch::own_identity;
use candler_core::jobs::runtime::session::read_spec;
use candler_core::jobs::runtime::{JobEntry, JobServices, supervise_entry};
use candler_core::runner::evidence::provenance::EvalInfra;
use candler_core::runner::exec::SkillTree;
use candler_core::store::Store;
use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::spec::{CommandSpec, PreparedFile};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_harness_claude_code::prepare::{skill_archive_command, skill_files};
use candler_schemas::execution::TreatmentRef;
use candler_schemas::primitives::JobId;

use super::run::STORE_ENV;
use super::{CliError, Exit, common};

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

/// The skill tree of Claude Code: the harness's crate reads the archive of a treatment.
struct ClaudeSkills;

impl SkillTree for ClaudeSkills {
    fn archive_command(
        &self,
        treatment: &TreatmentRef,
        source: &Path,
    ) -> Result<CommandSpec, DriverError> {
        skill_archive_command(treatment, source)
    }

    fn files(
        &self,
        treatment: &TreatmentRef,
        archive: &[u8],
    ) -> Result<Vec<PreparedFile>, DriverError> {
        skill_files(treatment, archive)
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

/// Supervises the job: reads its spec, takes its place in `job.json` and runs the orchestration.
///
/// # Errors
/// [`CliError::Failed`] when the spec is refused, the store or the job's directory cannot be
/// opened, or the supervisor fails; the exit is 2.
pub async fn run(args: Args, store: Option<&Path>) -> Result<Exit, CliError> {
    let job: JobId = args.job.parse().map_err(failed)?;
    let store = Store::open(root(store).await?.as_path()).map_err(failed)?;
    let dir = JobDir::open(store.layout(), &job).map_err(failed)?;
    let spec = read_spec(dir.path(), std::io::stdin()).map_err(failed)?;
    dir.set_supervisor(own_identity().map_err(failed)?)
        .map_err(failed)?;
    // TRAIT_OBJECT: the driver is the harness's, handed to the job as the trait it implements
    let driver: Arc<dyn HarnessDriver> = Arc::new(ClaudeCodeDriver::new().map_err(failed)?);
    // TRAIT_OBJECT: the skill tree is the harness's, handed to the job as the trait it implements
    let skills: Arc<dyn SkillTree> = Arc::new(ClaudeSkills);
    let entry = JobEntry {
        store,
        job,
        spec,
        services: JobServices {
            driver,
            skills,
            eval_infra: eval_infra(),
        },
    };
    let body: JobBody = orchestrate;
    supervise_entry(entry, body).await.map_err(failed)?;
    Ok(Exit::Success)
}
