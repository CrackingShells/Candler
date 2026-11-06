# Engine: planning, batching, outcome files

**Goal**: `engine::evaluate` fills the frozen seam. It plans each run's items from the checked suite, writes `not_applicable` for an `applies_to` miss and `blocked` for a dependent of a failed or blocked item, and sends dependents with `prior[]` in the same request. It runs requests concurrently through the client and writes one committed outcome file per request, so a regrade persists exactly what the plug-in received (ENG1–ENG3).
**Pre-conditions**:
- [ ] [cmd] ENV, and the L1 merges of `registry`, `run_context`, `protocol_client` and `selection_rates` are on `feat/m2a-evaluation-parity`'s first-parent history
- [ ] [cmd] `git log --first-parent --format=%s feat/m2a-evaluation-parity | grep -qx 'merge(evaluator-helper): land helper_runtime'` (the echo plug-in)
**Success Gates**:
- ⬜ [cmd] STD, SUBJ and SCAFFOLD pass, and OWNED passes with `^crates/candler-core/(src/evaluation/engine/|tests/(guard_engine\.rs|fixtures/engine/|public_surface\.rs)$)` with only the `engine_surface` block of `public_surface.rs` changed
- ⬜ [cmd] SPAWN, then `cargo nextest run -p candler-core -E 'binary(guard_engine)'` passes (ENG1–ENG3)
- ⬜ [nl] The integration review confirms the seams: the engine writes the outcome-file layout that `selection` reads, and uses the `context` digest projection and the client's accounting unchanged
**References**: [R01 §Evaluator protocol, §Run store](../../../../__design__/architecture.md) — requests, `applies_to`, `depends_on`, `prior`, outcome files; [R03 §Skeleton seams, §C6 dependencies, §I4](../../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md); `crates/candler-core/src/store/` — `OutcomeWriter::create`, `append`, `commit`; `crates/candler-core/tests/oracle/vectors/context_digest_runs.toml`; [R04 ENG1–ENG3](../../../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md)

## Step 1: Plan items, applies_to and dependencies
**Goal**: For each run, the engine decides which entries are sent, which are `not_applicable` and which are `blocked`, before any process starts.
**Implementation Logic**:
1. `plan(run, suite: &CheckedSuite) -> RunPlan`: entries whose `applies_to` does not match the run's arm become `not_applicable {reason: not_applicable_to_arm}`. They are core-owned and written without a request.
2. Entries are grouped so that every `depends_on` chain is in one request (C6 with C1 and C5). A dependent is evaluated after its dependencies within the request, and `prior[]` carries their outcomes. A dependency that ends `fail`, `blocked` or `not_measured` makes the dependent `blocked` with cause `{expectation: <id>}` (R01's `Cause::Expectation`), written by the core without sending it.
3. Dependency cycles are a planning error naming the cycle.
**Deliverables**: `crates/candler-core/src/evaluation/engine/plan.rs` (`plan`, `RunPlan`, `Batch`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(evaluation): plan items, applies_to and dependencies per run`

## Step 2: Run requests and commit outcome files
**Goal**: `evaluate` preflights environments, runs batches concurrently through the client, and commits one outcome file per request.
**Implementation Logic**:
1. `evaluate(request)`: load the registry and check the suite; refuse with `EvaluateError::EnvironmentNotReady` when a plug-in's `EnvironmentKey` has no `ready` marker (the preflight is the caller's: `surfaces` composes `register_evaluation_steps` in the binary, and the core never registers steps itself); for each run, `assemble` and `materialise` the context, then send its batches with `Client::evaluate` under a semaphore of `request.concurrency`.
2. Each request's outcomes, core-written rows included, go into one `OutcomeWriter` (`create`, `append`, `commit`). Every record carries the request id, the evaluator `name@version` and instrument from `describe`, the `context_digest` of the materialised context, and the `suite_entry_digest`. A request that fails leaves no committed file and is reported in `EvaluateReport`.
3. The workspace is removed after the request ends, success or not.
4. `pub fn regrade(store, plugin, suite, runs) -> Result<EvaluateReport, EvaluateError>`, a thin support function over `evaluate` with the defaults `golden_parity` uses.
**Deliverables**: `crates/candler-core/src/evaluation/engine/{mod,run,write}.rs` (`evaluate`, `regrade`, `EvaluateReport` filled)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(evaluation): run requests concurrently and commit outcome files`

## Step 3: Guard planning, the digest seam and I4
**Goal**: ENG1–ENG3 decide the engine with the helper's synthetic and echo plug-ins and a counter kind.
**Implementation Logic**:
1. `tests/fixtures/engine/`: a manifest and suite for the synthetic plug-in (4 entries: one with an `applies_to` miss, a dependency pair whose base fails, and a dependency pair whose base passes), a manifest for the echo plug-in, and a counter kind (a stub plug-in returning an incrementing value from a file in its own temp dir). Each plug-in runs from `python/candler-evaluator` via `uv run --frozen`.
2. `tests/guard_engine.rs` (`#![cfg(test)]`, store from the M1 mini-corpus import):
   - ENG1 (R6): the `applies_to` miss gives `not_applicable`; the dependent of the fail is `blocked`; the dependent of the pass got `prior[]` in the same request (read from the echo evidence). Oracle: hand literal. Bite: skip the dependency check.
   - ENG2 (R9): the echo plug-in returns what it received. The persisted `context_digest` equals the RFC 8785 sha256 of the echoed context, computed in the test with `candler_schemas::canonical`, and equals `live_context_runs.toml` for the three real runs. Oracle: the Python vectors. Bite: digest a different projection.
   - ENG3 (R5): two regrades with the counter kind; selection flags I4 on the second. Oracle: hand literal. Bite: reuse the cached outcome.
3. Fill `engine_surface`, delete scaffold tests, record bites.
**Deliverables**: `crates/candler-core/tests/guard_engine.rs` (ENG1–ENG3); `crates/candler-core/tests/fixtures/engine/`; the `engine_surface` block
**Consistency Checks**: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)' && cargo nextest run -p candler-core -E 'binary(guard_engine)'` (expected: PASS)
**Commit**: `test(evaluation): guard planning, the digest seam and i4`
