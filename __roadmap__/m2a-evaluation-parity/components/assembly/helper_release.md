# Helper release: models, version, CI

**Goal**: `candler-evaluator` is release-shaped. Its wire models are generated from the published schemas and decided by the shared WIRE1 vectors (PYM1). Its version is read from one place, its tag line is `candler-evaluator@X.Y.Z`, and a CI job runs PYSTD and the model regeneration check. Nothing is published.
**Pre-conditions**:
- [ ] [cmd] ENV, and `git log --first-parent --format=%s feat/m2a-evaluation-parity | grep -qx 'merge(evaluator-helper): land helper_runtime'`
- [ ] [cmd] `test -f crates/candler-schemas/schema/evaluator-message.schema.json` (from `protocol_wire`)
**Success Gates**:
- ⬜ [cmd] PYSTD, SUBJ and SCAFFOLD pass, and OWNED passes with `^(python/candler-evaluator/(src/candler_evaluator/(_models/|(loop|frames|__init__)\.py$)|scripts/|(tests/test_guard_wire_vectors\.py|pyproject\.toml|uv\.lock|README\.md|CHANGELOG\.md)$)|\.github/workflows/ci\.yml$)`
- ⬜ [cmd] `cd python/candler-evaluator && uv run --frozen pytest -q tests/test_guard_wire_vectors.py` passes (PYM1)
- ⬜ [cmd] `cd python/candler-evaluator && uv run --frozen python scripts/gen_models.py && git diff --exit-code src/candler_evaluator/_models`
- ⬜ [cmd] `test "$(cd python/candler-evaluator && uv run --frozen python -c 'import candler_evaluator;print(candler_evaluator.__version__)')" = "$(sed -n 's/^version = "\(.*\)"/\1/p' python/candler-evaluator/pyproject.toml)"`, and `grep -q 'candler-evaluator@' CONTRIBUTING.md`
- ⬜ [cmd] `grep -q 'python/candler-evaluator' .github/workflows/ci.yml` and the new job runs PYSTD and the regeneration check. It is decided by the PR's CI, recorded as not yet run until the push.
**References**: [R03 §Python helper](../../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md); `crates/candler-schemas/schema/evaluator-*.schema.json`; `crates/candler-schemas/tests/fixtures/rpc/` — the shared vectors; [R04 PYM1](../../../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md); [R06 CONTRIBUTING](../../../../CONTRIBUTING.md) — versioning lines

## Step 1: Generate the wire models from the schemas
**Goal**: The helper's message types come from the published JSON Schemas by a committed generator, and the loop uses them.
**Implementation Logic**:
1. `scripts/gen_models.py` runs `datamodel-code-generator` (a dev dependency pinned in `uv.lock`) over `../../crates/candler-schemas/schema/evaluator-*.schema.json` and `outcome-record.schema.json` into `src/candler_evaluator/_models/`, with a header naming the schema files and their sha256.
2. `loop.py` and `frames.py` parse and build messages through the generated models instead of the runtime leaf's plain dicts. HR1 and HR2 still pass unchanged.
**Deliverables**: `python/candler-evaluator/scripts/gen_models.py`; `python/candler-evaluator/src/candler_evaluator/_models/`; `loop.py`, `frames.py` (models in use)
**Consistency Checks**: `cd python/candler-evaluator && uv run --frozen python scripts/gen_models.py && git diff --exit-code src/candler_evaluator/_models && uv run --frozen pytest -q` (expected: PASS)
**Commit**: `feat(evaluator-helper): generate the wire models from the schemas`

## Step 2: Decide the models by the shared vectors
**Goal**: PYM1 shows the Python models accept and refuse exactly as the shared vector files say.
**Implementation Logic**:
`tests/test_guard_wire_vectors.py`, PYM1 (R10): every file under `crates/candler-schemas/tests/fixtures/rpc/valid/` parses with the generated envelope model, and every file under `invalid/` is refused, read by relative path from the one shared copy. Oracle: the same hand-written vectors as WIRE1. Bite: make a required field optional in the generated model and watch it fail. No model-drift test (dropped list; the regeneration gate covers it).
**Deliverables**: `python/candler-evaluator/tests/test_guard_wire_vectors.py` (PYM1)
**Consistency Checks**: `cd python/candler-evaluator && uv run --frozen pytest -q tests/test_guard_wire_vectors.py` (expected: PASS)
**Commit**: `test(evaluator-helper): decide the models by the shared wire vectors`

## Step 3: Version the helper and add its CI job
**Goal**: The version has one source, the tag line is documented, and CI checks the helper on every push.
**Implementation Logic**:
1. `__version__` reads `importlib.metadata.version("candler-evaluator")`; `pyproject.toml` holds the only literal. `README.md` and `CHANGELOG.md` of the package say that releases are tagged `candler-evaluator@X.Y.Z` and are never published to PyPI without Eliott's go.
2. `.github/workflows/ci.yml` gains a `helper` job: `astral-sh/setup-uv` pinned by sha, then PYSTD and the regeneration check from `python/candler-evaluator`. Ask the coordinator before the edit, since `ci.yml` is a Phase 0 file.
**Deliverables**: `python/candler-evaluator/{pyproject.toml,README.md,CHANGELOG.md}`; `src/candler_evaluator/__init__.py` (`__version__`); `.github/workflows/ci.yml` (`helper` job)
**Consistency Checks**: `cd python/candler-evaluator && uv run --frozen pytest -q && uv run --frozen ruff check` (expected: PASS)
**Commit**: `ci(evaluator-helper): version the helper and check it in ci`
