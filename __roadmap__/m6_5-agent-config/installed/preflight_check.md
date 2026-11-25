# Preflight check

**Goal**: Before any run, `candler resolve` runs each harness's own check on what Candler installed. It refuses a cell with any finding, quoting the harness, and records a clean verdict. The core also mounts each plan's read-only workspace file through `mount_inputs`, and refuses one that collides with a declared input. (P0)
**Estimate**: Sonnet, $9, cap $18. If the work outgrows the leaf or meets a question its brief does not answer, hand back with the question, the evidence and what is green. Builds use one slot of `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, and the leaf removes it when it merges. The leaf deletes the containers it starts.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds
**Success Gates**:
- ⬜ With a driver whose check reports a finding, `candler resolve` exits 2 with `experiment.harness_rejected_input`. The message names what was installed, where, the harness, and the harness's output. On the base, no such step exists, and the same experiment is refused earlier with `unknown_path`.
- ⬜ With a clean check, the verdict is stored beside the harness identity in the run's provenance. With a driver whose `config_check` is `None`, the cell is not refused and nothing is stored as checked.
- ⬜ Cells that share a harness, image and prepared files run the check once, which is counted on the step's image service.
- ⬜ G9 (core half). With a fake driver whose plan lists a read-only workspace file, the run's workspace holds it at the root, a write to it fails, it is not among the collected outputs, and the run is not sealed `input_leak`. On the base, `Plan` has no such field.
- ⬜ G10. With `instructions.workspace` set, an input declared at the workspace instruction file's path is refused at `resolve` with `experiment.instructions_collide`, naming the input. On the base, the experiment is refused earlier with `unknown_path`.
- ⬜ An adversarial Sonnet review finds no path where a finding lets the cell run, and no check container that outlives the step.
**References**: [Architecture report, v2](../../../__reports__/agent_config/02-architecture_agent_config_v2.md) — the shapes, the error model, the invariants and §Execution model; `crates/candler-core/src/resolve/steps/` (`HarnessIdentityStep`, the precedent); `ImageService`

## Step 1: The step
**Goal**: A preflight step that asks the harness and refuses on its answer.
**Implementation Logic**:
Add `HarnessConfigStep` next to `HarnessIdentityStep`, reusing its image-service path. It dedupes by (harness, image, digest of the prepared files). For each distinct entry it starts a no-network container from the cell's image, places the driver's prepared files there and mounts the read-only workspace file, runs `config_check`, reads the verdict with `read_config_check`, and removes the container. Use a fake driver in the core guards; the real harnesses are L3's.
**Deliverables**: `crates/candler-core/src/resolve/steps/config_check.rs` (`HarnessConfigStep`); the provenance field; core guards
**Consistency Checks**: `cargo nextest run -p candler-core` (expected: PASS)
**Commit**: `feat(api): refuse a cell its harness rejects`

## Step 2: The workspace file
**Goal**: The plan's read-only workspace file reaches the workspace root the way an input does.
**Implementation Logic**:
Store each read-only workspace file's text and add it to the mount list that `mount_inputs` builds, so the existing re-check covers it with the inputs. Do not write a second mount path. Before planning the runs, compare the file's path with every path the declared inputs mount, and refuse a match with `experiment.instructions_collide`. Use a fake driver in the core guards.
**Deliverables**: `crates/candler-core/src/runner/exec/mod.rs` (`mount_inputs`); the collision check in resolve; core guards for G9 and G10
**Consistency Checks**: `cargo nextest run -p candler-core` (expected: PASS)
**Commit**: `feat(api): mount the workspace instruction file read-only`
