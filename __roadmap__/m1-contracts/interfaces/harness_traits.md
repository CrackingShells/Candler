# Harness traits: driver and trace reader

**Goal**: `candler-harness` defines `HarnessDriver` and `TraceReader` (R01 §Traits) with their supporting data types, synchronous, `Send` and usable as `dyn` objects, and two fake harnesses of opposite shapes drive both traits.
**Pre-conditions**:
- [ ] [cmd] DEP(wire_foundations) and DEP(wire_trace) hold: `for l in wire_foundations wire_trace; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
**Success Gates**:
- ✅ [cmd] `cargo nextest run -p candler-harness -E 'binary(guard_two_shapes)'` passes, holding two B tests built from R01's text, not from the code: a Claude Code-shaped fake (`Lines` framing, a live reader, a signal-only stop, a `File` collect) and an OpenCode-shaped fake (`Document` framing, no live reader, `execution_outside_child`, an `Http` pre-stop with a body, a `Command` collect whose argv carries the session id taken from `observed().session_ids`); each drives a fixed input through `id`, `detect`, `feed`, `observed`, `finish` and `classify_exit`, and the test asserts the sessions, ids and `EndedBy` it states by hand
- ✅ [cmd] `cargo nextest run -p candler-harness -E 'binary(public_surface)'` passes: the file names every public item M2b uses, and asserts `Box<dyn HarnessDriver>` and `Box<dyn TraceReader>` are `Send` by a compile-time bound; it holds no behaviour assert
- ✅ [cmd] `! cargo tree -p candler-harness -e normal --prefix none | grep -Eq '^(tokio|futures) '`: the traits pull in no async runtime
- ✅ [cmd] STD and SUBJ pass
- ✅ [nl] The leaf report has a bite mutation per B test; api_review (end of this level) reports no open finding on this leaf
**References**: [R01 §Harness drivers (What the runner needs, Traits, OpenCode v2), §Ends, §Process supervision, §Agent environment and secrets](../../../__design__/architecture.md) — every signature; [R07](../../candler-v1/contracts/parity/README.md) — `m2b_execution_parity.md`, the first implementer of a driver

## Step 1: Define commands, stop specs and collect specs
**Goal**: The data a driver returns to the core exists, and none of it acts on a process.
**Implementation Logic**:
1. `CommandSpec {argv: Vec<String> (non-empty), env_overlay: Vec<EnvEntry {name: String, value: EnvValue}>, cwd?: RelPath}` with `EnvValue`: `Literal(String)` or `Secret(SecretName)`, a secret named, never held; `Debug` prints names only. A builder refuses an empty argv and a duplicate variable name.
2. `StopSpec {pre_stop: Option<Action>, signal: StopSignal (Term | Int | Hup), grace: Duration}`; `Action`: `Command(CommandSpec)` or `Http {method: HttpMethod, url: String, body: Option<String>}`. Whether the URL is loopback is the supervisor's check, not this type's.
3. `CollectSpec`: `File {glob: String}` or `Command {spec: CommandSpec, name: StreamName}`.
4. `Capabilities {graceful_stop, live_events, budgets: BudgetCaps {turns, cost}, in_harness_guard, census, execution_outside_child}`, all `bool`; `ProcessExit {status: ExitStatus, ended: ProcessEnd (Exited | Deadline | Stopped), tail: Vec<u8>}`; `Observed {session_ids: Vec<SessionId>}`; `Framing`: `Lines` | `Document`; `HarnessId` (wrapping `primitives::Name`; api_review G9); `FormatVersion (semver::Version)`; `Confidence` (`Certain` | `Likely`).
5. `Prepared {files: Vec<PreparedFile {path: RelPath, contents: Vec<u8>, executable: bool}>, env_overlay: Vec<EnvEntry>}`; `Cell<'a>` wraps `&'a [ResolvedValue]` with `get(&FieldPath) -> Option<&JsonValue>`; `Workspace<'a>` wraps the workspace root `&'a Path`.
6. Every public struct is `#[non_exhaustive]` with a builder or a constructor; every public enum is `#[non_exhaustive]` except `Framing` and `ExitStatus` (closed).
**Deliverables**: `crates/candler-harness/src/spec.rs` (`CommandSpec`, `CommandSpecBuilder`, `EnvEntry`, `EnvValue`, `StopSpec`, `StopSignal`, `Action`, `HttpMethod`, `CollectSpec`, `Capabilities`, `BudgetCaps`, `ProcessExit`, `ProcessEnd`, `Observed`, `Framing`, `HarnessId`, `FormatVersion`, `Confidence`, `Prepared`, `PreparedFile`, `Cell`, `Workspace`); `crates/candler-harness/src/lib.rs` (module declarations only; consumers import by module path, AGENTS.md)
**Consistency Checks**: `cargo clippy -p candler-harness --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(runner): define commands, stop specs and collect specs`

## Step 2: Define the harness driver and trace reader traits
**Goal**: The two traits exist with R01's signatures, object-safe and `Send`.
**Implementation Logic**:
1. `pub trait HarnessDriver: Send + Sync` with `id`, `supported -> semver::VersionReq`, `version_probe -> CommandSpec`, `parse_version(&[u8]) -> Result<semver::Version, DriverError>`, `capabilities`, `prepare(&Cell<'_>, &Workspace<'_>) -> Result<Prepared, DriverError>`, `command(&Cell<'_>, &Workspace<'_>) -> Result<CommandSpec, DriverError>`, `stop -> StopSpec`, `live_reader -> Option<Box<dyn TraceReader>>`, `offline_reader(&StreamName) -> Option<Box<dyn TraceReader>>` (the reader for a stream the core collected, such as an export), `classify_exit(&ProcessExit, &[Session]) -> EndedBy`, `collect(&Workspace<'_>, &Observed) -> Vec<CollectSpec>`.
2. `pub trait TraceReader: Send` with `id -> NameVersion` (the reader's name and version, written into the trace header and bumped whenever its output changes), `framing -> Framing`, `detect(&[u8]) -> Option<(FormatVersion, Confidence)>`, `feed(&mut self, FrameIn<'_>) -> Result<Vec<SessionEvent>, ReaderError>` with `FrameIn {raw_ref: RawRef, at: Option<Timestamp>, frame: &RawFrame}` (one line under `Lines`, the whole document under `Document`): the core supplies the frame's place and its wall time from the header's clock, so a reader fills `raw_ref` and `at` without knowing the clock, `observed(&self) -> Observed`, `finish(self: Box<Self>) -> Result<Vec<Session>, ReaderError>` (root first, children linked by `parent_session`).
3. `DriverError` and `ReaderError` (`thiserror`, `#[non_exhaustive]`); `ReaderError` names the frame index where one applies.
4. No method is `async` and no method spawns, signals or opens a socket; the docs say the core runs every command and HTTP call a driver describes.
**Deliverables**: `crates/candler-harness/src/driver.rs` (`HarnessDriver`, `DriverError`); `crates/candler-harness/src/reader.rs` (`TraceReader`, `FrameIn`, `ReaderError`); `crates/candler-harness/tests/public_surface.rs`
**Consistency Checks**: `cargo nextest run -p candler-harness -E 'binary(public_surface)'` (expected: PASS)
**Commit**: `feat(runner): define the harness driver and trace reader traits`

## Step 3: Drive two fake harnesses through both traits
**Goal**: The traits carry both harness shapes R01 names, so the second driver (M4) needs no trait change.
**Implementation Logic**:
1. `tests/guard_two_shapes.rs` with two fakes defined in the test: `LinesFake` reads one JSON object per frame and emits sessions from it; `DocumentFake` reads one JSON document holding a session list, as an export would; its driver returns it from `offline_reader` for the export stream and has no `live_reader`.
2. Each test holds its input as a literal and its expected sessions, ids and `EndedBy` as literals written from R01 §Ends and §OpenCode v2: the OpenCode-shaped fake's `collect` must place the observed session id into the export command's argv, and its `classify_exit` must read an error result from the sessions despite exit status 0.
3. Then delete the leaf's scaffolding and write the bite record.
**Deliverables**: `crates/candler-harness/tests/guard_two_shapes.rs`
**Consistency Checks**: `cargo nextest run -p candler-harness -E 'binary(guard_two_shapes)' && ! git grep --untracked -n 'mod scaffold' -- crates/candler-harness` (expected: PASS)
**Commit**: `test(runner): drive two fake harnesses through both traits`
