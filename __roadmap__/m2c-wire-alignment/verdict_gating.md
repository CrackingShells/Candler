# Verdict gating

**Goal**: The stratum verdict gates on the provenance that says whether two arms ran the same Candler: `eval_infra.commit` (recorded and equal), `eval_infra.dirty` (false in every arm) and `sandbox_contract` (equal). `eval_infra.tag`, `eval_infra.commits_ahead` and `provenance.packages.*` describe a run and gate nothing.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone: `git merge-base --is-ancestor feat/m2c-wire-alignment HEAD`
- [ ] [nl] OWNED: `crates/candler-core/src/manifest/compare.rs`, `crates/candler-core/tests/guard_comparability.rs`, `crates/candler-core/tests/fixtures/comparability/verdicts.json`
**Success Gates**:
- ⬜ [cmd] V1: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_comparability)'`, with one table row per D8 class:
  - unrecorded `commit` in every arm: refused, naming `provenance.eval_infra.commit` (this row is red on `main`);
  - unrecorded `dirty`: refused;
  - `dirty` true: refused;
  - differing `commit`: refused;
  - differing `sandbox_contract`: refused;
  - equal `commit`, differing `tag` or `commits_ahead`: comparable;
  - differing `provenance.packages.*`: comparable.
  Oracle: the rows of `verdicts.json`, written from D8 and not from the code
- ⬜ [cmd] `compare.rs` names no planner and no skill version: `! git grep -n -i -E 'planner|skill version|skill_version' -- crates/candler-core/src/manifest/compare.rs`
- ⬜ [cmd] STD (`-p candler-core`), LINUX and SUBJ pass
- ⬜ [nl] The leaf report records V1's bites: every `eval_infra.*` gating (the `tag` row turns red), and `leaf_key(None)` counted as equal (the `commit` row turns red)
- ⬜ [nl] The `verdict_gating` verifier closes with no open finding
**References**: [R10 D8, guard V1](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md); [R05 D3, E4](../../__reports__/candler-v1-review/00-findings_v0.md); [R06 KT "Intended deviations"](../../__reports__/m2b-execution-parity/04-knowledge_transfer_v0.md) — pooling with the golden runs is refused on `eval_infra`

## Step 1: Gate on identity, describe the rest
**Goal**: `provenance_refused` refuses exactly the D8 classes, and its doc says why each path gates or not.
**Implementation Logic**:
1. Write the new `verdicts.json` rows and see V1 red.
2. In `compare.rs`:
   - `gates_verdict` admits `provenance.eval_infra.commit`, `provenance.eval_infra.dirty` and `provenance.sandbox_contract` only;
   - `provenance_refused` refuses a stratum in which any arm lacks a recorded `commit` or `dirty`, any arm has `dirty` true, or a gated path differs. Keep the refusal's path naming, so the verdict says which path refused and why.
3. Reword the doc of `provenance_refused`, so that it names no planner and no skill version.
4. Update row C51 ("`skill_version` and `planner.*` are exempt") to the new D8 wording.
**Deliverables**: `crates/candler-core/src/manifest/compare.rs`: `gates_verdict`, `provenance_refused`; `crates/candler-core/tests/fixtures/comparability/verdicts.json`: the D8 rows; `crates/candler-core/tests/guard_comparability.rs` if the table reader needs a new column
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_comparability)'` (expected: PASS)
**Commit**: `feat(manifest): gate a stratum on the build's commit, not its tag`
