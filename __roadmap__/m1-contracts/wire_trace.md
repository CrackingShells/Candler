# Wire trace: raw frames and trace records

**Goal**: `candler-schemas` defines `raw-frames/1` and `normalised-trace/1` (R01 §Raw traces, §Normalised trace model): stream names, frames, sessions, events and the trace file with its ordering rules, published and drift-checked.
**Pre-conditions**:
- [ ] [cmd] `git log --format=%s feat/m1-contracts | grep -qx 'build(repo): scaffold the m1 crates, modules and dependencies'`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-schemas -E 'binary(guard_trace_wire)'` passes, holding four B tests: a hand-authored byte-literal trace file carrying `ext` and an unknown event kind reads and writes back byte-identical (I3, I18); a single-fault refusal table of seven mutations of one valid file, each refusal naming the offending session or `seq`, plus the valid base; `SessionId` accepts exactly the ids `EvidenceRef` accepts in `event:<id>#0`; hand-written frame vectors (CR, invalid UTF-8, no final `\n`, an empty line, `\r\n`) give the literal JSON lines the test states
- ⬜ [cmd] STD, DRIFT and SUBJ pass
- ⬜ [nl] The leaf report has a bite mutation per B test
- ⬜ [nl] The level's Grit review reports no open finding
**References**: [R01 §Raw traces, §Normalised trace model, §Wire encodings (Evidence refs)](../../__design__/architecture.md) — every shape and rule; [R04](../../__reports__/m1-contracts/00-test_definition_v0.md) — `guard_trace_wire`

## Step 1: Define raw frames and stream names
**Goal**: `candler_schemas::raw` holds the framed-file records.
**Implementation Logic**:
1. `StreamName`: `^[a-z0-9][a-z0-9._-]*$` (end-anchored); `stdout`, `stderr` and collected file names.
2. `Clock`, tagged on `kind`: `monotonic {anchor: ClockAnchor {wall: Timestamp, mono_ns: u64}}`, `unrecorded`.
3. `RawHeader {schema: "raw-frames/1", stream: StreamName, clock: Clock, source?: Digest}`.
4. `FrameData`, tagged on `kind`: `text {text}` (valid UTF-8) or `bytes {base64}` (standard alphabet, padded). `FrameData::from_bytes(&[u8])` picks `text` exactly when the bytes are valid UTF-8; `FrameData::to_bytes()` inverts it.
5. `RawFrame {t_mono_ns?: u64, data: FrameData}`; `RawRecord`, tagged on `kind`: `header` | `frame`. `RawRef {stream: StreamName, frame: u64}` (frame indices count from 0 after the header).
6. Splitting a stream into lines is `frames`' job (supervisor leaf); this module only states the shapes.
**Deliverables**: `crates/candler-schemas/src/raw.rs` (`StreamName`, `Clock`, `ClockAnchor`, `RawHeader`, `FrameData`, `RawFrame`, `RawRecord`, `RawRef`, `RawError`, `RAW_FRAMES_SCHEMA`)
**Consistency Checks**: `cargo clippy -p candler-schemas --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(schemas): define raw frames and stream names`

## Step 2: Define sessions and how a session ends
**Goal**: Sessions, their ids and `EndedBy` exist in `candler_schemas::trace`.
**Implementation Logic**:
1. `SessionId`: non-empty, no `#`, no white space (the spelling of §Wire encodings); `SessionId::child(&parent, span: &SpanId) -> SessionId` gives `<parent>/<span>`. `SpanId`: the harness's call id, same character rule.
2. `SessionSource {harness: NameToken, harness_version: semver::Version, reader: NameToken, reader_version: semver::Version}`.
3. `EndedBy`, tagged on `kind` (R01 §Ends): `completed`, `agent_error {reason?: ReasonCode}`, `cap {cap: turns | cost | wall}`, `gateway_error`, `budget_stop`, `killed`, `interrupted`, `unknown`. `#[non_exhaustive]`.
4. `Session {id, source, model?: String, started_at?: Timestamp, ended_at?: Timestamp, ended_by: EndedBy, parent_session?: SessionId, raw_ref: RawRef}`; the builder refuses a `parent_session` that is not the id's prefix.
**Deliverables**: `crates/candler-schemas/src/trace.rs` (`SessionId`, `SpanId`, `SessionSource`, `EndedBy`, `Cap`, `Session`, `SessionBuilder`, `TraceError`)
**Consistency Checks**: `cargo clippy -p candler-schemas --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(schemas): define sessions and how a session ends`

## Step 3: Define normalised trace events
**Goal**: Events and their kinds exist, and unknown kinds survive a read and a write.
**Implementation Logic**:
1. `Event {seq: u64, at?: Timestamp, kind: EventKind, span?: SpanId, raw_ref: RawRef, ext: serde_json::Map}` (`ext` always present, possibly empty).
2. `EventKind`, tagged on `kind`:
   - `user_message {text?}`, `assistant_message {text?}`;
   - `tool_call {name, origin: ToolOrigin, argv?: Vec<String>, wrapped_calls: Vec<WrappedCall>}`, with `ToolOrigin` tagged `builtin | mcp {server} | other`, and `WrappedCall {name, origin}`;
   - `tool_result {is_error: bool, denied?: Denial}`, with `Denial {by: harness | hook | user, reason?: String}`;
   - `notification {}` (the line's content goes in `ext`);
   - `usage {scope: message | session_total, input_tokens: u64, output_tokens: u64, cache_read_tokens: u64, cache_creation_tokens: u64, cost_usd?: Finite}`, a `session_total` being the session's running total, so a later one supersedes an earlier one;
   - `Unknown {kind: String, body: serde_json::Map}`, which reading keeps and writing reproduces (I18). Known kinds are `#[non_exhaustive]`.
3. `SessionEvent {session: SessionId, event: Event}`, the record a reader's `feed` returns.
**Deliverables**: `crates/candler-schemas/src/trace.rs` (`Event`, `EventKind`, `ToolOrigin`, `WrappedCall`, `Denial`, `UsageScope`, `SessionEvent`)
**Consistency Checks**: `cargo clippy -p candler-schemas --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(schemas): define normalised trace events`

## Step 4: Define the trace file and its ordering rules
**Goal**: `TraceFile` reads and writes `normalised-trace/1` and refuses every file that breaks R01's rules.
**Implementation Logic**:
1. `TraceHeader {schema: "normalised-trace/1", reader: NameVersion, raw: Vec<RawFileRef {stream: StreamName, digest: Digest}>}`; `TraceRecord`, tagged on `kind`: `header` | `session` | `event` (the event record is a `SessionEvent`).
2. `TraceFile {header, sessions: Vec<Session>, events: Vec<SessionEvent>}`; `TraceFile::read(&[u8]) -> Result<_, TraceError>` and `write(&self) -> Vec<u8>` (one compact JSON object per line, fields in declaration order, `\n` after each).
3. `read` refuses, each with an error naming the session or `seq`: a missing or second header; a session after an event; a child before its parent; a child id that is not `<parent>/<span>`; an event for an undeclared session; a `seq` gap or repeat within a session; a `raw_ref.stream` absent from `header.raw`.
4. `tests/guard_trace_wire.rs`: the four B tests of the Success Gates. The byte-literal file is written by hand from R01, in the writer's field order; it is a literal in the test, not a blessed output.
**Deliverables**: `crates/candler-schemas/src/trace.rs` (`TraceHeader`, `RawFileRef`, `TraceRecord`, `TraceFile`, `TraceFile::read`, `TraceFile::write`, `NORMALISED_TRACE_SCHEMA`); `crates/candler-schemas/tests/guard_trace_wire.rs`
**Consistency Checks**: `cargo nextest run -p candler-schemas -E 'binary(guard_trace_wire)'` (expected: PASS)
**Commit**: `feat(schemas): define the trace file and its ordering rules`

## Step 5: Publish the raw frame and trace schemas
**Goal**: `raw-frames.schema.json` and `normalised-trace.schema.json` are generated and drift-checked, and the scaffolding is gone.
**Implementation Logic**:
`raw::roots()` returns `raw-frames` (`RawRecord`); `trace::roots()` returns `normalised-trace` (`TraceRecord`). Bless and read the diff; the all-roots pattern test now covers `StreamName`, `SessionId` and `SpanId`. Delete every `mod scaffold` and write the bite record.
**Deliverables**: `crates/candler-schemas/src/raw.rs` (`roots`); `crates/candler-schemas/src/trace.rs` (`roots`); `crates/candler-schemas/schema/{raw-frames,normalised-trace}.schema.json`
**Consistency Checks**: `cargo nextest run -p candler-schemas -E 'binary(schema_drift) | test(no_published_pattern_depends_on_the_regex_engine)' && ! git grep --untracked -n 'mod scaffold' -- crates/candler-schemas` (expected: PASS)
**Commit**: `feat(schemas): publish the raw frame and trace schemas`
