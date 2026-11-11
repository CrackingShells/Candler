# Digest invariance

**Goal**: The finished branch is measured against a26fe53: no run key, cell digest, context digest or suite-entry digest of the golden and mini imports moves, no M2a parity fixture digest moves, no M2c smoke cell digest moves, and the recorded Claude Code version string is unchanged.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The golden corpus is in place: `test -d /Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`
- [ ] [nl] OWNED: `__reports__/m4-session-runner/digest_invariance_v0.md`; scratch stores and builds under `/private/tmp/claude-501/m4-digest/`, removed at the end
**Success Gates**:
- ⬜ [cmd] DIGEST and GOLDEN pass on the milestone tip
- ⬜ [nl] The report records, for the golden import (240 runs, 10,320 records) and the mini import (6 runs, 136 records), imported by a26fe53's binary and by the branch's into separate scratch stores: 0 moved run keys, cell digests, context digests and suite-entry digests, and every manifest difference listed with its cause
- ⬜ [nl] The report records the M2c smoke experiment (`__reports__/m2b-execution-parity/smoke_v2/`) resolved by both binaries with the same image: 0 moved cell digests, and the same `harness.version` read from the image
- ⬜ [nl] The report records that the oracle vectors of the M2a parity fixtures are unedited (DIGEST's numstat) and their guards pass
- ⬜ [nl] The `digest_invariance` verifier closes with no open finding: it attacks the method (a comparison that could not fail, two stores sharing a cache, an image rebuilt between the two resolutions, a digest computed by the same code on both sides) and every listed manifest difference
- ⬜ [cmd] SUBJ passes, and `/private/tmp/claude-501/m4-digest` is gone
**References**: [M2c's method](../../../../../../../__reports__/m2c-wire-alignment/digest_invariance_v1.md) — main against the branch, separate stores; [R01 §Run manifest, §Golden corpus](../../../../../../../__design__/architecture.md); [R09 Gotchas, Digests](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-approved-plan.md)

## Step 1: Measure main against the branch
**Goal**: Every digest the milestone must not move is computed twice, by two binaries, and compared.
**Implementation Logic**:
1. Build a26fe53's `candler` (a scratch worktree, its own target dir) and the branch's.
2. Import the golden and mini corpora with each into its own scratch store; list run keys, cell, context and suite-entry digests per store and diff them.
3. Resolve the M2c smoke experiment with each binary against the same base and env images (built once, ids recorded), without running an agent; diff the cell digests and `harness.version`.
4. Write the report (`writing-reports`, findings): the method, the counts, every difference with its cause, 0 moves or the moved digests as a defect. Remove the scratch worktree, stores, builds and any image the leaf built, by id.
**Deliverables**: `__reports__/m4-session-runner/digest_invariance_v0.md`
**Consistency Checks**: `grep -q '10,320' __reports__/m4-session-runner/digest_invariance_v0.md && grep -q '136' __reports__/m4-session-runner/digest_invariance_v0.md` (expected: PASS)
**Commit**: `reports(runner): measure the digests on main and the branch`
