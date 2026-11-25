# Preflight check

**Goal**: Before a job's first run, the core runs each harness's own check on what Candler installed, beside the start check of the declared servers. It refuses a cell with any finding, quoting the harness, and records a clean verdict. The core also mounts each plan's read-only workspace file through `mount_inputs`, and refuses, when the runs are planned, one that collides with a declared input. (P0)
**Estimate**: Sonnet, $9, cap $18. If the work outgrows the leaf or meets a question its brief does not answer, hand back with the question, the evidence and what is green. Builds use one slot of `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, and the leaf removes it when it merges. The leaf deletes the containers it starts.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds
**Success Gates**:
- ⬜ With a driver whose check reports a finding, the job is refused before its first run with `experiment.harness_rejected_input`, the way a declared server that exits at start is refused. The message names what was installed, where, the harness, and the harness's output. No run starts and no check container is left. On the base, the job runs.
- ⬜ With a clean check, the verdict is stored beside the harness identity in the run's provenance. With a driver whose `config_check` is `None`, the cell is not refused and nothing is stored as checked.
- ⬜ Cells that share a harness, image and prepared files run the check once, counted on the isolation the check uses.
- ⬜ G9 (core half). With a fake driver whose plan lists a read-only workspace file, the run's workspace holds it at the root, a write to it fails, it is not among the collected outputs, and the run is not sealed `input_leak`. On the base, `Plan` has no such field.
- ⬜ G10. With `instructions.workspace` set, an input declared at the workspace instruction file's path is refused when the runs are planned, before any container starts, with `experiment.instructions_collide`, naming the input. On the base, the job runs.
- ⬜ An adversarial Sonnet review finds no path where a finding lets the cell run, and no check container that outlives the step.
**References**: [Architecture report, v2](../../../__reports__/agent_config/02-architecture_agent_config_v2.md) — the shapes, the error model, the invariants and §Execution model; `crates/candler-core/src/jobs/orchestrate/servers.rs` (`check_servers`, the precedent, called from `run_job`); `crates/candler-core/src/runner/plan.rs` (`plan_runs`)

## Step 1: The check
**Goal**: A start check that asks the harness and refuses on its answer.
**Implementation Logic**:
Add `check_config` beside `check_servers` and call it from `run_job` next to it, reusing its grouping, container and scratch-removal path. For each planned run it asks the driver for the cell's plan, skips a driver whose `config_check` is `None`, and dedupes by (harness, image, digest of the prepared files). For each distinct entry it starts a no-network container from the cell's image, places the driver's prepared files there and mounts the read-only workspace file, runs `config_check`, reads the verdict with `read_config_check`, and removes the container. Use a fake driver in the core guards; the real harnesses are L3's.
**Deliverables**: `check_config` in `crates/candler-core/src/jobs/orchestrate/`; the provenance field; core guards
**Consistency Checks**: `cargo nextest run -p candler-core` (expected: PASS)
**Commit**: `feat(api): refuse a cell its harness rejects`

## Step 2: The workspace file
**Goal**: The plan's read-only workspace file reaches the workspace root the way an input does.
**Implementation Logic**:
Store each read-only workspace file's text and add it to the mount list that `mount_inputs` builds, so the existing re-check covers it with the inputs. Do not write a second mount path. When the runs are planned, compare the file's path with every path the declared inputs mount, and refuse a match with `experiment.instructions_collide`. Use a fake driver in the core guards.
**Deliverables**: `crates/candler-core/src/runner/exec/mod.rs` (`mount_inputs`); the collision check in planning; core guards for G9 and G10
**Consistency Checks**: `cargo nextest run -p candler-core` (expected: PASS)
**Commit**: `feat(api): mount the workspace instruction file read-only`
