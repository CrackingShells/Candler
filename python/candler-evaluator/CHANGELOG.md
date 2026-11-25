# Changelog

Releases of this package are tagged `candler-evaluator@X.Y.Z` and are never published to
PyPI without Eliott's go. The version is bumped by hand in `pyproject.toml` from the
`evaluator-helper` commits since the last tag; see `CONTRIBUTING.md`.

## Unreleased

- `RunContext` carries `trace` and `manifest` as `ServedFile`s (`path`, `abs`, `sha256`) and each
  `raw` stream with its `sha256`; `trace` was a bare path and is now a `ServedFile`.
- `read_trace` yields a trace's records as models generated from
  `normalised-trace.schema.json`, refusing a line the schema refuses.
- The wire models are generated from the published schemas by `scripts/gen_models.py`; the
  loop parses and builds every message through them, and the shared wire vectors decide
  what they accept.
- `candler_evaluator.__version__` reads the installed package version.
- `pydantic` is the package's one runtime dependency.
