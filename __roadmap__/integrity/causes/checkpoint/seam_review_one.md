# Seam review 1

**Goal**: A reviewer who wrote none of level 1 reads its whole merged diff across the seams, and level 1's measured spend decides whether P5 runs (D3 remedy).
**Estimate**: one fresh Claude reviewer, $4, cap $8, plus the coordinator's spend measurement. At the cap, stop, commit what is green, and list what is left.
**Pre-conditions**:
- [ ] `[nl]` Level 1 (`sealed_run_producer`, `collect_oracle`, `one_reader`, `kill_off_tty`) is merged into `feat/integrity`
**Success Gates**:
- ⬜ The review answers, with file and line: does any test still produce Candler state by hand on a path a consumer uses; does any input still have a second reader; did a layer creep in between an input and its use; does each leaf's choice follow or break the `AGENTS.md` paragraph
- ⬜ Each finding is fixed on `feat/integrity` with its own guard seen failing, or recorded with its reason
- ⬜ Level 1's spend, measured from transcripts, is set against its caps, and the decision on P5 is recorded: over the caps, P5 waits with option (b) recorded; otherwise it runs
**References**: [Cycle plan v2 §How this work is developed, §Cost](../../../../__reports__/diet/04-architecture_cycle_plan_v2.md) — the review questions and the checkpoint rule; [README.md](../../../../README.md) — the purpose paragraph

## Step 1: Review the seams
**Goal**: The seams between level 1's leaves are read by someone with the whole picture.
**Implementation Logic**:
Hand a fresh Claude reviewer plan v2, the README purpose paragraph and `git diff origin/main...feat/integrity`, not one leaf. It reports findings against the four questions; it does not edit. Fixes go through a short leaf-shaped change on `feat/integrity`, each with its guard seen failing.
**Deliverables**: `__reports__/diet/leaves/seam_review_one_v0.md` (§Findings, §Fixes)
**Consistency Checks**: `cargo nextest run --workspace` on the `feat/integrity` tip after the fixes (expected: PASS)
**Commit**: `reports(api): review the seams of level 1`

## Step 2: Spend checkpoint
**Goal**: P5 is decided from measured spend, before level 2 starts.
**Implementation Logic**:
Measure each level 1 row's spend from its transcripts (not from usage notes) and compare it with the caps. Record the decision on P5 and the spend so far against the $100. If P5 waits, mark `snapshot/catalog_snapshot.md` `blocked` with the reason.
**Deliverables**: `__reports__/diet/leaves/seam_review_one_v0.md` (§Checkpoint)
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.3.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/integrity/causes/checkpoint/` (expected: PASS)
**Commit**: `reports(api): decide the catalog snapshot at the spend checkpoint`
