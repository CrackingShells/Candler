# M4 Session runner and second harness

**Goal**: Agents can run in long-lived sessions, and a second harness, OpenCode v2, runs through the same driver traits.
**Pre-conditions**:
- [ ] `../../m2b_execution_parity.md` and `../m2c_wire_alignment.md` are `done` (the tree also waits for `../../m2a_evaluation_parity.md`; the reminder needs only M2b)
- [ ] Reminder `candler-M4` read: end rule, controls C1 and C2, observation hooks, per-model environment profiles, the 2026-10-05 findings
- [ ] Eliott asked before any RiVault probe: its model table and availability changed recently
**Success Gates**:
- ⬜ Against a recorded or stub harness, positive control C1 survives the end rule and negative control C2 ends at idle; one live confirmation of each, on a model Eliott chose, is recorded
- ⬜ A background-task notification starts a second turn in a session run (recorded fixture, plus one live confirmation)
- ⬜ `init_tools` is captured per run, and a mismatch between arms in a stratum invalidates that stratum through M2b's stratum verdict
- ⬜ Imported runs get a stratum verdict (amendment A4): two imported runs of one cell, imported under the same import rules, compare; an imported run and a live run of the same cell are refused together. R01 §Run manifest says which recorded values gate an imported stratum and what a legacy `eval_infra.dirty: true` means there. Live runs keep the rule that an unrecorded gated value never counts as equal
- ⬜ The OpenCode v2 driver passes the driver fixture suite shared with the Claude Code driver
- ⬜ OpenCode v2's facts left unconfirmed in R01 (event schema, exit codes, signal behaviour, budget options, binary name) are confirmed and recorded in `__design__/architecture.md`
- ⬜ `candler-core/src` names no harness (amendment A3):
  - **From the driver:** the image's harness version comes from `HarnessDriver::version_probe`, the census reads the init event through the driver's `TraceReader`, and the gateway's and config's variable names come from the driver. The core no longer runs `claude --version`, and it names no `ANTHROPIC_*`, `CLAUDE_CONFIG_DIR` or `system/init`.
  - **Guard:** the I1 guard, with A2's segment matching, denies the segments `claude`, `anthropic` and `opencode` in `candler-core/src` and `candler-schemas/src`. A planted `claude_version` in the core makes it fail. The binary and the driver crates are not scanned for these segments.
  - **Digests:** every cell digest of M1's golden import, M2a's parity fixtures and M2c's re-validation smoke is unchanged, and so is the recorded Claude Code version string.
**References**: [R01 §Harness drivers, §OpenCode v2 (second driver, M4), §Session runner (M4)](../../../../../../__design__/architecture.md) — traits, OpenCode differences, end rule and controls; [gap analysis A3](../../../../../../__reports__/candler-v1-review/01-gap_analysis_harness_env_v0.md) — the Claude Code content of the core (H1); [gap analysis A4](../../../../../../__reports__/m2c-wire-alignment/gap_analysis_imported_strata_v0.md) — why no imported stratum gets a verdict after M2c

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m4-session-runner/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`, covering:

- **Session runner** as R01 §Session runner describes it: input kept open, the end rule, controls C1 and C2, `init_tools`, observation hooks that never deny or modify, per-model environment profiles. The value of the idle threshold Q is decided in the plan.
- **OpenCode v2 driver** (`candler-harness-opencode`): confirm the unconfirmed facts first; force `--standalone`; `execution_outside_child` with an HTTP pre-stop action; collect through `opencode session export` with the observed session id; isolate `HOME`, XDG and `OPENCODE_DB`; MCP code mode as an explicit experiment factor.
- **Shared driver fixture suite** that both drivers pass, so the trait is not shaped like one harness.
- **Harness out of the core (A3)**: move the version probe, the census's init-event parser and the gateway's and config's variable names behind the driver traits. Record in R01 §Harness drivers which facts a driver supplies. Extend the I1 guard with the harness deny-list. Do this before the OpenCode driver lands, so OpenCode never needs a branch in the core. Source: `__reports__/candler-v1-review/01-gap_analysis_harness_env_v0.md`.
- **Imported strata (A4)**: M4 is the first caller of `stratum_verdict` in `src`. Since M2c, the verdict refuses every imported stratum, for two reasons. The importer puts the legacy `eval_infra.*` and `sandbox_contract` in `comparable.values`, while the verdict reads them from `measured`. And the legacy build recorded `dirty: true`. Decide, before the `init_tools` gate uses the verdict, what an imported arm is identified by: for example `import.rules` with the legacy values, rather than an exemption. Also decide whether a dirty legacy build is refused or compares with arms of the same dirty commit. Record both in R01 §Run manifest. Source: `__reports__/m2c-wire-alignment/gap_analysis_imported_strata_v0.md`.
- **Harness and trace types**: merge `FrameMode` (`candler-core/src/frames`) into `candler-harness`'s `Framing` as part of the A3 move, and rename the Rust field `Session::parent_session` (`candler-schemas/src/trace.rs`), keeping its wire name with `serde(rename)` so the schema files do not change.

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
**Deliverables**: `__reports__/m4-session-runner/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/parity/alignment/surfaces/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/alignment/surfaces/` (expected: PASS)
**Commit**: `reports(runner): close the m4-session-runner milestone`
