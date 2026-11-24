# Seam review 3

**Goal**: One review across the level's pull requests finds what no leaf verifier was placed to see, starting with what the level left behind, and the knowledge-transfer report records the level.
**Estimate**: Sonnet, $3, cap $6. At the cap, stop and list what is left.
**Pre-conditions**:
- [ ] `[nl]` `../snapshot_out.md` and `../parity_run.md` are `done`
**Success Gates**:
- ⬜ The review answers three questions first: did any leaf add where it should have replaced (a second producer, a second reader, a passthrough, a shim with no reader); does every setting that reaches the agent have one answer per driver and one statement on the pages; does one cell on both harnesses install the same servers and trace each MCP call in the same form
- ⬜ The report records, as an open question for the coordinator to take to the maintainer, where `tool-catalog-snapshot/1` should live long term, now that Candler neither writes nor reads it. The level does not decide it
- ⬜ Each finding names its cause, the leaves it crosses, and why no leaf verifier would have seen it
- ⬜ The level's spend is measured from transcripts per row against the plan's estimates and caps
- ⬜ `__reports__/diet/06-knowledge_transfer_integrity_v0.md` gains a level-3 section, or a v1 supersedes it, with the findings, the spend and what comes next
**References**: [Level-3 plan](../../../../../../../../../../../__reports__/diet/07-plan_integrity_level3_v1.md); the leaf reports `__reports__/diet/leaves/{declaration,mcp_render,trace_parity,pages,snapshot_out,parity_run}_v0.md`

## Step 1: Review across the level
**Goal**: The seams of level 3 are reviewed.
**Implementation Logic**:
A fresh Sonnet reviewer gets the level's merged diff on `feat/integrity`, the leaf reports and this leaf, with the three questions first. It reads code through colgrep only. Blocking findings go to the lead before any fix; non-blocking ones are recorded.
**Deliverables**: `__reports__/diet/leaves/seam_review_three_v0.md`
**Consistency Checks**: `git -C <worktree> log --oneline origin/feat/integrity -1` names the last level-3 merge (expected: PASS)
**Commit**: `reports(api): record seam review 3`

## Step 2: Record the level
**Goal**: The next milestone inherits level 3.
**Implementation Logic**:
Add to the knowledge-transfer report what removing the second catalog producer, folding the per-harness lists into one shape and aligning the trace looked like, what the review caught, the spend per row, and the follow-ups in order.
**Deliverables**: `__reports__/diet/06-knowledge_transfer_integrity_v0.md` or its v1
**Consistency Checks**: the report's spend table has one row per level-3 leaf (expected: PASS)
**Commit**: `reports(api): close M6c level 3`
