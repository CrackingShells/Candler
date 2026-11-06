# Run exec: spawn, ends, seal

**Goal**: `exec_run(&Runtime, &RunPlan, &Slot) -> RunOutcome` runs one attempt in its container: it takes the slot, spawns with a live observer and the stop hook, classifies the end by the R2 precedence, retries a gateway failure once as a new attempt, collects, and seals the run directory with `run.done` last.
**Pre-conditions**:
- [ ] [cmd] The Phase 0 scaffold is landed: `for s in '^docs(design): ' '^build(repo): ' '^ci(repo): ' '^feat(schemas): ' '^feat(orchestrator): scaffold'; do git log --format=%s origin/main..feat/m2b-execution-parity | grep -q "$s" || exit 1; done && test -s __roadmap__/m2b-execution-parity/feature_map/feature_map.json`
- [ ] [nl] api_review closed with no open finding on `Runtime`, `RunPlan`, `RunOutcome`, `Evidence`, `StopHook` and `Isolation`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
- [ ] [cmd] DEP(container_run), DEP(container_stop), DEP(gateway), DEP(run_evidence), DEP(claude_code_driver) and DEP(job_store) hold: `for l in container_run container_stop gateway run_evidence claude_code_driver job_store; do git log --first-parent --format=%s feat/m2b-execution-parity | grep -qx "merge(.*): land $l" || exit 1; done`
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_run_exec)'` passes, holding two B tests. B29: the ends table, rows E01–E18 in the R2 order, from recorded streams, three recorded inspect JSONs (clean, 124, OOM) and R01 §Ends. B30: the sealed run dir's path set equals R01's literal, `run.done` appears last when the run stops after each step k, and no stored path is absolute (`assert_clean` with the temp root as the needle)
- ⬜ [cmd] STD, LINUX and SUBJ pass
- ⬜ [nl] The bite record gives a tried mutation per B test: B29 swaps the `OOMKilled` and `ExitCode == 137` checks so an OOM reads as our stop; B30 writes `run.done` before `tree.json`
- ⬜ [cmd] No scaffolding: `! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/runner/exec crates/candler-core/tests/guard_run_exec.rs`
- ⬜ [cmd] No leaf edit outside the owned paths: `git diff --name-only <base>..HEAD | grep -v -e '^crates/candler-core/src/runner/exec/' -e '^crates/candler-core/tests/guard_run_exec.rs$' -e '^crates/candler-core/tests/scaffold_' | wc -l` prints 0
- ⬜ [nl] The plain verifier, limited to the ends table, reports no open finding: it states the oracle of each of B29's rows, names tautologies, checks that the precedence is the R2 order (our stop, daemon facts, `OOMKilled`, 137, gateway signals, reader, trace gateway, collection, input leak, qualifier), and re-runs three bites from the leaf report plus one of its own, at most two of them e2e
**References**:
- [R01 §Ends, §Run store, §Process supervision, I10, I11, I17, I20](../../../__design__/architecture.md) — ends, the run directory, the stop ladder
- [R03 §5, §6, §7, §10.4, §13](../../../__reports__/m2b-execution-parity/00-architecture_v0.md) — sequences, stubs, the ends table and its precedence, destroy timing (R1)
- [R04 B29, B30, the B29 pre-drafted rows](../../../__reports__/m2b-execution-parity/01-test_definition_v0.md) — E01–E18, oracles and bites
- [M1 store and supervise](../../m1-contracts/README.md) — `RunWriter`, `Store::{next_attempt, seal_interrupted}`, `SpawnRequestBuilder`, `OutputSpec`, `FrameObserver`
- [R06 feature map](../feature_map/README.md) — `run_sealed`, `run_measured`, `stderr_kept`, `stream_wellformed`

## Step 1: Spawn with a live observer and the stop hook
**Goal**: One attempt starts in its container under a slot, with its output framed and observed while it runs.
**Implementation Logic**:
1. `exec_run(rt: &Runtime, plan: &RunPlan, slot: &Slot) -> RunOutcome` (the S3 stub's async form returns a boxed future with `'a` on self and every borrowed argument) creates the run directory with `RunWriter::create(&Store, &RunManifest, Existing::Refuse)`, calls `Isolation::plan(&Cell, &Workspace)`, writes the driver's `Prepared` files (plus the skill tree: the core runs `skill_archive_command(&TreatmentRef, request.treatment_source)` through `Supervisor::run_captured` (D1) and passes the bytes to `skill_files`), and builds the request with `Isolation::run_request(&ContainerPlan, CommandSpec)`.
2. The spawn uses `Supervisor::spawn(SpawnRequest)` with `OutputSpec::Framed {dir, stream, mode: Lines, observer}` for stdout and for stderr (the stderr file is kept), the observer being the driver's live reader through a `FrameObserver`, and `SpawnRequestBuilder::stop_hook(Arc<dyn StopHook>)` with a clone of an `Arc<ContainerStopHook>` kept in `exec_run` so `final_inspect()` is readable after `wait` (R7). The per-run supervisor deadline is `wall_s`.
3. The slot is held for the whole attempt and released on every path, including an error before the spawn.
4. A fixed timeline of emitted events: `run.started` before the spawn, the live-reader events as frames arrive.
5. Class A in `#[cfg(test)] mod scaffold`: spawn wiring over the PATH-fake `docker` and recording fakes (the order of the calls, the slot released after an error).
**Deliverables**: `crates/candler-core/src/runner/exec/spawn.rs` (`exec_run`, `start_attempt`, `LiveObserver`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'test(scaffold)'` (expected: PASS)
**Commit**: `feat(runner): spawn a run in its container with a live observer`

## Step 2: Classify the end
**Goal**: `classify_end` takes the first matching row of the ends table in the R2 order.
**Implementation Logic**:
1. `classify_end(input: &EndInput) -> End` is a pure function. `EndInput` holds: the ladder's `Ended` (`Exited`, `Deadline`, `Stopped`), the stop's recorded cause (`budget_stop`, `killed` or none), `Option<Inspect>` (from `ContainerStopHook::final_inspect()` on the hook path, or the run's own inspect on a normal exit, or none), the gateway log or probe result, the driver's `classify_exit` result, the reader's outcome (sessions, `format_unknown`, `reader_error`), the collection result, and the input-leak result.
2. The order, with `Ended::Deadline` counted as our act in steps 2 and 4: (1) our stop with cause `budget_stop` or `killed`; (2) daemon facts (exit 125, never started, inspect missing) only with no stop of ours and no deadline, `invalid(infrastructure)`; with a deadline and no inspect the end is the wall cap and the missing inspect is recorded; (3) `OOMKilled`, `invalid(infrastructure)`, never retried; (4) exit 137 with no stop of ours and no deadline, `invalid(infrastructure)`; (5) gateway signals, `invalid(gateway)`; (6) `format_unknown`, then `reader_error`; (7) a trace with 0 tokens or an error result carrying a provider status, `invalid(gateway)`; (8) a collection error, `invalid(collection_error)`; (9) an input leak, `invalid(input_leak)`; (10) for a valid run, the wall cap (`Ended::Deadline` or exit 124) gives `cap{wall}`, then the trace's turn or cost cap, agent error, completed; a silent trace with no stop of ours and no deadline is `invalid(infrastructure)`.
3. `RunOutcome {key, status, ended_by, invalid, inspect}` carries the inspect fact, so an OOM or a missing inspect stays on the record while `ended_by` is `unknown`. Raw files are kept in every invalid case.
4. The rows E01–E18 of R04 live in a data file `tests/oracle/vectors/ends_table.toml` (D5: the lead writes it in S3, E01–E18 in the R2 order; this leaf adds rows, never edits them, and does not create the file).
5. Class A: the order of the checks over a hand-built `EndInput` per precedence pair (a stop with gateway noise, a deadline with a missing inspect, `OOMKilled` with 137).
**Deliverables**: `crates/candler-core/src/runner/exec/ends.rs` (`classify_end`, `EndInput`, `End`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'test(scaffold)'` (expected: PASS)
**Commit**: `feat(runner): classify the end of a run in the R2 order`

## Step 3: Gateway retry, collect and guarded destroy
**Goal**: A gateway failure is retried once as a new attempt, and the container is inspected, collected from and destroyed in the order R1 fixes.
**Implementation Logic**:
1. After `wait`, on `Ended::Exited`: inspect the run's container, collect through `Isolation::collect_and_destroy(&ContainerPlan, &Workspace, &Exit)`, which destroys only through the guard; on the hook path (`Ended::Deadline` or `Ended::Stopped`) the hook already destroyed it and `final_inspect()` supplies the facts (R1).
2. The reader normalises the raw streams (`normalise`), `census` and `input_leak` (`run_evidence`) run on the collected output, and `classify_end` takes all of it.
3. An end of `invalid(gateway)` on a first attempt returns a retry request: the caller (`run_plan` and the job) opens attempt `n+1` with `Store::next_attempt`; a second gateway failure is final (E06). `agent_error`, `infrastructure`, `budget_stop`, `killed` and `interrupted` never retry.
4. A failed collection is `collection_error`; the raw files are kept.
5. Class A: the retry rule over (first attempt, second attempt) pairs, and the call order inspect, collect, destroy.
**Deliverables**: `crates/candler-core/src/runner/exec/collect.rs` (`finish_attempt`, `RetryDecision`, `retry_decision`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'test(scaffold)'` (expected: PASS)
**Commit**: `feat(runner): retry a gateway failure once and collect after exit`

## Step 4: Seal the run directory
**Goal**: The run is sealed in the one order where `run.done` is last, and a reader never sees a half-sealed run as done.
**Implementation Logic**:
1. `seal_run(RunWriter, &RunManifest, &RunTree, &[RawStreamFile], &TraceFile) -> Result<(), StoreError>` calls in this order: `manifest` (the final manifest with the `measured[]` and `integrity`), `tree`, `raw` for each stream (the digest checked by the store), `trace`, and `done()` last (`RunWriter::done(self)`).
2. The sealed layout is R01's: `runs/<key>/manifest.json`, `run.done`, `raw/<stream>.jsonl.gz`, `trace.jsonl`, `tree.json`; no other file and no absolute path.
3. A failure before `done()` leaves the directory unsealed, and it is sealed `interrupted` only through `Store::seal_interrupted` by `run_plan` or `recover`, never by this code. `exec_run` never deletes a partial run.
4. Class A: the path set of a sealed fixture and the step-k interruption walk with a fake writer.
**Deliverables**: `crates/candler-core/src/runner/exec/seal.rs` (`seal_run`, `SealStep`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'test(scaffold)'` (expected: PASS)
**Commit**: `feat(runner): seal a run with run.done last`

## Step 5: B29 and B30 guards, scaffold deletion and bite record
**Goal**: The ends table and the seal order are held by two guards with independent oracles, and no scaffolding stays.
**Implementation Logic**:
1. `tests/guard_run_exec.rs` (`#![cfg(test)]`). B29 `b29_ends_table`: for each row E01–E18 of `tests/oracle/vectors/ends_table.toml`, builds the `EndInput` from a recorded stream (M1's mini-corpus and `candler-harness-claude-code/tests/fixtures/streams`, read by relative path), one of the three recorded inspect JSONs and the supervisor's own act, and asserts the expected end (status, `ended_by`, invalid reason, retry), with R01 §Ends as the source sentence in each row. Doc lines: `Guards: I10, I20 — the ends table in the R2 order`, `Oracle: recorded streams, three recorded inspect JSONs, R01 §Ends`, `Bite: swap the OOMKilled and ExitCode == 137 checks`.
2. B30 `b30_sealed_layout_and_done_last`: runs the seal with a store in a temp root, stops after each step k and asserts `run.done` is absent until the last step; the sealed path set equals the literal list from R01 §Run store, and `assert_clean(dir, <temp root>)` finds no absolute path. Positive control: a complete seal has `run.done`. Doc lines: `Guards: I11, I17 — the sealed run dir`, `Oracle: the path list in R01 §Run store`, `Bite: write run.done before tree.json`.
3. Delete every `#[cfg(test)] mod scaffold` and `tests/scaffold_*`. Try each mutation, restore with `git checkout -- <file>`, and write the bite record in the leaf report.
**Deliverables**: `crates/candler-core/tests/guard_run_exec.rs` (`b29_ends_table`, `b30_sealed_layout_and_done_last`); the leaf report's bite record
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_run_exec)' && ! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/runner/exec crates/candler-core/tests/guard_run_exec.rs` (expected: PASS)
**Commit**: `test(runner): guard the ends table and the seal order`
