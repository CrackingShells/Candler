//! Guards for the start check of a job's declared MCP servers: what a server that exits at start
//! does to the job, and what the check leaves alone.
//!
//! The job runs over the fake docker client of the job guards; the isolation is wrapped by a
//! double that answers each server's start probe from a script, so no container exists.
#![cfg(test)]

use crate::support;

use std::sync::{Arc, Mutex};

use candler_core::isolation::testing::ProbingIsolation;
use candler_core::isolation::{Isolation, StartProbe};
use candler_core::jobs::model::{JobCause, JobControl, JobOutcome, JobSpec};
use candler_core::jobs::orchestrate::orchestrate_with;
use candler_core::jobs::orchestrate::session::{JobHost, OrchestrateError};
use candler_schemas::execution::ExperimentFile;
use serde_json::{Value, json};
use support::job_orchestrate::{FakeHost, Rig, spec};
use support::run_plan::Facts;
use tokio::sync::watch;

/// The host of the job guards, whose isolation is wrapped by a [`ProbingIsolation`] the test reads.
struct Probed {
    inner: Arc<FakeHost>,
    answers: Vec<(String, StartProbe)>,
    probing: Mutex<Option<Arc<ProbingIsolation>>>,
}

impl JobHost for Probed {
    fn docker_cli(
        &self,
        supervisor: Arc<candler_core::supervise::Supervisor>,
        record_dir: std::path::PathBuf,
    ) -> Result<
        candler_core::isolation::docker::DockerCli,
        candler_core::isolation::docker::DockerError,
    > {
        self.inner.docker_cli(supervisor, record_dir)
    }

    fn gateway<'a>(
        &'a self,
        rt: &'a candler_core::runner::Runtime,
        spec: &'a JobSpec,
    ) -> futures::future::BoxFuture<
        'a,
        Result<Arc<dyn candler_core::isolation::gateway::Gateway>, OrchestrateError>,
    > {
        self.inner.gateway(rt, spec)
    }

    fn isolation(
        &self,
        isolation: candler_core::isolation::container::DockerIsolation,
    ) -> Arc<dyn Isolation> {
        let probing = Arc::new(ProbingIsolation::new(self.inner.isolation(isolation)));
        for (program, answer) in &self.answers {
            probing.answer(program, answer.clone()); // BORROW: the double owns its answers
        }
        *self.probing.lock().expect("the double") = Some(Arc::clone(&probing)); // BORROW: the test reads the same double
        probing
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

/// A one-run job whose experiment declares `servers` under `mcp.servers`, as `(key, value)`.
fn job_with(servers: &[(&str, Value)]) -> JobSpec {
    let mut job = spec(&["m1"], &facts(), 3600, 1);
    let mut text = String::from(
        "schema = \"experiment/1\"\nreplicates = 1\n\n[[factors]]\npath = \"model.id\"\nlevels = [\"m1\"]\n\n[budget.per_run]\nwall_s = 3600\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"htn\"\n",
    );
    for (key, value) in servers {
        text.push_str(&format!("\n[fixed.mcp.servers.{key}]\n"));
        for (field, leaf) in value.as_object().expect("a server object") {
            text.push_str(&format!("{field} = {leaf}\n"));
        }
    }
    job.experiment = ExperimentFile::from_toml_str(&text).expect("an experiment file");
    job
}

/// Runs `job` to its end over a probing host; returns the outcome, the commands probed and the
/// number of run requests made.
fn run(job: JobSpec, answers: Vec<(String, StartProbe)>) -> (JobOutcome, Vec<Vec<String>>, usize) {
    let rig = Rig::new(0);
    let host = Arc::new(Probed {
        inner: rig.host(),
        answers,
        probing: Mutex::new(None),
    });
    let (cancel, _keep) = watch::channel::<Option<JobCause>>(None);
    let ctl = JobControl {
        cancel: Arc::new(cancel),
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let shared: Arc<dyn JobHost> = host.clone();
    let outcome = runtime.block_on(orchestrate_with(rig.runtime(), job, ctl, shared));
    let probing = host.probing.lock().expect("the double").clone();
    let probing = probing.expect("the job built an isolation");
    (outcome, probing.probed(), probing.runs())
}

/// Guards: a declared server that exits at start refuses the job before any run, and the refusal
/// names the server and carries what it printed.
/// Oracle: the literal `boom: no module named x` the scripted server prints.
#[test]
fn server_that_exits_refuses_the_job_with_its_stderr() {
    let job = job_with(&[("alpha", json!({"command": "badserver", "args": ["--x"]}))]);
    let answers = vec![(
        String::from("badserver"),
        StartProbe::Exited {
            output: String::from("boom: no module named x"),
        },
    )];
    let (outcome, probed, runs) = run(job, answers);
    assert_eq!(probed, vec![vec!["badserver".to_owned(), "--x".to_owned()]]);
    assert_eq!(runs, 0, "no run starts");
    assert!(outcome.runs.is_empty(), "no run is recorded");
}

/// Guards: a declared server that is still running after the wait lets the job start.
/// Oracle: the one probed command and the run request that follows it.
#[test]
fn server_that_stays_up_lets_the_job_start() {
    let job = job_with(&[("alpha", json!({"command": "goodserver"}))]);
    let (outcome, probed, runs) = run(job, Vec::new());
    assert_eq!(probed, vec![vec!["goodserver".to_owned()]]);
    assert_eq!(runs, 1, "the run starts");
    assert_eq!(outcome.runs.len(), 1);
}

/// Guards: a server declared by `url` is not started.
/// Oracle: no probed command; the run starts.
#[test]
fn remote_server_is_skipped() {
    let job = job_with(&[("remote", json!({"url": "https://example.test/mcp"}))]);
    let (_, probed, runs) = run(job, Vec::new());
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
    let (_, probed, runs) = run(job_with(&[]), Vec::new());
    assert!(probed.is_empty());
    assert_eq!(runs, 1);
}
