# M5 Container isolation

**Goal**: Runs can execute in a disposable container with outcomes injected from outside, a whole-container census, an egress allowlist and plug-in-declared environments.
**Pre-conditions**:
- [ ] `../m4_session_runner.md` is `done` (the tree also waits for `../m3_lean_cli.md`)
- [ ] Reminder `candler-M5` read (image contents, injection from outside, census before destroy, egress allowlist, macOS host as its own stratum)
- [ ] Docker available on the development machine (not needed before this milestone)
**Success Gates**:
- ⬜ On a test experiment, the ground-truth file is absent from the container at every point of the run (I8)
- ⬜ The census after SessionEnd lists every process, and a stray started inside the container is reported
- ⬜ Egress outside the allowlist fails while the model gateway works
- ⬜ The agent-side environment a plug-in declares (what the agent's isolation must contain) is provisioned by the core, with no hand-edited sandbox contract; evaluator environments are M2a's
- ⬜ Runs on the macOS host and in containers fall in separate strata through M2b's stratum verdict
**References**: [R01 §Contracts & Invariants (I8, I14), §Process supervision](../../../../../../__design__/architecture.md) — isolation interface, ground-truth rule, census

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m5-container-isolation/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`, covering: the container backend behind `Isolation` (choosing it is open in R01); the image (agent harness, a shell, the runtime, procps); outcome injection from outside; the census; the egress allowlist; agent-side environments declared by plug-ins, which replace per-domain sandbox contracts such as the HTN contract v6 that Phase 8 would otherwise need. The plug-in manifest has no field for them yet: the plan adds one as an optional field, a minor bump on the `schemas@` line, allowed before the 1.0 freeze. Stop escalation ends with destroying the container.
Adversarial verification is required for I8: attack every path by which ground truth could enter the boundary (mounts, environment, logs, the driver's files).
**Deliverables**: `__roadmap__/m5-container-isolation/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m5-container-isolation/` (expected: PASS)
**Commit**: `rdm(isolation): plan the m5-container-isolation milestone`

## Step 2: Build and land

**Goal**: The milestone campaign is executed and lands on `main`.
**Implementation Logic**:
Execute the campaign on `feat/m5-container-isolation`, then land it on `main` with one `--no-ff` merge. The Success Gates above must pass on `main` after the merge. If a design decision changed, `__design__/architecture.md` changes in the same merge.
**Deliverables**: the container `Isolation` backend, injection, census and egress allowlist in `candler-core`; plug-in environment provisioning; their tests
**Consistency Checks**: `cargo test --workspace` (expected: PASS)
**Commit**: `merge(isolation): land container isolation`

## Step 3: Close out

**Goal**: The milestone is recorded and the HTN lane's Phase 8 can declare its environment.
**Implementation Logic**:
Write a knowledge-transfer report under `__reports__/m5-container-isolation/`. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott: `candler-M5` complete; a log line on `pro-htn-phase-8-skill`. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/m5-container-isolation/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/parity/surfaces/isolation/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/surfaces/isolation/` (expected: PASS)
**Commit**: `reports(isolation): close the m5-container-isolation milestone`
