# Dogfooding pass

**Goal**: Two RiVault models, with only what an outside consumer has, use Candler on the after tip for what its README claims, and every friction they meet is attributed to the agent or to Candler; the milestone's knowledge transfer follows (P7).
**Estimate**: RiVault agents and a RiVault first classification, one Claude reader: $7, cap $14. Each model gets one attempt, with a turn cap and a wall-clock cap set from P1's probe times; no rerun unless a fix reopens a piece. At the cap, stop, commit what is green, and list what is left.
**Pre-conditions**:
- [ ] `[nl]` `../seam_review_two.md` is `done`, and `feat/integrity` is green
- [ ] `[cmd]` the RiVault check script of the `using-rivault` skill reports the VPN and keys working
**Success Gates**:
- ⬜ Each agent worked in a scratch directory outside the repo with only the installed `candler` binary and its `--help`, the README, `docs/` and the library's rendered rustdoc; its transcript shows whether it opened the crate source in cargo's dependency directory, and if it did, that is a friction
- ⬜ In a toy domain that names nothing of HTN, each agent wrote a two-arm experiment with a few replicates under container isolation and one MCP tool server, ran it with OpenCode on a RiVault model, checked each run's tool-catalog snapshot and provenance, graded the artifacts with an evaluator plug-in fresh and already on disk, then did the experiment, the run and the grading again through the op API
- ⬜ Every friction is classified from the kept transcripts by whether what the agent needed was documented and reachable: one model missed it (the agent's), both missed it (Candler's documentation or surface), not documented or not reachable (Candler's; its piece reopens if the cause is D1 to D5), missing (a finding for Eliott)
- ⬜ The golden covers ran with `CANDLER_GOLDEN_CORPUS` set, and the nextest output lists them as passed, not skipped
- ⬜ The before tip is used only where it shows a cause gone (the malformed MCP server; the S1 and B1 reverts); the plug-in path layout (`context.root/path` meeting `outputs/outputs/…`) and audit-1 (the cell count in `resolve`) are judged from the transcripts
- ⬜ The report gives each step's latency, Candler's own run times, RiVault tokens as tokens and USD measured from transcripts; Docker images, containers and networks the pass created are removed by name
**References**: [Cycle plan v2 §The dogfooding pass, §Open items](../../../../../../../__reports__/diet/04-architecture_cycle_plan_v2.md) — the design, the classification table and what it settles; [README.md](../../../../../../../README.md) — the claims the task comes from

## Step 1: Run the pass
**Goal**: Two consumer-agent transcripts on the after tip.
**Implementation Logic**:
Install `candler` from the `feat/integrity` tip and render the rustdoc. Pick two models from different families from RiVault's live list, both under OpenCode as the consumer harness (Claude Code is not one of them). Give each the same task brief, written from the README claims, and the outside-consumer material only. Keep the transcripts, Candler's store and the timings.
**Deliverables**: the transcripts and stores under the pass's scratch directory, named in the report
**Consistency Checks**: both transcripts end with the task done or with the step that blocked named (expected: PASS)
**Commit**: `reports(api): record the dogfooding runs`

## Step 2: Classify and report
**Goal**: Each friction has an owner, and the golden covers have run.
**Implementation Logic**:
A RiVault model makes the first classification pass over the transcripts. One Claude agent checks every friction rated as Candler's and rereads a sample of the others. Run the golden covers with the corpus set. Write the findings report; a friction whose cause is D1 to D5 reopens its piece, one with no capability behind it goes to Eliott through the coordinator.
**Deliverables**: `__reports__/diet/05-findings_dogfooding_pass_v0.md`
**Consistency Checks**: `cargo nextest run --workspace --run-ignored all -E 'test(/golden/)'` with `CANDLER_GOLDEN_CORPUS` set lists the golden tests as passed (expected: PASS)
**Commit**: `reports(api): classify the dogfooding pass`

## Step 3: Knowledge transfer
**Goal**: The next milestone inherits what M6c learned, measured.
**Implementation Logic**:
Write the knowledge-transfer report: what each cause's removal looked like in practice, what the seam reviews caught that leaf verifiers would not have, measured spend per row from transcripts against estimates and caps, the pieces that waited (P3, P4·S, and P5 if it did) with what would bring them back, and Next. Mark this leaf `done` with `dirtree-rdm update`.
**Deliverables**: `__reports__/diet/06-knowledge_transfer_integrity_v0.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.3.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/integrity/causes/checkpoint/snapshot/review/dogfood/` (expected: PASS)
**Commit**: `reports(api): close M6c Integrity`
