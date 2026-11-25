# Helper context

**Goal**: A plug-in written with the Python helper reads what it declared without parsing the wire: `RunContext` carries `trace`, `raw` and `manifest` as paths with their digests, and a reader yields the trace's events as typed models generated from `normalised-trace.schema.json`, refusing a line the schema refuses (non-critical: no gate on HTN's path depends on it; RiVault trial candidate)
**Estimate**: RiVault trial (Reasoning-Medium, then Reasoning-Large), falling back to Sonnet, $5, cap $10, plus a mandatory Sonnet review briefed without the run's summary. At the cap, stop, commit what is green, and list what is left in the leaf report. No Rust build is expected; if one is needed, it uses the target dir the campaign README names and the leaf removes it at its merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/run-context HEAD` succeeds
- [ ] `[nl]` `../schema_exit_needs.md` is `done` (the schemas and the generated models it regenerated are on the tip)
**Success Gates**:
- ⬜ `tests/test_guard_context.py`, over hand-written wire contexts, shows `RunContext.from_wire` keeping `trace`, `raw` (with `sha256`) and `manifest`, each `None` or empty when absent. On the base `manifest` is dropped and `trace` is read as a bare path. Bite: `manifest` dropped by `from_wire`
- ⬜ A trace reader in the helper yields typed events from `context.trace`, and the same test file shows it reading hand-written lines with `exit`, `origin: {kind: mcp, server, tool}` and `argv`, and refusing a line with an unknown field. Bite: a model that accepts unknown fields
- ⬜ The generated `Needs.raw` and `Needs.manifest` are typed `bool | None`, while the schema and the Rust reader refuse `null` (§Wire encodings, "Absent vs null"). A describe result the helper writes never carries `"raw": null` or `"manifest": null`, guarded over the helper's own writer with the shared wire vectors as oracle. Bite: the writer emits `null` for an unset need
- ⬜ `scripts/gen_models.py` generates the trace models from `normalised-trace.schema.json` deterministically, and CI's regeneration check passes
- ⬜ `docs/contract/evaluator-helper.md` lists the new fields and the reader, within its Notes limit and with stamps; `pyproject.toml`'s version is bumped by hand to the next minor, as CONTRIBUTING.md §Evaluator helper line says, and no tag is cut
- `uv run pytest`, `uv run ruff check`, `uv run ruff format --check` and `uv run --frozen python scripts/gen_models.py` with no diff, all from `python/candler-evaluator`
**References**: [plan §Protocol, schemas, helper, §Test plan](../../../__reports__/run-context/00-plan_run_context_v0.md). Code:
- `python/candler-evaluator/src/candler_evaluator/plugin.py`: `RunContext`, `RunContext.from_wire`;
- `python/candler-evaluator/scripts/gen_models.py`: `roots`;
- `python/candler-evaluator/tests/test_guard_context.py`;
- `crates/candler-schemas/schema/normalised-trace.schema.json`;
- `docs/contract/evaluator-helper.md`.

## Step 1: Guards on the base
**Goal**: The context and reader guards are written first and seen red on the base.
**Implementation Logic**:
Extend `test_guard_context.py` with wire contexts that carry the new parts and trace lines written by hand from the schema. Use the project's environment (`uv`, from `python/candler-evaluator`). Run on the base and record each failing message.
**Deliverables**: `tests/test_guard_context.py`
**Consistency Checks**: the new tests on the base (expected: FAIL)
**Commit**: `test(evaluator-helper): state the stored run's parts and a typed trace`

## Step 2: Fields and reader
**Goal**: The guards pass.
**Implementation Logic**:
Add `normalised-trace.schema.json` to `gen_models.py`'s roots and regenerate. Give `RunContext` the `manifest` field and keep each served file's digest beside its path. Add a reader that opens `context.trace` and validates each line strictly against the generated event model, yielding the models. Update the contract page and bump the version.
**Deliverables**: `plugin.py` (or a module beside it for the reader); `scripts/gen_models.py`; `_models/`; `docs/contract/evaluator-helper.md`; `pyproject.toml`
**Consistency Checks**: `uv run pytest` (expected: PASS)
**Commit**: `feat(evaluator-helper): hand a kind the stored run's parts and a typed trace`

## Step 3: Bite and review
**Goal**: Each bite turns a guard red, and a Sonnet reviewer read the change.
**Implementation Logic**:
On a scratch branch apply each bite and record each failing message. Delete every `confirm_` test. A Sonnet reviewer, briefed with this leaf and the diff but not the run's summary, is asked to find a wire context or trace line the helper reads differently from the Rust reader.
**Deliverables**: `__reports__/run-context/leaves/helper_context_v0.md`
**Consistency Checks**: `git grep -n confirm_ -- crates python` prints nothing (expected: PASS)
**Commit**: `reports(evaluator-helper): close the stored run's parts in the helper`
