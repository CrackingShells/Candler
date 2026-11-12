//! The shared fixture suite: one set of checks that every harness driver runs on its own recorded
//! fixtures.
//!
//! [`run`] takes a driver, a [`Setup`] (the cell and workspace its commands are built from) and
//! the driver's [`Case`]s. A case is one recorded stream with what a reader must make of it. The
//! suite makes no assumption about a harness: it takes the framing from the reader, the census
//! source from the driver, the stop from the driver's own spec, and a case with no inventory must
//! leave the census unrecorded. A driver crate calls [`run`] from a test named `conformance_*`.

use std::fmt;

use candler_schemas::raw::{FrameData, RawFrame, RawRef, StreamName};
use candler_schemas::trace::{EndedBy, Session, SessionId};

use crate::driver::{DriverError, HarnessDriver};
use crate::reader::{FrameIn, TraceReader};
use crate::spec::{
    Action, Cell, CensusSource, CollectSpec, CommandSpec, EnvValue, FileRoot, Framing, HttpMethod,
    InitReport, Observed, ObservedEnd, PendingState, ProcessExit, SessionClose, SessionRecord,
    StateKind, StopSpec, Workspace,
};

/// A session id plain enough to stand in a script, used to check that a placeholder resolves.
const SAMPLE_ID: &str = "ses_conformance";
/// A session id no script may receive, used to check that a placeholder refuses it.
const UNSAFE_ID: &str = "ses_1;touch$IFS/x";

/// Which of the driver's readers reads a case.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReaderSource {
    /// The reader of the child's live output.
    Live,
    /// The reader of a stream the core collected after the run, such as an export or a census
    /// record.
    Offline(StreamName),
}

/// One session a reader must produce.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExpectedSession {
    /// The id of the session.
    pub id: SessionId,
    /// The id of its parent; `None` for a root.
    pub parent: Option<SessionId>,
    /// How it ended.
    pub ended_by: EndedBy,
}

impl ExpectedSession {
    /// An expected session.
    #[must_use]
    pub const fn new(id: SessionId, parent: Option<SessionId>, ended_by: EndedBy) -> Self {
        Self {
            id,
            parent,
            ended_by,
        }
    }
}

/// One record fed to a session tracker.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordBytes {
    /// A frame of the live output.
    Stream(Vec<u8>),
    /// A line of the hook log.
    Hook(Vec<u8>),
}

/// A record fed to a session tracker and, when given, the state the tracker must report after it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TrackStep {
    /// The record.
    pub record: RecordBytes,
    /// The state the tracker must report after the record.
    pub state: Option<PendingState>,
}

impl TrackStep {
    /// A step.
    #[must_use]
    pub const fn new(record: RecordBytes, state: Option<PendingState>) -> Self {
        Self { record, state }
    }
}

/// One recorded stream and what the driver must make of it.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Case {
    /// The name the failures cite.
    pub name: String,
    /// The reader that reads it.
    pub reader: ReaderSource,
    /// The stream name its frames carry.
    pub stream: StreamName,
    /// The recorded bytes: lines for a Lines reader, a whole document for a Document reader.
    pub bytes: Vec<u8>,
    /// Whether the reader must recognise the start of the bytes.
    pub detects: bool,
    /// The sessions the reader must finish with, root first; `None` for a stream that is not read
    /// into sessions, such as a census record.
    pub sessions: Option<Vec<ExpectedSession>>,
    /// The session ids the reader must have observed, in order.
    pub observed_ids: Vec<SessionId>,
    /// The inventory the reader must report; `None` means it must report none: unrecorded.
    pub init: Option<InitReport>,
    /// The end the stream itself must have told, if any.
    pub observed_end: Option<ObservedEnd>,
    /// The exit the run is classified from, with the class of the run's end; `None` for a stream
    /// that is not classified. Without sessions the classification sees an empty list, as when
    /// the export could not be collected.
    pub end: Option<(ProcessExit, EndedBy)>,
    /// What the driver must collect for the observed sessions, when the case states it.
    pub collect: Option<Vec<CollectSpec>>,
    /// An export read after the live stream: the stream it is read as, its bytes and the sessions
    /// it must give. The export is authoritative: the run is classified from its sessions, with
    /// the live reader's observed end beside them.
    pub export: Option<(StreamName, Vec<u8>, Vec<ExpectedSession>)>,
    /// Records for the session tracker, in the order the core reads them.
    pub track: Vec<TrackStep>,
}

impl Case {
    /// A case that expects no session, no inventory, no end class and nothing collected.
    #[must_use]
    pub const fn new(
        name: String,
        reader: ReaderSource,
        stream: StreamName,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            name,
            reader,
            stream,
            bytes,
            detects: true,
            sessions: None,
            observed_ids: Vec::new(),
            init: None,
            observed_end: None,
            end: None,
            collect: None,
            export: None,
            track: Vec::new(),
        }
    }

    /// The sessions the reader must finish with, and the ids it must have observed.
    #[must_use]
    pub fn with_sessions(
        mut self,
        sessions: Vec<ExpectedSession>,
        observed_ids: Vec<SessionId>,
    ) -> Self {
        self.sessions = Some(sessions);
        self.observed_ids = observed_ids;
        self
    }

    /// The session ids the reader must have observed, for a stream that is not read into sessions.
    #[must_use]
    pub fn with_observed_ids(mut self, observed_ids: Vec<SessionId>) -> Self {
        self.observed_ids = observed_ids;
        self
    }

    /// The inventory the reader must report.
    #[must_use]
    pub fn with_init(mut self, init: InitReport) -> Self {
        self.init = Some(init);
        self
    }

    /// The class `exit` must be classified as.
    #[must_use]
    pub fn with_end(mut self, exit: ProcessExit, end: EndedBy) -> Self {
        self.end = Some((exit, end));
        self
    }

    /// The end the stream must have told.
    #[must_use]
    pub fn with_observed_end(mut self, end: ObservedEnd) -> Self {
        self.observed_end = Some(end);
        self
    }

    /// Whether the reader must recognise the bytes.
    #[must_use]
    pub const fn detects(mut self, detects: bool) -> Self {
        self.detects = detects;
        self
    }

    /// An export that classifies the run, read as `stream` by the driver's offline reader.
    #[must_use]
    pub fn with_export(
        mut self,
        stream: StreamName,
        bytes: Vec<u8>,
        sessions: Vec<ExpectedSession>,
    ) -> Self {
        self.export = Some((stream, bytes, sessions));
        self
    }

    /// What the driver must collect.
    #[must_use]
    pub fn with_collect(mut self, collect: Vec<CollectSpec>) -> Self {
        self.collect = Some(collect);
        self
    }

    /// The records for the session tracker.
    #[must_use]
    pub fn with_track(mut self, track: Vec<TrackStep>) -> Self {
        self.track = track;
        self
    }
}

/// What the suite builds the driver's commands from.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Setup<'a> {
    /// The cell the commands are built for.
    pub cell: &'a Cell<'a>,
    /// The workspace they are built in.
    pub workspace: &'a Workspace<'a>,
    /// Texts that must appear in no argument, no literal variable and no prepared file: the
    /// values of the secrets the core holds.
    pub canaries: &'a [String],
}

impl<'a> Setup<'a> {
    /// A setup.
    #[must_use]
    pub const fn new(
        cell: &'a Cell<'a>,
        workspace: &'a Workspace<'a>,
        canaries: &'a [String],
    ) -> Self {
        Self {
            cell,
            workspace,
            canaries,
        }
    }
}

/// A check the driver failed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Failure {
    /// The case the check ran on; `None` for a check of the driver itself.
    pub case: Option<String>,
    /// The check that failed.
    pub check: &'static str,
    /// What was found.
    pub detail: String,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.case {
            Some(case) => write!(f, "{} (case {case}): {}", self.check, self.detail),
            None => write!(f, "{}: {}", self.check, self.detail),
        }
    }
}

impl std::error::Error for Failure {}

/// A failure of a driver-level check.
fn driver_failure(check: &'static str, detail: String) -> Failure {
    Failure {
        case: None,
        check,
        detail,
    }
}

/// A failure of a check on one case.
fn case_failure(case: &Case, check: &'static str, detail: String) -> Failure {
    Failure {
        // BORROW: .clone() copies the case name into the failure
        case: Some(case.name.clone()),
        check,
        detail,
    }
}

/// Turns a driver's error into the failure of `check`.
fn driven<T>(check: &'static str, result: Result<T, DriverError>) -> Result<T, Failure> {
    result.map_err(|error| driver_failure(check, error.to_string())) // BORROW: .to_string() gives owned text
}

/// Whether `name` is a valid environment variable name.
fn is_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Whether `url` is an HTTP URL whose host is a loopback address.
fn is_loopback_http(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = if authority.starts_with('[') {
        authority
            .split(']')
            .next()
            .map_or("", |h| h.trim_start_matches('['))
    } else {
        authority.split(':').next().unwrap_or_default()
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// The names `{env:NAME}` references in `text`, in order.
fn env_references(text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("{env:") {
        let after = rest.get(start.saturating_add(5)..).unwrap_or_default();
        let Some((name, tail)) = after.split_once('}') else {
            break;
        };
        names.push(name);
        rest = tail;
    }
    names
}

/// Whether `inner` is `outer` or lies inside it, by path component.
fn within(inner: &str, outer: &str) -> bool {
    let mut outer_parts = outer.split('/');
    let mut inner_parts = inner.split('/');
    loop {
        match (outer_parts.next(), inner_parts.next()) {
            (None, _) => return true,
            (Some(a), Some(b)) if a == b => {}
            (Some(_), _) => return false,
        }
    }
}

/// Whether `text` runs `pgrep` or `pkill` with the full-command-line option: `-f` in any option
/// cluster (`-af`, `-9f`, `-fl`) or `--full`. On Linux such a match finds the script's own shell.
fn matches_full_command_line(text: &str) -> bool {
    text.split([';', '|', '&', '(', ')', '`', '\n'])
        .any(|segment| {
            let words: Vec<&str> = segment.split_whitespace().collect();
            let Some(at) = words
                .iter()
                .position(|word| matches!(word.trim_matches(['\'', '"']), "pgrep" | "pkill"))
            else {
                return false;
            };
            words.iter().skip(at.saturating_add(1)).any(|word| {
                word.trim_matches(['\'', '"']) == "--full"
                    || word
                        .trim_matches(['\'', '"'])
                        .strip_prefix('-')
                        .is_some_and(|cluster| !cluster.starts_with('-') && cluster.contains('f'))
            })
        })
}

/// Checks the invariants of a command: it has a program, carries no secret and sets no name the
/// core owns.
fn check_command(
    check: &'static str,
    spec: &CommandSpec,
    setup: &Setup<'_>,
    owned: &[String],
) -> Result<(), Failure> {
    let program_ok = spec
        .argv()
        .first()
        .is_some_and(|program| !program.is_empty());
    if !program_ok {
        return Err(driver_failure(
            check,
            "the command has no program".to_owned(),
        )); // BORROW: .to_owned() makes the owned text
    }
    for canary in setup.canaries {
        // BORROW: explicit .as_str() instead of Deref coercion
        if spec.argv().iter().any(|arg| arg.contains(canary.as_str())) {
            return Err(driver_failure(
                check,
                "a secret value is in the argument vector".to_owned(), // BORROW: .to_owned() makes the owned text
            ));
        }
    }
    for entry in spec.env_overlay() {
        match &entry.value {
            EnvValue::Literal(text) => {
                // BORROW: explicit .as_str() instead of Deref coercion
                if setup.canaries.iter().any(|c| text.contains(c.as_str())) {
                    return Err(driver_failure(
                        check,
                        format!("a secret value is in variable {}", entry.name),
                    ));
                }
            }
            EnvValue::Secret(_) => {
                return Err(driver_failure(
                    check,
                    format!("variable {} carries a secret into the agent", entry.name),
                ));
            }
        }
        if owned.contains(&entry.name) {
            return Err(driver_failure(
                check,
                format!("the command sets {}, which the core owns", entry.name),
            ));
        }
    }
    // A script that finds a process with `pgrep -f` matches its own `sh -c` on Linux.
    if matches_full_command_line(&spec.argv().join(" ")) {
        return Err(driver_failure(
            check,
            "the command matches a process with `pgrep -f`, which finds its own shell".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    if let Some(cwd) = spec.cwd() {
        // BORROW: explicit .as_str() instead of Deref coercion
        let text = cwd.as_str();
        if text.starts_with('/') || text.split('/').any(|part| part == "..") {
            return Err(driver_failure(
                check,
                format!("the working directory {text} leaves the workspace"),
            ));
        }
    }
    Ok(())
}

/// Checks an action of a stop or a close.
fn check_action(
    check: &'static str,
    action: &Action,
    setup: &Setup<'_>,
    owned: &[String],
) -> Result<(), Failure> {
    match action {
        Action::Command(spec) | Action::Exec(spec) => check_command(check, spec, setup, owned)?,
        Action::Http { method, url, .. } => {
            let _: &HttpMethod = method;
            // BORROW: explicit .as_str() instead of Deref coercion
            if !is_loopback_http(url.as_str()) {
                return Err(driver_failure(
                    check,
                    format!("the call {url} is not an HTTP call to a loopback address"),
                ));
            }
        }
    }
    if action.uses_session_id() {
        let sample: SessionId = SAMPLE_ID.parse().map_err(|_| {
            driver_failure(check, "the sample session id does not parse".to_owned()) // BORROW: .to_owned() makes the owned text
        })?;
        let unsafe_id: SessionId = UNSAFE_ID.parse().map_err(|_| {
            driver_failure(check, "the unsafe session id does not parse".to_owned()) // BORROW: .to_owned() makes the owned text
        })?;
        let Some(filled) = action.resolve(Some(&sample)) else {
            return Err(driver_failure(
                check,
                "the placeholder cannot be filled with a plain session id".to_owned(), // BORROW: .to_owned() makes the owned text
            ));
        };
        if filled.uses_session_id() {
            return Err(driver_failure(
                check,
                "the placeholder is still there after it is filled".to_owned(), // BORROW: .to_owned() makes the owned text
            ));
        }
        if action.resolve(Some(&unsafe_id)).is_some() {
            return Err(driver_failure(
                check,
                "a session id that could change a script is written into the action".to_owned(), // BORROW: .to_owned() makes the owned text
            ));
        }
        if action.resolve(None).is_some() {
            return Err(driver_failure(
                check,
                "the action resolves with no session id".to_owned(), // BORROW: .to_owned() makes the owned text
            ));
        }
    }
    Ok(())
}

/// Checks the stop spec.
fn check_stop(stop: &StopSpec, setup: &Setup<'_>, owned: &[String]) -> Result<(), Failure> {
    if stop.grace.is_zero() {
        return Err(driver_failure(
            "stop",
            "the grace period is zero".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    if let Some(action) = &stop.pre_stop {
        check_action("stop", action, setup, owned)?;
    }
    Ok(())
}

/// The names the core owns: the gateway's, and the state paths'.
fn owned_names(driver: &dyn HarnessDriver) -> Vec<String> {
    // TRAIT_OBJECT: the suite checks any driver
    let mut names = driver
        .gateway()
        .map(|gateway| gateway.reserved)
        .unwrap_or_default();
    names.extend(driver.state_paths().into_iter().map(|path| path.env));
    names
}

/// Checks the gateway names and the state paths.
fn check_names(driver: &dyn HarnessDriver) -> Result<(), Failure> {
    // TRAIT_OBJECT: the suite checks any driver
    let Some(gateway) = driver.gateway() else {
        return Err(driver_failure(
            "gateway",
            "the driver names no gateway variables".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    };
    for name in std::iter::once(&gateway.base_url)
        .chain(std::iter::once(&gateway.auth_token))
        .chain(gateway.reserved.iter())
    {
        // BORROW: explicit .as_str() instead of Deref coercion
        if !is_env_name(name.as_str()) {
            return Err(driver_failure(
                "gateway",
                format!("{name:?} is not a variable name"),
            ));
        }
    }
    if gateway.base_url == gateway.auth_token {
        return Err(driver_failure(
            "gateway",
            "the address and the token share one variable".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    for name in [&gateway.base_url, &gateway.auth_token] {
        if !gateway.reserved.contains(name) {
            return Err(driver_failure(
                "gateway",
                format!("{name} is set by the core but not reserved"),
            ));
        }
    }
    let mut seen: Vec<&String> = gateway.reserved.iter().collect();
    seen.sort();
    if seen.windows(2).any(|pair| pair.first() == pair.get(1)) {
        return Err(driver_failure(
            "gateway",
            "a reserved name is listed twice".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    let states = driver.state_paths();
    let mut envs: Vec<&String> = states.iter().map(|path| &path.env).collect();
    for env in &envs {
        // BORROW: explicit .as_str() instead of Deref coercion
        if !is_env_name(env.as_str()) {
            return Err(driver_failure(
                "state_paths",
                format!("{env:?} is not a variable name"),
            ));
        }
        if gateway.reserved.contains(env) {
            return Err(driver_failure(
                "state_paths",
                format!("{env} is also a gateway name"),
            ));
        }
    }
    envs.sort();
    if envs.windows(2).any(|pair| pair.first() == pair.get(1)) {
        return Err(driver_failure(
            "state_paths",
            "two state paths share a variable".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    for (index, first) in states.iter().enumerate() {
        for second in states.iter().skip(index.saturating_add(1)) {
            // BORROW: explicit .as_str() instead of Deref coercion
            let (a, b) = (first.subpath.as_str(), second.subpath.as_str());
            let file_is_parent = (first.kind == StateKind::File && within(b, a))
                || (second.kind == StateKind::File && within(a, b));
            if a == b || file_is_parent {
                return Err(driver_failure(
                    "state_paths",
                    format!(
                        "the state paths {a} and {b} are one path, or a file is the other's parent"
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Checks the capability rules and the session surface.
fn check_capabilities(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the suite checks any driver
    setup: &Setup<'_>,
    cases: &[Case],
    owned: &[String],
) -> Result<(), Failure> {
    let caps = driver.capabilities();
    let spec = driven("session", driver.session(setup.cell, setup.workspace))?;
    if caps.session && spec.is_none() {
        return Err(driver_failure(
            "session",
            "the session capability is declared and the driver returns no session spec".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    if caps.session && driver.session_tracker().is_none() {
        return Err(driver_failure(
            "session",
            "the session capability is declared and the driver returns no tracker".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    if !caps.session && spec.is_some() {
        return Err(driver_failure(
            "session",
            "the driver returns a session spec without the session capability".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    if let Some(spec) = &spec {
        check_command("session", &spec.command, setup, owned)?;
        for canary in setup.canaries {
            // BORROW: explicit .as_str() instead of Deref coercion
            if String::from_utf8_lossy(&spec.input).contains(canary.as_str()) {
                return Err(driver_failure(
                    "session",
                    "a secret value is in the session input".to_owned(), // BORROW: .to_owned() makes the owned text
                ));
            }
        }
        if let SessionClose::Action(action) = &spec.close {
            check_action("session", action, setup, owned)?;
        }
        if let Some(hooks) = &spec.hooks {
            let declared = declared_names(driver);
            for file in &hooks.files {
                if file.root != FileRoot::Home {
                    return Err(driver_failure(
                        "session",
                        format!("hook file {} is not rooted at the agent's home", file.path),
                    ));
                }
                check_file_references("session", file, &declared)?;
                let text = String::from_utf8_lossy(&file.contents);
                // BORROW: explicit .as_str() instead of Deref coercion
                if setup.canaries.iter().any(|c| text.contains(c.as_str())) {
                    return Err(driver_failure(
                        "session",
                        format!("a secret value is in hook file {}", file.path),
                    ));
                }
            }
        }
    }
    if caps.census {
        if let Some(stream) = driver.census_stream()
            && driver.offline_reader(&stream).is_none()
        {
            return Err(driver_failure(
                "census",
                "the census capability is declared and the census stream has no offline reader"
                    .to_owned(), // BORROW: .to_owned() makes the owned text
            ));
        }
        let from_stream = driver.census_stream().is_some();
        let from_init = cases.iter().any(|case| case.init.is_some());
        if !from_stream && !from_init {
            return Err(driver_failure(
                "census",
                "the census capability is declared and the driver has neither a census stream nor an init report".to_owned(), // BORROW: .to_owned() makes the owned text
            ));
        }
    } else if let Some(case) = cases.iter().find(|case| case.init.is_some()) {
        return Err(case_failure(
            case,
            "census",
            "the case expects an inventory and the census capability is off".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    if caps.execution_outside_child && driver.stop().pre_stop.is_none() {
        return Err(driver_failure(
            "stop",
            "execution lives outside the child and the stop has no pre-stop".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    Ok(())
}

/// The variable names a written configuration may refer to: the gateway's and the state paths'.
fn declared_names(driver: &dyn HarnessDriver) -> Vec<String> {
    // TRAIT_OBJECT: the suite checks any driver
    let mut names: Vec<String> = driver
        .gateway()
        .map(|gateway| {
            let mut all = gateway.reserved;
            all.push(gateway.base_url);
            all.push(gateway.auth_token);
            all
        })
        .unwrap_or_default();
    names.extend(driver.state_paths().into_iter().map(|path| path.env));
    names
}

/// Checks that a written file names only declared variables in its `{env:NAME}` references.
fn check_file_references(
    check: &'static str,
    file: &crate::spec::PreparedFile,
    declared: &[String],
) -> Result<(), Failure> {
    let text = String::from_utf8_lossy(&file.contents);
    for name in env_references(&text) {
        if !declared.iter().any(|known| known == name) {
            return Err(driver_failure(
                check,
                format!(
                    "file {} refers to {{env:{name}}}, which the driver does not declare",
                    file.path
                ),
            ));
        }
    }
    Ok(())
}

/// Checks the commands and files the driver builds from the cell.
fn check_builds(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the suite checks any driver
    setup: &Setup<'_>,
    owned: &[String],
) -> Result<(), Failure> {
    let command = driven("command", driver.command(setup.cell, setup.workspace))?;
    check_command("command", &command, setup, owned)?;
    check_command("version_probe", &driver.version_probe(), setup, owned)?;
    let prepared = driven("prepare", driver.prepare(setup.cell, setup.workspace))?;
    for entry in &prepared.env_overlay {
        if owned.contains(&entry.name) {
            return Err(driver_failure(
                "prepare",
                format!("prepare sets {}, which the core owns", entry.name),
            ));
        }
        match &entry.value {
            EnvValue::Literal(text) => {
                // BORROW: explicit .as_str() instead of Deref coercion
                if setup.canaries.iter().any(|c| text.contains(c.as_str())) {
                    return Err(driver_failure(
                        "prepare",
                        format!("a secret value is in variable {}", entry.name),
                    ));
                }
            }
            EnvValue::Secret(_) => {
                return Err(driver_failure(
                    "prepare",
                    format!("variable {} carries a secret into the agent", entry.name),
                ));
            }
        }
    }
    let declared = declared_names(driver);
    for file in &prepared.files {
        check_file_references("prepare", file, &declared)?;
        let text = String::from_utf8_lossy(&file.contents);
        // BORROW: explicit .as_str() instead of Deref coercion
        if setup.canaries.iter().any(|c| text.contains(c.as_str())) {
            return Err(driver_failure(
                "prepare",
                format!("a secret value is in file {}", file.path),
            ));
        }
    }
    check_stop(&driver.stop(), setup, owned)
}

/// A frame of `bytes` for a reader of the given framing, each with its place in `stream`.
fn frames(framing: Framing, stream: &StreamName, bytes: &[u8]) -> Vec<(RawRef, RawFrame)> {
    let data: Vec<&[u8]> = match framing {
        Framing::Lines => bytes
            .split_inclusive(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .collect(),
        Framing::Document => vec![bytes],
    };
    (0_u64..)
        .zip(data)
        .map(|(index, chunk)| {
            (
                // BORROW: .clone() gives each frame its own stream name
                RawRef::new(stream.clone(), index),
                RawFrame::new(None, FrameData::from_bytes(chunk)),
            )
        })
        .collect()
}

/// The reader of a case, or the failure that the driver has none.
fn reader_of(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the suite checks any driver
    case: &Case,
) -> Result<Box<dyn TraceReader>, Failure> {
    // TRAIT_OBJECT: the driver's reader is chosen at runtime
    let reader = match &case.reader {
        ReaderSource::Live => driver.live_reader(),
        ReaderSource::Offline(stream) => driver.offline_reader(stream),
    };
    reader.ok_or_else(|| {
        case_failure(
            case,
            "reader",
            "the driver has no reader for the case".to_owned(), // BORROW: .to_owned() makes the owned text
        )
    })
}

/// Checks the census source of the case's inventory against the driver.
fn check_census_source(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the suite checks any driver
    case: &Case,
) -> Result<(), Failure> {
    let Some(init) = &case.init else {
        return Ok(());
    };
    let stream = driver.census_stream();
    let offline_stream = match &case.reader {
        ReaderSource::Offline(name) => Some(name),
        ReaderSource::Live => None,
    };
    let from_stream = stream.is_some() && stream.as_ref() == offline_stream;
    let labelled_ok = if from_stream {
        matches!(init.source, CensusSource::Gateway | CensusSource::Collected)
    } else {
        init.source == CensusSource::Harness
    };
    if !labelled_ok {
        return Err(case_failure(
            case,
            "census",
            format!(
                "the inventory is labelled {:?} and the case's reader is a {} source",
                init.source,
                if from_stream {
                    "gateway or collected"
                } else {
                    "harness"
                }
            ),
        ));
    }
    if init.source == CensusSource::Harness && stream.is_some() && case.reader == ReaderSource::Live
    {
        return Err(case_failure(
            case,
            "census",
            "the driver names a census stream, so a live reader's inventory is not its census"
                .to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    Ok(())
}

/// Checks the sessions a reader finished with.
fn check_sessions(
    case: &Case,
    expected: &[ExpectedSession],
    sessions: &[Session],
) -> Result<(), Failure> {
    let got: Vec<(SessionId, Option<SessionId>, EndedBy)> = sessions
        .iter()
        .map(|s| {
            (
                // BORROW: .clone() copies the facts out of the session
                s.id().clone(),
                s.parent_session().cloned(),
                s.ended_by().clone(),
            )
        })
        .collect();
    let want: Vec<(SessionId, Option<SessionId>, EndedBy)> = expected
        .iter()
        // BORROW: .clone() copies the expectation
        .map(|s| (s.id.clone(), s.parent.clone(), s.ended_by.clone()))
        .collect();
    if got != want {
        return Err(case_failure(
            case,
            "sessions",
            format!("expected {want:?}, the reader finished with {got:?}"),
        ));
    }
    if sessions
        .first()
        .is_some_and(|root| root.parent_session().is_some())
    {
        return Err(case_failure(
            case,
            "sessions",
            "the first session is not a root".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    Ok(())
}

/// Reads the case's export with the driver's offline reader.
fn read_export(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the suite checks any driver
    case: &Case,
    stream: &StreamName,
    bytes: &[u8],
) -> Result<Vec<Session>, Failure> {
    let Some(mut reader) = driver.offline_reader(stream) else {
        return Err(case_failure(
            case,
            "export",
            "the driver has no offline reader for the export".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    };
    for (raw_ref, frame) in frames(reader.framing(), stream, bytes) {
        reader
            .feed(FrameIn::new(raw_ref, None, &frame))
            .map_err(|error| case_failure(case, "export", error.to_string()))?; // BORROW: .to_string() gives owned text
    }
    reader
        .finish()
        .map_err(|error| case_failure(case, "export", error.to_string())) // BORROW: .to_string() gives owned text
}

/// Checks what the driver collects: every command is sound, and a command of a harness that keeps
/// per-run state runs inside the isolation, where the run's state environment is.
fn check_collect(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the suite checks any driver
    setup: &Setup<'_>,
    observed: &Observed,
    owned: &[String],
) -> Result<(), Failure> {
    let has_state = !driver.state_paths().is_empty();
    for spec in driver.collect(setup.workspace, observed) {
        match &spec {
            CollectSpec::Command { spec, .. } => {
                check_command("collect", spec, setup, owned)?;
                if has_state {
                    return Err(driver_failure(
                        "collect",
                        "the driver keeps per-run state and collects with a host command, which has none of the run's state environment".to_owned(), // BORROW: .to_owned() makes the owned text
                    ));
                }
            }
            CollectSpec::Exec { spec, .. } => check_command("collect", spec, setup, owned)?,
            CollectSpec::File { .. } => {}
        }
    }
    Ok(())
}

/// Runs one case.
fn check_case(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the suite checks any driver
    setup: &Setup<'_>,
    case: &Case,
    owned: &[String],
) -> Result<(), Failure> {
    check_census_source(driver, case)?;
    let mut reader = reader_of(driver, case)?;
    let sample = case
        .bytes
        .get(..case.bytes.len().min(4096))
        .unwrap_or_default();
    if case.detects && reader.detect(sample).is_none() {
        return Err(case_failure(
            case,
            "detect",
            "the reader does not recognise the start of the recorded stream".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    for (raw_ref, frame) in frames(reader.framing(), &case.stream, &case.bytes) {
        let frame_in = FrameIn::new(raw_ref, None, &frame);
        reader
            .feed(frame_in)
            .map_err(|error| case_failure(case, "feed", error.to_string()))?; // BORROW: .to_string() gives owned text
    }
    let observed: Observed = reader.observed();
    if observed.session_ids != case.observed_ids {
        return Err(case_failure(
            case,
            "observed",
            format!(
                "expected ids {:?}, the reader observed {:?}",
                case.observed_ids, observed.session_ids
            ),
        ));
    }
    if observed.init != case.init {
        return Err(case_failure(
            case,
            "census",
            format!(
                "expected inventory {:?}, the reader observed {:?}",
                case.init, observed.init
            ),
        ));
    }
    if observed.end != case.observed_end {
        return Err(case_failure(
            case,
            "observed",
            format!(
                "expected end {:?}, the reader observed {:?}",
                case.observed_end, observed.end
            ),
        ));
    }
    if let Some(want) = &case.collect {
        let got = driver.collect(setup.workspace, &observed);
        if &got != want {
            return Err(case_failure(
                case,
                "collect",
                format!("expected {want:?}, the driver collects {got:?}"),
            ));
        }
    }
    check_collect(driver, setup, &observed, owned)?;
    let mut sessions = match &case.sessions {
        Some(expected) => {
            let sessions = reader
                .finish()
                .map_err(|error| case_failure(case, "finish", error.to_string()))?; // BORROW: .to_string() gives owned text
            check_sessions(case, expected, &sessions)?;
            sessions
        }
        None => Vec::new(),
    };
    if let Some((stream, bytes, expected)) = &case.export {
        let exported = read_export(driver, case, stream, bytes)?;
        check_sessions(case, expected, &exported)?;
        sessions = exported;
    }
    if let Some((exit, end)) = &case.end {
        let class = driver.classify_end(exit, &sessions, &observed);
        if &class != end {
            return Err(case_failure(
                case,
                "classify_end",
                format!("expected {end:?}, the driver classifies {class:?}"),
            ));
        }
    }
    check_track(driver, case)
}

/// Feeds the case's records to a fresh session tracker.
fn check_track(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the suite checks any driver
    case: &Case,
) -> Result<(), Failure> {
    if case.track.is_empty() {
        return Ok(());
    }
    let Some(mut tracker) = driver.session_tracker() else {
        return Err(case_failure(
            case,
            "tracker",
            "the case has records for a tracker and the driver has none".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    };
    for (index, step) in case.track.iter().enumerate() {
        let record = match &step.record {
            // BORROW: explicit .as_slice() instead of Deref coercion
            RecordBytes::Stream(bytes) => SessionRecord::Stream(bytes.as_slice()),
            // BORROW: explicit .as_slice() instead of Deref coercion
            RecordBytes::Hook(bytes) => SessionRecord::Hook(bytes.as_slice()),
        };
        tracker
            .feed(record)
            .map_err(|error| case_failure(case, "tracker", format!("record {index}: {error}")))?;
        if let Some(want) = &step.state {
            let got = tracker.state();
            if &got != want {
                return Err(case_failure(
                    case,
                    "tracker",
                    format!("after record {index} expected {want:?}, the tracker reports {got:?}"),
                ));
            }
        }
    }
    Ok(())
}

/// Runs the suite on `driver` and its recorded `cases`.
///
/// # Errors
/// Returns the first [`Failure`]: a capability that the driver's specs do not back, a gateway or
/// state name that is invalid, a command that carries a secret or sets a name the core owns, a
/// stop that cannot work, or a case whose reader, inventory, sessions, end or tracked state is
/// not what the case records.
pub fn run(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the suite checks any driver
    setup: &Setup<'_>,
    cases: &[Case],
) -> Result<(), Failure> {
    let owned = owned_names(driver);
    check_names(driver)?;
    check_capabilities(driver, setup, cases, &owned)?;
    check_builds(driver, setup, &owned)?;
    for case in cases {
        check_case(driver, setup, case, &owned)?;
    }
    Ok(())
}
