# M6 Second plug-in, paired statistics, freeze

**Goal**: A second, unrelated plug-in and paired statistics are in, and the contracts freeze at 1.0.0.
**Pre-conditions**:
- [ ] `../m5_container_isolation.md` is `done`, and M2a before it (the tree also waits for `../m7_mcp_server.md`)
- [ ] Reminder `candler-M6` read (plug-in parts, statistics list, deterministic grading)
- [ ] R05's test definition read: `00-test_definition_v0.md` beside the R05 design
**Success Gates**:
- ⬜ The `waiting-on-processes` plug-in grades its fixtures as correct, false success or stray process, deterministically
- ⬜ Paired statistics (McNemar, CMH, bootstrap, fixed-sequence; alongside M3's Wilson intervals and Holm adjustment) run on declared pairing keys, and the pairing keys, execution order and blocking factor are decided and recorded in `__design__/architecture.md`
- ⬜ Every item of R01's "Deliberately open" list is either decided in the design doc or explicitly carried past 1.0
- ⬜ Core `v1.0.0` and `schemas@1.0.0` are tagged on `main`; the harness traits and the evaluator protocol are frozen
**References**: [R01 §Data Model, §Deliberately open, §Contracts & Invariants (I5)](../../../../../../../__design__/architecture.md) — what freezes, what is still open

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m6-second-plugin-freeze/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`, covering: the `waiting-on-processes` plug-in (passive fixtures, a server stub, scheduler shims, an outcome schedule, the outcome grader) in its own repository or the skill's, decided in the plan; paired statistics in `candler-core`; the pairing model (replicate and attempt exist since M2b; pairing keys, execution order and a declared blocking factor are new); the `without_skill` parity fixture and per-arm `instrument_error` reporting from R01's open list; the freeze and the 1.0.0 release. Grading stays deterministic; probabilistic evaluators stay descriptive only (I5).
Adversarial verification is required for the freeze: attack anything the second plug-in needed that the contract lacked, before it becomes a breaking change.
**Deliverables**: `__roadmap__/m6-second-plugin-freeze/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m6-second-plugin-freeze/` (expected: PASS)
**Commit**: `rdm(release): plan the m6-second-plugin-freeze milestone`

## Step 2: Build and land

**Goal**: The milestone campaign is executed, lands on `main`, and 1.0.0 is tagged.
**Implementation Logic**:
Execute the campaign on `feat/m6-second-plugin-freeze`, then land it on `main` with one `--no-ff` merge and tag the release through the release tooling. The Success Gates above must pass on `main`. Publishing to crates.io and PyPI is Eliott's call.
**Deliverables**: paired statistics in `candler-core`; the `waiting-on-processes` plug-in; frozen schemas, traits and protocol; tags `v1.0.0` and `schemas@1.0.0`
**Consistency Checks**: `cargo test --workspace` (expected: PASS)
**Commit**: `merge(release): land the second plug-in and freeze the contract at 1.0.0`

## Step 3: Close out

**Goal**: The campaign is complete.
**Implementation Logic**:
Write a knowledge-transfer report under `__reports__/m6-second-plugin-freeze/`. Mark this leaf `done` with `dirtree-rdm update`, and check the campaign README's Success Gates. Propose the reminder reconciliation to Eliott: `candler-M6` complete. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/m6-second-plugin-freeze/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/parity/surfaces/isolation/freeze/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/surfaces/isolation/freeze/` (expected: PASS)
**Commit**: `reports(release): close the m6-second-plugin-freeze milestone`
