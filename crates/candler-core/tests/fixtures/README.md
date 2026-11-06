# Fixtures of candler-core

Data the guards under `tests/` read. Nothing here is generated at test time; the one exception
is the gateway e2e, which makes its CA and server certificates with the `openssl` CLI in a
per-test temporary directory and commits none.

| Path | Read by | Holds |
|---|---|---|
| `mini-corpus/` | `guard_import_mini`, `guard_legacy_mapping`, `guard_oracle_mini`, `guard_seams` | a legacy results folder in the shape of the golden corpus, small enough for CI |
| `comparability/verdicts.json` | `guard_comparability` | the pre-drafted stratum-verdict rows (B03), written from I7's text; rows may be added, never edited |
| `docker/fake-docker` | process-level tests only | a PATH-fake `docker` that logs its argv; never an argv oracle |
| `docker/inspect/{clean,exit124,oom}.json` | `guard_run_exec` | real `docker inspect` output of a clean exit, a self-expiry and an OOM kill, recorded by `docker/record_inspect.sh` |
| `seams/fake_harness.sh` | `guard_seams` | a harness that plays back a recorded stream |
| `supervise/tree.sh` | `guard_supervise` | a process tree that ignores `TERM` |

## Mini-corpus

`mini-corpus/` has the layout of `campaign/20260806-cont-train` of the golden corpus: cell
directories with their `experiment.json`, `progress.jsonl`, `contamination_catalog.jsonl`,
`_run_cache/` and thirteen or two `eval-K-*` directories, beside the folder-level
`PROVENANCE.json` and `expectations.jsonl`. `mini-corpus/README.md` describes the folder in full.

Three runs are real, copied byte for byte from the golden corpus:

| Name | Cell | Run | Kind |
|---|---|---|---|
| `real_clean` | haiku-4-5, skill v0.2.3 | `run-6` | complete, valid |
| `real_incomplete` | haiku-4-5, skill v0.2.3 | `run-17` | an `incomplete` run, valid |
| `real_zero_mappings` | haiku-4-5, skill v0.2.6 | `run-12` | a run with zero mappings; its cell makes `factors` non-empty |

Three runs are synthetic, written by `tests/oracle/make_synthetic.py` into the cell
`synthetic_cell`, each with two eval directories:

| Name | Run | Kind |
|---|---|---|
| `synthetic_contaminated` | `run-1` | `invalid{input_leak}`, with no outcome file |
| `synthetic_text_value` | `run-2` | a text-valued row, and integer-, float- and exponent-literal rows |
| `synthetic_not_applicable` | `run-3` | a row with the legacy status `not_applicable` |

The importer skips the cell's `shared_with_skill/`, a `.DS_Store` and the AppleDouble files under
`__MACOSX/`; they are present so that the skip rules run. The `.DS_Store` files are committed with
`git add -f`.

The three real agent streams live once, under `mini-corpus/`; other fixtures point at them by
relative path.

### PROVENANCE

`mini-corpus/PROVENANCE.toml` lists every vendored file with its source path and sha256, and marks
each run `synthetic = true` or `false`. Its header names the source: tag `candler-m2a-golden`,
root `campaign/20260806-cont-train`. Real files are copied by `tests/oracle/select_mini.py` and
synthetic files are written by `make_synthetic.py`. The check:

```sh
cd crates/candler-core/tests/fixtures/mini-corpus
uv run python ../../oracle/select_mini.py --check .
```

`mini-corpus/PROVENANCE.json` is the folder's own provenance file as the importer reads it: the
golden folder's file trimmed to the vendored runs, with an entry per synthetic run. It names the
evaluator `htn-creator-eval@0.4.2` under `pinned.describe`.

The expected values for these runs come from `tests/oracle/` (the independent oracle and
`vectors/context_digest_runs.toml`), never from the importer.

## Seams fixture

`seams/fake_harness.sh STREAM.jsonl.gz` writes the decompressed stream to standard output, one
line to standard error and exits 0, or 1 if the stream cannot be played. It never prints its
environment, so a secret that reaches the store has come from the supervisor. `guard_seams` plays
the `real_clean` stream through it.

## Supervise fixture

`supervise/tree.sh PIDFILE` appends `<label> <pid>` lines for itself (`leader`), a grandchild in its
group (`group`) and a grandchild in a session of its own (`detached`), then `ready`.
