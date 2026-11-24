# Sealed run producer

**Goal**: Sealed runs have one test producer, Candler's own runner under a scripted agent, and the guards on S1's path get their runs from it (P1, cause D1).
**Estimate**: Sonnet, $10 with its probe, cap $20. At the cap, stop, commit what is green, and list what is left in the leaf report.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/integrity HEAD` succeeds
**Success Gates**:
- ⬜ With the pre-S1 sealing (digests without blobs) reapplied on a scratch branch, the guards that read outputs fail, because their runs come from the runner; on the base, the same revert left them green
- ⬜ The probe's wall times (one analyze guard on the real runner, and a scripted native run) are in the leaf report, for the dogfooding pass's caps
- ⬜ No guard on S1's path calls `seal_synthetic`; it is deleted if no caller is left, and no new hand-made seeder is added
- ⬜ No digest or golden vector is edited
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with `CARGO_TARGET_DIR=/Users/hacker/Documents/tmp/claude-worktrees/Candler/candler-diet/target`, and `container-e2e` is green on the pull request
**References**: [Cycle plan v2 §D1, §Proof](../../../__reports__/diet/04-architecture_cycle_plan_v2.md) — the mechanism, the "who produces this state" table and the observation

## Step 1: Probe the real runner
**Goal**: Know whether the real native runner is fast enough to produce the analyze-heavy guards' state.
**Implementation Logic**:
Take one analyze guard that seals its runs by hand today (for example in `guard_analyze.rs`) and produce its two-arm store with a native scripted experiment, a scripted agent and a scripted evaluator, through Candler's own runner. Time it against the hand-made version. If the runner is too slow for the analyze-heavy guards, the fallback is a single seeder that seals through the runner's own sealing step, with tree and blobs; record the choice and the times.
**Deliverables**: `__reports__/diet/leaves/sealed_run_producer_v0.md` (§Probe: times, choice)
**Consistency Checks**: the probed guard passes on its new store (expected: PASS)
**Commit**: `reports(api): record the sealed-run producer probe`

## Step 2: Build the producer
**Goal**: One support module produces sealed runs the way real use does.
**Implementation Logic**:
In `candler-testkit` or the `it` support, as the probe decides, add the producer: it runs the native runner with a scripted agent and an optional scripted evaluator, and returns the store and run keys. It is the one place tests get Candler state from. Keep it direct: a function or two, no trait or builder layer it does not need.
**Deliverables**: the producer module and its public functions, named in the leaf report
**Consistency Checks**: `cargo nextest run --workspace` (expected: PASS)
**Commit**: `test(runner): add one test producer of sealed runs`

## Step 3: Move the guards on S1's path
**Goal**: The guards that read outputs see what the runner seals.
**Implementation Logic**:
Find every guard that reads outputs from a run made by `seal_synthetic` (colgrep: callers of `seal_synthetic`, readers of run outputs) and give it a run from the producer. The other seeders and fakes stay until someone next touches them, for their own reasons. Delete `seal_synthetic` if no caller is left.
**Deliverables**: the moved guards; `crates/candler-core/tests/it/guard_run_context.rs` without the S1-path uses of `seal_synthetic`
**Consistency Checks**: `cargo nextest run --workspace` (expected: PASS)
**Commit**: `test(runner): take the output-reading guards' runs from the runner`

## Step 4: Bite and verify
**Goal**: The moved guards catch S1's mechanism, and an agent that did not write them agrees.
**Implementation Logic**:
On a scratch branch from the base, reapply the pre-S1 sealing and run the guards: they stay green, which is S1's mechanism. On a scratch branch from the tip, do the same: they fail. Record both outputs. Then hand a fresh Sonnet verifier this leaf, the leaf report and the diff, with the brief: find a guard on S1's path that still makes its own sealed run.
**Deliverables**: `__reports__/diet/leaves/sealed_run_producer_v0.md` (§Bites, §Verifier, §Follow-ups)
**Consistency Checks**: the pre-S1 sealing on the tip makes the moved guards fail (expected: FAIL)
**Commit**: `reports(api): close the sealed-run producer`
