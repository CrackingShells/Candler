# Ground-truth refusal

**Goal**: Resolve refuses a cell when the bytes of an evaluator-only input sit in agent-bound bytes: an experiment's or plug-in's build context, a plug-in environment's agent inputs, or the treatment tree (D9). A resolve with no evaluator-only input does no extra work.
**Estimate**: $2.5 (Sonnet, one sitting)
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] OWNED: `crates/candler-core/src/resolve/steps/ground_truth.rs` (new) and its registration line in `resolve/steps/mod.rs`, the code `isolation.ground_truth_exposed` (its row in `docs/contract/codes.toml`, in `crates/candler-core/src/resolve/refusal.rs` and in the blessed `docs/contract/index.md`, which `contract_pages` checks together), a new `crates/candler-core/tests/it/guard_ground_truth_refusal.rs`. `refusal.rs` is also edited by `egress_attack` at L1: different level, so no race
**Success Gates**:
- ⬜ [cmd] The refusal rows pass, each refused with `isolation.ground_truth_exposed` naming the input and the place: an evaluator-only input's bytes in an experiment's build context, in a plug-in environment's context, in a plug-in environment's agent input, and in the treatment tree: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_ground_truth_refusal::refuses_/)'`
- ⬜ [cmd] No existing experiment is refused, and the contract pages agree with the new code: GOLDEN, DIGEST, `cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_input_seam::/) | test(/contract_pages/)'`
- ⬜ [cmd] The code is in the table: `grep -q isolation.ground_truth_exposed docs/contract/codes.toml docs/contract/index.md`
- ⬜ [cmd] SPAWN, STD (`-p candler-core -p candler`), LINUX and SUBJ pass (README, standard gates)
- ⬜ [nl] This newly refused input is reported to the coordinator before the merge, and the coordination session's ruling is recorded in the leaf report
- ⬜ [nl] The leaf report records each Bite and the residual: a renamed, compressed or transformed copy is not detected by digest. The step's guard against reading the evaluator-only input into agent-bound bytes is one `if`, read in review; no separate test is kept for it
**References**: [R04 D9 and its trigger](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md); [R03 Test design](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5b-approved-plan.md); [R01 §Contracts & Invariants, I8](../../__design__/architecture.md); [moved leaf](../m5-container-isolation/client/seams/egress/ground_truth_refusal.md) — source text

## Step 1: Refuse ground truth in agent-bound bytes
**Goal**: The walk compares the digests of evaluator-only inputs (`InputAudience::Evaluators`, `experiment/inputs.rs`) with the files of every agent-bound tree, and refuses on a match.
**Implementation Logic**:
1. Guards first, each seen red: `guard_ground_truth_refusal::refuses_*`, through resolve (the consumer is the eval author), one row per place: build context, plug-in context, plug-in agent input, treatment tree. Oracle: the canary's digest, computed independently in the test. Bite: plug-in agent inputs skipped.
2. Verify first how the build context is already walked (`build_inputs`) and how the treatment tree is read (`treatment:` sources, `SkillTree`). Re-use those walks; never walk twice.
3. Bound the cost: the step runs only when an evaluator-only input exists, and it reuses the digests the walks already compute.
4. Add the code `isolation.ground_truth_exposed` to `codes.toml`, `refusal.rs` and the blessed `index.md` (README, Contract pages gotcha). The refusal names the input and the place.
5. Report the newly refused input to the coordinator before the merge.
**Deliverables**: `crates/candler-core/src/resolve/steps/ground_truth.rs`: `GroundTruthStep`; `crates/candler-core/src/resolve/steps/mod.rs`: its registration; `docs/contract/codes.toml`, `crates/candler-core/src/resolve/refusal.rs`, `docs/contract/index.md`: `isolation.ground_truth_exposed`; `crates/candler-core/tests/it/guard_ground_truth_refusal.rs`: `refuses_*`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_ground_truth_refusal::/)'` (expected: PASS)
**Commit**: `feat(isolation): refuse ground truth in what the agent receives`
