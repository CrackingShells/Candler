//! Claude Code as a session: the command that keeps stdin open, the first message, the hooks that
//! observe the session, and the tracker that follows turns and pending work.
//!
//! A turn ends at the `result` line of stdout. What is pending comes from the `Stop` hook payload
//! (`background_tasks` and `session_crons`), which is written before the turn's `result`; a turn
//! that ends with no `Stop` line, such as one that hit `--max-turns`, leaves the list as the latest
//! `background_tasks_changed` line made it. That line lists background tasks only: a wake-up or a
//! cron appears in the `Stop` payload alone. A list or an entry that cannot be read (a missing or
//! renamed key, an entry with no id, a line about the list that does not parse) makes the state
//! unknown and keeps the last readable list: pending work that cannot be read is never "nothing".
//! The state is known only when both the task list and the wake-up and cron list were read, and
//! only a `Stop` payload lists the second, so it is known only once every ended turn has its own
//! `Stop` payload: a turn that ends with no `Stop` line leaves the state unknown from then on
//! (an earlier turn's list says nothing about what this turn scheduled) and the session runs to
//! its cap. A `result` line may be read before its turn's `Stop` line, and the state is unknown
//! in between.
//!
//! A `Stop` payload is tied to its turn by what the recorded sessions show it carries: a
//! `prompt_id` that differs from turn to turn, and a `last_assistant_message` equal to the text
//! of that turn's `result` line. A payload whose `prompt_id` was already read is a duplicate and
//! counts for nothing. Otherwise it counts for the earliest ended turn that has none, and only
//! when its message equals that turn's `result` text; a payload read before its turn's `result`
//! waits for the turn that ends next, under the same test. The stream carries no `prompt_id`, so
//! the text is the only link between the two records, and a turn whose `Stop` payload cannot be
//! linked stays uncovered: the state is then unknown, which keeps the input open.

use std::str::FromStr as _;

use candler_harness::driver::DriverError;
use candler_harness::spec::{
    CommandSpec, HookSpec, PendingItem, PendingKind, PendingState, PreparedFile, SessionClose,
    SessionRecord, SessionSpec, SessionTracker,
};
use candler_schemas::primitives::RelPath;
use candler_schemas::raw::StreamName;
use serde_json::{Map, Value, json};

use crate::driver::{
    BUDGET_COST, BUDGET_TURNS, MODEL_EFFORT, MODEL_ID, PERMISSION_MODE, PROGRAM, PROMPT_TEMPLATE,
    PROMPT_VALUES, cap_text, invalid, render_prompt, required_text, value_at,
};

/// The home directory the core gives every agent container.
const AGENT_HOME: &str = "/home/agent";
/// The settings file that installs the hooks, relative to the agent's home.
const SETTINGS: &str = ".candler/settings.json";
/// The file the hooks append to, relative to the agent's home.
const HOOK_LOG: &str = ".candler/hooks.jsonl";
/// The name of the raw stream that archives the hook log.
const HOOK_STREAM: &str = "hooks";
/// The hook events the driver installs.
const HOOK_EVENTS: [&str; 2] = ["Stop", "SessionEnd"];

/// The hook command: appends the payload, with the container's clock, as one JSON line to the log.
/// It prints nothing on stdout or stderr, whatever the log does, never fails and never decides, so
/// it cannot block or steer the agent.
fn hook_command() -> String {
    format!(
        "p=$(cat); {{ printf '{{\"written_ns\":%s,\"payload\":%s}}\\n' \"$(date +%s%N)\" \"$p\" >> \"$HOME/{HOOK_LOG}\"; }} 2>/dev/null || true; exit 0"
    )
}

/// The settings file that installs the observation hooks.
fn settings_json() -> Value {
    let hooks: Map<String, Value> = HOOK_EVENTS
        .into_iter()
        .map(|event| {
            (
                event.to_owned(), // BORROW: .to_owned() makes the owned key
                json!([{ "hooks": [{ "type": "command", "command": hook_command() }] }]),
            )
        })
        .collect();
    json!({ "hooks": hooks })
}

/// The hook spec: the settings file and where the log goes.
fn hooks() -> Result<HookSpec, DriverError> {
    let rel = |text: &str| {
        RelPath::from_str(text).map_err(|error| invalid(text, error.to_string())) // BORROW: .to_string() gives owned text
    };
    let mut text = serde_json::to_string_pretty(&settings_json())
        .map_err(|error| invalid(SETTINGS, error.to_string()))?; // BORROW: .to_string() gives owned text
    text.push('\n');
    let stream = StreamName::from_str(HOOK_STREAM)
        .map_err(|error| invalid(HOOK_STREAM, error.to_string()))?; // BORROW: .to_string() gives owned text
    Ok(HookSpec::new(
        vec![PreparedFile::in_home(
            rel(SETTINGS)?,
            text.into_bytes(),
            false,
        )],
        rel(HOOK_LOG)?,
        stream,
    ))
}

/// The prompt of the cell, with its values filled in.
fn prompt(cell: &candler_harness::spec::Cell<'_>) -> Result<String, DriverError> {
    let template = required_text(cell, PROMPT_TEMPLATE)?;
    let values = match value_at(cell, PROMPT_VALUES)? {
        None => None,
        Some(Value::Object(map)) => Some(map),
        Some(
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_),
        ) => {
            return Err(invalid(PROMPT_VALUES, "is not an object".to_owned())); // BORROW: .to_owned() makes the owned text
        }
    };
    render_prompt(template, values)
}

/// The session of a cell: stdin open, the prompt as the first message, closed by end of input, with
/// the observation hooks installed from a settings file outside the workspace.
///
/// # Errors
/// [`DriverError`] when a value of the cell is missing or cannot be used.
pub fn spec(cell: &candler_harness::spec::Cell<'_>) -> Result<SessionSpec, DriverError> {
    let model = required_text(cell, MODEL_ID)?;
    let mode = required_text(cell, PERMISSION_MODE)?;
    let prompt = prompt(cell)?;
    // BORROW: .to_owned() makes each owned argument the command keeps
    let mut argv = vec![
        PROGRAM.to_owned(),
        "--print".to_owned(),
        "--input-format".to_owned(),
        "stream-json".to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
        "--model".to_owned(),
        model.to_owned(),
        "--setting-sources".to_owned(),
        "project,local".to_owned(),
        "--permission-mode".to_owned(),
        mode.to_owned(),
    ];
    if let Some(effort) = value_at(cell, MODEL_EFFORT)? {
        let Value::String(effort) = effort else {
            return Err(invalid(MODEL_EFFORT, "is not text".to_owned())); // BORROW: .to_owned() makes the owned text
        };
        argv.push("--effort".to_owned()); // BORROW: .to_owned() makes the owned argument
        argv.push(effort.clone()); // BORROW: .clone() copies the text out of the cell
    }
    if let Some(turns) = cap_text(cell, BUDGET_TURNS, true)? {
        argv.push("--max-turns".to_owned()); // BORROW: .to_owned() makes the owned argument
        argv.push(turns);
    }
    if let Some(cost) = cap_text(cell, BUDGET_COST, false)? {
        argv.push("--max-budget-usd".to_owned()); // BORROW: .to_owned() makes the owned argument
        argv.push(cost);
    }
    argv.push("--settings".to_owned()); // BORROW: .to_owned() makes the owned argument
    argv.push(format!("{AGENT_HOME}/{SETTINGS}"));
    let command = CommandSpec::builder().argv(argv).build()?;
    let message = json!({
        "type": "user",
        "message": { "role": "user", "content": [{ "type": "text", "text": prompt }] },
    });
    let mut input = message.to_string().into_bytes(); // BORROW: .to_string() gives owned text
    input.push(b'\n');
    Ok(SessionSpec::new(command, input, SessionClose::Eof).with_hooks(hooks()?))
}

/// A turn that has ended.
#[derive(Debug)]
struct EndedTurn {
    /// The text of its `result` line.
    text: String,
    /// Whether a `Stop` payload was linked to it.
    covered: bool,
}

/// Follows a Claude Code session: the turns that have ended, whether the latest has, and what is
/// pending.
#[derive(Debug, Default)]
pub struct Tracker {
    /// The turns that have ended.
    turns: u64,
    /// The turns that have ended, each with the text of its `result` line and whether a `Stop`
    /// payload was linked to it. The state is known only when every one is covered.
    ended: Vec<EndedTurn>,
    /// The message of a `Stop` payload read while no ended turn waited for one.
    ahead: Option<String>,
    /// The keys of the `Stop` payloads read: the `prompt_id`, or the payload's own text.
    seen: Vec<String>,
    /// Whether the latest turn has ended and no later one has started.
    turn_ended: bool,
    /// Whether the background tasks were read: from a `background_tasks_changed` line or a `Stop`
    /// payload, and false after a list, an entry or a line about them that could not be read.
    tasks_known: bool,
    /// Whether the wake-ups and crons were read. Only a `Stop` payload lists them, so a session
    /// whose turn ends with no `Stop` line keeps this false.
    timers_known: bool,
    /// The background tasks last listed.
    tasks: Vec<PendingItem>,
    /// The wake-ups and crons last listed by a `Stop` payload.
    timers: Vec<PendingItem>,
}

/// The text field `key` of `object`, or an empty string.
fn text_of<'a>(object: &'a Map<String, Value>, key: &str) -> &'a str {
    object.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// The list `key` of `object`.
fn list_of<'a>(object: &'a Map<String, Value>, key: &str) -> &'a [Value] {
    object
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

impl Tracker {
    /// Reads one stdout line.
    fn stream(&mut self, line: &[u8]) {
        let Ok(Value::Object(object)) = serde_json::from_slice::<Value>(line) else {
            // A line that mentions the task list and cannot be parsed leaves the list unread.
            let needle = b"background_tasks_changed";
            if line.windows(needle.len()).any(|window| window == needle) {
                self.tasks_known = false;
            }
            return;
        };
        match (text_of(&object, "type"), text_of(&object, "subtype")) {
            ("result", _) => {
                self.turns = self.turns.saturating_add(1);
                self.turn_ended = true;
                let text = text_of(&object, "result").to_owned(); // BORROW: the turn owns its text
                let covered = self.ahead.take().is_some_and(|message| message == text);
                self.ended.push(EndedTurn { text, covered });
            }
            ("assistant" | "user", _) | ("system", "init" | "task_notification") => {
                self.turn_ended = false;
            }
            ("system", "background_tasks_changed") => {
                match pending_items(list_of(&object, "tasks"), "task_id", |_| PendingKind::Task) {
                    Some(tasks) if object.get("tasks").is_some_and(Value::is_array) => {
                        self.tasks = tasks;
                        self.tasks_known = true;
                    }
                    Some(_) | None => self.tasks_known = false,
                }
            }
            _ => {}
        }
    }

    /// Reads one hook-log line.
    fn hook(&mut self, line: &[u8]) -> Result<(), DriverError> {
        let record = |detail: &str| DriverError::Record {
            detail: detail.to_owned(), // BORROW: .to_owned() makes the owned text the error keeps
        };
        let Ok(Value::Object(entry)) = serde_json::from_slice::<Value>(line) else {
            self.unread();
            return Err(record("a hook line is not a JSON object"));
        };
        let Some(Value::Object(payload)) = entry.get("payload") else {
            self.unread();
            return Err(record("a hook line has no payload object"));
        };
        if text_of(payload, "hook_event_name") != "Stop" {
            return Ok(());
        }
        let key = match payload.get("prompt_id").and_then(Value::as_str) {
            Some(id) if !id.is_empty() => id.to_owned(), // BORROW: the tracker owns its keys
            _ => Value::Object(payload.clone()).to_string(), // BORROW: the tracker owns its keys
        };
        if self.seen.contains(&key) {
            return Ok(());
        }
        self.seen.push(key);
        let message = text_of(payload, "last_assistant_message");
        match self.ended.iter_mut().find(|turn| !turn.covered) {
            Some(turn) => turn.covered = turn.text == message,
            None => self.ahead = Some(message.to_owned()), // BORROW: the tracker owns the message
        }
        let readable = |key: &str| payload.get(key).is_some_and(Value::is_array);
        let tasks = pending_items(list_of(payload, "background_tasks"), "id", |_| {
            PendingKind::Task
        })
        .filter(|_| readable("background_tasks"));
        let timers = pending_items(list_of(payload, "session_crons"), "id", |cron| {
            // A cron entry that does not say it is one-shot is treated as recurring.
            if cron.get("recurring").and_then(Value::as_bool) == Some(false) {
                PendingKind::WakeUp
            } else {
                PendingKind::Cron
            }
        })
        .filter(|_| readable("session_crons"));
        let complete = tasks.is_some() && timers.is_some();
        match tasks {
            Some(tasks) => {
                self.tasks = tasks;
                self.tasks_known = true;
            }
            None => self.tasks_known = false,
        }
        match timers {
            Some(timers) => {
                self.timers = timers;
                self.timers_known = true;
            }
            None => self.timers_known = false,
        }
        if complete {
            Ok(())
        } else {
            Err(record(
                "a Stop payload lists pending work that cannot be read",
            ))
        }
    }

    /// Marks both lists unread.
    const fn unread(&mut self) {
        self.tasks_known = false;
        self.timers_known = false;
    }
}

/// The pending items of `entries`, each with the non-empty text id at `id_key`, or `None` when an
/// entry is not an object or has no such id: one entry that cannot be read makes the list unread.
fn pending_items(
    entries: &[Value],
    id_key: &str,
    kind: impl Fn(&Map<String, Value>) -> PendingKind,
) -> Option<Vec<PendingItem>> {
    entries
        .iter()
        .map(|entry| {
            let object = entry.as_object()?;
            let id = object
                .get(id_key)
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())?;
            Some(PendingItem::new(kind(object), id.to_owned())) // BORROW: .to_owned() makes the owned id
        })
        .collect()
}

impl SessionTracker for Tracker {
    fn feed(&mut self, record: SessionRecord<'_>) -> Result<(), DriverError> {
        match record {
            SessionRecord::Stream(line) => {
                self.stream(line);
                Ok(())
            }
            SessionRecord::Hook(line) => self.hook(line),
            // A record kind this tracker does not know carries nothing it reads.
            _ => Ok(()),
        }
    }

    fn state(&self) -> PendingState {
        let pending = self
            .tasks
            .iter()
            .chain(self.timers.iter())
            .cloned() // BORROW: .cloned() copies each item out of the tracker
            .collect();
        PendingState::new(
            self.turns,
            self.turn_ended,
            self.tasks_known && self.timers_known && self.ended.iter().all(|turn| turn.covered),
            pending,
        )
    }
}
