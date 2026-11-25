# Preflight check

**Goal**: Before any run, `candler resolve` runs each harness's own check on what Candler installed. It refuses a cell with any finding, quoting the harness, and records a clean verdict. (P0)
**Estimate**: Sonnet, $7, cap $14. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use one slot of `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, and the leaf removes it when it merges. The leaf deletes the containers it starts.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds
**Success Gates**:
- ⬜ With a driver whose check reports a finding, `candler resolve` exits 2 with `experiment.harness_rejected_input`. The message names what was installed, where, the harness, and the harness's output. On the base, no such step exists, and the same experiment is refused earlier with `unknown_path`.
- ⬜ With a clean check, the verdict is stored beside the harness identity in the run's provenance. With a driver whose `config_check` is `None`, the cell is not refused and nothing is stored as checked.
- ⬜ Cells that share a harness, image, instructions and hooks run the check once, which is counted on the step's image service.
- ⬜ An adversarial Sonnet review finds no path where a finding lets the cell run, and no check container that outlives the step.
**References**: [Architecture report](../../__reports__/agent_config/00-architecture_agent_config_v0.md) — the shapes, the error model, the invariants and §Execution model; `crates/candler-core/src/resolve/steps/` (`HarnessIdentityStep`, the precedent); `ImageService`

## Step 1: The step
**Goal**: A preflight step that asks the harness and refuses on its answer.
**Implementation Logic**:
Add `HarnessConfigStep` next to `HarnessIdentityStep`, reusing its image-service path. It dedupes by (harness, image, instructions, hooks). For each distinct entry it starts a no-network container from the cell's image, places the driver's prepared files there, runs `config_check`, reads the verdict with `read_config_check`, and removes the container. Use a fake driver in the core guards; the real harnesses are L3's.
**Deliverables**: `crates/candler-core/src/resolve/steps/config_check.rs` (`HarnessConfigStep`); the provenance field; core guards
**Consistency Checks**: `cargo nextest run -p candler-core` (expected: PASS)
**Commit**: `feat(api): refuse a cell its harness rejects`
