# Changelog

Releases of this package are tagged `candler-evaluator@X.Y.Z` and are never published to
PyPI without Eliott's go. The version is bumped by hand in `pyproject.toml` from the
`evaluator-helper` commits since the last tag; see `CONTRIBUTING.md`.

## Unreleased

- `RunContext` carries `trace` and `manifest` as `ServedFile`s (`path`, `abs`, `sha256`) and each
  `raw` stream with its `sha256`; `trace` was a bare path and is now a `ServedFile`.
- `read_trace` yields a trace's records as models generated from
  `normalised-trace.schema.json`, refusing a line those models refuse: not JSON, an unknown
  field, a `null` on an optional field or a wrong type, and a file out of order as Candler's
  reader refuses it (an empty file, a record before the header, a second header, a session
  after an event, a session declared twice) with a `TraceOrderError`, a `ValueError`. The rules that relate
  records, such as `seq` gaps, are not checked.
- The wire models are generated from the published schemas by `scripts/gen_models.py`; the
  loop parses and builds every message through them, and the shared wire vectors decide
  what they accept.
- `candler_evaluator.__version__` reads the installed package version.
- `pydantic` is the package's one runtime dependency, at 2.14 or later.
- The generated models refuse `null` where the schema does and let an optional field be left
  out. A plug-in that declares `null` for a need is refused with error -32000, not read as
  absent.
