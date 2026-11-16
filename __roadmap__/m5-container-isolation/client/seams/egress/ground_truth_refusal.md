# Ground-truth refusal

**Goal**: Resolve refuses a cell when the bytes of an evaluator-only input sit in agent-bound bytes: an experiment's or plug-in's build context, a plug-in environment's agent inputs, or the treatment tree (D9, N5). A resolve with no evaluator-only input does no extra work.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] OWNED: `crates/candler-core/src/resolve/steps/ground_truth.rs` (new) and its registration line, the code `isolation.ground_truth_exposed`, a new guard file
**Success Gates**:
- ⬜ [cmd] The refusal table passes, each row refused with `isolation.ground_truth_exposed` naming the input and the place: an evaluator-only input's bytes in an experiment's build context, in a plug-in environment's context, in a plug-in environment's agent input, and in the treatment tree: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_ground_truth_refusal::refuses_/)'`
- ⬜ [cmd] Without an evaluator-only input, nothing is hashed (a recording fake of the walk sees no file): `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_ground_truth_refusal::no_evaluator_input_/)'`
- ⬜ [cmd] No existing experiment is refused: GOLDEN, DIGEST, and `cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_input_seam::/)'`
- ⬜ [cmd] SPAWN, STD (`-p candler-core`), LINUX and SUBJ pass
- ⬜ [nl] This newly refused input is reported to the coordinator before the merge, and the coordination session's ruling is recorded
- ⬜ [nl] The leaf report records each bite. The residual is stated: a renamed, compressed or transformed copy is not detected by digest
**References**: [R06 D9 and its trigger](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md); [R01 §Contracts & Invariants, I8](../../../../../__design__/architecture.md)

## Step 1: Refuse ground truth in agent-bound bytes
**Goal**: The walk compares the digests of evaluator-only inputs with the files of every agent-bound tree, and refuses on a match.
**Implementation Logic**:
1. Guards first, seen red:
   - `refuses_*`: one row per place;
   - `no_evaluator_input_*`.

   Oracle: the canary's digest, computed independently in the test. Bite: the plug-in environment's inputs skipped.
2. Verify first how the build context is already walked (`build_inputs`) and how the treatment tree is read (`treatment:` sources, `SkillTree`). Re-use those walks; never walk twice.
3. Bound the cost: the step runs only when an evaluator-only input exists, and it reuses the digests the walks already compute.
**Deliverables**: `crates/candler-core/src/resolve/steps/ground_truth.rs`: `GroundTruthStep`; the code `isolation.ground_truth_exposed` in `docs/contract/codes.toml`; `crates/candler-core/tests/it/guard_ground_truth_refusal.rs`: `refuses_*`, `no_evaluator_input_*`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_ground_truth_refusal::/)'` (expected: PASS)
**Commit**: `feat(isolation): refuse ground truth in what the agent receives`
