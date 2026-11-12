#![cfg(test)]
//! The shared fixture suite on the `OpenCode` driver: the nine recorded runs of `opencode_facts`.
//!
//! Every case reads its bytes from the fixtures and its facts (session id, exit status, export
//! outcome, terminal error type) from the fixture's hand-written `expected.toml` and `exit.toml`.
//! The class each run must end as is a literal below, derived by hand from R01 §Ends (a gateway or
//! provider failure, an agent error, a stop that the core classifies) and the case's recorded
//! outcome; it never comes from the driver.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use candler_harness::conformance::{Case, ExpectedSession, ReaderSource, Setup, run};
use candler_harness::spec::{
    Cell, CollectSpec, CommandSpec, ObservedEnd, ProcessEnd, ProcessExit, Workspace,
};
use candler_harness_opencode::driver::OpenCodeDriver;
use candler_schemas::primitives::{FieldPath, ReasonCode};
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, SessionId};
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::json;

// TRAIT_OBJECT: a test returns any error its steps raise; the error is only printed
type Res<T> = Result<T, Box<dyn Error>>;

/// A value that stands for the gateway's token; it must appear in no command and no file.
const CANARY: &str = "sk-conformance-canary-7f3a";

/// One recorded run and what the driver must make of it: the class of its end read from the
/// export, and the class when only the live stream is left.
struct Run {
    case: &'static str,
    outcome: &'static str,
    live_end: Option<ObservedEnd>,
    with_export: EndedBy,
    live_only: EndedBy,
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("streams")
}

fn read(case: &str, file: &str) -> Res<Vec<u8>> {
    Ok(std::fs::read(fixtures().join(case).join(file))?)
}

/// The value of `key = <value>` in a hand-written fixture note, any trailing comment cut.
fn note(case: &str, file: &str, key: &str) -> Res<String> {
    let text = String::from_utf8(read(case, file)?)?;
    let line = text
        .lines()
        .find(|line| line.starts_with(&format!("{key} = ")))
        .ok_or_else(|| format!("{case}/{file} has no `{key}`"))?;
    let value = line.split_once(" = ").map_or("", |(_, v)| v);
    let value = value.split(" #").next().unwrap_or_default().trim();
    Ok(value.trim_matches('"').to_owned())
}

fn sid(text: &str) -> Res<SessionId> {
    Ok(text.parse()?)
}

fn error_kind(kind: &str) -> ObservedEnd {
    ObservedEnd::Error {
        kind: kind.to_owned(), // BORROW: .to_owned() makes the owned text
    }
}

fn reason(text: &str) -> Res<EndedBy> {
    Ok(EndedBy::agent_error(Some(ReasonCode::from_str(text)?)))
}

fn runs() -> Res<Vec<Run>> {
    Ok(vec![
        Run {
            case: "completed_o1",
            outcome: "succeeded",
            live_end: Some(ObservedEnd::Finished),
            with_export: EndedBy::Completed,
            live_only: EndedBy::Completed,
        },
        Run {
            case: "interrupt_api",
            outcome: "interrupted",
            live_end: Some(ObservedEnd::Aborted),
            with_export: EndedBy::Unknown,
            live_only: EndedBy::Unknown,
        },
        Run {
            case: "refused_key",
            outcome: "failed",
            live_end: Some(error_kind("provider.auth")),
            with_export: EndedBy::GatewayError,
            live_only: EndedBy::GatewayError,
        },
        Run {
            case: "sigint_mid_tool",
            outcome: "interrupted",
            live_end: None,
            with_export: EndedBy::Unknown,
            live_only: EndedBy::Unknown,
        },
        Run {
            case: "sigterm_mid_tool",
            outcome: "interrupted",
            live_end: None,
            with_export: EndedBy::Unknown,
            live_only: EndedBy::Unknown,
        },
        Run {
            case: "unknown_model_local",
            outcome: "failed",
            live_end: Some(error_kind("provider.no-route")),
            with_export: reason("provider_no_route")?,
            live_only: reason("provider_no_route")?,
        },
        Run {
            case: "unknown_model_remote",
            outcome: "failed",
            live_end: Some(error_kind("provider.invalid-request")),
            with_export: EndedBy::GatewayError,
            live_only: EndedBy::GatewayError,
        },
        Run {
            case: "unreachable_dns",
            outcome: "interrupted",
            live_end: None,
            with_export: EndedBy::Unknown,
            live_only: EndedBy::Unknown,
        },
        Run {
            case: "unreachable_refused",
            outcome: "failed",
            live_end: Some(error_kind("provider.transport")),
            with_export: EndedBy::GatewayError,
            live_only: EndedBy::GatewayError,
        },
    ])
}

fn exit_of(case: &str) -> Res<ProcessExit> {
    let code: i32 = note(case, "exit.toml", "exit_status")?.parse()?;
    Ok(ProcessExit::new(
        ExitStatus::Code { code },
        ProcessEnd::Exited,
        Vec::new(),
    ))
}

fn stream(name: &str) -> Res<StreamName> {
    Ok(StreamName::from_str(name)?)
}

/// The command that collects the export of `id`: written by hand from `opencode_facts` rows 4 and 6
/// (`session export` needs `--standalone` and the run's state, so it runs inside the isolation).
fn export_collect(id: &SessionId, name: &StreamName) -> Res<CollectSpec> {
    let argv = ["opencode", "session", "export", "--standalone", id.as_str()]
        .map(str::to_owned) // BORROW: .to_owned() makes owned arguments
        .to_vec();
    Ok(CollectSpec::exec(
        CommandSpec::builder().argv(argv).build()?,
        name.clone(), // BORROW: .clone() copies the name
    ))
}

/// Every run twice: the live stream with its export (the export is the trace) and the live stream
/// alone (the export could not be collected).
fn cases() -> Res<Vec<Case>> {
    let (stdout, export) = (stream("stdout")?, stream("export")?);
    let mut cases = Vec::new();
    for run in runs()? {
        let id = sid(&note(run.case, "expected.toml", "session_id")?)?;
        let recorded = note(run.case, "expected.toml", "export_outcome")?;
        assert_eq!(recorded, run.outcome, "{}: the recorded outcome", run.case);
        let exit = exit_of(run.case)?;
        let live = |name: String| -> Res<Case> {
            let case = Case::new(
                name,
                ReaderSource::Live,
                stdout.clone(), // BORROW: .clone() copies the name
                read(run.case, "stdout.jsonl")?,
            )
            .with_observed_ids(vec![id.clone()]) // BORROW: .clone() copies the id
            .with_collect(vec![export_collect(&id, &export)?]);
            Ok(match &run.live_end {
                Some(end) => case.with_observed_end(end.clone()), // BORROW: .clone() copies the end
                None => case,
            })
        };
        cases.push(
            live(format!("{}_with_export", run.case))?
                .with_export(
                    export.clone(), // BORROW: .clone() copies the name
                    read(run.case, "export.json")?,
                    vec![ExpectedSession::new(
                        id.clone(),
                        None,
                        run.with_export.clone(),
                    )], // BORROW: .clone() copies
                )
                .with_end(exit.clone(), run.with_export.clone()), // BORROW: .clone() copies
        );
        cases.push(live(format!("{}_live_only", run.case))?.with_end(exit, run.live_only));
        // The export read on its own, as the core reads a collected stream.
        cases.push(
            Case::new(
                format!("{}_export", run.case),
                ReaderSource::Offline(export.clone()), // BORROW: .clone() copies the name
                export.clone(),                        // BORROW: .clone() copies the name
                read(run.case, "export.json")?,
            )
            .with_sessions(
                vec![ExpectedSession::new(id.clone(), None, run.with_export)], // BORROW: .clone() copies
                vec![id],
            ),
        );
    }
    // The live stream ended finished and the export says interrupted: the export wins.
    let id = sid(&note("interrupt_api", "expected.toml", "session_id")?)?;
    cases.push(
        Case::new(
            "export_wins_over_the_live_end".to_owned(), // BORROW: .to_owned() makes the owned name
            ReaderSource::Live,
            stdout,
            read("completed_o1", "stdout.jsonl")?,
        )
        .with_observed_ids(vec![sid(&note(
            "completed_o1",
            "expected.toml",
            "session_id",
        )?)?])
        .with_observed_end(ObservedEnd::Finished)
        .with_export(
            export,
            read("interrupt_api", "export.json")?,
            vec![ExpectedSession::new(id, None, EndedBy::Unknown)],
        )
        .with_end(exit_of("interrupt_api")?, EndedBy::Unknown),
    );
    Ok(cases)
}

/// The cell the driver builds its commands from.
fn values() -> Res<Vec<ResolvedValue>> {
    [
        ("model.id", json!("claude-haiku-4-5")),
        ("model.endpoint", json!("https://api.example.test")),
        ("permission_mode", json!("bypassPermissions")),
        ("prompt.template", json!("Build {thing} now")),
        ("prompt.values", json!({"thing": "a domain"})),
    ]
    .into_iter()
    .map(|(path, value)| {
        Ok(ResolvedValue::recorded(
            FieldPath::from_str(path)?,
            JsonValue::new(value)?,
        ))
    })
    .collect()
}

/// Guards: R01 §Harness drivers, §OpenCode v2 — the recorded runs of OpenCode V2 read, classify and
/// collect through the shared traits: sessions from the export, the end from the export's outcome
/// over the stream's last error event, the collect command inside the isolation.
/// Oracle: the fixtures' hand-written `expected.toml` and `exit.toml`, and R01 §Ends by hand.
#[test]
fn conformance_opencode_recorded_runs() -> Res<()> {
    let values = values()?;
    let cell = Cell::new(&values);
    let workspace = Workspace::new(Path::new("."));
    let canaries = [CANARY.to_owned()]; // BORROW: .to_owned() makes the owned canary
    let driver = OpenCodeDriver::new()?;
    run(
        &driver,
        &Setup::new(&cell, &workspace, &canaries),
        &cases()?,
    )?;
    Ok(())
}
