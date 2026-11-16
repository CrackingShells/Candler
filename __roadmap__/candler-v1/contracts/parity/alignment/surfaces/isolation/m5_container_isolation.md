# M5 Container isolation

**Goal**: Runs can execute in a disposable container whose network is closed to everything but an allowlist and the model gateway, with the plug-in's agent environment provisioned by the core and the Docker plumbing built once.
**Pre-conditions**:
- [ ] `../m4_session_runner.md` is `done` (the tree also waits for `../m3_lean_cli.md`)
- [ ] Reminder `candler-M5` read (egress allowlist, plug-in environments, the I8 guard, the Docker client)
**Success Gates**:
- ⬜ Egress is an allowlist of hostnames and ports: a listed host and port is reachable from the container, everything else fails, and the model gateway still works
- ⬜ The agent-side environment a plug-in declares is provisioned by the core, with no hand-edited sandbox contract. It stays thin because `isolation.build.*` already builds environments; evaluator environments are M2a's
- ⬜ A guard proves that on a test experiment the ground-truth file is absent from the agent container at every point of the run (I8)
- ⬜ One Docker client and one control-container builder serve every caller (the interlude's A5), with multi-id inspect and guard re-use (A1) and `docker create` instead of cidfile polling (A8) as part of it; `__reports__/interlude-audit/00-findings_audit_v0.md` has the findings
- ⬜ Every cell digest of M1's golden import and M2a's parity fixtures is unchanged
**References**: [R01 §Contracts & Invariants (I8, I14), §Process supervision](../../../../../../../__design__/architecture.md) — isolation interface, ground-truth rule; [interlude audit findings](../../../../../../../__reports__/interlude-audit/00-findings_audit_v0.md) — A1, A5 and A8 (one Docker client); [R01 §Future features](../../../../../../../__design__/architecture.md) — what M5 leaves out and its triggers

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m5-container-isolation/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`, covering: the egress allowlist behind `Isolation` over M2b's Docker backend, with hostnames and ports declared by the experiment or plug-in; agent-side environments declared by plug-ins, which replace per-domain sandbox contracts such as the HTN contract v6 that Phase 8 would otherwise need (the plug-in manifest has no field for them yet: the plan adds one as an optional field, a minor bump on the `schemas@` line); the I8 guard; and the Docker client work (A5, A1, A8), which is a refactor of what exists and goes first so the allowlist builds on it. Constraint: M5 must not tie the core further to Python. Today's Python venv is the only environment kind; M5 adds no Python path, variable, marker or version to the core and does not add a second kind (see Future features).
M5 does not build outcome injection, a whole-container census, a macOS host backend or an environment-kind abstraction. Each is a Future features item with its trigger in R01.
Adversarial verification is required for I8: attack every path by which ground truth could enter the boundary (mounts, environment, logs, the driver's files), and for the allowlist: attack every way a container could reach a host that is not listed (DNS, IP literals, redirects, the gateway as a proxy).
**Deliverables**: `__roadmap__/m5-container-isolation/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m5-container-isolation/` (expected: PASS)
**Commit**: `rdm(isolation): plan the m5-container-isolation milestone`

## Step 2: Build and land

**Goal**: The milestone campaign is executed and lands on `main`.
**Implementation Logic**:
Execute the campaign on `feat/m5-container-isolation`, then land it on `main` with one `--no-ff` merge. The Success Gates above must pass on `main` after the merge. If a design decision changed, `__design__/architecture.md` changes in the same merge.
**Deliverables**: the egress allowlist and plug-in environment provisioning over M2b's Docker backend in `candler-core`; the single Docker client and control-container builder; the I8 guard; their tests
**Consistency Checks**: `cargo nextest run --workspace` (expected: PASS)
**Commit**: `merge(isolation): land container isolation`

## Step 3: Close out

**Goal**: The milestone is recorded and the HTN lane's Phase 8 can declare its environment.
**Implementation Logic**:
Write a knowledge-transfer report under `__reports__/m5-container-isolation/`. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott: `candler-M5` complete; a log line on `pro-htn-phase-8-skill`. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/m5-container-isolation/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/` (expected: PASS)
**Commit**: `reports(isolation): close the m5-container-isolation milestone`
