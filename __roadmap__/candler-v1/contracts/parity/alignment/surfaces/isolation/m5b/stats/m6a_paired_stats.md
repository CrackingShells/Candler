# M6a Paired statistics

**Goal**: `analyze` compares paired arms with McNemar, CMH, bootstrap and fixed-sequence tests alongside M3's Wilson intervals and Holm adjustment, and reports each arm's `instrument_error` count. An arm can be a treatment: one experiment declares the treatment as a factor, and each cell resolves its own treatment, env lock and image (amendment A7).
**Pre-conditions**:
- [x] `../README.md`'s M5b is `done` (the leaf `../../m5_container_isolation.md`, M5a, before it), and M2a (the tree also waits for M5a and M5b; the reminder needs only M2a) — M5b's own gates are met (PR #38, merge 1e6dc91); its last gate, `stats/`, is this level
- [x] Reminder `candler-M6` read (statistics list, pairing model) — 2026-11-17, M6a coordination
**Success Gates**:
- ⬜ McNemar, CMH, bootstrap and fixed-sequence tests run on declared pairing keys, and each equals a second engine's table on the existing HTN golden runs
- ⬜ The pairing keys, the execution order and the declared blocking factor are decided and recorded in `__design__/architecture.md` §Analysis statistics
- ⬜ `analyze` reports `instrument_error` per arm, with the arm's counts of the other invalid causes unchanged
- ⬜ An experiment declaring two treatment levels plans one cell per level; cells whose `treatment:` sources or treatment trees differ get their own env lock and image, and cells with equal inputs share one
- ⬜ `analyze` runs the paired tests between two treatment arms of one experiment, and a one-treatment experiment keeps the analysis it has today
- ⬜ A treatment factor never moves a setting into a cell: every path `is_setting` names stays out of the cell digest (I7)
- ⬜ Every cell digest of M1's golden import and M2a's parity fixtures is unchanged
**References**: [R01 §Analysis statistics, §Contracts & Invariants (I5)](../../../../../../../../../__design__/architecture.md) — today's defaults (rate, Wilson, Fisher with Newcombe, Holm) and the grading rule; [R04 Golden reference](../../../../../../../README.md) — listed in the campaign README; the runs the gates test on

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m6a-paired-stats/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`, covering: the paired tests in `candler-core` beside M3's Wilson and Holm; the pairing model (replicate and attempt exist since M2b and start times are recorded; pairing keys, execution order and a declared blocking factor are new, and the plan decides them before any code is written); per-arm `instrument_error` reporting. Treatment arms (A7, source `__reports__/candler-v1-review/03-findings_amendment_a7_v0.md`): today `request.treatment` is a setting, a setting-valued factor is refused (`PlanError::SettingAsFactor`), and one resolve yields one treatment, one env lock and one image for every cell. The plan decides, before any code, which path varies per cell, how the setting rule treats it, and how per-cell treatment, lock and image resolution is ordered and shared between cells with equal inputs. The paired tests apply to treatment arms like any other declared factor. The statistics gates are tested on existing HTN golden runs, and the treatment gates on resolve, plan and image tests, so the milestone runs no new model call. Probabilistic evaluators stay descriptive only (I5).
**Deliverables**: `__roadmap__/m6a-paired-stats/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m6a-paired-stats/` (expected: PASS)
**Commit**: `rdm(stats): plan the m6a-paired-stats milestone`

## Step 2: Build and land

**Goal**: The milestone campaign is executed and lands on `main`.
**Implementation Logic**:
Execute the campaign on `feat/m6a-paired-stats`, then land it on `main` with one `--no-ff` merge. The Success Gates above must pass on `main` after the merge. The pairing decisions go to `__design__/architecture.md` in the same merge.
**Deliverables**: the paired tests, per-arm `instrument_error` reporting and treatment as a declared factor in `candler-core`; the pairing and treatment-factor decisions in `__design__/architecture.md`; their tests
**Consistency Checks**: `cargo nextest run --workspace` (expected: PASS)
**Commit**: `merge(stats): land paired statistics`

## Step 3: Close out

**Goal**: The milestone is recorded.
**Implementation Logic**:
Write a knowledge-transfer report under `__reports__/m6a-paired-stats/`. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott: `candler-M6` complete. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/m6a-paired-stats/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/m5b/stats/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/m5b/stats/` (expected: PASS)
**Commit**: `reports(stats): close the m6a-paired-stats milestone`
