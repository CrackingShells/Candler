# audit_fixes

**Goal**: Fix each README claim the audit found broken, when one capped piece can; move the rest to Next with their evidence.
**Estimate**: about 35 Sonnet calls ($1.2 with the verifier), cap 55. At 55 calls, stop, commit what is green, and list what is left in the leaf report.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/diet HEAD` succeeds, with every L0 piece merged into `feat/diet`
- [ ] `[nl]` OWNED = `named in the audit findings; listed in the leaf report before the first edit`
**Success Gates**:
- ⬜ Each broken claim from `01-findings_e2e_audit_v0.md` is fixed with a guard seen red first, or listed in Next with its evidence and why
- ⬜ The audit step that showed the claim broken, rerun, now shows it working
- ⬜ `cargo nextest run --workspace` passes
- ⬜ `cargo fmt --check` passes
- ⬜ `cargo clippy --workspace --all-targets -- -D warnings` passes
- ⬜ `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes
- ⬜ DIGEST unchanged: `cargo nextest run --workspace -E 'test(/cell_digest|context_digest|build_context/)'` passes with no vector edited
- ⬜ GOLDEN unchanged: `cargo nextest run --workspace -E 'test(/golden/)'` passes with no vector edited
- ⬜ Ownership: `git diff --name-only feat/diet...HEAD | grep -vE 'named in the audit findings; listed in the leaf report before the first edit'` prints nothing
- ⬜ A fresh Sonnet verifier, given only this leaf and the diff, reports no promise left without a test and no gate unmet
**References**: [Diet plan](../../../../__reports__/diet/00-architecture_diet_plan_v0.md) — §Decisions to approve (K6) and the audit findings report; [AGENTS.md](../../../../AGENTS.md) — checks and conventions

## Step 1: Choose what to fix
**Goal**: Only defects that break a README claim, within the cap.
**Implementation Logic**:
Read `__reports__/diet/01-findings_e2e_audit_v0.md`. For each broken claim, estimate calls; take the ones that fit in the cap together, cheapest proof first. Write the owned files for each before editing. If no claim is broken, mark this leaf done with a one-line report.
**Deliverables**: the leaf report's plan table (claim, fix, owned files, estimate).
**Consistency Checks**: the plan fits the cap (expected: PASS)
**Commit**: `reports(api): plan the audit fixes`

## Step 2: Fix with a guard first
**Goal**: Each fix is held by a guard at the consumer interface.
**Implementation Logic**:
For each chosen claim, write the guard, see it red, fix at the one owner, see it green, rerun the audit step on RiVault (one run, the one that showed the defect). Make one commit per claim, with the cog scope of the area it touches.
**Deliverables**: the fixes and their guards.
**Consistency Checks**: `cargo nextest run --workspace` (expected: PASS)
**Commit**: `fix(api): restore the broken readme claim`

## Step 3: Verify
**Goal**: An agent that did not write the change attacks it.
**Implementation Logic**:
Hand a fresh Sonnet verifier this leaf, the leaf report and `git diff feat/diet...HEAD`, with the brief: check each fixed claim against the audit evidence and each moved claim's reason. Fix what it finds with a fresh agent, once; list the rest as follow-ups.
**Deliverables**: `__reports__/diet/leaves/audit_fixes_v0.md` with First reads, Bites, Verifier and Follow-ups.
**Consistency Checks**: the Success Gates above (expected: PASS)
**Commit**: `reports(api): close audit_fixes`
