//! Guards for the start check of a job's declared MCP servers: what a server that exits at start
//! does to the job, and what the check leaves alone.
//!
//! The job runs over the fake docker client of the job guards; the isolation is wrapped by a
//! double that answers each server's start probe from a script, so no container exists.
#![cfg(test)]

use crate::support;

use std::fmt::Write as _;
use std::sync::Arc;

use candler_core::isolation::StartProbe;
use candler_core::jobs::model::{JobOutcome, JobSpec};
use candler_core::jobs::orchestrate::session::JobHost;
use candler_schemas::execution::ExperimentFile;
use serde_json::{Value, json};
use support::job_orchestrate::{Probed, Rig, run_to_end, spec};
use support::run_plan::Facts;

/// The facts of every cell: no secret is declared.
fn facts() -> Facts {
    Facts {
        secrets: Vec::new(),
        commit: support::job_orchestrate::skill_repo().1.clone(), // BORROW: the facts own the commit text
        ..Facts::base()
    }
}

/// A one-run job whose experiment declares `servers` under `mcp.servers`, as `(key, value)`.
fn job_with(servers: &[(&str, Value)]) -> JobSpec {
    let mut job = spec(&["m1"], &facts(), 3600, 1);
    let mut text = String::from(
        "schema = \"experiment/1\"\nreplicates = 1\n\n[[factors]]\npath = \"model.id\"\nlevels = [\"m1\"]\n\n[budget.per_run]\nwall_s = 3600\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"htn\"\n",
    );
    for (key, value) in servers {
        let _ = write!(text, "\n[fixed.mcp.servers.{key}]\n");
        for (field, leaf) in value.as_object().expect("a server object") {
            let _ = writeln!(text, "{field} = {leaf}");
        }
    }
    job.experiment = ExperimentFile::from_toml_str(&text).expect("an experiment file");
    job
}

/// Runs `job` to its end over a probing host; returns the outcome, the commands probed and the
/// number of run requests made.
fn run(
    job: JobSpec,
    answers: Vec<(String, StartProbe)>,
    eof_exits: Vec<String>,
    needs: Vec<String>,
) -> (JobOutcome, Vec<Vec<String>>, usize) {
    let rig = Rig::new(0);
    let mut host = Probed::new(rig.host());
    host.answers = answers;
    host.eof_exits = eof_exits;
    host.needs = needs;
    let host = Arc::new(host);
    let shared: Arc<dyn JobHost> = host.clone();
    let outcome = run_to_end(rig.runtime(), job, shared);
    let probing = host.probing();
    (outcome, probing.probed(), probing.runs())
}

/// Guards: a declared server that exits at start refuses the job before any run, and the refusal
/// line names the server and carries what it printed.
/// Oracle: the literal `boom: no module named x` the scripted server prints. The job runs in a
/// child of this test binary, so its stderr line can be read.
/// Bite: the refusal drops the server's output.
#[test]
fn server_that_exits_refuses_the_job_with_its_stderr() {
    const NAME: &str = "guard_server_check::server_that_exits_refuses_the_job_with_its_stderr";
    if std::env::var_os("CANDLER_SERVER_CHECK_CHILD").is_some() {
        let job = job_with(&[("alpha", json!({"command": "badserver", "args": ["--x"]}))]);
        let answers = vec![(
            String::from("badserver"),
            StartProbe::Exited {
                output: String::from("boom: no module named x"),
            },
        )];
        let (outcome, probed, runs) = run(job, answers, Vec::new(), Vec::new());
        assert_eq!(probed, vec![vec!["badserver".to_owned(), "--x".to_owned()]]);
        assert_eq!(runs, 0, "no run starts");
        assert!(outcome.runs.is_empty(), "no run is recorded");
        return;
    }
    let child = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args([NAME, "--exact", "--nocapture", "--test-threads=1"])
        .env("CANDLER_SERVER_CHECK_CHILD", "1")
        .output()
        .expect("the child ran");
    let stderr = String::from_utf8_lossy(&child.stderr);
    assert!(child.status.success(), "the child's guards held: {stderr}");
    assert!(
        stderr.contains("the server `alpha` exited at start: boom: no module named x"),
        "the refusal line: {stderr}"
    );
}

/// Guards: a declared server that is still running after the wait lets the job start, with its
/// standard input held open (the scripted server exits at end of input).
/// Oracle: the one probed command and the run request that follows it.
/// Bite: close standard input at start.
#[test]
fn server_that_stays_up_lets_the_job_start() {
    let job = job_with(&[("alpha", json!({"command": "goodserver"}))]);
    let (outcome, probed, runs) = run(
        job,
        Vec::new(),
        vec![String::from("goodserver")],
        Vec::new(),
    );
    assert_eq!(probed, vec![vec!["goodserver".to_owned()]]);
    assert_eq!(runs, 1, "the run starts");
    assert_eq!(outcome.runs.len(), 1);
}

/// Guards: a server declared by `url` is not started.
/// Oracle: no probed command; the run starts.
/// Bite: start the servers that have a `url`.
#[test]
fn remote_server_is_skipped() {
    let job = job_with(&[("remote", json!({"url": "https://example.test/mcp"}))]);
    let (_, probed, runs) = run(job, Vec::new(), Vec::new(), Vec::new());
    assert!(
        probed.is_empty(),
        "no container is started for a url server"
    );
    assert_eq!(runs, 1);
}

/// Guards: a job that declares no stdio server starts no check.
/// Oracle: no probed command; the run starts.
#[test]
fn job_without_stdio_servers_starts_no_check() {
    let (_, probed, runs) = run(job_with(&[]), Vec::new(), Vec::new(), Vec::new());
    assert_eq!(probed, Vec::<Vec<String>>::new());
    assert_eq!(runs, 1);
}

/// Guards: the container a server is probed in holds the files a run's would: a server whose
/// command is a file of the treatment's skill tree passes the check.
/// Oracle: the skill file `.claude/skills/htn/SKILL.md` the skill repository fixture commits.
/// Bite: stage the cell's files outside the probe's mounts.
#[test]
fn server_in_the_skill_tree_passes_the_check() {
    let file = ".claude/skills/htn/SKILL.md";
    let job = job_with(&[("alpha", json!({"command": file}))]);
    let (_, probed, runs) = run(job, Vec::new(), Vec::new(), vec![String::from(file)]);
    assert_eq!(probed, vec![vec![file.to_owned()]]);
    assert_eq!(runs, 1, "the file is there, so the run starts");
}
