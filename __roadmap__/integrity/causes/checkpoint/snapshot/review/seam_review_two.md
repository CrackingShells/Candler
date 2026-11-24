# Seam review 2

**Goal**: Everything merged since seam review 1 is read across its seams before an outside consumer uses the tip (D3 remedy).
**Estimate**: one fresh Claude reviewer, $4, cap $8. At the cap, stop, commit what is green, and list what is left.
**Pre-conditions**:
- [ ] `[nl]` `../catalog_snapshot.md` is `done` or `blocked`, and every fix from seam review 1 is merged into `feat/integrity`
**Success Gates**:
- ⬜ The review asks seam review 1's questions of the diff since it, and whether the snapshot holds at its seam (resolve to runner to store to `inspect`); if P5 waited, it says so and reviews the fixes alone
- ⬜ Each finding is fixed on `feat/integrity` with its own guard seen failing, or recorded with its reason
**References**: [Cycle plan v2 §How this work is developed](../../../../../../__reports__/diet/04-architecture_cycle_plan_v2.md) — the review questions; [README.md](../../../../../../README.md) — the purpose paragraph

## Step 1: Review the seams
**Goal**: The level 2 seams are read by someone with the whole picture.
**Implementation Logic**:
Hand a fresh Claude reviewer plan v2, the README purpose paragraph, seam review 1's report and the diff since it. It reports findings; it does not edit. Fixes follow as in seam review 1.
**Deliverables**: `__reports__/diet/leaves/seam_review_two_v0.md`
**Consistency Checks**: `cargo nextest run --workspace` on the `feat/integrity` tip after the fixes (expected: PASS)
**Commit**: `reports(api): review the seams of level 2`
