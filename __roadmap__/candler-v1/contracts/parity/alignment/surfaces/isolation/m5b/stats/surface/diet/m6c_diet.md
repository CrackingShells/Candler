# M6c Architecture diet

**Goal**: Before M7 puts an MCP server over it, Candler does what its README purpose paragraph says, shown end to end, and carries no test or code the next agent would not want to inherit. S1 (a native run seals the digests of its outputs but not their bytes, so `evaluate` cannot grade it) is fixed at the seal with the one guard that would have caught it (amendment A12).
**Pre-conditions**:
- [ ] `../m6b_surface_consolidation.md` is `done` (PR #56)
- [ ] The plan read: `__reports__/diet/00-architecture_diet_plan_v0.md`, with decisions K1 to K6 approved
**Success Gates**:
- ⬜ A run that writes a file is graded from its stored bytes: the S1 guard was seen failing on the M6b tip and passes
- ⬜ Each claim of the README purpose paragraph (as written in `254ff4a0`) has a verdict (works, broken, not built) from real RiVault runs whose stored bytes an agent read, in `__reports__/diet/01-findings_e2e_audit_v0.md`; each broken claim is fixed or in Next with its evidence
- ⬜ The suite holds guards at the six consumer interfaces: about 525 nextest tests from 836, each deletion listed with its covering guard or "structure", each merged or rewritten guard seen red once
- ⬜ The dead paths, duplicated machinery, second sources and silent acceptance the plan names are removed or refused, with no digest or golden vector moved
- ⬜ `README.md` opens with the purpose paragraph again (K3), and R01 records the test policy and the store layout
- ⬜ Spend is measured from transcripts and the whole diet stays within $100
**References**: [R01 §Contracts & Invariants (I11, I17), §Testing](../../../../../../../../../../../__design__/architecture.md) — append-only records, relative paths, the store layout; [AGENTS.md §Sustainable, scalable, efficient](../../../../../../../../../../../AGENTS.md) — the triad; [Diet plan](../../../../../../../../../../../__reports__/diet/00-architecture_diet_plan_v0.md) — evidence, pieces and cost

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/diet/` exists and its decisions are approved.
**Implementation Logic**:
The plan works from evidence: the S1 observation on `claude/htn-smoke-defects`, a family-by-family classification of every test file, a dead-and-duplicate sweep with file and line citations, and M6b's diet follow-ups. It answers M6b's open question (take only the rows that remove a second source or a silent acceptance). Each piece is about 25 to 40 tool calls in its own worktree, priced as calls times context times cache-read rate, with a cap written as a stop action.
**Deliverables**: `__reports__/diet/00-architecture_diet_plan_v0.md`; `__roadmap__/diet/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/diet/` (expected: PASS)
**Commit**: `rdm(api): add the architecture diet campaign`

## Step 2: Build and land

**Goal**: The campaign's four levels are built and merged into `feat/diet`, then into `main`.
**Implementation Logic**:
Run `__roadmap__/diet/` level by level: S1 and the seven culls in parallel (at most three building at once), then shed and audit, then audit fixes, then close out. Each leaf has a fresh verifier; each level closes with the workspace gates and a spend check against its caps.
**Deliverables**: the merged campaign; the leaf reports under `__reports__/diet/leaves/`
**Consistency Checks**: `cargo nextest run --workspace` (expected: PASS)
**Commit**: `merge(api): land the architecture diet`

## Step 3: Close out

**Goal**: The milestone is recorded.
**Implementation Logic**:
Write the knowledge-transfer report under `__reports__/diet/`. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/diet/02-knowledge_transfer_diet_v0.md`; this leaf's status in `__roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/m5b/stats/surface/diet/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/m5b/stats/surface/diet/` (expected: PASS)
**Commit**: `reports(api): close the architecture diet milestone`
