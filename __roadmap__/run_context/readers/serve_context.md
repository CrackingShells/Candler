# Serve the run

**Goal**: A kind that declares `needs.trace`, `needs.raw` or `needs.manifest` is handed exactly the stored bytes of that part, copied into its private workspace, read-only, with the digest in its context and in the context digest; a run that lacks the part is refused with a code, never served empty. Every stored file reaches the workspace through one copy routine: `materialise` has one routine that copies a stored file given its source and expected digest, and `Assembled` carries one list of `(stored source, workspace path, digest)` instead of a field per part (critical path)
**Estimate**: Sonnet, $12, cap $24. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use the target dir the campaign README names; the leaf removes it at its merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/run-context HEAD` succeeds
- [ ] `[nl]` `../schema_exit_needs.md` is `done`
**Success Gates**:
- ⬜ In `guard_run_context.rs`, a kind declaring `trace`, `raw` and `manifest` over a sealed run gets `context.trace`, `context.raw[]` and `context.manifest` whose files hash to the sha256 the test takes from the store's own paths (`runs/<key>/trace.jsonl`, `raw/<stream>.jsonl.gz`, `manifest.json`), with mode `0444` read back, and whose `sha256` fields equal those hashes. On the base `needs.trace` is silently ignored and the guard fails. Bites: (a) restore the silent ignore; (b) a hard link instead of a copy
- ⬜ `a_plug_in_cannot_write_through_to_the_store` extends to the trace, raw streams and manifest: the store's bytes are the same after a careless plug-in writes to its copies. Bite: a hard link instead of a copy
- ⬜ `the_live_context_digest_is_the_independent_vector` keeps every existing row unchanged and green, and new rows for contexts that serve a trace, raw streams or a manifest, written by `gen_live_context.py` over python `rfc8785`, pass. Bites: (a) the trace key added even when not served (old rows go red); (b) left out when served (new rows go red)
- ⬜ A kind that declares a part the run does not hold is refused with a new `ContextError` variant and a code in `docs/contract/codes.toml`, guarded by a row in `needs_inputs_refused_when_the_run_cannot_serve_them`'s pattern or `guard_silent_acceptance.rs`'s table, with the code written by hand. If the leaf finds that every sealed run holds a trace, raw streams and a manifest by construction (imported runs included), there is nothing to refuse for that part: the report says why, with the producer that guarantees it, and no guard is written for it
- ⬜ The needs the engine folds across a plug-in's kinds (`crates/candler-core/src/evaluation/engine/run.rs`, which unions them field by field) carry `raw` and `manifest`, so a part one kind declares is never dropped before `assemble`; the serving guard declares each part through that path
- ⬜ The existing `guard_run_context.rs` guards (artifacts, inputs, corrupt blob, isolation, write-through) pass unchanged after the refactor; that is the refactor's guard, and the leaf adds no test of its own for it. `materialise.rs` has one copy routine, and the report shows its call sites
- ⬜ `assemble.rs`'s doc states what is served and refused, with no history; a note `__reports__/run-context/01-ruling_trace_served_v0.md` records that the M2a ruling "trace not served" (`__reports__/m2a-evaluation-parity/l1/00-rulings_v0.md:75`) is replaced, and why
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass with the target dir the campaign README names; `container-e2e` is green on the pull request
**References**: [plan §Each need, §Wire shape of a served file, §Test plan](../../../__reports__/run-context/00-plan_run_context_v0.md). Code:
- `crates/candler-core/src/evaluation/context/{assemble,materialise,mod}.rs`: `assemble`, `Assembled`, `copy_artifact`, `materialise`, `ContextError`;
- `crates/candler-core/src/store/{layout,run}.rs`: the run's stored paths, `RunWriter::done`;
- `crates/candler-core/tests/it/guard_run_context.rs`, `guard_silent_acceptance.rs`;
- `crates/candler-core/tests/oracle/vectors/{gen_live_context.py,live_context_runs.toml}`;
- `docs/contract/codes.toml`.

## Step 1: Guards on the base
**Goal**: The serving, write-through, digest and refusal guards are written first and seen red on the base.
**Implementation Logic**:
Runs come from the mini-corpus import `guard_run_context.rs` already uses, or from a run under the scripted agent; no test builds a run or a trace by hand. Write the serving guard and extend write-through to the new parts. Extend `gen_live_context.py` with the new contexts and add their rows without regenerating the existing ones. Write the refusal row once Step 2's reading of the store says which parts can be missing; until then, record in the report which import paths store no trace. Run every guard on the base and record each failing message.
**Deliverables**: `guard_run_context.rs`; `gen_live_context.py` and `live_context_runs.toml`
**Consistency Checks**: on the base, every new guard fails and every existing digest row passes (expected: PASS)
**Commit**: `test(evaluation): state the serving of the trace, raw streams and manifest`

## Step 2: One copy path, every part
**Goal**: The guards pass, and every stored file goes through one routine.
**Implementation Logic**:
Turn `copy_artifact` into the routine that copies any stored file into the workspace given its source path and expected digest (create-new, `0600`, digest check, then `0444` or `0555` by the execute bit), and route artifacts and inputs through it unchanged in behaviour. Replace `Assembled`'s per-part fields with one list of `(stored source, workspace path, digest)`. In `assemble`, add the trace, each raw stream and the manifest to that list when declared, hashing each once (the store records no digest for them); refuse a declared part the run lacks with the new `ContextError` variant and its code. Fill `context.trace`, `context.raw[]` and `context.manifest` from the list. Rewrite `assemble.rs`'s doc and write the ruling note.
**Deliverables**: `crates/candler-core/src/evaluation/context/{assemble,materialise,mod}.rs`; `docs/contract/codes.toml`; `__reports__/run-context/01-ruling_trace_served_v0.md`
**Consistency Checks**: every `guard_run_context.rs` guard on the tip (expected: PASS)
**Commit**: `feat(evaluation): serve a run's trace, raw streams and manifest to a kind that declares them`

## Step 3: Bite and verify
**Goal**: Each guard goes red under its bite.
**Implementation Logic**:
On a scratch branch apply each named bite one at a time and record each failing message. Delete every `confirm_` test. A fresh Sonnet verifier gets the leaf, the report and the diff, and is asked to find a path by which a plug-in reads or writes a stored file other than through the one copy routine, a part served without its digest in the context digest, or a declared part served empty.
**Deliverables**: `__reports__/run-context/leaves/serve_context_v0.md`
**Consistency Checks**: `git grep -n confirm_ -- crates python` prints nothing (expected: PASS)
**Commit**: `reports(evaluation): close the serving of the stored run`
