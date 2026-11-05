//! The sessions of a stream: roots, subagent children, the numbering of their events, and how each ended.

use std::collections::{HashMap, HashSet};

use candler_harness::reader::ReaderError;
use candler_schemas::primitives::{ReasonCode, Timestamp};
use candler_schemas::raw::RawRef;
use candler_schemas::trace::{
    Cap, EndedBy, Event, EventKind, Session, SessionEvent, SessionId, SessionSource, SpanId,
};
use serde_json::{Map, Value};

use crate::parse::Draft;
use crate::usage::Tokens;

/// How a root session ended, from its last `result` line.
///
/// A budget or turn cap is a cap; a non-null `api_error_status` is a gateway error; `success`
/// without `is_error` is completed; any other `is_error: true` result is an agent error, with the
/// `terminal_reason` (or, when there is none, a subtype other than `success`) as its reason when
/// that is a reason code. A refusal by the model is an agent error, not a gateway error.
pub(crate) fn ended_by_of(line: &Map<String, Value>) -> EndedBy {
    let text_of = |key: &str| line.get(key).and_then(Value::as_str);
    let subtype = text_of("subtype");
    let is_error = line.get("is_error").and_then(Value::as_bool);
    let api_error = line
        .get("api_error_status")
        .is_some_and(|status| !status.is_null());
    if subtype == Some("error_max_turns") {
        EndedBy::cap(Cap::Turns)
    } else if subtype == Some("error_max_budget_usd") {
        EndedBy::cap(Cap::Cost)
    } else if api_error {
        EndedBy::GatewayError
    } else if subtype == Some("success") && is_error != Some(true) {
        EndedBy::Completed
    } else if is_error == Some(true) {
        let reason = text_of("terminal_reason").or(subtype.filter(|kind| *kind != "success"));
        EndedBy::agent_error(reason.and_then(|code| code.parse::<ReasonCode>().ok()))
    } else {
        EndedBy::Unknown
    }
}

/// One session of the stream.
struct Node {
    /// The id of the session.
    id: SessionId,
    /// The id of the parent session, for a subagent session.
    parent: Option<SessionId>,
    /// The index of the root session this session belongs to.
    root: usize,
    /// The id of the call that started the session, for a subagent session.
    span: Option<String>,
    /// The `seq` of the next event.
    next_seq: u64,
    /// The first frame of the session.
    raw_ref: RawRef,
    /// The model the session ran.
    model: Option<String>,
    /// The earliest time of an event of the session.
    started_at: Option<Timestamp>,
    /// The latest time of an event of the session.
    ended_at: Option<Timestamp>,
    /// The tokens of the `result` lines so far, summed.
    running: Tokens,
    /// How the latest `result` line ends the session.
    ended_by: Option<EndedBy>,
}

/// The sessions of one stream and the maps that attribute lines to them.
#[derive(Default)]
pub(crate) struct Registry {
    /// The sessions, in the order they appeared.
    nodes: Vec<Node>,
    /// The index of each session by id.
    index: HashMap<SessionId, usize>,
    /// The session that holds each call, by call id.
    owner: HashMap<String, usize>,
    /// The ids of the calls that have a result.
    answered: HashSet<String>,
    /// The ids of the messages whose usage was counted.
    counted: HashSet<String>,
    /// The root session of the latest line that named one.
    current: Option<usize>,
}

impl Registry {
    /// The session at `index`.
    fn node(&mut self, index: usize) -> Result<&mut Node, ReaderError> {
        self.nodes
            .get_mut(index)
            .ok_or_else(|| ReaderError::Incomplete {
                // BORROW: .to_owned() makes the owned text the error keeps
                detail: "a session index names no session".to_owned(),
            })
    }

    /// The index of the root session `id`, added when it is new.
    fn root(&mut self, id: SessionId, raw_ref: &RawRef) -> usize {
        if let Some(index) = self.index.get(&id) {
            return *index;
        }
        let index = self.nodes.len();
        // BORROW: .clone() keeps the id in the index and in the session
        self.index.insert(id.clone(), index);
        self.nodes.push(Node {
            id,
            parent: None,
            root: index,
            span: None,
            next_seq: 0,
            // BORROW: .clone() keeps the first frame of the session
            raw_ref: raw_ref.clone(),
            model: None,
            started_at: None,
            ended_at: None,
            running: Tokens::default(),
            ended_by: None,
        });
        index
    }

    /// The index of the subagent session of the call `span` held by the session `owner`, added when it is new.
    fn child(&mut self, owner: usize, span: &str, raw_ref: &RawRef) -> Result<usize, ReaderError> {
        let call = span.parse::<SpanId>()?;
        let holder = self.node(owner)?;
        // BORROW: .clone() keeps the id of the parent in the child
        let parent = holder.id.clone();
        let root = holder.root;
        let id = SessionId::child(&parent, &call);
        if let Some(index) = self.index.get(&id) {
            return Ok(*index);
        }
        let index = self.nodes.len();
        // BORROW: .clone() keeps the id in the index and in the session
        self.index.insert(id.clone(), index);
        self.nodes.push(Node {
            id,
            parent: Some(parent),
            root,
            // BORROW: .to_owned() makes the owned call id the session keeps
            span: Some(span.to_owned()),
            next_seq: 0,
            // BORROW: .clone() keeps the first frame of the session
            raw_ref: raw_ref.clone(),
            model: None,
            started_at: None,
            ended_at: None,
            running: Tokens::default(),
            ended_by: None,
        });
        Ok(index)
    }

    /// Opens the root session `session_id` when it is new and makes it the current session.
    ///
    /// `first_frame` is the earliest frame the session holds.
    ///
    /// # Errors
    /// Returns the [`ReaderError`] of an id that cannot be a session id.
    pub(crate) fn enter(
        &mut self,
        session_id: &str,
        first_frame: &RawRef,
    ) -> Result<(), ReaderError> {
        let index = self.root(session_id.parse::<SessionId>()?, first_frame);
        self.current = Some(index);
        Ok(())
    }

    /// Reports whether a session is known yet.
    pub(crate) const fn has_session(&self) -> bool {
        self.current.is_some()
    }

    /// The session a line belongs to, or `None` when no session is known yet.
    ///
    /// A line with a `session_id` belongs to that root session; a line without one belongs to the
    /// root session of the latest line that had one. An `assistant` or `user` line with a
    /// `parent_tool_use_id` belongs to the subagent session of that call, and any other line with
    /// one belongs to the session that holds the call.
    ///
    /// # Errors
    /// Returns the [`ReaderError`] of an id that cannot be a session or span id.
    pub(crate) fn resolve(
        &mut self,
        session_id: Option<&str>,
        parent: Option<&str>,
        message_line: bool,
        raw_ref: &RawRef,
    ) -> Result<Option<usize>, ReaderError> {
        let root = match session_id {
            Some(text) => {
                let index = self.root(text.parse::<SessionId>()?, raw_ref);
                self.current = Some(index);
                index
            }
            None => match self.current {
                Some(index) => index,
                None => return Ok(None),
            },
        };
        let target = match parent {
            None => root,
            Some(call) => {
                let owner = self.owner.get(call).copied().unwrap_or(root);
                if message_line {
                    self.child(owner, call, raw_ref)?
                } else {
                    owner
                }
            }
        };
        Ok(Some(target))
    }

    /// The event `draft` of the session `target`, read from the frame `raw_ref` at time `at`.
    ///
    /// A call records its session as the holder of the call id, and a result records its call as
    /// answered.
    pub(crate) fn emit(
        &mut self,
        target: usize,
        raw_ref: &RawRef,
        at: Option<&Timestamp>,
        draft: Draft,
    ) -> Result<SessionEvent, ReaderError> {
        if let Some(span) = &draft.span {
            // BORROW: .as_str().to_owned() makes the owned call id the maps keep
            let call = span.as_str().to_owned();
            if matches!(draft.kind, EventKind::ToolCall { .. }) {
                self.owner.insert(call, target);
            } else if matches!(draft.kind, EventKind::ToolResult { .. }) {
                self.answered.insert(call);
            }
        }
        let node = self.node(target)?;
        let seq = node.next_seq;
        node.next_seq = seq.saturating_add(1);
        // BORROW: .clone() keeps the frame of the event
        let mut event = Event::new(seq, draft.kind, raw_ref.clone()).with_ext(draft.ext);
        if let Some(span) = draft.span {
            event = event.with_span(span);
        }
        if let Some(at) = at {
            // BORROW: .clone() keeps the time of the event
            event = event.with_at(at.clone());
            if node.started_at.as_ref().is_none_or(|first| at < first) {
                node.started_at = Some(at.clone());
            }
            if node.ended_at.as_ref().is_none_or(|last| at > last) {
                node.ended_at = Some(at.clone()); // BORROW: .clone() gives owned copy
            }
        }
        // BORROW: .clone() keeps the id of the session in the record
        Ok(SessionEvent::new(node.id.clone(), event))
    }

    /// Records the model of the session `target` when it has none yet.
    pub(crate) fn set_model(
        &mut self,
        target: usize,
        model: Option<String>,
    ) -> Result<(), ReaderError> {
        let node = self.node(target)?;
        if node.model.is_none() {
            node.model = model;
        }
        Ok(())
    }

    /// Reports whether `id` is a message id not seen before, and counts it.
    pub(crate) fn first_line_of_message(&mut self, id: &str) -> bool {
        // BORROW: .to_owned() makes the owned id the set keeps
        self.counted.insert(id.to_owned())
    }

    /// Adds the tokens of one turn to the session `target` and returns the running sum.
    pub(crate) fn add_turn(&mut self, target: usize, turn: Tokens) -> Result<Tokens, ReaderError> {
        let node = self.node(target)?;
        node.running = node.running.plus(turn);
        Ok(node.running)
    }

    /// Records how the latest `result` line ends the session `target`.
    pub(crate) fn set_ended_by(
        &mut self,
        target: usize,
        ended_by: EndedBy,
    ) -> Result<(), ReaderError> {
        self.node(target)?.ended_by = Some(ended_by);
        Ok(())
    }

    /// The ids of the root sessions, in the order they appeared.
    pub(crate) fn root_ids(&self) -> Vec<SessionId> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(index, node)| node.root == *index)
            // BORROW: .clone() copies the ids out of the registry
            .map(|(_, node)| node.id.clone())
            .collect()
    }

    /// The sessions with `source`, each root followed by its subagent sessions, every parent before its children.
    ///
    /// A root ends as its last `result` line says, or `unknown` without one. A subagent session
    /// ends `completed` when the call that started it has a result, else `unknown`.
    ///
    /// # Errors
    /// Returns the [`ReaderError`] of a session that cannot be built.
    pub(crate) fn into_sessions(self, source: &SessionSource) -> Result<Vec<Session>, ReaderError> {
        let Self {
            mut nodes,
            answered,
            ..
        } = self;
        // The nodes are in order of appearance, so a stable sort by root keeps every parent first.
        nodes.sort_by_key(|node| node.root);
        nodes
            .into_iter()
            .map(|node| {
                let ended_by = match (&node.parent, &node.span) {
                    (None, _) => node.ended_by.unwrap_or(EndedBy::Unknown),
                    (Some(_), Some(call)) if answered.contains(call) => EndedBy::Completed,
                    (Some(_), Some(_) | None) => EndedBy::Unknown,
                };
                // BORROW: .clone() gives each session its own source
                let mut session = Session::builder()
                    .id(node.id)
                    .source(source.clone())
                    .ended_by(ended_by)
                    .raw_ref(node.raw_ref);
                if let Some(parent) = node.parent {
                    session = session.parent_session(parent);
                }
                if let Some(model) = node.model {
                    session = session.model(model);
                }
                if let Some(at) = node.started_at {
                    session = session.started_at(at);
                }
                if let Some(at) = node.ended_at {
                    session = session.ended_at(at);
                }
                session.build().map_err(ReaderError::from)
            })
            .collect()
    }
}
