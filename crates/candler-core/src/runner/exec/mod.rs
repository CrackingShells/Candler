//! Executing one run: spawning its container, ending it, collecting its files and sealing it.

pub mod cell;
pub mod collect;
pub mod ends;
pub mod gather;
pub mod seal;
pub mod spawn;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::time::Duration;

use candler_harness::spec::{
    Cell, CommandSpec, Framing, Observed, PreparedFile, ProcessExit, Workspace,
};
use candler_schemas::container::ContainerRole;
use candler_schemas::execution::{EventPayload, Integrity, InvalidReason, RunManifest, RunTree};
use candler_schemas::primitives::{ContainerId, Digest, Marker, RunKey};
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{RawFileRef, TraceFile, TraceHeader};
use futures::future::BoxFuture;
use tokio::sync::watch;
use tokio::time::MissedTickBehavior;

pub use self::cell::{CellRun, ExecError, SkillTree};
use self::collect::finish_attempt;
use self::ends::{EndInput, TraceFacts, classify_end};
use self::gather::{collect_outputs, spent};
use self::seal::{RawStreamFile, seal_run};
use self::spawn::{Started, live_observer, require_isolation, start_attempt};
use super::evidence::census::census;
use super::evidence::leak::input_leak;
use super::evidence::provenance::provenance_values;
use super::evidence::run::{RunFacts, run_values};
use super::evidence::tree::tree_json;
use super::evidence::{Evidence, Inputs};
use super::plan::RunPlan;
use super::{RunControl, RunOutcome, RunStatus, Runtime};
use crate::isolation::container::mounts::{MountMode, MountRoots, MountSpec, mounts};
use crate::isolation::model::ContainerPlan;
use crate::isolation::{Isolation, IsolationError, RunOutputs};
use crate::jobs::model::JobCause;
use crate::jobs::slots::Slot;
use crate::store::blob::digest_of_file;
use crate::store::run::{Existing, RunWriter};
use crate::supervise::SuperviseError;
use crate::supervise::now;
use crate::supervise::spawn::{OutputSpec, SpawnRequest};
use crate::supervise::stop::{Ended, Exit};

/// The most bytes of a skill archive the core reads.
const ARCHIVE_LIMIT: usize = 64 * 1024 * 1024;

/// Executes the run `plan` while holding `slot`.
///
/// One attempt runs and is sealed. A gateway failure on a first attempt comes back with
/// [`ends::Retry::Once`]: the caller opens the next attempt with `Store::next_attempt` and calls
/// again. An error means no run directory was made, or the run is left unsealed for
/// `Store::seal_interrupted`; it never means a deleted run.
///
/// # Errors
/// [`ExecError::Isolation`] when the runtime has no isolation or the run cannot be placed,
/// [`ExecError::Store`] when the run cannot be written, [`ExecError::Manifest`] when its manifest
/// cannot be built, [`ExecError::Driver`] when the harness
/// cannot describe the run and [`ExecError::Supervise`] when the child cannot be run.
pub fn exec_run<'a>(
    rt: &'a Runtime,
    plan: &'a RunPlan,
    slot: &'a Slot,
    ctl: &'a RunControl,
) -> BoxFuture<'a, Result<RunOutcome, ExecError>> {
    Box::pin(drive(rt, plan, slot, ctl))
}

/// An error of the run's own directory that no store call reports.
fn local(op: &str, error: &io::Error) -> IsolationError {
    IsolationError::Plan {
        detail: format!("{op}: {error}"),
    }
}

/// The stream named `name`.
fn stream(name: &str) -> Result<StreamName, IsolationError> {
    StreamName::from_str(name).map_err(|error| IsolationError::Plan {
        detail: error.to_string(), // BORROW: .to_string() gives owned text
    })
}

/// The manifest of a run in the state `integrity` says.
fn manifest_of(
    rt: &Runtime,
    plan: &RunPlan,
    cell: &CellRun,
    integrity: Integrity,
    measured: Vec<candler_schemas::values::MeasuredValue>,
    started_at: candler_schemas::primitives::Timestamp,
    ended_at: Option<candler_schemas::primitives::Timestamp>,
) -> Result<RunManifest, ExecError> {
    // BORROW: a stored comparable wins over the fresh one, so a forced run keeps its record
    let comparable = plan.stored.as_ref().unwrap_or(&cell.comparable).clone();
    let mut builder = RunManifest::builder()
        .run(plan.key.clone()) // BORROW: the manifest owns the key
        .experiment(cell.experiment.clone()) // BORROW: the manifest owns the digest
        .comparable(comparable)
        .integrity(integrity)
        .job(rt.job.clone()) // BORROW: the manifest owns the job id
        .started_at(started_at);
    for factor in &cell.factors {
        builder = builder.factor(factor.clone()); // BORROW: the manifest owns its factors
    }
    for source in &cell.sources {
        builder = builder.source(source.clone()); // BORROW: the manifest owns its sources
    }
    for secret in &cell.secrets {
        builder = builder.secret(secret.clone()); // BORROW: the manifest owns its secrets
    }
    for deviation in &plan.deviations {
        builder = builder.deviation(deviation.clone()); // BORROW: the manifest owns its deviations
    }
    for input in &cell.evaluation_inputs {
        builder = builder.evaluation_input(input.clone()); // BORROW: the manifest owns its evaluation inputs
    }
    for value in measured {
        builder = builder.measured(value);
    }
    if let Some(ended) = ended_at {
        builder = builder.ended_at(ended);
    }
    Ok(builder.build()?)
}

/// Makes the mount point of each input in the workspace, mounts each input of the cell read-only
/// there from its blob in the store, and checks the whole mount list again.
fn mount_inputs(
    rt: &Runtime,
    container: &mut ContainerPlan,
    workspace_dir: &Path,
    inputs: &Inputs,
) -> Result<(), IsolationError> {
    if inputs.digests.is_empty() {
        return Ok(());
    }
    for input in &inputs.digests {
        // The mount point is made here, by the host user, so the engine does not make it as root.
        let relative = Path::new(&input.path);
        if !relative
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
        {
            return Err(IsolationError::Plan {
                detail: format!("the input path `{}` leaves the workspace", input.path),
            });
        }
        let point = workspace_dir.join(relative);
        if let Some(parent) = point.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| local("cannot make an input's directory", &e))?;
        }
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&point)
            .map_err(|e| local("cannot make an input's mount point", &e))?;
        container.mounts.push(MountSpec {
            source: rt.store.layout().blob(&input.digest),
            target: format!("{}/{}", container.workdir.trim_end_matches('/'), input.path),
            mode: MountMode::ReadOnly,
        });
    }
    container.mounts = mounts(container, &MountRoots::from_plan(container)).map_err(|error| {
        IsolationError::Plan {
            detail: error.to_string(), // BORROW: .to_string() gives owned text
        }
    })?;
    Ok(())
}

/// Writes `file` under `root`, refusing to replace a file.
fn write_file(root: &Path, file: &PreparedFile) -> io::Result<Digest> {
    let target = root.join(file.path.as_str()); // BORROW: explicit .as_str()
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)?;
    out.write_all(&file.contents)?;
    drop(out);
    if file.executable {
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
    }
    digest_of_file(&target).map_err(|error| io::Error::other(error.to_string()))
}

/// Archives the skill tree of the cell's treatment and returns its files; none when the
/// treatment pins no commit.
async fn skill_files(
    rt: &Runtime,
    cell: &CellRun,
    comparable: &candler_schemas::execution::Comparable,
    scratch: &Path,
) -> Result<Vec<PreparedFile>, ExecError> {
    let treatment = comparable.treatment();
    if treatment.commit.is_none() {
        return Ok(Vec::new());
    }
    let source = cell
        .treatment_source
        .as_deref()
        .ok_or_else(|| IsolationError::Plan {
            detail: "the treatment is pinned to a commit and names no source".to_owned(), // BORROW: owned text
        })?;
    let command = rt.skills.archive_command(treatment, source)?;
    let record = scratch.join("archive");
    fs::create_dir_all(&record).map_err(|e| local("cannot make the archive directory", &e))?;
    let marker = Marker::from_str("skill-archive").map_err(|error| IsolationError::Plan {
        detail: error.to_string(), // BORROW: .to_string() gives owned text
    })?;
    let path = std::env::var("PATH").unwrap_or_default();
    let request = SpawnRequest::builder()
        .argv(command.argv().to_vec()) // BORROW: the request owns its arguments
        .env(vec![
            ("PATH".to_owned(), path), // BORROW: owned pair for the environment
            ("LC_ALL".to_owned(), "C".to_owned()), // BORROW: owned pair for the environment
            ("GIT_TERMINAL_PROMPT".to_owned(), "0".to_owned()), // BORROW: owned pair
        ])
        .marker(marker)
        .record_dir(record)
        .build()
        .map_err(|error| IsolationError::Plan {
            detail: error.to_string(), // BORROW: .to_string() gives owned text
        })?;
    let captured = rt.supervisor.run_captured(request, ARCHIVE_LIMIT).await?;
    let ok = matches!(captured.exit.status, ExitStatus::Code { code: 0 });
    if !ok || captured.stdout.len() >= ARCHIVE_LIMIT {
        return Err(IsolationError::Plan {
            detail: "the skill tree could not be archived".to_owned(), // BORROW: owned detail
        }
        .into());
    }
    Ok(rt.skills.files(treatment, &captured.stdout)?)
}

/// Waits for `child`, and stops it when the job's cancel carries a cause.
async fn wait_or_cancel(
    child: crate::supervise::spawn::Child,
    mut cancel: watch::Receiver<Option<JobCause>>,
) -> (Result<Exit, SuperviseError>, Option<JobCause>) {
    let stop = child.stop_handle();
    let mut cause: Option<JobCause> = *cancel.borrow();
    if cause.is_some() {
        stop.request();
    }
    let wait = child.wait();
    tokio::pin!(wait);
    let mut watching = cause.is_none();
    loop {
        tokio::select! {
            result = &mut wait => return (result, cause),
            changed = cancel.changed(), if watching => match changed {
                Ok(()) => {
                    let seen: Option<JobCause> = *cancel.borrow_and_update();
                    if seen.is_some() {
                        cause = seen;
                        stop.request();
                        watching = false;
                    }
                }
                Err(_) => watching = false,
            },
        }
    }
}

/// The stream a raw file archives, from its name `<stream>.jsonl.gz`.
fn stream_of(path: &Path) -> Option<StreamName> {
    let name = path.file_name()?.to_str()?;
    StreamName::from_str(name.strip_suffix(".jsonl.gz")?).ok()
}

/// Millionths of a dollar in `usd`, rounded; a negative or non-finite cost counts as none.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    reason = "the value is rounded, finite and not negative before the cast"
)]
fn micro_usd(usd: f64) -> u64 {
    if usd.is_finite() && usd > 0.0 {
        // CAST: rounded, finite and positive, so the conversion cannot wrap
        (usd * 1_000_000.0).round() as u64
    } else {
        0
    }
}

/// How often the run asks the isolation to record its container while the client runs.
const RECORD_POLL: Duration = Duration::from_millis(100);

/// Asks `isolation` every [`RECORD_POLL`] to record the run's container, and resolves with its id
/// once it is recorded. A refused or failed record ends the polling without an id: the label
/// sweep still bounds the container.
async fn record_when_known(
    isolation: &dyn Isolation, // TRAIT_OBJECT: the isolation is docker, or a fake in tests
    container: &ContainerPlan,
    key: &RunKey,
) -> Option<ContainerId> {
    let mut tick = tokio::time::interval(RECORD_POLL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        match isolation.record_container(container, key).await {
            Ok(Some(cid)) => return Some(cid),
            Ok(None) => {}
            // EXPLICIT: a refused or failed record is not retried; the label sweep bounds the container
            Err(_error) => return None,
        }
    }
}

/// Emits the creation of the run's agent container, now that its record is written.
fn announce_agent(rt: &Runtime, plan: &RunPlan, cid: Option<ContainerId>) {
    if let Some(cid) = cid {
        rt.sink.emit(
            Some(&plan.key),
            EventPayload::container_created(cid, ContainerRole::Agent),
        );
    }
}

/// The body of [`exec_run`].
async fn drive(
    rt: &Runtime,
    plan: &RunPlan,
    _slot: &Slot,
    ctl: &RunControl,
) -> Result<RunOutcome, ExecError> {
    // The caller owns the slot and releases it when it drops; the borrow lasts as long as the run.
    let isolation = require_isolation(rt)?;
    let cell = rt
        .cells
        .get(&plan.cell)
        .ok_or_else(|| IsolationError::Plan {
            detail: "the job holds no record of the run's cell".to_owned(), // BORROW: owned detail
        })?;
    let comparable = plan.stored.as_ref().unwrap_or(&cell.comparable);
    let started_at = now()?;
    let first = manifest_of(
        rt,
        plan,
        cell,
        Integrity::invalid(InvalidReason::Interrupted),
        Vec::new(),
        started_at.clone(), // BORROW: the final manifest repeats the start
        None,
    )?;
    let writer = RunWriter::create(&rt.store, &first, Existing::Refuse)?;
    let layout = rt.store.layout();
    let workspace_dir = layout.run_dir(&plan.key).join("workspace");
    fs::create_dir(&workspace_dir).map_err(|e| local("cannot make the workspace", &e))?;
    let scratch: PathBuf = layout
        .job_dir(&rt.job)
        .join("scratch")
        .join(plan.key.to_string());
    let frames = scratch.join("frames");
    fs::create_dir_all(&frames).map_err(|e| local("cannot make the frame directory", &e))?;

    let view = Cell::new(comparable.values());
    let workspace = Workspace::new(&workspace_dir);
    let prepared = rt.driver.prepare(&view, &workspace)?;
    let skills = skill_files(rt, cell, comparable, &scratch).await?;
    let rendered: BTreeSet<&str> = prepared.files.iter().map(|f| f.path.as_str()).collect(); // BORROW: explicit .as_str()
    let mut written: BTreeMap<String, Digest> = BTreeMap::new();
    for file in &skills {
        if rendered.contains(file.path.as_str()) {
            // BORROW: the rendered set holds `&str`, so the path is lent as one
            return Err(IsolationError::Plan {
                detail: format!(
                    "the skill tree holds `{}`, which prepare renders",
                    file.path
                ),
            }
            .into());
        }
    }
    for file in skills.iter().chain(&prepared.files) {
        let digest = write_file(&workspace_dir, file)
            .map_err(|e| local("cannot write a workspace file", &e))?;
        written.insert(file.path.as_str().to_owned(), digest); // BORROW: the map owns its paths
    }

    let mut command = rt.driver.command(&view, &workspace)?;
    if !prepared.env_overlay.is_empty() {
        let mut builder = CommandSpec::builder().argv(command.argv().to_vec()); // BORROW: the new command owns its arguments
        for entry in prepared.env_overlay.iter().chain(command.env_overlay()) {
            builder = builder.env(entry.clone()); // BORROW: the new command owns its entries
        }
        if let Some(cwd) = command.cwd() {
            builder = builder.cwd(cwd.clone()); // BORROW: the new command owns its directory
        }
        command = builder
            .build()
            .map_err(candler_harness::driver::DriverError::from)?;
    }
    let mut container = isolation.plan(&view, &workspace)?;
    mount_inputs(rt, &mut container, &workspace_dir, &cell.inputs)?;

    let out_stream = stream("stdout")?;
    let err_stream = stream("stderr")?;
    let mut reader_id = None;
    let mut handle = None;
    let reader = rt.driver.live_reader();
    let framing = reader
        .as_ref()
        .map_or(Framing::Lines, |reader| reader.framing());
    let mut stdout = OutputSpec::framed(frames.clone(), out_stream.clone(), framing); // BORROW: the spec owns its directory and stream
    if let Some(reader) = reader {
        reader_id = Some(reader.id());
        let (observer, live) = live_observer(reader, out_stream);
        stdout = stdout.with_observer(Box::new(observer));
        handle = Some(live);
    }
    let stderr = OutputSpec::framed(frames, err_stream, Framing::Lines);
    let isolated = isolation.run_request(
        &container,
        command,
        Marker::for_run(&plan.key),
        plan.key.clone(), // BORROW: the request owns its key
        RunOutputs::new(stdout, stderr),
    )?;

    rt.sink.emit(Some(&plan.key), EventPayload::run_started());
    let attempt_began = std::time::Instant::now();
    let Started { child, end_facts } = start_attempt(rt, isolated)?;
    // The agent's container is recorded as soon as its cidfile names it, while the client runs.
    let (mut polled, mut recorded) = (false, false);
    let (waited, cause) = {
        let wait = wait_or_cancel(child, ctl.cancel.clone()); // BORROW: the wait owns a receiver
        let poll = record_when_known(isolation.as_ref(), &container, &plan.key);
        tokio::pin!(wait, poll);
        loop {
            tokio::select! {
                result = &mut wait => break result,
                cid = &mut poll, if !polled => {
                    polled = true;
                    recorded = cid.is_some();
                    announce_agent(rt, plan, cid);
                }
            }
        }
    };
    if !recorded {
        // EXPLICIT: a failed record leaves the container to the label sweep
        let cid = isolation
            .record_container(&container, &plan.key)
            .await
            .ok()
            .flatten();
        announce_agent(rt, plan, cid);
    }
    let exit = waited?;
    let duration_ms = u64::try_from(attempt_began.elapsed().as_millis()).unwrap_or(u64::MAX);
    let finished = finish_attempt(
        isolation.as_ref(),
        &container,
        &workspace,
        &exit,
        &plan.key,
        end_facts.as_ref(),
    )
    .await;

    // What the reader made of the output.
    let mut reader_failed = false;
    let (mut sessions, mut events) = (Vec::new(), Vec::new());
    if let Some(live) = handle {
        match live.finish() {
            Ok(trace) => {
                sessions = trace.sessions;
                events = trace.events;
            }
            Err(_) => reader_failed = true,
        }
    }
    let raw_refs: Vec<RawFileRef> = exit
        .raw
        .iter()
        .filter_map(|info| {
            stream_of(&info.path).map(|name| RawFileRef::new(name, info.digest.clone())) // BORROW: the header owns the digest
        })
        .collect();
    let reader_name = match reader_id {
        Some(id) => id,
        None => format!("{}@0.0.0", rt.driver.id()).parse().map_err(
            |error: candler_schemas::primitives::FormatError| IsolationError::Plan {
                detail: error.to_string(), // BORROW: .to_string() gives owned text
            },
        )?,
    };
    let header = TraceHeader::new(reader_name, raw_refs);
    // BORROW: the header and the parts are cloned because the failure arm below still reads them
    let trace = if let Ok(trace) = TraceFile::new(header.clone(), sessions.clone(), events.clone())
    {
        trace
    } else {
        reader_failed = true;
        sessions.clear();
        events.clear();
        TraceFile::new(header, Vec::new(), Vec::new()).map_err(|error| IsolationError::Plan {
            detail: format!("the trace cannot be built: {error}"),
        })?
    };
    let used = spent(&events);
    let facts = if reader_failed {
        TraceFacts::ReaderError
    } else if sessions.is_empty() {
        TraceFacts::Silent
    } else {
        let process = ProcessExit::new(exit.status, exit.ended.into(), exit.stderr_tail.clone()); // BORROW: the exit keeps its tail
        gather::read_facts(rt.driver.classify_exit(&process, &sessions), &used)
    };

    // What the run left in its workspace.
    let mut collect_ok = finished.collect_ok;
    let observed = Observed::new(sessions.iter().map(|s| s.id().clone()).collect()); // BORROW: the view owns its ids
    let specs = rt.driver.collect(&workspace, &observed);
    let outputs = if let Ok(mut found) = collect_outputs(&workspace_dir, &specs, &cell.inputs) {
        found.retain(|out| written.get(&out.path) != Some(&out.digest));
        found
    } else {
        collect_ok = false;
        Vec::new()
    };
    let mut evidence = Evidence {
        sessions: sessions.clone(), // BORROW: the trace keeps its own copy
        events: events.clone(),     // BORROW: the trace keeps its own copy
        outputs,
        tree: RunTree::builder()
            .build()
            .map_err(|error| IsolationError::Plan {
                detail: error.to_string(), // BORROW: .to_string() gives owned text
            })?,
    };
    let inputs = &cell.inputs;
    let leaked = input_leak(&evidence, inputs).is_some();
    match tree_json(&evidence, inputs) {
        Ok(tree) => evidence.tree = tree,
        Err(_) => collect_ok = false,
    }
    let mut measured = census(&evidence.sessions, &evidence.events).unwrap_or_default();

    let gateway_failed = match &rt.gateway {
        Some(gateway) => gateway
            .service
            .run_signals(&gateway.handle, &plan.key)
            .await
            .is_ok_and(|signals| !signals.is_empty()),
        None => false,
    };
    let inspect = finished.outcome.as_ref().and_then(|o| o.inspect.clone()); // BORROW: the outcome keeps its inspect
    let client_exit = match exit.status {
        ExitStatus::Code { code } => Some(code),
        ExitStatus::Signal { .. } => None,
    };
    let stopped = (exit.ended == Ended::Stopped).then_some(cause).flatten();
    let input = EndInput::new()
        .with_client_exit(client_exit)
        .with_container_started(inspect.is_some() || client_exit != Some(125))
        .with_inspect(inspect.clone()) // BORROW: the outcome keeps its own copy
        .with_stop(stopped)
        .with_deadline(exit.ended == Ended::Deadline)
        .with_gateway_failed(gateway_failed)
        .with_trace(facts)
        .with_collect_ok(collect_ok)
        .with_input_leak(leaked)
        .with_attempt(plan.attempt);
    let end = classify_end(&input);
    measured.extend(run_values(&RunFacts {
        ended_by: &end.ended_by,
        client_exit,
        duration_ms,
        events: &evidence.events,
    }));
    measured.extend(provenance_values(&rt.eval_infra, &cell.packages));

    let integrity = end.invalid.map_or(Integrity::Valid, Integrity::invalid);
    let last = manifest_of(
        rt,
        plan,
        cell,
        integrity,
        measured,
        started_at,
        Some(now()?),
    )?;
    let raw_files: Vec<RawStreamFile> = exit
        .raw
        .iter()
        .filter_map(|info| {
            stream_of(&info.path)
                .map(|name| RawStreamFile::new(name, info.path.clone(), info.digest.clone())) // BORROW: the file list owns its path and digest
        })
        .collect();
    seal_run(writer, &last, &evidence.tree, &raw_files, &trace)?;
    // EXPLICIT: the scratch directory holds only what the seal moved or no longer needs
    let _ = fs::remove_dir_all(&scratch);

    rt.sink.emit(
        Some(&plan.key),
        EventPayload::run_ended(end.ended_by.clone(), Some(exit.status)), // BORROW: the outcome keeps its own copy
    );
    let usage = used.usage();
    ctl.meter.add(usage.tokens, micro_usd(usage.cost_usd));
    Ok(RunOutcome {
        key: plan.key.clone(), // BORROW: the outcome owns the key
        status: if end.invalid.is_none() {
            RunStatus::Measured
        } else {
            RunStatus::Invalid
        },
        ended_by: end.ended_by,
        invalid: end.invalid,
        inspect,
        usage,
        retry: end.retry,
    })
}
