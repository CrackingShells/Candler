# Adversarial review

**Goal**: The whole `feat/run-context` diff is judged by a reviewer who built none of it, against the README purpose paragraph quoted verbatim, the codebase's idioms (AGENTS.md: one producer per stored state, one reader per input that refuses what it does not understand; Grit; source files say what is), and waste at the three levels (how it was developed, what it leaves in the code, how it is consumed); every finding is fixed or ruled on before the integration pull request is proposed
**Estimate**: Sonnet, $5, cap $10, review only; fixes are dispatched as their own work and costed apart. At the cap, stop and report what was read and what was not. The review builds nothing; a fix that builds uses the target dir the campaign README names and removes it at its merge.
**Pre-conditions**:
- [ ] `[cmd]` `git merge-base --is-ancestor feat/run-context HEAD` succeeds in the reviewer's checkout
- [ ] `[nl]` `../graded_run_sees_trace.md` is `done` and merged into `feat/run-context`
**Success Gates**:
- ⬜ A findings report `__reports__/run-context/03-review_adversarial_v0.md` covers the whole diff `origin/main...feat/run-context`. For each finding it gives the file and line, the failure it would cause, and its class (defect, idiom, waste). It answers at least: is any stored part served by a second path; does any reader accept what the schema refuses; does the context digest change for a context serving none of the new parts; does any doc carry history or plan references; did the refactor leave `materialise` simpler, judged by its call sites, not its line count
- ⬜ Every finding is fixed on `feat/run-context` (the fix's commit cited) or ruled on by the coordinator with the reason recorded in the report; a finding that needs the maintainer goes to them before the pull request
- ⬜ After the fixes, the project's checks pass on the tip and `container-e2e` is green on the integration pull request draft
**References**: [README.md](../../../../../README.md) purpose paragraph; [AGENTS.md](../../../../../AGENTS.md); [plan](../../../../../__reports__/run-context/00-plan_run_context_v0.md); every leaf report under `__reports__/run-context/leaves/`.

## Step 1: Review
**Goal**: The findings report exists.
**Implementation Logic**:
A fresh Sonnet reviewer gets the README paragraph, AGENTS.md, the plan, and the diff; not the leaf reports' conclusions until it has read the diff once. It reads code through colgrep with absolute paths in its own checkout of `feat/run-context`. It writes findings, not fixes.
**Deliverables**: `__reports__/run-context/03-review_adversarial_v0.md`
**Consistency Checks**: the report lists every file in `git diff --stat origin/main...feat/run-context` as read or says why not (expected: PASS)
**Commit**: `reports(evaluation): review the run context campaign adversarially`

## Step 2: Fix or rule
**Goal**: No finding is left open.
**Implementation Logic**:
The coordinator dispatches each fix to an implementer on its own worktree and merges it as any leaf; a finding not fixed gets a ruling in the report. Rerun the checks and `container-e2e`.
**Deliverables**: the fixes; the rulings in `03-review_adversarial_v0.md`
**Consistency Checks**: `cargo nextest run --workspace` on the tip (expected: PASS)
**Commit**: `reports(evaluation): close the adversarial review`
