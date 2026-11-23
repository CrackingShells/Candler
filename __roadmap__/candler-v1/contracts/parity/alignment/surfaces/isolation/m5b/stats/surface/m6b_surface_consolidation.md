# M6b Surface consolidation

**Goal**: Before M7 puts an MCP server over it, Candler's consumer surface is sanitised. Every consumability finding of the M7 pre-study has an explicit, approved verdict on whether it is Candler's responsibility. The accepted ones land, consolidating sustainable, scalable and efficient at three positions (the development process, the codebase, and consumption). Nothing lands as a dangling feature (amendment A11).
**Pre-conditions**:
- [x] `../m6a_paired_stats.md` is `done` (PR #52, closed in #54)
- [ ] The M7 pre-study read: `__reports__/m7-mcp-server/01-findings_agent_consumability_v1.md`, its evidence (`00-inventory_*`, `01-input_htn_lane_v0.md`) and Eliott's round-02 rulings in that folder's `README.md`
**Success Gates**:
- ⬜ Every input has a recorded verdict, approved by Eliott before any code: the 21 gotchas (G1–G21), the 15 computable chores, the HTN lane's asks, and M6a's follow-ups F2, F4 and F5. A verdict says whether the item is Candler's responsibility given why the repo exists, its refactoring blast radius, its effect on the triad at each of the three positions, and whether it follows Candler's idioms. A rejected item names its reason, or the owner it belongs to
- ⬜ What is and is not Candler's responsibility on the consumer surface is recorded in `__design__/architecture.md`, including the split between the web-wide surface (docs: what Candler is and whether it fits) and the interactive surfaces (CLI, MCP server, skills), which must work in an isolated environment with no internet
- ⬜ Every accepted item lands with its tests; no item lands without an approved verdict
- ⬜ The refusal-code registry (`docs/contract/codes.toml`, 377 rows at the start) has its number and granularity reviewed, with the outcome in the verdicts; no remedy hints are added by default
- ⬜ Every golden-import and parity digest is unchanged, unless an approved verdict says why it moves
**References**: [R01 §Operation API, §CLI surface, §Contracts & Invariants](../../../../../../../../../../__design__/architecture.md) — the surfaces and invariants the verdicts judge against; [AGENTS.md §Sustainable, scalable, efficient](../../../../../../../../../../AGENTS.md) — the triad

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m6b-surface-consolidation/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`. The pre-study is an inventory, not a backlog: its items, and the solutions it proposed, are inputs to judge. Question each ask itself as well as its value. For example, the HTN "agent-world probe" inspects a container that is a consequence of the experiment files, so a mismatch is a Candler defect for Candler's own guards, not something a consumer should check. Judge each item against why Candler exists (`README.md`: a domain-agnostic core for evaluating LLM agents, execution and evaluation behind one operation API) and against the triad at the three positions: nine dimensions, with compromises stated where they conflict. Everything raised may still land, but each decision is explicit. The campaign's first level produces the verdicts and the responsibility record for Eliott's review; build levels follow only for accepted items. Evidence comes from code, the reports and our own agents' use of Candler. No user-behaviour baseline is planned. Every question put to Eliott explains its terms and options, since he does not see the subagents' and peer sessions' context. Budget the milestone coordinator as its own row (M6a lesson).
**Deliverables**: `__roadmap__/m6b-surface-consolidation/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m6b-surface-consolidation/` (expected: PASS)
**Commit**: `rdm(api): plan the m6b-surface-consolidation milestone`

## Step 2: Build and land

**Goal**: The milestone campaign is executed and lands on `main`.
**Implementation Logic**:
Execute the campaign on `feat/m6b-surface-consolidation`, then land it on `main` with one `--no-ff` merge. The Success Gates above must pass on `main` after the merge. The responsibility record and any changed design decision go to `__design__/architecture.md` in the same merge.
**Deliverables**: the verdicts report under `__reports__/m6b-surface-consolidation/`; the accepted changes and their tests; `__design__/architecture.md` updated
**Consistency Checks**: `cargo nextest run --workspace` (expected: PASS)
**Commit**: `merge(api): land the surface consolidation`

## Step 3: Close out

**Goal**: The milestone is recorded.
**Implementation Logic**:
Write a knowledge-transfer report under `__reports__/m6b-surface-consolidation/`. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/m6b-surface-consolidation/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/m5b/stats/surface/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/m5b/stats/surface/` (expected: PASS)
**Commit**: `reports(api): close the m6b-surface-consolidation milestone`
