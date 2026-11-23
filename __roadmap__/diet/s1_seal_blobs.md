# s1_seal_blobs

**Goal**: A sealed native or session run holds the bytes of every output it names, so `candler evaluate` grades what the agent wrote (defect S1).
**Estimate**: about 40 Opus calls ($2.3, $2.7 with the verifier), cap 60. At 60 calls, stop, commit what is green, and list what is left in the leaf report.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/diet HEAD` succeeds
- [ ] `[nl]` OWNED = `^crates/candler-core/src/runner/exec/|^crates/candler-core/src/store/|^crates/candler-core/tests/it/(guard_run_exec|guard_run_context|main)\.rs$|^crates/candler/tests/it/(support/cli_run|guard_input_seam|guard_run_evaluate|main)\.rs$|^__design__/architecture\.md$|^__reports__/diet/leaves/s1_seal_blobs`. `support/cli_run.rs` and `guard_input_seam.rs` belong to this piece only
**Success Gates**:
- ⬜ The new guard `a_run_that_writes_a_file_is_graded_from_its_stored_bytes` was seen failing on the `feat/diet` tip with "open a blob failed" and passes after the fix
- ⬜ After the seal, `blobs/<sha256>` exists for every file entry of `tree.json`, and `runs/<key>/workspace/` is gone (K2), unless step 1 found a reader
- ⬜ `guard_run_exec` b30 states the new seal step and why
- ⬜ `cargo nextest run -p candler-core -p candler` passes
- ⬜ `cargo fmt --check` passes
- ⬜ `cargo clippy --workspace --all-targets -- -D warnings` passes
- ⬜ `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes
- ⬜ DIGEST unchanged: `cargo nextest run --workspace -E 'test(/cell_digest|context_digest|build_context/)'` passes with no vector edited
- ⬜ GOLDEN unchanged: `cargo nextest run --workspace -E 'test(/golden/)'` passes with no vector edited
- ⬜ Ownership: `git diff --name-only feat/diet...HEAD | grep -vE '^crates/candler-core/src/runner/exec/|^crates/candler-core/src/store/|^crates/candler-core/tests/it/(guard_run_exec|guard_run_context|main)\.rs$|^crates/candler/tests/it/(support/cli_run|guard_input_seam|guard_run_evaluate|main)\.rs$|^__design__/architecture\.md$|^__reports__/diet/leaves/s1_seal_blobs'` prints nothing
- ⬜ A fresh Sonnet verifier, given only this leaf and the diff, reports no promise left without a test and no gate unmet
**References**: [Diet plan §Current state, §Proposed state, §Contracts](../../__reports__/diet/00-architecture_diet_plan_v0.md) — where S1 lives and the seal contract; [R01 store layout](../../__design__/architecture.md) — I11, I17 and the store-layout row; `claude/htn-smoke-defects:__reports__/htn-smoke-defects/00-observation_native_output_blobs_v0.md` — the observation

## Step 1: Write the guard and see it fail
**Goal**: The one end-to-end guard that would have caught S1.
**Implementation Logic**:
First check with colgrep (`run_dir(`, `"workspace"`, `workspace/`) that nothing reads `runs/<key>/workspace/` after the seal; if a reader exists, stop K2 and keep the directory. Then make the fake Docker in `support/cli_run.rs` write one file into the bind-mounted workspace on `start -a`, as an agent would. Add a guard at the CLI: `candler run` an experiment whose plug-in suite needs `artifacts`, then `candler evaluate`, and assert that the outcome is computed from the file's bytes. Run it on the tip and record the failure text.
**Deliverables**: `crates/candler/tests/it/support/cli_run.rs` (fake `start -a` writes a file); `crates/candler/tests/it/guard_run_evaluate.rs` with `a_run_that_writes_a_file_is_graded_from_its_stored_bytes`.
**Consistency Checks**: the new guard fails on the tip with "open a blob failed" (expected: FAIL); every other test in `guard_input_seam` and `guard_cli_evaluate` still passes with the writing fake (expected: PASS)
**Commit**: `test(runner): guard that a written output is graded from its stored bytes`

## Step 2: Store blobs at the seal
**Goal**: The seal is the one owner of output bytes.
**Implementation Logic**:
Give `seal_run_observed` the workspace root. Before it writes the manifest, store every file entry of the tree with `put_blob_from_path(path, Some(&sha256))`, so a changed file fails the seal and a run with `run.done` never names a missing blob. A halt between steps leaves only content-addressed blobs, which are not records (I11). Add the step to `SealStep` so the seal observer and b30 see it. After the seal, remove `workspace/` (K2). The session runner goes through the same `drive`, so it gets the fix with no second path. Update the R01 store-layout row: blobs for native and session outputs, no kept `workspace/`.
**Deliverables**: `crates/candler-core/src/runner/exec/seal.rs` (`seal_run`, `seal_run_observed`, `SealStep`); `crates/candler-core/src/runner/exec/mod.rs` (`drive` passes the workspace root, removes `workspace/`); `crates/candler-core/tests/it/guard_run_exec.rs` (b30); `__design__/architecture.md` (store layout).
**Consistency Checks**: the step 1 guard passes; `cargo nextest run -p candler-core -E 'test(/b30|seal|run_context/)'` (expected: PASS)
**Commit**: `fix(runner): store each output's bytes as a blob when the run is sealed`

## Step 3: Bite and verify
**Goal**: The guard catches the defect it was written for, and an agent that did not write it agrees.
**Implementation Logic**:
Bite: skip the blob step; see the step 1 guard red with the same text; revert. Bite: store a blob with the wrong digest; see the seal fail; revert. Then hand a fresh Sonnet verifier this leaf, the leaf report and the diff, with the brief: find a run path (native, session, resume) that seals without blobs, and any reader of `workspace/` left behind.
**Deliverables**: `__reports__/diet/leaves/s1_seal_blobs_v0.md` with First reads, Bites, Verifier and Follow-ups.
**Consistency Checks**: the Success Gates above (expected: PASS)
**Commit**: `reports(api): close the s1 seal fix`
