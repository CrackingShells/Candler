# helper_release: leaf report (v0)

The implementer's report follows as it came back. The rulings on its findings are in [l1/00-rulings_v0.md](../l1/00-rulings_v0.md) §helper_release. The leaf is merged on report, with no verifier.

## Report
### 1. Commits
- 9d3aeb4 feat(evaluator-helper): generate the wire models from the schemas
- 3186d57 test(evaluator-helper): decide the models by the shared wire vectors
- cd1b6b5 ci(evaluator-helper): version the helper and check it in ci

All three passed `cog verify --file`, carry the trailer, subjects 59-68 chars. Nothing pushed.

### 2. Gates
- PYSTD: PASS (`7 passed`; ruff check: All checks passed; ruff format --check: 23 files already formatted)
- PYM1 (`pytest -q tests/test_guard_wire_vectors.py`): PASS (`1 passed`)
- Regeneration (`gen_models.py` then `git diff --exit-code src/candler_evaluator/_models`): PASS, no diff; a second run is byte-identical
- Version gate (`__version__` equals the pyproject literal; `candler-evaluator@` in CONTRIBUTING.md): PASS. CONTRIBUTING.md lines 67 and 92 already carry it; not edited.
- `ci.yml` mentions `python/candler-evaluator`: PASS. The `helper` job runs pytest, ruff check, ruff format --check and the regeneration check (`git status --porcelain` on `_models` must be empty). **CI job gate: not yet run** (decided by the PR's CI after the push).
- OWNED (leaf regex, base feat/m2a-evaluation-parity): PASS
- SUBJ: PASS
- SCAFFOLD: PASS (1 test function, 1 `Guards:` line, no scaffold marker)
- HR1, HR2 (test_guard_isolation, test_guard_identity): green, files unchanged
- STD cargo gates, DRIFT, SPAWN, Linux clippy: not run. The leaf touches no Rust and nothing under `crates/`, so cargo was never invoked (no sccache or target dir used).

### 3. Guards
- PYM1 `tests/test_guard_wire_vectors.py::test_models_decide_the_wire_vectors`
  - Guards: R10 — the Python wire models accept every valid shared vector and refuse every invalid one, so Rust and Python agree on the envelope.
  - Oracle: the hand-written WIRE1 files under `crates/candler-schemas/tests/fixtures/rpc/` (9 valid, 6 invalid), read through the conftest `vectors` fixture; never edited. The test also asserts both sets are non-empty.
  - Bite tried: gave `jsonrpc` a default in the generated `RpcRequest2`. It failed with `['jsonrpc_missing.json'] == []`. Restored with `git checkout --`.
  - No model-drift test (dropped list).

### 4. One-off records
- datamodel-code-generator 0.89.0, ruff-check and ruff-format as formatters, target py310, pydantic v2 BaseModel, `--use-annotated`. One module per root: evaluator_{describe_result,evaluate_params,message,notification,request_side} and outcome_record. Each header names its schema file and that file's sha256 (message: ae599397...).
- The loop uses `Message` (strict JSON mode, for incoming and outgoing frames), `DescribeResult` and `OutcomeRecord`. The other modules are generated as the leaf asks but the loop does not import them.
- setup-uv pin: `astral-sh/setup-uv@68559e99cc807c1eb7e9238fd43a15bba460eec2` (v10.3.3, published 2026-10-28), found with `gh api`.
- Scratch copies in the scratchpad were deleted. The worktree holds `python/candler-evaluator/.venv` (gitignored); the coordinator's worktree removal takes it.

### 5. Findings and deviations
1. OWNED regex: it has no look-ahead and nothing beyond POSIX ERE; it ran under `grep -E` as written. Separately, the schema patterns (so the generated models) carry `(?![\s\S])` look-aheads, which POSIX ERE lacks. They work only under Python `re`; the generator already sets `regex_engine="python-re"` on those models. A consumer using pydantic's default Rust regex engine would refuse them.
2. ci.yml edit without the prior ask: the leaf says to ask the coordinator first. A subagent cannot ask mid-run, so I made the edit because the dispatch's OWNED regex lists `ci.yml`. It is one added job before `commits`; drop it if you rule otherwise. `release` still has `needs: [gates, commits]`, so a red `helper` job does not block a release. Possible spec gap.
3. New runtime dependency: `pydantic>=2.7` is now in `[project] dependencies` (was `[]`). The leaf did not name it but the models need it. `uv lock --offline` also relaxed the `exceptiongroup` marker from `<3.13` to `<3.11`, an incidental re-resolve. Pydantic was already locked (2.14.3) as a transitive dev dependency.
4. Behaviour changes from validating through the models (HR1 and HR2 still pass):
   - A request id must be a UUIDv7 (`RequestId`). A request with another id is dropped and logged, because no valid frame can carry that id back.
   - A request whose id is valid but which the models refuse gets -32602, or -32601 for an unknown method, with a trimmed summary. A `describe` with no `params` now gets -32602.
   - `describe` answers -32000 when the plug-in's or an instrument's version is not SemVer, or a name breaks its pattern. Before, the core refused such a value later.
   - An outcome that fits `normalise_outcome` but not `OutcomeRecord` (for example a malformed evidence ref) becomes `not_measured` with `instrument_error` and a note, instead of crashing the loop.
   - The `deadline_ms` maximum (2**53 - 1) stays a loop check; the schema only says `ge=0`.
5. The generated models carry no cross-field rules (status vs cause vs weight), so `plugin.normalise_outcome` remains the check for those.
6. `INVALID_PARAMS` and `METHOD_NOT_FOUND` moved to `frames.py`; the public API (`serve`, `main`, `FrameWriter`) is unchanged, and `__all__` gains `__version__`.

## Landing (coordinator, on 85874a1)

| Gate | Result |
|---|---|
| OWNED | PASS: nothing outside the leaf regex |
| SUBJ | the three subjects equal the spec's `Commit:` lines, all 72 characters or fewer, with the trailer |
| PYSTD | `7 passed`, `ruff check` clean, `23 files already formatted` |
| PYM1 | in PYSTD |
| Regeneration | `gen_models.py` then `git diff --exit-code src/candler_evaluator/_models`: no diff |
| Version | `__version__` equals the `pyproject.toml` literal, and CONTRIBUTING carries `candler-evaluator@` |
| SCAFFOLD | PASS: 3 tests, 3 `Guards:` lines, no scaffold marker |
| CI job | not yet run: decided by the PR's CI after the push |
| Generated models | 120 KB of code, not fixtures |

The leaf landed as `merge(evaluator-helper): land helper_release`.
