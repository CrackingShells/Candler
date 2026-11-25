//! Guards for the start check of the files Candler installs for a harness: what a finding does to
//! the job, what a clean verdict leaves in the run, how often the check runs, and how the plan's
//! read-only workspace file reaches the run.
//!
//! The job runs over the fake docker client of the job guards; the isolation is wrapped by a
//! double that answers each command run for its output from a script, so no container exists. The
//! harness is a stand-in whose check, files and workspace file the test sets.
#![cfg(test)]

use crate::support;

use std::fs;
use std::sync::Arc;

use candler_core::isolation::container::mounts::{MountMode, MountSpec};
use candler_core::jobs::model::{JobOutcome, JobSpec};
use candler_core::jobs::orchestrate::session::JobHost;
use candler_core::runner::plan::refuse_instruction_collision;
use candler_harness::driver::{DriverError, HarnessDriver, Plan};
use candler_harness::reader::TraceReader;
use candler_harness::spec::{
    AgentSetting, Answer, Cell, CheckVerdict, CollectSpec, CommandSpec, DriverFact, EnvEntry,
    HarnessId, Observed, Prepared, PreparedFile, ProcessExit, StatePath, StopSpec, Workspace,
};
use candler_schemas::execution::Comparable;
use candler_schemas::primitives::RelPath;
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::{Value, json};
use support::job_orchestrate::{Probed, QuietDriver, Rig, run_to_end, spec};
use support::run_plan::Facts;

/// The text of the workspace file the stand-in harness lists.
const WORKSPACE_TEXT: &str = "be brief";

/// The program of the stand-in harness's check.
const CHECK: &str = "harness-check";

/// The quiet harness, with a check, installed files and a workspace file the test sets.
struct Checking {
    inner: QuietDriver,
    /// Whether the harness has a check.
    check: bool,
    /// Whether the installed files name the cell's model, so cells of two models differ.
    vary: bool,
    /// Whether the run collects every file of the workspace as an output.
    collect_all: bool,
}

impl Checking {
    fn new(check: bool) -> Self {
        Self {
            inner: QuietDriver,
            check,
            vary: false,
            collect_all: false,
        }
    }
}

impl HarnessDriver for Checking {
    fn config_check(&self, _plan: &Plan) -> Option<CommandSpec> {
        self.check.then(|| {
            CommandSpec::builder()
                .argv(vec![CHECK.to_owned()])
                .build()
                .expect("a command")
        })
    }

    fn read_config_check(&self, output: &[u8]) -> CheckVerdict {
        let text = String::from_utf8_lossy(output);
        if text.contains("REJECT") {
            CheckVerdict::Rejected {
                output: text.into_owned(),
            }
        } else {
            CheckVerdict::Clean
        }
    }

    fn workspace_instruction_file(&self) -> &'static str {
        "AGENTS.md"
    }

    fn skills(&self) -> candler_harness::skills::SkillTree {
        self.inner.skills()
    }

    fn base_image(&self) -> candler_harness::spec::BaseImage {
        self.inner.base_image()
    }

    fn detect(&self, sample: &[u8]) -> Option<semver::Version> {
        self.inner.detect(sample)
    }

    fn id(&self) -> HarnessId {
        self.inner.id()
    }

    fn supported(&self) -> semver::VersionReq {
        self.inner.supported()
    }

    fn version_probe(&self) -> CommandSpec {
        self.inner.version_probe()
    }

    fn parse_version(&self, output: &[u8]) -> Result<semver::Version, DriverError> {
        self.inner.parse_version(output)
    }

    fn answer(&self, setting: AgentSetting) -> Answer {
        self.inner.answer(setting)
    }

    fn fact(&self, fact: DriverFact) -> Answer {
        self.inner.fact(fact)
    }

    fn gateway(&self) -> Option<candler_harness::spec::GatewayEnv> {
        self.inner.gateway()
    }

    fn state_paths(&self) -> Vec<StatePath> {
        self.inner.state_paths()
    }

    fn plan(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Plan, DriverError> {
        let model = if self.vary {
            cell.value_str("model.id")
                .ok()
                .flatten()
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        } else {
            String::new()
        };
        let settings = PreparedFile::in_home(
            "settings.json".parse::<RelPath>().expect("a path"),
            format!("{{\"model\":\"{model}\"}}").into_bytes(),
            false,
        );
        let mut plan = self.inner.plan(cell, workspace)?;
        plan.prepared = Prepared::new(vec![settings], Vec::<EnvEntry>::new());
        plan.workspace_files = vec![PreparedFile::new(
            "AGENTS.md".parse::<RelPath>().expect("a path"),
            WORKSPACE_TEXT.as_bytes().to_vec(),
            false,
        )];
        Ok(plan)
    }

    fn stop(&self) -> StopSpec {
        self.inner.stop()
    }

    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        self.inner.live_reader()
    }

    fn offline_reader(
        &self,
        stream: &StreamName,
        servers: &[String],
    ) -> Option<Box<dyn TraceReader>> {
        self.inner.offline_reader(stream, servers)
    }

    fn classify_exit(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy {
        self.inner.classify_exit(exit, sessions)
    }

    fn collect(&self, workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec> {
        if self.collect_all {
            vec![CollectSpec::file("**".to_owned())]
        } else {
            self.inner.collect(workspace, observed)
        }
    }
}

/// The facts of every cell: no secret is declared.
fn facts() -> Facts {
    Facts {
        secrets: Vec::new(),
        commit: support::job_orchestrate::skill_repo().1.clone(), // BORROW: the facts own the commit text
        ..Facts::base()
    }
}

/// A job over `models`, whose cells the quiet harness runs.
fn job(models: &[&str]) -> JobSpec {
    spec(models, &facts(), 3600, 1)
}

/// What a job left: its world, its outcome and the host that scripted it.
struct Ran {
    rig: Rig,
    outcome: JobOutcome,
    host: Arc<Probed>,
}

/// Runs `job` under `driver` to its end, with the check printing `printed`.
fn run(job: JobSpec, driver: Checking, printed: &str) -> Ran {
    let rig = Rig::new(0);
    let mut host = Probed::new(rig.host());
    host.prints = vec![(CHECK.to_owned(), printed.to_owned())];
    let host = Arc::new(host);
    let shared: Arc<dyn JobHost> = host.clone();
    let mut rt = rig.runtime();
    rt.driver = Arc::new(driver);
    let outcome = run_to_end(rt, job, shared);
    Ran { rig, outcome, host }
}

/// The file `name` of the first run's directory, read as JSON.
fn run_file(ran: &Ran, name: &str) -> Value {
    let key = &ran.outcome.runs.first().expect("one run").key;
    let path = ran
        .rig
        .store
        .layout()
        .root()
        .join("runs")
        .join(key.to_string())
        .join(name);
    serde_json::from_slice(&fs::read(path).expect("a run file")).expect("JSON")
}

/// The value the manifest measured at `path`.
fn measured_at(manifest: &Value, path: &str) -> Option<Value> {
    manifest["measured"]
        .as_array()?
        .iter()
        .find(|value| value["path"] == path)
        .map(|value| value["value"].clone()) // BORROW: .clone() owns the value
}

/// The mount in `mounts` whose target ends with `/{target}`.
fn mount_at<'m>(mounts: &'m [MountSpec], target: &str) -> Option<&'m MountSpec> {
    mounts
        .iter()
        .find(|mount| mount.target.ends_with(&format!("/{target}")))
}

/// Guards: a harness check that reports a finding refuses the job before its first run, and the
/// refusal line names what was installed, where, the harness and the harness's output.
/// Oracle: the literal `REJECT: unknown key hooks.Foo` the scripted check prints, and the file
/// names the stand-in driver lists. The job runs in a child of this test binary, so its stderr
/// line can be read.
/// Bite: the refusal drops the harness's output, or the check runs after the runs are scheduled.
#[test]
fn harness_finding_refuses_the_job_before_any_run() {
    const NAME: &str = "guard_config_check::harness_finding_refuses_the_job_before_any_run";
    if std::env::var_os("CANDLER_CONFIG_CHECK_CHILD").is_some() {
        let ran = run(
            job(&["m1"]),
            Checking::new(true),
            "REJECT: unknown key hooks.Foo",
        );
        let probing = ran.host.probing();
        assert_eq!(probing.executed().len(), 1, "the check ran once");
        assert_eq!(probing.runs(), 0, "no run starts");
        assert!(ran.outcome.runs.is_empty(), "no run is recorded");
        return;
    }
    let child = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args([NAME, "--exact", "--nocapture", "--test-threads=1"])
        .env("CANDLER_CONFIG_CHECK_CHILD", "1")
        .output()
        .expect("the child ran");
    let stderr = String::from_utf8_lossy(&child.stderr);
    assert!(child.status.success(), "the child's guards held: {stderr}");
    assert!(
        stderr.contains(
            "Candler installed the files `settings.json`, `AGENTS.md` at the agent's home and the workspace; claude-code rejects it: REJECT: unknown key hooks.Foo"
        ),
        "the refusal line: {stderr}"
    );
}

/// Guards: a clean verdict is stored in the run's provenance, and a harness with no check leaves
/// the run with nothing stored as checked and the job not refused.
/// Oracle: the literal `provenance.harness_config_check` path and `clean` value, read from the
/// sealed manifest; the check's container holds the workspace file read-only at the root.
/// Bite: the verdict is dropped between the check and the run's manifest, or the check container
/// is given no workspace file.
#[test]
fn clean_verdict_is_stored_and_a_harness_without_check_stores_none() {
    let checked = run(job(&["m1"]), Checking::new(true), "ok");
    assert_eq!(checked.outcome.runs.len(), 1, "the run starts");
    assert_eq!(
        measured_at(
            &run_file(&checked, "manifest.json"),
            "provenance.harness_config_check"
        ),
        Some(json!("clean"))
    );
    let executed = checked.host.probing().executed();
    let (argv, mounts) = executed.first().expect("the check ran");
    assert_eq!(argv, &vec![CHECK.to_owned()]);
    let file = mount_at(mounts, "AGENTS.md").expect("the check container mounts the file");
    assert_eq!(file.mode, MountMode::ReadOnly);
    assert_eq!(
        fs::read_to_string(&file.source).expect("a blob"),
        WORKSPACE_TEXT
    );

    let unchecked = run(job(&["m1"]), Checking::new(false), "ok");
    assert_eq!(unchecked.outcome.runs.len(), 1, "the run starts");
    assert!(
        unchecked.host.probing().executed().is_empty(),
        "no check container for a harness with no check"
    );
    assert_eq!(
        measured_at(
            &run_file(&unchecked, "manifest.json"),
            "provenance.harness_config_check"
        ),
        None
    );
}

/// Guards: cells that share a harness, an image and the installed files are checked once, and
/// cells whose installed files differ are each checked.
/// Oracle: the number of commands the isolation double ran for their output.
/// Bite: key the check on the cell instead of on the installed files, or never on them.
#[test]
fn cells_sharing_their_files_are_checked_once() {
    let shared = run(job(&["m1", "m2"]), Checking::new(true), "ok");
    assert_eq!(shared.outcome.runs.len(), 2, "both runs start");
    assert_eq!(shared.host.probing().executed().len(), 1, "one check");

    let mut differing = Checking::new(true);
    differing.vary = true;
    let apart = run(job(&["m1", "m2"]), differing, "ok");
    assert_eq!(apart.outcome.runs.len(), 2, "both runs start");
    assert_eq!(apart.host.probing().executed().len(), 2, "one check each");
}

/// Guards: the plan's read-only workspace file reaches the run's container at the workspace root
/// from its blob, read-only, is no output of the run, and does not make the run an input leak.
/// Oracle: the blob's bytes, the mount's mode and path, the run's `tree.json` and integrity
/// written by the run.
/// Bite: leave the file out of the mount list, or out of the paths collection skips.
#[test]
fn workspace_file_is_mounted_read_only_and_is_no_output() {
    let mut driver = Checking::new(false);
    driver.collect_all = true;
    let ran = run(job(&["m1"]), driver, "");
    assert_eq!(ran.outcome.runs.len(), 1, "the run starts");
    let mounts = ran.host.probing().run_mounts();
    let file = mount_at(mounts.first().expect("a run request"), "AGENTS.md")
        .expect("the run mounts the file");
    assert_eq!(file.mode, MountMode::ReadOnly);
    assert_eq!(
        fs::read_to_string(&file.source).expect("a blob"),
        WORKSPACE_TEXT
    );
    let tree = run_file(&ran, "tree.json").to_string();
    assert!(!tree.contains("AGENTS.md"), "the file is no output: {tree}");
    let manifest = run_file(&ran, "manifest.json");
    assert_ne!(
        manifest["integrity"]["reason"], "input_leak",
        "the file is no input leak: {manifest}"
    );
}

/// A text as a resolved value's content.
fn json_value(text: &str) -> JsonValue {
    JsonValue::new(json!(text)).expect("a value")
}

/// A cell of the fixture's resolution, with the workspace instruction text when `text` is set.
fn cell_with(text: Option<&str>) -> Comparable {
    let mut values = support::run_plan::resolution(&facts()).values;
    if let Some(text) = text {
        values.push(ResolvedValue::recorded(
            "instructions.workspace".parse().expect("a path"),
            json_value(text),
        ));
    }
    Comparable::from_resolved(&values).expect("a cell")
}

/// Guards: an input declared at the harness's workspace instruction file is refused for a cell
/// that sets the workspace instruction text, naming the input; the same input is no collision for
/// a cell that sets none, and an input elsewhere is none for a cell that does.
/// Oracle: the input kind `rules` and the file `AGENTS.md` written by hand.
/// Bite: compare the path with the text's file by prefix only, or skip the cell's text.
#[test]
fn input_at_the_instruction_file_collides_with_the_workspace_text() {
    let driver = Checking::new(false);
    let at_file = [(String::from("AGENTS.md"), String::from("rules"))];
    let elsewhere = [(String::from("docs/AGENTS.md"), String::from("rules"))];
    let text = cell_with(Some("be brief"));
    let none = cell_with(None);

    let refused =
        refuse_instruction_collision(&driver, [&text], &at_file).expect_err("a collision");
    assert!(
        matches!(
            &refused,
            DriverError::InstructionsCollide { input, file, .. } if input == "rules" && file == "AGENTS.md"
        ),
        "{refused:?}"
    );
    assert!(refuse_instruction_collision(&driver, [&none], &at_file).is_ok());
    assert!(refuse_instruction_collision(&driver, [&text], &elsewhere).is_ok());
}
