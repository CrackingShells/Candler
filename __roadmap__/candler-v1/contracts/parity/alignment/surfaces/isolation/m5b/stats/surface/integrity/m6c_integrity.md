# M6c Integrity

**Goal**: Before M7 puts an MCP server over it, the causes that produced S1 and B1 are gone from Candler, not only their instances: each kind of state Candler stores has one producer and that producer is Candler's own code (D1); each input a consumer writes has one reader, below every surface (D5); milestones are cut by concept, gated by observations and reviewed across seams (D3); and `AGENTS.md` says what sane work means in Candler (D4). The diet's first half (S1, B1, three silent acceptances, the cull) landed as PR #57 (amendment A12); amendment A13 renamed the milestone Integrity and rewrote this leaf.
**Pre-conditions**:
- [x] `../m6b_surface_consolidation.md` is `done` (PR #56)
- [x] The diet's first half landed on `main` as PR #57 (`70d4c3bf`)
- [x] The plan read: `__reports__/diet/04-architecture_cycle_plan_v2.md`, with Eliott's rulings applied and the cut order accepted
**Success Gates**:
- ⬜ Sealed runs have one test producer: with the pre-S1 sealing reapplied on a scratch branch, the guards that read outputs fail; on the base the same revert left them green
- ⬜ Collect has an oracle outside the driver: with B1's fix reverted on a scratch branch, the shared conformance case fails for OpenCode only, naming the files the agent wrote
- ⬜ One reader per input: an experiment with a malformed MCP server is refused by `resolve` with the field path, with the same record from the CLI and the op API, where the base ran it with the server silently absent; the diff adds no trait, wrapper type or module between an input and its use
- ⬜ The `AGENTS.md` section "Sustainable, scalable, efficient" is replaced by the approved paragraph, and the leaf template carries the conventions of D3
- ⬜ The three seam reviews ran, each on its levels' whole diff, and their findings are resolved or recorded
- ⬜ `inspect` shows the census and the resolved `mcp.*` values a sealed run ran with, and no catalog snapshot that Candler did not observe
- ⬜ The dogfooding pass ran on the after tip with two RiVault models under OpenCode, every friction is classified by whose failure it is, the golden covers are listed as passed with the corpus set, and latency and USD are reported
- ⬜ Spend is measured from transcripts per row and no row passed its cap without stopping; levels 1 and 2, coordinator included, stay within the $127 the maintainer approved, and level 3, its lead included, within $70
**References**: [Cycle plan v2](../../../../../../../../../../../__reports__/diet/04-architecture_cycle_plan_v2.md) — causes, proof, pieces, order and cost; [AGENTS.md](../../../../../../../../../../../AGENTS.md) — where the D4 paragraph lands; [README.md](../../../../../../../../../../../README.md) — the purpose paragraph, the yardstick

## Step 1: Plan the rest of the milestone

**Goal**: The causes behind the diet's defects are named, and the plan that removes them is approved.
**Implementation Logic**:
Done before this amendment. The cycle plan went through three rounds (`02-…_v0`, `03-…_v1`, `04-…_v2` under `__reports__/diet/`), each checked by the milestone coordinator, with Eliott's rulings applied in v2 and the cut order fixed in place. It names four causes (D1, D3, D4, D5), the change that removes each, the observation that proves each, the pieces by concept, and a cost fitted to a fresh $100.
**Deliverables**: `__reports__/diet/04-architecture_cycle_plan_v2.md`; this leaf; the campaign `__roadmap__/integrity/`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.3.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/integrity/` (expected: PASS)
**Commit**: `rdm(api): add the M6c Integrity campaign`

## Step 2: Build and land

**Goal**: The campaign `__roadmap__/integrity/` is built level by level, merged into `feat/integrity`, then into `main`.
**Implementation Logic**:
Cut `feat/integrity` from `origin/main`. Run the campaign by depth: the conventions first; then P1, P2, the P4 chain (B, A1, A2) and P6 in parallel, at most three building at once on the shared target dir; then seam review 1 with the spend checkpoint that decides P5; then P5; then seam review 2; then the dogfooding pass. A roadmap merge waits for a green `container-e2e`. P3 and P4·S wait outside the caps, as v2's cut order says.
**Deliverables**: the merged campaign; the leaf and review reports under `__reports__/diet/leaves/`
**Consistency Checks**: `cargo nextest run --workspace` on the `feat/integrity` tip (expected: PASS)
**Commit**: `merge(api): land M6c Integrity`

## Step 3: Close out

**Goal**: The milestone is recorded.
**Implementation Logic**:
The knowledge-transfer report is written by the dogfooding leaf. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott through the coordinator, and apply it only on his yes, through `sw_write.py`.
**Deliverables**: this leaf's status in `__roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/m5b/stats/surface/integrity/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.3.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/m5b/stats/surface/integrity/` (expected: PASS)
**Commit**: `reports(api): close the M6c Integrity milestone`
