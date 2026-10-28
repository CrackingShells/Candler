# M3 Lean CLI

**Goal**: Every operation of the API is reachable through a thin CLI that derives what it can and works from any directory, and runs flow into evaluation end to end.
**Pre-conditions**:
- [ ] `../m2b_execution_parity.md` is `done` (the tree also waits for `../m2a_evaluation_parity.md`; the reminder needs only M2b)
- [ ] Reminder `candler-M3` read (derive before defaulting, two entry calls, every option justified)
**Success Gates**:
- ⬜ Each of the top 10 quick wins of R03 is covered by a named test
- ⬜ Every verb is tested from an unrelated working directory (I14)
- ⬜ `candler ls` and `candler watch --live` work, and `resolve` reports the source of every value (I12)
- ⬜ Every CLI option either cannot be derived or cannot live in the experiment file, and its help text says which
- ⬜ A fresh `candler run` followed by `candler evaluate` grades the new runs end to end, with `evaluation.outcome` events in the job's stream
- ⬜ `evaluate` runs as a job, its evaluator processes take slots and are covered by the stray sweep (I15)
- ⬜ `ingest` (raw trace directories and results folders), `inspect` and `analyze` work; `analyze` reports rates with Wilson intervals, Holm-adjusted comparisons, `not_measured` and `not_applicable` counts with causes, and invalid runs per arm (I10)
**References**: [R01 §Operation API, §CLI surface over the jobs, §Wire schema: outcome record](../../../../../__design__/architecture.md) — operations, resolution order, CLI verbs, rates and the selection rule

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m3-lean-cli/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`, covering: `evaluate` as a job (M2a made it synchronous because M2b's job supervisor did not exist yet); the live `run` then `evaluate` flow; the `ingest`, `inspect` and `analyze` verbs with unpaired statistics (`statrs`); `watch --live` and `ls` (deciding where `ls` looks, provisional in R01); resolution reporting; the quick wins of R03 (`/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion/__reports__/candler_core/02-findings_cli_defaults_v0.md`), several of which (derived servers, real preflight, cwd-independent paths) M2b may already have covered; two entry calls (run an experiment, act on a results folder); `--no-color` behaviour derived from the TTY.
Adversarial verification is required for `analyze` only (measurement semantics: denominators, `blocked` counted as a failure, rates carrying their counts, I2); the CLI gates are mechanical.
**Deliverables**: `__roadmap__/m3-lean-cli/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m3-lean-cli/` (expected: PASS)
**Commit**: `rdm(cli): plan the m3-lean-cli milestone`

## Step 2: Build and land

**Goal**: The milestone campaign is executed and lands on `main`.
**Implementation Logic**:
Execute the campaign on `feat/m3-lean-cli`, then land it on `main` with one `--no-ff` merge. The Success Gates above must pass on `main` after the merge. If a design decision changed, `__design__/architecture.md` changes in the same merge.
**Deliverables**: the `candler` CLI verbs and their tests, run from an unrelated directory; `evaluate` as a job; `ingest`, `inspect`, `analyze` with unpaired statistics in `candler-core`
**Consistency Checks**: `cargo test -p candler` (expected: PASS)
**Commit**: `merge(cli): land the lean CLI`

## Step 3: Close out

**Goal**: The milestone is recorded.
**Implementation Logic**:
Write a knowledge-transfer report under `__reports__/m3-lean-cli/`. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott: `candler-M3` complete; `candler-M7` startable. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/m3-lean-cli/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/parity/surfaces/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/surfaces/` (expected: PASS)
**Commit**: `reports(cli): close the m3-lean-cli milestone`
