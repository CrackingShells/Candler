# Mini-corpus

A committed folder in the shape of the golden corpus (`campaign/20260806-cont-train`), small enough
for CI. `PROVENANCE.toml` lists every file with its source path and sha256, and marks the
synthetic runs. `uv run python ../../oracle/select_mini.py --check .` verifies the files against it.

- Real runs, copied byte for byte by `oracle/select_mini.py`: `real_clean` (run-6) and
  `real_incomplete` (run-17) of the haiku-4-5 / skill v0.2.3 cell, and `real_zero_mappings` (run-12)
  of the haiku-4-5 / skill v0.2.6 cell, so `factors` is non-empty. Each run keeps its stream,
  outputs, `eval_metadata.json` in every eval directory and, in the first eval directory, the
  `eval-K-grading.json` files plus `grading_summary.json`, `timing.json` and `metrics.json` (the
  last three are identical in all thirteen eval directories of a run).
- Skipped by the importer: the cell's `shared_with_skill/`, a `.DS_Store` (committed with
  `git add -f`: the repository ignores that name) and the AppleDouble file under `__MACOSX/`. They are kept on purpose, so that the importer's skip
  rules are exercised.
- Synthetic runs, written by `oracle/make_synthetic.py` from the legacy format as the design
  describes it, in the cell `synthetic_cell`: `synthetic_contaminated` (run-1),
  `synthetic_text_value` (run-2) and `synthetic_not_applicable` (run-3). They have two eval
  directories instead of thirteen.
- `expectations.jsonl` and `PROVENANCE.json` are trimmed to the vendored runs; the synthetic runs add
  their rows and `runs[]` entries.
- The three real streams live here once; other fixtures point at them by relative path.
