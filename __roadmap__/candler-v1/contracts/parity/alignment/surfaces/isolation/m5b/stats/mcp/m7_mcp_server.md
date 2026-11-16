# M7 MCP server

**Goal**: Agents can design and drive experiments through an MCP server over the same operation API the CLI uses.
**Pre-conditions**:
- [ ] `../m6a_paired_stats.md` is `done` (the reminder needs only M3; the tree also waits for M4, M5a, M5b and M6a)
- [ ] Reminder `candler-M7` read
**Success Gates**:
- ⬜ The read-only operations (`resolve`, `jobs`, `events`, `show`, `inspect`, `analyze`, `diff`) are exposed, and results come back as ids plus compact summaries
- ⬜ `run`, `evaluate` and `cancel` require a budget, and a `run` without one is refused (I13)
- ⬜ No tool exposes a live run's ground truth (I8)
- ⬜ The MCP server and the CLI return the same results for the same operation
- ⬜ A set of agent skills lets an agent use the MCP server to author plug-ins, drive runs and compute evals; each skill is exercised by an agent that completes its task through the server alone, and they land before M8
**References**: [R01 §Operation API, §Budget, §Contracts & Invariants (I8, I13)](../../../../../../../../../../__design__/architecture.md) — operations, budgets, surface rules

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m7-mcp-server/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`, covering: the MCP server as a `candler` subcommand built on `rmcp`; read-only operations first, then the budgeted mutating ones; the tool shapes (open in R01); ids-not-dumps results; the agent skills, one per task (author a plug-in, drive a run, compute an eval), each built on the contract pages (`docs/contract/`) and the server's tools, and tested by an agent that follows only the skill.
Adversarial verification is required for budget enforcement and I8 at the surface: attack every way a driving agent could spend without a budget or read a live run's ground truth.
**Deliverables**: `__roadmap__/m7-mcp-server/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m7-mcp-server/` (expected: PASS)
**Commit**: `rdm(mcp): plan the m7-mcp-server milestone`

## Step 2: Build and land

**Goal**: The milestone campaign is executed and lands on `main`.
**Implementation Logic**:
Execute the campaign on `feat/m7-mcp-server`, then land it on `main` with one `--no-ff` merge. The Success Gates above must pass on `main` after the merge. If a design decision changed, `__design__/architecture.md` changes in the same merge.
**Deliverables**: the `candler mcp` subcommand and its tools; surface tests comparing MCP and CLI results; the agent skills
**Consistency Checks**: `cargo test -p candler` (expected: PASS)
**Commit**: `merge(mcp): land the MCP server`

## Step 3: Close out

**Goal**: The milestone is recorded.
**Implementation Logic**:
Write a knowledge-transfer report under `__reports__/m7-mcp-server/`. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott: `candler-M7` complete. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/m7-mcp-server/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/stats/mcp/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/stats/mcp/` (expected: PASS)
**Commit**: `reports(mcp): close the m7-mcp-server milestone`
