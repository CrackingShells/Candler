# M2a Evaluation parity

**Goal**: Candler with the HTN plug-in regrades the golden corpus and reproduces the reference exactly.
**Pre-conditions**:
- [ ] `../m1_contracts.md` is `done` and merged on `main`
- [ ] `pro-htn-phase-6-golden` signed off by Eliott (done 2026-10-28, sign-off pending)
- [ ] Reminder `candler-M2a` read: where its notes differ from this leaf, this leaf wins (the checker is not wrapped as scripts but run through the Python helper)
**Success Gates**:
- ⬜ On the imported golden corpus, `candler evaluate` with the HTN plug-in reproduces `(status, value, weight, cause)` for all 10,320 rows: `candler diff` against the imported reference reports no change
- ⬜ Per-cell gate and quality rates equal today's `_outcome.py` rates for all 12 cells
- ⬜ A second regrade of the same runs is identical (I4)
- ⬜ An item that hangs or exits costs one `not_measured` (`deadline` or `instrument_error`), not its run
- ⬜ Every HTN outcome names the checker pinned at mcp-python-ingestion `28be2c15` with its digest and `dirty = false`
**References**: [R01 §Plug-ins, §Evaluator protocol, §Run store, §Golden corpus](../../../../__design__/architecture.md) — registry, protocol, selection rule, parity test and triage

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m2a-evaluation-parity/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`, covering:

- **Registry** in `candler-core`: plug-in manifests and suites read without spawning anything; one uv environment per lock digest, built by a preflight step registered into M1's `resolve`.
- **`evaluate` is synchronous in this milestone**: it runs in the calling process, starts evaluator processes through M1's supervisor core (own process group, deadlines, stop escalation), without the job supervisor, slots or stray sweep, which M2b builds in parallel. M3 turns it into a job.
- **Evaluation engine**: `applies_to` and dependency `blocked` decided by the core; the run context with the workspace-shaped view (hard links or clones), `inputs[]` and `prior[]`; the protocol client (stdin open until the result, per-item deadlines, cancel); one outcome file per request; the selection rule; rates over weights.
- **Python helper** `python/candler-evaluator` in this repo: the JSON-RPC loop, the file-descriptor stdout guard, one forked child per item with its deadline and the thread-count fallback, instrument digests and dirty flags, models generated from the JSON Schema. Its tag line is added to CONTRIBUTING and to the release configuration.
- **HTN plug-in** in mcp-python-ingestion, branch `feat/skill-htn-creator`, next to the skill: its manifest; a suite file translated from `evals/eval.json` (43 expectations; the `{domain_name}` and `{servers_dir}` templates become `metadata` and `inputs` needs); evaluator kinds covering today's `grading_spec` types; the checker `htn-creator-scripts` pinned at `28be2c15`; Tier D through `check_tier_d` with the servers tree as an input and the rikyu snapshot.
- **Surfaces**: minimal `candler evaluate`, `show` and `diff`.

Adversarial verification is required for golden parity (measurement semantics). Attack the comparison itself: does `diff` compare all four fields, are `blocked` and `not_measured` rows compared, can a mismatch hide in evidence or in a skipped row?
Gotchas: test `fork` on macOS early, with its fallback; the reference's causes come from the per-run `eval-N-grading.json` (`blocked_by`), not from `expectations.jsonl`, which has no cause column; in this corpus D-E1 and D-E2 are only `blocked` or `not_measured`, so the text-value path is tested with synthetic fixtures; C-M2's false-fires (88 of 89) must be reproduced, not fixed; work in mcp-python-ingestion follows that repo's commit vocabulary and runs pytest with `mamba run -n forMpyIn`. Parity unblocks `pro-htn-phase-6-matrix` and `pro-htn-phase-7-tier`.
**Deliverables**: `__roadmap__/m2a-evaluation-parity/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m2a-evaluation-parity/` (expected: PASS)
**Commit**: `docs(roadmap): plan the m2a-evaluation-parity milestone`

## Step 2: Build and land

**Goal**: The milestone campaign is executed and lands on `main`, and the HTN plug-in lands on `feat/skill-htn-creator`.
**Implementation Logic**:
Execute the campaign on `feat/m2a-evaluation-parity` in Candler, and on a topic branch of mcp-python-ingestion for the HTN plug-in. Land each with one `--no-ff` merge into its integration branch. The Success Gates above must pass after both merges. If a design decision changed, `__design__/architecture.md` changes in the same merge.
**Deliverables**: registry and evaluation engine in `candler-core`; `python/candler-evaluator`; the HTN plug-in package in mcp-python-ingestion; `candler evaluate`, `show`, `diff`; a golden-parity test
**Consistency Checks**: `cargo test --workspace` (expected: PASS)
**Commit**: `merge(evaluation): land evaluation parity with the HTN plug-in`

## Step 3: Close out

**Goal**: The milestone is recorded and the HTN lane can use Candler.
**Implementation Logic**:
Write a knowledge-transfer report under `__reports__/m2a-evaluation-parity/`. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott: `candler-M2a` complete; log lines on `pro-htn-phase-6-matrix` and `pro-htn-phase-7-tier`. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/m2a-evaluation-parity/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/parity/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/` (expected: PASS)
**Commit**: `docs(reports): close the m2a-evaluation-parity milestone`
