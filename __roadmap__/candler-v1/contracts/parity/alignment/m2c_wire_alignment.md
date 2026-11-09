# M2c Wire alignment

**Goal**: The `experiment/1` wire type declares inputs, keeps table-valued cell values whole and names the treatment's source, so the v5 input features run and no smoke experiment needs `--set`.
**Pre-conditions**:
- [ ] `../m2a_evaluation_parity.md` and `../m2b_execution_parity.md` are `done` and merged on `main`. M2b's first gate is carved out for feature_map rows S2, X19, X32 and the `layout` check, which this leaf owns
- [ ] Gap analysis read: `__reports__/m2c-wire-alignment/gap_analysis_v0.md`
- [ ] Eliott's go and model choice for the re-validation runs, which spend tokens; RiVault only after asking him
**Success Gates**:
- ⬜ An experiment file that declares inputs produces runs whose `comparable.inputs[]` holds one `{kind, digest}` entry per input, mounted read-only in the isolation; feature_map rows S2, X19, X32 and the `layout` check pass on a re-validation of M2b's equivalence smoke
- ⬜ An input declared for evaluators only never enters the isolation (I8), and an evaluator whose `needs.inputs[]` names its kind gets it in its run context
- ⬜ A table value under `[fixed]` at a declared leaf path (`prompt.values`) reaches the cell as one value; the smoke experiment sets `prompt.values` and the treatment source in the file, with no `--set`
- ⬜ The treatment source is recorded for every run and stays out of the cell digest: two runs whose treatments have the same commit and different sources have the same cell digest
- ⬜ The change is additive: every cell digest in M1's golden-corpus import and in M2a's parity fixtures is unchanged, and an experiment file without the new fields lists the same cells as before
- ⬜ One `schemas` commit blesses the DRIFT check, the `schemas@` line takes a minor bump, and `__design__/architecture.md` §Experiment file and §Run manifest describe the new fields in the same merge
**References**: [R01 §Experiment file, §Run manifest, §Evaluator protocol, §Invariants (I7, I8)](../../../../../__design__/architecture.md) — the wire type, `comparable.inputs[]`, the evaluator's `needs.inputs[]` and run context; [gap analysis](../../../../../__reports__/m2c-wire-alignment/gap_analysis_v0.md) — M2b's smoke findings F5 and F7 and the treatment source

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m2c-wire-alignment/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`, covering:

- **Input declaration**: an inputs table in `ExperimentFile`, with each entry giving a kind and a path relative to the experiment file, plus whether the entry is for the agent (mounted read-only) or for evaluators only. Resolution hashes each input into `comparable.inputs[]`, in the `{kind, path, digest}` shape R01 gives the evaluator run context. Core already builds the rest end to end: `Inputs` on `CellRun`, read-only mounts, `invalid(input_leak)` (B24), and mount points hidden from `tree.json`. This step only wires the declaration to it.
- **Nested `[fixed]` tables**: flattening must stop at declared leaf paths. The rule must not make `candler-schemas` depend on core's `field_paths.toml`. Decide between two options: the leaf declarations move into `candler-schemas`, or flattening moves into core's `resolve` and the wire type keeps the table. Record the choice in R01.
- **Treatment source**: a source on `TreatmentRef` is a location, not an identity. It goes to `sources[]` (or an operational setting), never into `comparable.treatment`. A treatment given only by tag is resolved to a commit before the cell digest is computed, so the digest never depends on where the treatment was fetched from.
- **Schema change**: one DRIFT bless in a `schemas` commit, as a minor bump. Follow the schema-versioning rule in `CONTRIBUTING.md` on whether `experiment/1` keeps its tag (additive fields), and check M2a's and M2b's parsers against the new shape.
- **Re-validation**: M2b's equivalence smoke re-run with inputs declared and without `--set`, recorded beside M2b's reports.

Adversarial verification is required for the digest invariance (the golden-corpus import, the parity fixtures, a source-only change) and for I8 (an evaluator-only input must not appear in the isolation, the mounts or `tree.json`).
Gotchas: `flatten_fixed` (`execution.rs`) rejects an empty nested table today, so keep that error at leaf paths. An absolute-path string value is refused by `Comparable::from_resolved`, so inputs must arrive as digests and never as `values[]`. `request.treatment_source` was a `--set` workaround in M2b's smoke, so remove it from the smoke experiment rather than keep it as a second route.
**Deliverables**: `__roadmap__/m2c-wire-alignment/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m2c-wire-alignment/` (expected: PASS)
**Commit**: `rdm(experiment): plan the m2c-wire-alignment milestone`

## Step 2: Build and land

**Goal**: The milestone campaign is executed and lands on `main`.
**Implementation Logic**:
Execute the campaign on `feat/m2c-wire-alignment`, then land it on `main` with one `--no-ff` merge through a PR. The Success Gates above must pass on `main` after the merge, and the release job must publish the new `schemas@` version.
**Deliverables**: the input declaration, the leaf-path rule and the treatment source in `candler-schemas` (`ExperimentFile`, `TreatmentRef`); input resolution into `comparable.inputs[]` and the source into `sources[]` in `candler-core`; the blessed schema and the `__design__/architecture.md` sections; the re-validation report
**Consistency Checks**: `cargo nextest run --workspace` (expected: PASS)
**Commit**: `merge(experiment): align the experiment file with inputs and sources`

## Step 3: Close out

**Goal**: The milestone is recorded and the surfaces can start cold.
**Implementation Logic**:
Write a knowledge-transfer report under `__reports__/m2c-wire-alignment/`. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott: `candler-M2c` complete; `candler-M3` and `candler-M4` startable. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/m2c-wire-alignment/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/parity/alignment/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/alignment/` (expected: PASS)
**Commit**: `reports(experiment): close the m2c-wire-alignment milestone`
