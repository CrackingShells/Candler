//! One evaluator process, one request: spawn, send, collect, end, reap.
//!
//! The exchange keeps stdin open after the request line until the answer arrives, because a
//! `cancel` notification travels on it (R01 §Evaluator protocol, Lifetime). It reads stdout as
//! frames, drains stderr into a bounded tail, and ends every process through the child's stop
//! handle: the only stop path.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use candler_schemas::evaluation::rpc::{Message, ResponseResult, RpcNotification, RpcRequest};
use candler_schemas::evaluation::{CancelParams, ERROR_REQUEST_CANCELLED, Notification, Request};
use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::RequestId;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::process::{ChildStderr, ChildStdin};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use super::deadline::{Clock, sleep_for};
use super::framing::{Frame, FrameReader};
use super::{ClientError, EvaluatorCommand};
use crate::supervise::Supervisor;
use crate::supervise::stop::{Exit, StopHandle};

/// How many bytes of the end of stderr are kept as evidence.
const STDERR_KEEP: usize = 16 * 1024;

/// How long the stderr collector gets to finish once the process is gone.
const STDERR_JOIN: Duration = Duration::from_secs(1);

/// The timing a session runs on.
#[derive(Clone)]
pub(crate) struct Settings {
    // TRAIT_OBJECT: the clock is injected so that tests can drive deadlines
    pub(crate) clock: Arc<dyn Clock>,
    /// How long a process gets after `cancel` and the close of stdin before it is stopped.
    pub(crate) cancel_grace: Duration,
    /// The grace of the supervisor's stop ladder.
    pub(crate) stop_grace: Duration,
}

/// How the exchange ended.
#[derive(Debug)]
pub(crate) enum Ending {
    /// The evaluator answered with a result.
    Answered(ResponseResult),
    /// The evaluator answered with an error that is not a cancellation, or broke the exchange.
    Fault(String),
    /// The evaluator answered `-32800` although the client had not cancelled.
    CancelledByEvaluator(String),
    /// The evaluator closed its output without answering.
    Eof,
    /// The request deadline fired first.
    Deadline,
    /// The caller cancelled.
    Cancelled,
}

/// What an exchange collected.
#[derive(Debug)]
pub(crate) struct Exchange {
    pub(crate) ending: Ending,
    /// The outcome records received before the exchange ended, in arrival order.
    pub(crate) outcomes: Vec<OutcomeRecord>,
    /// Faults of the evaluator that did not end the exchange, in the order they happened.
    pub(crate) notes: Vec<String>,
    /// The end of the evaluator's stderr.
    pub(crate) stderr: String,
    /// Why the process could not be ended, when it could not.
    pub(crate) stop_fault: Option<String>,
}

/// Sends lines to the evaluator's stdin from a task of its own, so that a full pipe never blocks
/// the reader of stdout.
struct StdinWriter {
    lines: Option<mpsc::UnboundedSender<Vec<u8>>>,
    task: JoinHandle<()>,
}

impl StdinWriter {
    fn start(mut stdin: ChildStdin) -> Self {
        let (lines, mut queue) = mpsc::unbounded_channel::<Vec<u8>>();
        let task = tokio::spawn(async move {
            while let Some(bytes) = queue.recv().await {
                if stdin.write_all(&bytes).await.is_err() || stdin.flush().await.is_err() {
                    break;
                }
            }
            // A child that closed its input early makes this fail, which is its right.
            let _ = stdin.shutdown().await;
        });
        Self {
            lines: Some(lines),
            task,
        }
    }

    /// Queues one line.
    fn send(&self, mut line: String) {
        line.push('\n');
        if let Some(lines) = &self.lines {
            // A closed queue means the writer ended, and the child no longer reads.
            let _ = lines.send(line.into_bytes());
        }
    }

    /// Closes stdin once the queued lines are written.
    fn close(&mut self) {
        self.lines = None;
    }
}

impl Drop for StdinWriter {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Reads `stderr` to its end, keeping the last [`STDERR_KEEP`] bytes.
async fn collect_tail(mut stderr: ChildStderr, kept: Arc<Mutex<VecDeque<u8>>>) {
    let mut chunk = vec![0_u8; 8 * 1024];
    while let Ok(count) = stderr.read(&mut chunk).await {
        let Some(bytes) = chunk.get(..count).filter(|bytes| !bytes.is_empty()) else {
            break;
        };
        let mut held = kept.lock().unwrap_or_else(PoisonError::into_inner);
        held.extend(bytes.iter().copied());
        let over = held.len().saturating_sub(STDERR_KEEP);
        held.drain(..over);
    }
}

/// Completes when the caller cancels; never completes for a caller that dropped its sender.
async fn cancelled(cancel: &mut Option<watch::Receiver<bool>>) {
    let waited = match cancel.as_mut() {
        Some(receiver) => receiver.wait_for(|asked| *asked).await.is_ok(),
        None => false,
    };
    if waited {
        return;
    }
    // A watch whose sender was dropped is "not cancelled", and stays so.
    *cancel = None;
    std::future::pending::<()>().await;
}

/// The line of a request.
fn request_line(id: &RequestId, call: Request) -> Result<String, ClientError> {
    // BORROW: .clone() gives the request its own id
    serde_json::to_string(&RpcRequest::new(id.clone(), call)).map_err(ClientError::Encode)
}

/// The line of a `cancel` notification for `id`.
fn cancel_line(id: &RequestId) -> Option<String> {
    // BORROW: .clone() gives the notification its own id
    let cancel = Notification::Cancel(CancelParams::new(id.clone()));
    serde_json::to_string(&RpcNotification::new(cancel)).ok()
}

/// Takes one frame into the collected state.
fn take_frame(
    frame: Frame,
    id: &RequestId,
    outcomes: &mut Vec<OutcomeRecord>,
    notes: &mut Vec<String>,
    non_json: &mut u64,
) -> Option<Ending> {
    match frame {
        Frame::Message(message) => take_message(*message, id, outcomes, notes),
        Frame::NotJson => {
            *non_json += 1;
            None
        }
        Frame::Malformed(detail) => {
            notes.push(format!("a line is not a protocol message ({detail})"));
            None
        }
        Frame::Oversize => {
            notes.push("a line was longer than the frame limit and was dropped".to_owned());
            None
        }
        Frame::Partial => {
            notes.push("a partial frame was cut short by the end of the output".to_owned());
            None
        }
        Frame::Eof => Some(Ending::Eof),
        Frame::Broken(detail) => {
            notes.push(detail);
            Some(Ending::Eof)
        }
    }
}

/// Takes one protocol message into the collected state.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "the wildcard of the notification match also covers the known progress and log variants, which are ignored; listing them beside it would trip match_same_arms"
)]
fn take_message(
    message: Message,
    id: &RequestId,
    outcomes: &mut Vec<OutcomeRecord>,
    notes: &mut Vec<String>,
) -> Option<Ending> {
    match message {
        Message::Notification(note) => match note.notification {
            Notification::Outcome(params) => {
                // BORROW: .clone() keeps the record
                outcomes.push(params.record().clone());
            }
            Notification::Cancel(_) => {
                notes.push("the evaluator sent a cancel notification".to_owned());
            }
            _ => {}
        },
        Message::Response(response) => {
            if response.id == *id {
                return Some(Ending::Answered(response.result));
            }
            notes.push(format!(
                "a response answers another request ({})",
                response.id
            ));
        }
        Message::Error(response) => {
            if response.id != *id {
                notes.push(format!(
                    "an error answers another request ({})",
                    response.id
                ));
            } else if response.error.code == i64::from(ERROR_REQUEST_CANCELLED) {
                return Some(Ending::CancelledByEvaluator(response.error.message));
            } else {
                return Some(Ending::Fault(format!(
                    "the evaluator answered error {}: {}",
                    response.error.code, response.error.message
                )));
            }
        }
        Message::Request(_) => notes.push("the evaluator sent a request".to_owned()),
        _ => notes.push("the evaluator sent a message of an unknown shape".to_owned()),
    }
    None
}

/// Reads and drops frames until the stream ends.
async fn discard<R: tokio::io::AsyncRead + Unpin>(frames: &mut FrameReader<R>) {
    while !matches!(frames.next().await, Frame::Eof | Frame::Broken(_)) {}
}

/// Lets the process finish for `grace`, discarding what it still writes, then stops it through
/// its handle. Returns why it could not be ended, when it could not.
async fn settle<R: tokio::io::AsyncRead + Unpin>(
    frames: &mut FrameReader<R>,
    waiter: &mut JoinHandle<Result<Exit, crate::supervise::SuperviseError>>,
    stop: &StopHandle,
    clock: &dyn Clock,
    grace: Duration,
    notes: &mut Vec<String>,
) -> Option<String> {
    let finished = tokio::select! {
        done = async {
            discard(frames).await;
            (&mut *waiter).await
        } => Some(done),
        () = sleep_for(clock, grace) => None,
    };
    let joined = if let Some(done) = finished {
        done
    } else {
        notes.push("the evaluator did not exit in the grace and was stopped".to_owned());
        stop.request();
        waiter.await
    };
    match joined {
        Ok(Ok(_)) => None,
        Ok(Err(error)) => Some(error.to_string()),
        Err(error) => Some(format!("the wait for the evaluator failed: {error}")),
    }
}

/// Runs one request in one evaluator process and returns what it collected.
///
/// # Errors
/// Returns [`ClientError::Spawn`] if the process cannot start, [`ClientError::Pipes`] if it has no
/// stdio pipes and [`ClientError::Encode`] if the request cannot be written.
pub(crate) async fn exchange(
    supervisor: &Supervisor,
    command: &EvaluatorCommand,
    settings: &Settings,
    id: &RequestId,
    call: Request,
    deadline: Option<Duration>,
    mut cancel: Option<watch::Receiver<bool>>,
) -> Result<Exchange, ClientError> {
    let line = request_line(id, call)?;
    let clock: &dyn Clock = &*settings.clock;
    let mut child = supervisor.spawn(command.spawn_request(settings.stop_grace)?)?;
    let stop = child.stop_handle();
    let (Some(stdin), Some(stdout), Some(stderr)) =
        (child.take_stdin(), child.take_stdout(), child.take_stderr())
    else {
        stop.request();
        let _ = child.wait().await;
        return Err(ClientError::Pipes);
    };
    let mut waiter = tokio::spawn(child.wait());
    let kept = Arc::new(Mutex::new(VecDeque::new()));
    // BORROW: .clone() shares the tail with the collector
    let mut collector = tokio::spawn(collect_tail(stderr, Arc::clone(&kept)));
    let mut writer = StdinWriter::start(stdin);
    writer.send(line);

    let limit_at = deadline.map(|limit| clock.now() + limit);
    let mut sleeper = Box::pin(async {
        match limit_at {
            Some(at) => clock.sleep_until(at).await,
            None => std::future::pending::<()>().await,
        }
    });
    let mut frames = FrameReader::new(stdout);
    let mut outcomes = Vec::new();
    let mut notes = Vec::new();
    let mut non_json = 0_u64;
    let ending = loop {
        tokio::select! {
            biased;
            () = cancelled(&mut cancel) => break Ending::Cancelled,
            () = &mut sleeper => break Ending::Deadline,
            frame = frames.next() => {
                if let Some(ending) = take_frame(frame, id, &mut outcomes, &mut notes, &mut non_json) {
                    break ending;
                }
            }
        }
    };
    // Whatever arrives after the end is discarded, so the outcomes are final here.
    if matches!(ending, Ending::Deadline | Ending::Cancelled)
        && let Some(line) = cancel_line(id)
    {
        writer.send(line);
    }
    writer.close();
    let stop_fault = settle(
        &mut frames,
        &mut waiter,
        &stop,
        clock,
        settings.cancel_grace,
        &mut notes,
    )
    .await;
    if tokio::time::timeout(STDERR_JOIN, &mut collector)
        .await
        .is_err()
    {
        collector.abort();
    }
    if non_json > 0 {
        notes.push(format!("{non_json} output line(s) were not JSON"));
    }
    let tail: Vec<u8> = kept
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .copied()
        .collect();
    Ok(Exchange {
        ending,
        outcomes,
        notes,
        stderr: String::from_utf8_lossy(&tail).into_owned(),
        stop_fault,
    })
}
