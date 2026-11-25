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
use std::sync::atomic::{AtomicUsize, Ordering};

use candler_core::isolation::ExecOutput;
use candler_core::isolation::container::mounts::{MountMode, MountSpec};
use candler_core::jobs::model::{JobOutcome, JobSpec};
use candler_core::jobs::orchestrate::session::JobHost;
use candler_harness::driver::{DriverError, HarnessDriver, Plan};
use candler_harness::reader::TraceReader;
use candler_harness::spec::StateKind;
use candler_harness::spec::{
    AgentSetting, Answer, Cell, CheckVerdict, CollectSpec, CommandSpec, DriverFact, EnvEntry,
    EnvValue, HarnessId, Observed, Prepared, PreparedFile, ProcessExit, StatePath, StopSpec,
    Workspace,
};
use candler_schemas::primitives::RelPath;
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};
use serde_json::{Value, json};
use support::job_orchestrate::{Probed, QuietDriver, Rig, run_to_end, spec};
use support::run_plan::Facts;

/// The text of the workspace file the stand-in harness lists.
const WORKSPACE_TEXT: &str = "be brief";

/// The variable the stand-in's state path and its check both set.
const STATE_VARIABLE: &str = "XDG_DATA_HOME";

/// The program of the stand-in harness's check.
const CHECK: &str = "harness-check";

/// The quiet harness, with a check, installed files and a workspace file the test sets.
struct Checking {
    inner: QuietDriver,
    /// Whether the harness has a check.
    check: bool,
    /// What of the check's input names the cell's model, so cells of two models differ.
    vary: Vary,
    /// Whether the run collects every file of the workspace as an output.
    collect_all: bool,
    /// Whether a state variable of the run is one the check's own environment sets.
    shared_state: bool,
    /// How many times the harness was asked to plan a cell.
    planned: Arc<AtomicUsize>,
}

/// What differs between the cells of two models.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Vary {
    /// Nothing: the cells share everything the check reads.
    Nothing,
    /// The text of an installed file in the agent's home.
    Settings,
    /// A variable of the check command's environment.
    CommandEnv,
    /// The check command's working directory.
    CommandCwd,
    /// Which list a workspace file is in: the writable files or the read-only ones.
    ListOfFile,
}

impl Checking {
    fn new(check: bool) -> Self {
        Self {
            inner: QuietDriver,
            check,
            vary: Vary::Nothing,
            collect_all: false,
            shared_state: false,
            planned: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl HarnessDriver for Checking {
    fn config_check(&self, plan: &Plan) -> Option<CommandSpec> {
        self.check.then(|| {
            let mut builder = CommandSpec::builder().argv(vec![CHECK.to_owned()]);
            let model = plan
                .prepared
                .env_overlay
                .iter()
                .find(|entry| entry.name == "MODEL")
                .and_then(|entry| {
                    if let EnvValue::Literal(text) = &entry.value {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            if self.shared_state {
                builder = builder.env(EnvEntry::literal(
                    STATE_VARIABLE.to_owned(),
                    "/tmp/candler-config-check".to_owned(),
                ));
            }
            if self.vary == Vary::CommandEnv {
                builder = builder.env(EnvEntry::literal("CHECK_MODEL".to_owned(), model.clone()));
            }
            if self.vary == Vary::CommandCwd {
                builder = builder.cwd(model.parse::<RelPath>().expect("a path"));
            }
            builder.build().expect("a command")
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
        let mut paths = self.inner.state_paths();
        if self.shared_state {
            paths.push(StatePath::new(
                STATE_VARIABLE.to_owned(),
                StateKind::Dir,
                ".local/share".parse::<RelPath>().expect("a path"),
            ));
        }
        paths
    }

    fn plan(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Plan, DriverError> {
        self.planned.fetch_add(1, Ordering::SeqCst);
        let model = cell
            .value_str("model.id")
            .ok()
            .flatten()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let settings_text = if self.vary == Vary::Settings {
            model.clone()
        } else {
            String::new()
        };
        let settings = PreparedFile::in_home(
            "settings.json".parse::<RelPath>().expect("a path"),
            format!("{{\"model\":\"{settings_text}\"}}").into_bytes(),
            false,
        );
        let notes = PreparedFile::new(
            "notes.md".parse::<RelPath>().expect("a path"),
            b"notes".to_vec(),
            false,
        );
        let mut plan = self.inner.plan(cell, workspace)?;
        let mut files = vec![settings];
        let mut read_only = vec![PreparedFile::new(
            "AGENTS.md".parse::<RelPath>().expect("a path"),
            WORKSPACE_TEXT.as_bytes().to_vec(),
            false,
        )];
        if self.vary == Vary::ListOfFile && model == "m2" {
            read_only.push(notes);
        } else if self.vary == Vary::ListOfFile {
            files.push(notes);
        }
        // The model's name rides in the plan's environment, which the check does not read, so a
        // variant of the command can name it without the files differing.
        let rides = vec![EnvEntry::literal("MODEL".to_owned(), model)];
        plan.prepared = Prepared::new(files, rides);
        plan.workspace_files = read_only;
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

/// Runs `job` under `driver` to its end, over a host `script` has set up.
fn run_with(job: JobSpec, driver: Checking, script: impl FnOnce(&mut Probed)) -> Ran {
    let rig = Rig::new(0);
    let mut host = Probed::new(rig.host());
    script(&mut host);
    let host = Arc::new(host);
    let shared: Arc<dyn JobHost> = host.clone();
    let mut rt = rig.runtime();
    rt.driver = Arc::new(driver);
    let outcome = run_to_end(rt, job, shared);
    Ran { rig, outcome, host }
}

/// Runs `job` under `driver` to its end, with the check printing `printed`.
fn run(job: JobSpec, driver: Checking, printed: &str) -> Ran {
    run_with(job, driver, |host| {
        host.prints = vec![(CHECK.to_owned(), printed.to_owned())];
    })
}

/// Runs `job` under `driver` to its end, with the check ending as `output` says.
fn run_ending(job: JobSpec, driver: Checking, output: ExecOutput) -> Ran {
    run_with(job, driver, |host| {
        host.ran = vec![(CHECK.to_owned(), output)];
    })
}

/// The supervisor's stderr of the job `child` runs when this test runs again as a child of the
/// test binary, or `None` in the child itself, where `child` runs. The job's refusal is a line
/// of that stderr.
fn refusal_of(test: &str, child: impl FnOnce()) -> Option<String> {
    if std::env::var_os("CANDLER_CONFIG_CHECK_CHILD").is_some() {
        child();
        return None;
    }
    let out = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args([test, "--exact", "--nocapture", "--test-threads=1"])
        .env("CANDLER_CONFIG_CHECK_CHILD", "1")
        .output()
        .expect("the child ran");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "the child's guards held: {stderr}");
    Some(stderr)
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
/// Bite: the refusal drops the harness's output, the check runs after the runs are scheduled, or the
/// check plans its cell twice.
#[test]
fn harness_finding_refuses_the_job_before_any_run() {
    let line = refusal_of(
        "guard_config_check::harness_finding_refuses_the_job_before_any_run",
        || {
            let driver = Checking::new(true);
            let planned = Arc::clone(&driver.planned);
            let ran = run(job(&["m1"]), driver, "REJECT: unknown key hooks.Foo");
            assert_eq!(
                planned.load(Ordering::SeqCst),
                1,
                "the check plans its cell once"
            );
            let probing = ran.host.probing();
            assert_eq!(probing.executed().len(), 1, "the check ran once");
            assert_eq!(probing.runs(), 0, "no run starts");
            assert!(ran.outcome.runs.is_empty(), "no run is recorded");
        },
    );
    let Some(stderr) = line else { return };
    assert!(
        stderr.contains(
            "Candler installed the files `settings.json`, `AGENTS.md` at the agent's home and the workspace; claude-code rejects it: REJECT: unknown key hooks.Foo"
        ),
        "the refusal line: {stderr}"
    );
}

/// Guards: a harness check that exits non-zero with its finding on standard error refuses the
/// job as a finding, quoting that text, and not as a check that could not run; the exit status
/// does not decide the verdict.
/// Oracle: the literal `REJECT: unknown key hooks.Foo` the scripted check prints on standard
/// error and the exit code `1`, which `OpenCode`'s check exits with for every report.
/// Bite: read the verdict from standard output alone, or treat a non-zero exit as a check that
/// could not run.
#[test]
fn finding_on_stderr_with_a_failing_exit_refuses_as_a_finding() {
    let line = refusal_of(
        "guard_config_check::finding_on_stderr_with_a_failing_exit_refuses_as_a_finding",
        || {
            let ran = run_ending(
                job(&["m1"]),
                Checking::new(true),
                ExecOutput::exited(1, b"REJECT: unknown key hooks.Foo".to_vec()),
            );
            assert_eq!(ran.host.probing().runs(), 0, "no run starts");
            assert!(ran.outcome.runs.is_empty(), "no run is recorded");
        },
    );
    let Some(stderr) = line else { return };
    assert!(
        stderr.contains("claude-code rejects it: REJECT: unknown key hooks.Foo"),
        "the refusal quotes the harness: {stderr}"
    );
    assert!(
        !stderr.contains("could not run"),
        "a finding is no failed check: {stderr}"
    );
}

/// Guards: a check that did not run, printed nothing or printed more than the core keeps is
/// refused as a check that could not run, whatever the stand-in's reader would say of the text:
/// no output, a command that is missing (`127`), the in-container timeout (`124`), a signal and
/// output cut at the capture limit.
/// Oracle: the exit codes `127` and `124` that the shell and `timeout` give, signal 9, and the
/// wording of the refusal written by hand.
/// Bite: pass the empty or cut output to the driver, or read the exit code as a verdict.
#[test]
fn a_check_that_did_not_run_or_printed_nothing_is_refused() {
    let mut killed = ExecOutput::printed(b"fine".to_vec());
    killed.status = ExitStatus::Signal { signal: 9 };
    let mut cut = ExecOutput::printed(b"fine".to_vec());
    cut.truncated = true;
    let line = refusal_of(
        "guard_config_check::a_check_that_did_not_run_or_printed_nothing_is_refused",
        || {
            for (case, output) in [
                ("empty", ExecOutput::printed(Vec::new())),
                ("missing", ExecOutput::exited(127, b"not found".to_vec())),
                ("timeout", ExecOutput::exited(124, Vec::new())),
                ("signal", killed),
                ("cut", cut),
            ] {
                let ran = run_ending(job(&["m1"]), Checking::new(true), output);
                assert_eq!(ran.host.probing().runs(), 0, "{case}: no run starts");
            }
        },
    );
    let Some(stderr) = line else { return };
    for expected in [
        "the harness check could not run: the check printed nothing",
        "the harness check could not run: the check command did not run: it exited 127: not found",
        "it exited 124",
        "the check was ended by signal 9",
        "the check printed more than the core keeps",
    ] {
        assert!(stderr.contains(expected), "{expected}: {stderr}");
    }
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
/// cells that differ in what the check reads are each checked: an installed file, the command's
/// environment or working directory, and which list (writable or read-only) a file is in.
/// Oracle: the number of commands the isolation double ran for their output.
/// Bite: key the check on the cell instead of on what it reads, or leave one of them out of the key.
#[test]
fn cells_sharing_their_files_are_checked_once() {
    let shared = run(job(&["m1", "m2"]), Checking::new(true), "ok");
    assert_eq!(shared.outcome.runs.len(), 2, "both runs start");
    assert_eq!(shared.host.probing().executed().len(), 1, "one check");

    for vary in [
        Vary::Settings,
        Vary::CommandEnv,
        Vary::CommandCwd,
        Vary::ListOfFile,
    ] {
        let mut differing = Checking::new(true);
        differing.vary = vary;
        let apart = run(job(&["m1", "m2"]), differing, "ok");
        assert_eq!(apart.outcome.runs.len(), 2, "both runs start");
        assert_eq!(apart.host.probing().executed().len(), 2, "one check each");
    }
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

/// Guards: a variable the check's command sets wins over the container's own: a state variable of
/// the run with the same name is left out of the check container's environment, which the
/// isolation would otherwise refuse as a name set twice, and the job is not refused.
/// Oracle: the variable name `XDG_DATA_HOME` and its values written by hand; the environment the
/// isolation double recorded for the check container.
/// Bite: leave the run's variable in the check container, which then holds the name twice or the
/// run's value.
#[test]
fn the_checks_own_variable_wins_over_the_runs() {
    let mut driver = Checking::new(true);
    driver.shared_state = true;
    let ran = run(job(&["m1"]), driver, "ok");
    assert_eq!(ran.outcome.runs.len(), 1, "the run starts");
    let probing = ran.host.probing();
    let held = probing.executed_env();
    let env = held.first().expect("the check ran");
    assert!(
        env.iter().all(|(name, _)| name != STATE_VARIABLE),
        "the check container keeps the run's variable: {env:?}"
    );
}
