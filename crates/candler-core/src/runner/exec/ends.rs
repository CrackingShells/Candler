//! How a run ended: the pure classification of its facts, in a fixed order of precedence.

use candler_schemas::execution::InvalidReason;
use candler_schemas::trace::{Cap, EndedBy};

use crate::isolation::model::Inspect;
use crate::jobs::model::JobCause;

/// The exit status the docker client reports when the daemon itself failed.
const DAEMON_FAILURE: i32 = 125;
/// The exit status of a process the kernel killed.
const KILLED: i64 = 137;
/// The exit status of a run that the container's own `timeout` expired.
const SELF_EXPIRY: i64 = 124;
/// The same status as the docker client reports it.
const SELF_EXPIRY_CLIENT: i32 = 124;

/// What the reader made of the run's output.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TraceFacts {
    /// The run wrote nothing a reader could use.
    Silent,
    /// The format of the output is not one a reader knows.
    FormatUnknown,
    /// The reader failed on the output.
    ReaderError,
    /// The reader finished: how the session ended and the tokens it used.
    Read {
        /// The end the reader attributes to the session.
        ended_by: EndedBy,
        /// The tokens the session used, in all.
        tokens: u64,
    },
}

/// Everything the classification reads about a run. Build it with the `with_*` setters.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool is one fact of the table"
)]
#[derive(Debug, Clone)]
pub struct EndInput {
    /// The exit status of the docker client.
    client_exit: Option<i32>,
    /// The final inspect of the container, when there is one.
    inspect: Option<Inspect>,
    /// Whether the container started.
    container_started: bool,
    /// The supervisor's own act, when there was one.
    stop: Option<JobCause>,
    /// Whether the supervisor's wall deadline fired.
    deadline: bool,
    /// Whether the gateway reported a failure for the run.
    gateway_failed: bool,
    /// What the reader made of the output.
    trace: TraceFacts,
    /// Whether the collection of the run's files succeeded.
    collect_ok: bool,
    /// Whether a secret or ground truth reached the run.
    input_leak: bool,
    /// The attempt number, from 1.
    attempt: u32,
    /// Whether the session runner itself stopped the harness: past the grace after it closed the
    /// input, or at a cap of the cell.
    session_stop: bool,
    /// The cap the session runner ended the run at, when it did.
    session_cap: Option<Cap>,
}

impl EndInput {
    /// A neutral input: attempt 1, started, no stop, no deadline, no inspect, a silent trace.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            client_exit: Some(0),
            inspect: None,
            container_started: true,
            stop: None,
            deadline: false,
            gateway_failed: false,
            trace: TraceFacts::Silent,
            collect_ok: true,
            input_leak: false,
            attempt: 1,
            session_stop: false,
            session_cap: None,
        }
    }

    /// Sets the exit status of the docker client.
    #[must_use]
    pub const fn with_client_exit(mut self, exit: Option<i32>) -> Self {
        self.client_exit = exit;
        self
    }

    /// Sets the final inspect of the container.
    #[must_use]
    pub fn with_inspect(mut self, inspect: Option<Inspect>) -> Self {
        self.inspect = inspect;
        self
    }

    /// Sets whether the container started.
    #[must_use]
    pub const fn with_container_started(mut self, started: bool) -> Self {
        self.container_started = started;
        self
    }

    /// Sets the supervisor's own act.
    #[must_use]
    pub const fn with_stop(mut self, stop: Option<JobCause>) -> Self {
        self.stop = stop;
        self
    }

    /// Sets whether the supervisor's wall deadline fired.
    #[must_use]
    pub const fn with_deadline(mut self, deadline: bool) -> Self {
        self.deadline = deadline;
        self
    }

    /// Sets whether the gateway reported a failure for the run.
    #[must_use]
    pub const fn with_gateway_failed(mut self, failed: bool) -> Self {
        self.gateway_failed = failed;
        self
    }

    /// Sets what the reader made of the output.
    #[must_use]
    pub fn with_trace(mut self, trace: TraceFacts) -> Self {
        self.trace = trace;
        self
    }

    /// Sets whether the collection of the run's files succeeded.
    #[must_use]
    pub const fn with_collect_ok(mut self, ok: bool) -> Self {
        self.collect_ok = ok;
        self
    }

    /// Sets whether a secret or ground truth reached the run.
    #[must_use]
    pub const fn with_input_leak(mut self, leak: bool) -> Self {
        self.input_leak = leak;
        self
    }

    /// Sets the attempt number, from 1.
    #[must_use]
    pub const fn with_attempt(mut self, attempt: u32) -> Self {
        self.attempt = attempt;
        self
    }

    /// Sets whether the session runner stopped the harness itself. That is the arm's own end, not
    /// the experimenter's: the run is judged as one the harness ended.
    #[must_use]
    pub const fn with_session_stop(mut self, stopped: bool) -> Self {
        self.session_stop = stopped;
        self
    }

    /// Sets the cap the session runner ended the run at. It implies a stop by the runner.
    #[must_use]
    pub const fn with_session_cap(mut self, cap: Option<Cap>) -> Self {
        self.session_cap = cap;
        self
    }
}

impl Default for EndInput {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether the run is tried again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Retry {
    /// A gateway failure of the first attempt: try once more as a new attempt.
    Once,
    /// A failure that is never tried again.
    Never,
    /// Nothing to try again.
    None,
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct End {
    /// The reason the run is invalid, or `None` when it counts.
    pub invalid: Option<InvalidReason>,
    /// How the session ended.
    pub ended_by: EndedBy,
    /// Whether the run is tried again.
    pub retry: Retry,
}

impl End {
    /// A run that counts.
    fn valid(ended_by: EndedBy, retry: Retry) -> Self {
        Self {
            invalid: None,
            ended_by,
            retry,
        }
    }

    /// A run that does not count, for `reason`.
    fn invalid(reason: InvalidReason, ended_by: EndedBy, retry: Retry) -> Self {
        Self {
            invalid: Some(reason),
            ended_by,
            retry,
        }
    }
}

/// How the end of a gateway failure is retried: once, on the first attempt only.
const fn gateway_retry(attempt: u32) -> Retry {
    if attempt <= 1 {
        Retry::Once
    } else {
        Retry::Never
    }
}

/// The end of a run that is still valid, from its wall expiry and its trace.
#[expect(
    clippy::match_same_arms,
    reason = "EndedBy is non_exhaustive in another crate, so the wildcard must stay beside the listed variants, and both give Unknown"
)]
fn qualify(input: &EndInput) -> EndedBy {
    let expired = input.deadline
        || input.client_exit == Some(SELF_EXPIRY_CLIENT)
        || input
            .inspect
            .as_ref() // BORROW: read the exit code without taking the inspect
            .is_some_and(|inspect| inspect.state.exit_code == SELF_EXPIRY);
    if expired {
        return EndedBy::cap(Cap::Wall);
    }
    if let Some(cap) = input.session_cap {
        return EndedBy::cap(cap);
    }
    match &input.trace {
        TraceFacts::Read { ended_by, .. } => match ended_by {
            EndedBy::Completed | EndedBy::AgentError { .. } | EndedBy::Cap { .. } => {
                ended_by.clone() // BORROW: owned end of the session
            }
            EndedBy::GatewayError
            | EndedBy::BudgetStop
            | EndedBy::Killed
            | EndedBy::Interrupted
            | EndedBy::Unknown => EndedBy::Unknown,
            _ => EndedBy::Unknown,
        },
        TraceFacts::Silent | TraceFacts::FormatUnknown | TraceFacts::ReaderError => {
            EndedBy::Unknown
        }
    }
}

/// Classifies the end of a run from its facts. Pure; the first matching rule wins.
///
/// 1. our own stop; 2. the daemon's facts; 3. out of memory; 4. a kill by the kernel; 5. a gateway
///    signal; 6. an unreadable output; 7. a trace that blames the gateway; 8. a failed collection;
///    9. an input leak; 10. how the session ended. A stop by the session runner (a cap of the
///    cell, or the grace past a closed input) is the arm's own end: it counts as ours for rules
///    2 to 4 and ends the run at the cap it names.
#[must_use]
pub fn classify_end(input: &EndInput) -> End {
    let ours =
        input.stop.is_some() || input.deadline || input.session_stop || input.session_cap.is_some();
    match input.stop {
        Some(JobCause::BudgetStop) => {
            return End::invalid(InvalidReason::BudgetStop, EndedBy::BudgetStop, Retry::None);
        }
        Some(JobCause::Killed) => {
            return End::invalid(InvalidReason::Killed, EndedBy::Killed, Retry::None);
        }
        None => {}
    }
    let infra = |retry| End::invalid(InvalidReason::Infrastructure, EndedBy::Unknown, retry);
    if !ours
        && (input.client_exit == Some(DAEMON_FAILURE)
            || !input.container_started
            || input.inspect.is_none())
    {
        return infra(Retry::None);
    }
    if input
        .inspect
        .as_ref() // BORROW: read the state without taking the inspect
        .is_some_and(|inspect| inspect.state.oom_killed)
    {
        return infra(Retry::Never);
    }
    if !ours
        && input
            .inspect
            .as_ref() // BORROW: read the state without taking the inspect
            .is_some_and(|inspect| inspect.state.exit_code == KILLED)
    {
        return infra(Retry::None);
    }
    if input.gateway_failed {
        return End::invalid(
            InvalidReason::Gateway,
            EndedBy::GatewayError,
            gateway_retry(input.attempt),
        );
    }
    match input.trace {
        TraceFacts::FormatUnknown => {
            return End::invalid(InvalidReason::FormatUnknown, EndedBy::Unknown, Retry::None);
        }
        TraceFacts::ReaderError => {
            return End::invalid(InvalidReason::ReaderError, EndedBy::Unknown, Retry::None);
        }
        TraceFacts::Read { .. } | TraceFacts::Silent => {}
    }
    if let TraceFacts::Read { ended_by, tokens } = &input.trace
        && (*tokens == 0 || *ended_by == EndedBy::GatewayError)
    {
        return End::invalid(
            InvalidReason::Gateway,
            EndedBy::GatewayError,
            gateway_retry(input.attempt),
        );
    }
    if !input.collect_ok {
        return End::invalid(InvalidReason::CollectionError, qualify(input), Retry::None);
    }
    if input.input_leak {
        return End::invalid(InvalidReason::InputLeak, qualify(input), Retry::None);
    }
    let ended_by = qualify(input);
    if ended_by == EndedBy::Unknown && !ours {
        return infra(Retry::None);
    }
    let retry = if matches!(ended_by, EndedBy::AgentError { .. }) {
        Retry::Never
    } else {
        Retry::None
    };
    End::valid(ended_by, retry)
}
