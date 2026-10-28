# M4 Session runner and second harness

**Goal**: Agents can run in long-lived sessions, and a second harness, OpenCode v2, runs through the same driver traits.
**Pre-conditions**:
- [ ] `../m2b_execution_parity.md` is `done` (the tree also waits for `../m2a_evaluation_parity.md`; the reminder needs only M2b)
- [ ] Reminder `candler-M4` read: end rule, controls C1 and C2, observation hooks, per-model environment profiles, the 2026-10-05 findings
- [ ] Eliott asked before any RiVault probe: its model table and availability changed recently
**Success Gates**:
- ⬜ Against a recorded or stub harness, positive control C1 survives the end rule and negative control C2 ends at idle; one live confirmation of each, on a model Eliott chose, is recorded
- ⬜ A background-task notification starts a second turn in a session run (recorded fixture, plus one live confirmation)
- ⬜ `init_tools` is captured per run, and a mismatch between arms in a stratum invalidates that stratum through M2b's stratum verdict
- ⬜ The OpenCode v2 driver passes the driver fixture suite shared with the Claude Code driver
- ⬜ OpenCode v2's facts left unconfirmed in R01 (event schema, exit codes, signal behaviour, budget options, binary name) are confirmed and recorded in `__design__/architecture.md`
**References**: [R01 §Harness drivers, §OpenCode v2 (second driver, M4), §Session runner (M4)](../../../../../__design__/architecture.md) — traits, OpenCode differences, end rule and controls

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m4-session-runner/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`, covering:

- **Session runner** as R01 §Session runner describes it: input kept open, the end rule, controls C1 and C2, `init_tools`, observation hooks that never deny or modify, per-model environment profiles. The value of the idle threshold Q is decided in the plan.
- **OpenCode v2 driver** (`candler-harness-opencode`): confirm the unconfirmed facts first; force `--standalone`; `execution_outside_child` with an HTTP pre-stop action; collect through `opencode session export` with the observed session id; isolate `HOME`, XDG and `OPENCODE_DB`; MCP code mode as an explicit experiment factor.
- **Shared driver fixture suite** that both drivers pass, so the trait is not shaped like one harness.

R05 holds the earlier design for between-turn behaviour. Adversarial verification is required for driver-trait fit: attack anything OpenCode needed that the trait could not express.
**Deliverables**: `__roadmap__/m4-session-runner/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m4-session-runner/` (expected: PASS)
**Commit**: `rdm(runner): plan the m4-session-runner milestone`

## Step 2: Build and land

**Goal**: The milestone campaign is executed and lands on `main`.
**Implementation Logic**:
Execute the campaign on `feat/m4-session-runner`, then land it on `main` with one `--no-ff` merge. The Success Gates above must pass on `main` after the merge. The design doc's OpenCode section changes in the same merge.
**Deliverables**: the session runner in `candler-core`; the `candler-harness-opencode` crate (`HarnessDriver`, `TraceReader`); the shared driver fixture suite
**Consistency Checks**: `cargo test --workspace` (expected: PASS)
**Commit**: `merge(runner): land the session runner and the OpenCode driver`

## Step 3: Close out

**Goal**: The milestone is recorded.
**Implementation Logic**:
Write a knowledge-transfer report under `__reports__/m4-session-runner/`. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott: `candler-M4` complete; `candler-M5` startable. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/m4-session-runner/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/parity/surfaces/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/surfaces/` (expected: PASS)
**Commit**: `reports(runner): close the m4-session-runner milestone`
