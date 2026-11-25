# candler-evaluator

The Python helper for Candler evaluator plug-ins. It speaks the evaluator protocol on a
private pipe, isolates each item, reports the identity of its instruments and checks every
message against models generated from the published wire schemas. The docstrings of the package
are the source of the API below; this page says each part once.

## Writing a plug-in

A plug-in is a module. It exposes `KINDS`, a mapping from a kind's name to the function that
grades one item, and may expose `NAME`, `VERSION`, `DIST`, `INSTRUMENTS`, `KIND_SPECS` and
`DETERMINISTIC` (`Plugin.from_module` reads them; a module with no `KINDS` is refused).

- **A kind** is a function `kind(params, context, prior)`. `params` is the suite entry's params,
  `context` is a `RunContext` and `prior` holds the outcome records already made for the run.
  It returns an `Outcome` or a mapping with the same keys: `status` (one of `pass`, `fail`,
  `blocked`, `not_measured`, `not_applicable`) and the optional `value`, `weight`, `cause` and
  `evidence`. The helper checks the result against the outcome rules.
- **`RunContext`** is what a kind may read of one run: `root`, `artifacts`, `metadata`,
  `scratch`, `run`, `raw`, `trace`, `manifest`, `artifact_files`, `snapshots` and `inputs`. A
  kind never imports the class; it reads the object it is given. `trace` and `manifest` are a
  `ServedFile` (`path`, `abs`, `sha256`) or `None`; `raw` is a tuple of mappings with `stream`,
  `path` and `sha256`. `candler_evaluator.read_trace(context.trace)` yields the trace's lines as
  models generated from `normalised-trace.schema.json` and raises `pydantic.ValidationError` at
  a line the schema refuses.
- **`KINDS`** maps each kind's name to its function.
- **`needs`** says what a kind reads: `artifacts` (globs), `inputs`, `metadata` (field paths),
  `snapshots`, `trace`, `raw` and `manifest` (the last three are booleans). It is set per kind in
  `KIND_SPECS`.
- **`params_schema`** is the JSON Schema of a kind's params, also set per kind in `KIND_SPECS`.
  `default_deadline_ms` sits beside them.

The example below is an excerpt of `examples/synthetic_plugin/synthetic_plugin.py`, the file the
engine tests of `candler-core` import. The file's other kinds misbehave on purpose, to test the
isolation.

```python
NAME = "synthetic"
VERSION = "0.1.0"


def _passed(note):
    return {"status": "pass", "evidence": [{"note": note}]}


def ok(params, context, prior):
    return _passed("ok")
```

A plug-in that exposes `KINDS = {"ok": ok}` serves the kind `ok`.

## Wire models

`src/candler_evaluator/_models/` is generated from `crates/candler-schemas/schema/` and never
edited by hand. After a schema changes, regenerate it from this directory:

```sh
uv run --frozen python scripts/gen_models.py
```

CI regenerates the models and fails when they differ from the committed ones.

## Version and releases

The version lives only in `pyproject.toml`; `candler_evaluator.__version__` reads it from the
installed package metadata. A release is tagged `candler-evaluator@X.Y.Z`, and the package is
never published to PyPI without Eliott's go. See `CONTRIBUTING.md` at the repository root.

## Checks

```sh
uv run --frozen pytest -q && uv run --frozen ruff check && uv run --frozen ruff format --check
```
