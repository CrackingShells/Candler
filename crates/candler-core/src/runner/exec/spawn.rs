//! Starting one attempt of a run in its container, with a reader that watches the output live.

use std::sync::{Arc, Mutex, PoisonError};

use candler_harness::reader::{FrameIn, ReaderError, TraceReader};
use candler_schemas::raw::{RawFrame, RawRef, StreamName};
use candler_schemas::trace::{Session, SessionEvent};

use crate::isolation::{EndFacts, IsolatedRequest, Isolation, IsolationError};
use crate::runner::Runtime;
use crate::supervise::SuperviseError;
use crate::supervise::spawn::{Child, FrameObserver};

/// What the live reader holds while the run writes.
struct Live {
    /// The reader, until the handle finishes it.
    // TRAIT_OBJECT: the reader is the harness driver's
    reader: Option<Box<dyn TraceReader>>,
    /// The events the reader returned so far, in order.
    events: Vec<SessionEvent>,
    /// The first frame error, which stops the reading.
    error: Option<ReaderError>,
}

/// The observer that feeds each flushed frame of the run's stream to the live reader.
pub struct LiveObserver {
    /// The stream the frames belong to.
    stream: StreamName,
    /// The state shared with the handle.
    shared: Arc<Mutex<Live>>,
}

/// The sessions and events a live reader produced.
#[derive(Debug)]
#[non_exhaustive]
pub struct LiveTrace {
    /// The sessions of the stream, the root first.
    pub sessions: Vec<Session>,
    /// The events of the stream, in the order the frames came.
    pub events: Vec<SessionEvent>,
}

/// The caller's end of a [`LiveObserver`]: it takes the result once the run has ended.
pub struct LiveHandle {
    /// The state shared with the observer.
    shared: Arc<Mutex<Live>>,
}

/// A live observer for `reader` on `stream`, and the handle that finishes it.
#[must_use]
// TRAIT_OBJECT: the reader is the harness driver's
pub fn live_observer(
    reader: Box<dyn TraceReader>,
    stream: StreamName,
) -> (LiveObserver, LiveHandle) {
    let shared = Arc::new(Mutex::new(Live {
        reader: Some(reader),
        events: Vec::new(),
        error: None,
    }));
    (
        LiveObserver {
            stream,
            shared: Arc::clone(&shared),
        },
        LiveHandle { shared },
    )
}

impl FrameObserver for LiveObserver {
    fn frame(&mut self, index: u64, frame: &RawFrame) {
        let mut live = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        if live.error.is_some() {
            return;
        }
        let Some(reader) = live.reader.as_mut() else {
            return;
        };
        let raw_ref = RawRef::new(self.stream.clone(), index); // BORROW: the reference owns its stream name
        match reader.feed(FrameIn::new(raw_ref, None, frame)) {
            Ok(mut events) => live.events.append(&mut events),
            Err(error) => live.error = Some(error),
        }
    }
}

impl LiveHandle {
    /// Ends the stream and returns what the reader made of it.
    ///
    /// # Errors
    /// Returns the first frame error, or the reader's own error when it cannot finish.
    pub fn finish(self) -> Result<LiveTrace, ReaderError> {
        let mut live = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(error) = live.error.take() {
            return Err(error);
        }
        let Some(reader) = live.reader.take() else {
            return Err(ReaderError::Incomplete {
                detail: "the live reader was already finished".to_owned(), // BORROW: owned detail
            });
        };
        let events = std::mem::take(&mut live.events);
        let sessions = reader.finish()?;
        Ok(LiveTrace { sessions, events })
    }
}

/// A run's child and the facts its end will leave behind.
#[non_exhaustive]
pub struct Started {
    /// The supervised child.
    pub child: Child,
    // TRAIT_OBJECT: the facts come from the hook the request carries
    /// What the run's end leaves behind, read after the wait.
    pub end_facts: Arc<dyn EndFacts>,
}

/// The isolation of the runtime.
///
/// # Errors
/// Returns [`IsolationError::Unavailable`] when the runtime has none: a run never falls back to
/// the host.
// TRAIT_OBJECT: the runtime holds its isolation as a trait object
pub fn require_isolation(rt: &Runtime) -> Result<&Arc<dyn Isolation>, IsolationError> {
    rt.isolation
        .as_ref() // BORROW: lend the isolation without taking it out of the runtime
        .ok_or_else(|| IsolationError::Unavailable {
            detail: "the runtime has no isolation".to_owned(), // BORROW: owned detail for the error
        })
}

/// Spawns the request of one attempt. The stop hook is the one the request carries.
///
/// # Errors
/// Returns the supervisor's error when the program cannot start.
pub fn start_attempt(rt: &Runtime, isolated: IsolatedRequest) -> Result<Started, SuperviseError> {
    let IsolatedRequest { request, end_facts } = isolated;
    let child = rt.supervisor.spawn(request)?;
    Ok(Started { child, end_facts })
}
