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

use super::ps::store_root;
use super::run::STORE_ENV;
use super::{CliError, Exit};

/// The arguments of `candler supervise`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The job to supervise.
    #[arg(long)]
    pub job: String,
}

/// The import package of the planner library the env image carries, whose distribution and
/// version every run records.
const PLANNER_PACKAGE: &str = "gtpyhop";

/// The build of the evaluation infrastructure this binary is, as `build.rs` read it from git.
fn eval_infra() -> EvalInfra {
    EvalInfra {
        tag: option_env!("CANDLER_EVAL_INFRA_TAG").map(str::to_owned), // BORROW: the record owns its text
        commits_ahead: option_env!("CANDLER_EVAL_INFRA_COMMITS_AHEAD").and_then(|n| n.parse().ok()),
        commit: option_env!("CANDLER_EVAL_INFRA_COMMIT").map(str::to_owned), // BORROW: the record owns its text
        dirty: option_env!("CANDLER_EVAL_INFRA_DIRTY").map(|flag| flag == "true"),
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

/// The store root: the launcher's, or else the one the working directory names.
fn root() -> Result<PathBuf, CliError> {
    match std::env::var_os(STORE_ENV) {
        Some(root) => Ok(PathBuf::from(root)),
        None => store_root(None),
    }
}

/// Supervises the job: reads its spec, takes its place in `job.json` and runs the orchestration.
///
/// # Errors
/// [`CliError::Failed`] when the spec is refused, the store or the job's directory cannot be
/// opened, or the supervisor fails; the exit is 2.
pub async fn run(args: Args) -> Result<Exit, CliError> {
    let job: JobId = args.job.parse().map_err(failed)?;
    let store = Store::open(root()?.as_path()).map_err(failed)?;
    let dir = JobDir::open(store.layout(), &job).map_err(failed)?;
    let spec = read_spec(dir.path(), std::io::stdin()).map_err(failed)?;
    dir.set_supervisor(own_identity().map_err(failed)?)
        .map_err(failed)?;
    // TRAIT_OBJECT: the driver is the harness's, handed to the job as the trait it implements
    let driver: Arc<dyn HarnessDriver> = Arc::new(ClaudeCodeDriver::new());
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
            planner_package: Some(String::from(PLANNER_PACKAGE)),
        },
    };
    let body: JobBody = orchestrate;
    supervise_entry(entry, body).await.map_err(failed)?;
    Ok(Exit::Success)
}
